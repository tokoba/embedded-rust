#!/usr/bin/env bash
set -euo pipefail

# Colored messages (only if TTY)
if [[ -t 1 ]]; then
  BOLD=$(tput bold || true)
  RESET=$(tput sgr0 || true)
  GREEN=$(tput setaf 2 || true)
  RED=$(tput setaf 1 || true)
  YELLOW=$(tput setaf 3 || true)
else
  BOLD=""; RESET=""; GREEN=""; RED=""; YELLOW=""
fi

die() { echo -e "${RED}Error:${RESET} $*" >&2; exit 1; }
info() { echo -e "${GREEN}➤${RESET} $*"; }
warn() { echo -e "${YELLOW}⚠${RESET} $*"; }

usage() {
  cat <<'EOF'
Usage: ./scripts/new_crate.sh <crate-name>

Create a new Embassy-rs STM32 crate from crates/template/.

Options:
  -h, --help            Show this help

Examples:
  ./scripts/new_crate.sh uart_echo
  ./scripts/new_crate.sh my_sensor
EOF
}

if [[ $# -ne 1 ]]; then
  usage
  exit 1
fi

case "$1" in
  -h|--help)
    usage
    exit 0
    ;;
esac

CRATE_NAME="$1"

# Rust package name validation (lowercase letters, digits, '_' and '-', start with a letter)
if ! [[ "${CRATE_NAME}" =~ ^[a-z][a-z0-9_-]*$ ]]; then
  die "invalid crate name: '${CRATE_NAME}' (use lower-case letters, digits, '_' and '-' and start with a letter)"
fi

if [[ "${CRATE_NAME}" == "template" ]]; then
  die "crate name 'template' is reserved"
fi

# Resolve paths
SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
CRATES_DIR="${REPO_ROOT}/crates"
SCRIPTS_DIR="${REPO_ROOT}/scripts"
TEMPLATE_DIR="${CRATES_DIR}/template"
DEST_DIR="${CRATES_DIR}/${CRATE_NAME}"
RUNNER_TEMPLATE="${SCRIPTS_DIR}/runner/run_template.sh"
DEST_RUNNER="${SCRIPTS_DIR}/runner/run_${CRATE_NAME}.sh"

# Check template existence
[[ -d "${TEMPLATE_DIR}" ]] || die "template directory not found: ${TEMPLATE_DIR}"

# Check runner template existence (fail fast to avoid partial creation)
[[ -f "${RUNNER_TEMPLATE}" ]] || die "runner template script not found: ${RUNNER_TEMPLATE}"

# Check destination
[[ ! -e "${DEST_DIR}" ]] || die "destination already exists: ${DEST_DIR}"
[[ ! -e "${DEST_RUNNER}" ]] || die "runner script already exists: ${DEST_RUNNER}"

# Create destination directory
mkdir -p "${DEST_DIR}"

# Cleanup + rollback on failure
CREATED_DEST=1
CREATED_RUNNER=0
TMP_FILES=()
cleanup() {
  local ec=$?
  for f in "${TMP_FILES[@]}"; do rm -f "$f" || true; done
  if (( ec != 0 )); then
    if (( CREATED_RUNNER )); then rm -f "${DEST_RUNNER}" || true; fi
    if (( CREATED_DEST )); then rm -rf "${DEST_DIR}" || true; fi
  fi
}
trap cleanup EXIT

info "Creating new Embassy crate: ${BOLD}${CRATE_NAME}${RESET}"
info "Template: ${TEMPLATE_DIR}"
info "Destination: ${DEST_DIR}"

# Copy entire template directory (including dotfiles)
cp -a "${TEMPLATE_DIR}/." "${DEST_DIR}/"

CARGO_TOML="${DEST_DIR}/Cargo.toml"
[[ -f "${CARGO_TOML}" ]] || die "Cargo.toml not found in template output: ${CARGO_TOML}"

# Temporary file for Cargo.toml modification
TMP_FILE=$(mktemp); TMP_FILES+=("${TMP_FILE}")

# Modify Cargo.toml: package.name and [[bin]] name/path
awk -v crate_name="${CRATE_NAME}" '
BEGIN {
    in_package = 0
    in_bin = 0

    package_name_rewritten = 0
    bin_section_seen = 0
    bin_name_written = 0
    bin_path_written = 0
}

function flush_bin_defaults() {
    if (in_bin) {
        if (!bin_name_written) {
            print "name = \"" crate_name "\""
        }
        if (!bin_path_written) {
            print "path = \"src/main.rs\""
        }
    }
}

{
    # [[bin]] section start
    if ($0 ~ /^\[\[bin\]\][[:space:]]*$/) {
        flush_bin_defaults()

        print $0
        in_package = 0
        in_bin = 1

        bin_section_seen = 1
        bin_name_written = 0
        bin_path_written = 0
        next
    }

    # Regular [section] start
    if ($0 ~ /^\[[^]]+\][[:space:]]*$/) {
        flush_bin_defaults()

        in_package = ($0 == "[package]")
        in_bin = 0

        print $0
        next
    }

    # [package] name
    if (in_package && $0 ~ /^[[:space:]]*name[[:space:]]*=/ && !package_name_rewritten) {
        print "name = \"" crate_name "\""
        package_name_rewritten = 1
        next
    }

    # [[bin]] name
    if (in_bin && $0 ~ /^[[:space:]]*name[[:space:]]*=/ && !bin_name_written) {
        print "name = \"" crate_name "\""
        bin_name_written = 1
        next
    }

    # [[bin]] path
    if (in_bin && $0 ~ /^[[:space:]]*path[[:space:]]*=/ && !bin_path_written) {
        print "path = \"src/main.rs\""
        bin_path_written = 1
        next
    }

    print $0
}

