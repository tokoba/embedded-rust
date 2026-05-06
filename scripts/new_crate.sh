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
  cat <<'USAGE'
Usage:
  scripts/new_crate.sh [--lib | --bin | --binary] [--edition 2015|2018|2021|2024]
                       [--open | --no-open] [--path DIR] <crate-name>

Options:
  -l, --lib             Create a library crate (default)
  -b, --bin, --binary   Create a binary crate
  --edition <ver>       Set edition (default: 2024)
  --open                Open the created crate in VS Code
  --no-open             Do not open VS Code
  --path, --dir DIR     Target parent directory (default: <repo-root>/crates)
  -h, --help            Show this help

Examples:
  scripts/new_crate.sh hello_lib
  scripts/new_crate.sh --binary hello_cli
  scripts/new_crate.sh --bin --edition 2024 --path ./packages hello_lib
USAGE
}

# defaults
# 現在の西暦は2025年でRust Edition 2024が最新版である。
CRATE_TYPE="lib"      # lib/bin
EDITION="2024"        # Use latest stable 2024 edition (safe default)
OPEN_MODE="auto"      # auto/yes/no
TARGET_DIR=""         # parent dir (default resolved later)
CRATE_NAME=""

if [[ $# -eq 0 ]]; then
  usage
  exit 1
fi

# parse args
while [[ $# -gt 0 ]]; do
  case "$1" in
    -l|--lib) CRATE_TYPE="lib"; shift ;;
    -b|--bin|--binary) CRATE_TYPE="bin"; shift ;;
    --edition)
      [[ $# -ge 2 ]] || die "missing value for --edition"
      case "$2" in
        2015|2018|2021|2024) EDITION="$2" ;;
        *) die "unsupported edition: $2 (use 2015, 2018, 2021, or 2024)" ;;
      esac
      shift 2
      ;;
    --path|--dir)
      [[ $# -ge 2 ]] || die "missing value for --path/--dir"
      TARGET_DIR="$2"
      shift 2
      ;;
    --open) OPEN_MODE="yes"; shift ;;
    --no-open) OPEN_MODE="no"; shift ;;
    -h|--help) usage; exit 0 ;;
    --) shift; break ;;
    -*)
      die "Unknown option: $1"
      ;;
    *)
      if [[ -n "${CRATE_NAME}" ]]; then
        die "Multiple crate names provided: '${CRATE_NAME}' and '$1'"
      fi
      CRATE_NAME="$1"
      shift
      ;;
  esac
done

# required args
[[ -n "${CRATE_NAME}" ]] || { usage; die "crate-name is required"; }

# crate name validation (lowercase letters, digits, '_' and '-', start with a letter)
if ! [[ "${CRATE_NAME}" =~ ^[a-z][a-z0-9_-]*$ ]]; then
  die "invalid crate name: '${CRATE_NAME}' (use lower-case letters, digits, '_' and '-' and start with a letter)"
fi

# env checks
command -v cargo >/dev/null 2>&1 || die "cargo not found. Please install Rust toolchain."

# resolve paths
SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
CRATES_DIR="${TARGET_DIR:-${ROOT_DIR}/crates}"
NEW_DIR="${CRATES_DIR}/${CRATE_NAME}"

# check existence
if [[ -e "${NEW_DIR}" ]]; then
  die "target directory already exists: ${NEW_DIR}"
fi

mkdir -p "${CRATES_DIR}"

# create crate
info "Creating ${CRATE_TYPE} crate '${CRATE_NAME}' at '${NEW_DIR}'"
cargo new "--${CRATE_TYPE}" --vcs none "${NEW_DIR}"

CARGO_TOML="${NEW_DIR}/Cargo.toml"

# Configure workspace inheritance by replacing the entire Cargo.toml
info "Configuring workspace inheritance..."

# Create a completely new Cargo.toml with correct structure
TEMP_TOML=$(mktemp)
{
  # Extract name from the original file
  NAME=$(grep '^name = ' "$CARGO_TOML" | sed 's/name = "//; s/"//')

  # Write the complete new structure
  echo "[package]"
  echo "name = \"$NAME\""
  echo "edition = \"${EDITION}\""
  echo "version = { workspace = true }"
  echo "authors = { workspace = true }"
  echo "license = { workspace = true }"
  echo "description = { workspace = true }"
  echo "readme = { workspace = true }"
  echo "repository = { workspace = true }"
  echo "rust-version = { workspace = true }"
  echo "publish = { workspace = true }"
  echo ""
  echo "[dependencies]"
  echo "thiserror = { workspace = true }"
  echo "serde = { workspace = true }"
  echo "serde_json = { workspace = true }"
  echo "tracing = { workspace = true }"
  echo "tracing-subscriber = { workspace = true }"
  echo ""
  echo "[lints]"
  echo "workspace = true"
} > "$TEMP_TOML"

# Replace the original file
mv "$TEMP_TOML" "$CARGO_TOML"

# sample code initialization
if [[ "${CRATE_TYPE}" == "bin" ]]; then
  cat > "${NEW_DIR}/src/main.rs" <<'RS'
fn main() {
    println!("Hello from binary crate!");
}
RS
else
  cat > "${NEW_DIR}/src/lib.rs" <<'RS'
//! Crate documentation

pub fn hello() -> &'static str {
    "hello from lib crate"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        assert_eq!(hello(), "hello from lib crate");
    }
}
RS
fi

# open in VS Code (auto: open if 'code' exists)
# if [[ "${OPEN_MODE}" == "yes" ]] || { [[ "${OPEN_MODE}" == "auto" ]] && command -v code >/dev/null 2>&1; }; then
#   code "${NEW_DIR}" -r || warn "VSCode open failed"
# fi

info "Created crate: ${BOLD}${CRATE_NAME}${RESET} at ${NEW_DIR}"
echo "Workspace members are matched by crates/* — Cargo.tomlのmembers更新は不要です。"
