#!/bin/bash
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
  cat << 'EOF'
Usage: ./scripts/size/binarysize.sh <crate-name>

Examples:
  ./scripts/size/binarysize.sh button_led
EOF
}

# 引数が1つでない場合はusageを表示して終了
if [[ $# -ne 1 ]]; then
  usage
  exit 1
fi

# -h|--helpオプションが指定された場合はusageを表示して終了
case "$1" in
  -h|--help)
    usage
    exit 0
    ;;
esac

# crate name を引数として与えられた場合
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
DEST_DIR="${CRATES_DIR}/${CRATE_NAME}"
DEST_RUNNER="${SCRIPTS_DIR}/${CRATE_NAME}.sh"
TARGET_ARCH="thumbv7em-none-eabihf"

echo "[サイズチェック/bloatチェック]"
echo "-------------------------------------"
echo "[target] ${CRATE_NAME}"
echo "-------------------------------------"
cargo size --release --bin ${CRATE_NAME}  --target ${TARGET_ARCH}
echo "-------------------------------------"
cargo size --release --bin ${CRATE_NAME} --target ${TARGET_ARCH} -- -A
echo "-------------------------------------"
cargo bloat --release --bin ${CRATE_NAME} --target ${TARGET_ARCH} --crates
echo "-------------------------------------"
cargo bloat --release --bin ${CRATE_NAME} --target ${TARGET_ARCH} -n 10
echo "-------------------------------------"