END {
    flush_bin_defaults()

    if (!package_name_rewritten) {
        print "Cargo.toml does not contain [package].name" > "/dev/stderr"
        exit 1
    }

    # Add [[bin]] section if not present in template
    if (!bin_section_seen) {
        print ""
        print "[[bin]]"
        print "name = \"" crate_name "\""
        print "path = \"src/main.rs\""
    }
}
' "${CARGO_TOML}" > "${TMP_FILE}"

# Replace Cargo.toml with modified version
mv "${TMP_FILE}" "${CARGO_TOML}"

# --- Feature 1: Replace info! message in src/main.rs ---
MAIN_RS="${DEST_DIR}/src/main.rs"
[[ -f "${MAIN_RS}" ]] || die "src/main.rs not found in template output: ${MAIN_RS}"

TMP_MAIN=$(mktemp); TMP_FILES+=("${TMP_MAIN}")
awk -v crate_name="${CRATE_NAME}" '
BEGIN { replaced = 0 }
{
  if ($0 ~ /^[[:space:]]*info!\("template crate started"\);[[:space:]]*$/) {
    print "info!(\"" crate_name " crate started\");"
    replaced++
    next
  }
  print $0
}
END {
  if (replaced != 1) {
    if (replaced == 0) {
      print "src/main.rs: target line not found: info!(\"template crate started\");" > "/dev/stderr"
    } else {
      print "src/main.rs: target line matched multiple times: " replaced > "/dev/stderr"
    }
    exit 2
  }
}
' "${MAIN_RS}" > "${TMP_MAIN}"
mv "${TMP_MAIN}" "${MAIN_RS}"

# --- Feature 2: Generate runner script scripts/{crate_name}.sh ---
TMP_RUN=$(mktemp); TMP_FILES+=("${TMP_RUN}")
awk -v crate_name="${CRATE_NAME}" '
BEGIN { replaced = 0 }
{
  if ($0 ~ /^[[:space:]]*cargo[[:space:]]+run[[:space:]]+--bin[[:space:]]+template[[:space:]]*$/) {
    print "cargo run --bin " crate_name
    replaced++
    next
  }
  print $0
}
END {
  if (replaced != 1) {
    if (replaced == 0) {
      print "scripts/template.sh: target line not found: cargo run --bin template" > "/dev/stderr"
    } else {
      print "scripts/template.sh: target line matched multiple times: " replaced > "/dev/stderr"
    }
    exit 2
  }
}
' "${RUNNER_TEMPLATE}" > "${TMP_RUN}"
mv "${TMP_RUN}" "${DEST_RUNNER}"
chmod +x "${DEST_RUNNER}" || true
CREATED_RUNNER=1

info "Created crate: ${DEST_DIR}"
echo ""
echo "Next steps:"
echo "  1. Modify ${BOLD}src/main.rs${RESET} to implement your application"
echo "  2. Build with: ${BOLD}cargo build -p ${CRATE_NAME} --release${RESET}"
echo "  3. Run with: ${BOLD}./scripts/${CRATE_NAME}.sh${RESET}"
