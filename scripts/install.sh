#!/usr/bin/env bash
# ============================================================================
# STM32F767ZI Embassy プロジェクト依存関係インストールスクリプト
# ============================================================================
# このスクリプトは scripts/ 内のすべてのスクリプトを実行するために
# 必要な依存関係をインストールします。
# ============================================================================

set -euo pipefail

# Colors for output (TTY対応)
if [[ -t 1 ]]; then
  BOLD=$(tput bold 2>/dev/null || true)
  RESET=$(tput sgr0 2>/dev/null || true)
  GREEN=$(tput setaf 2 2>/dev/null || true)
  RED=$(tput setaf 1 2>/dev/null || true)
  YELLOW=$(tput setaf 3 2>/dev/null || true)
  BLUE=$(tput setaf 4 2>/dev/null || true)
else
  BOLD=""; RESET=""; GREEN=""; RED=""; YELLOW=""; BLUE=""
fi

info() { echo -e "${GREEN}[INFO]${RESET} $*"; }
warn() { echo -e "${YELLOW}[WARN]${RESET} $*"; }
error() { echo -e "${RED}[ERROR]${RESET} $*" >&2; }
section() { echo -e "\n${BLUE}${BOLD}=== $* ===${RESET}"; }

# ============================================================================
# 1. Rust ツールチェーン確認
# ============================================================================
section "Rust ツールチェーン確認"

if ! command -v rustup &>/dev/null; then
  error "rustup が見つかりません。Rust をインストールしてください:"
  echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  exit 1
fi

info "rustup が見つかりました: $(rustup --version)"

# ============================================================================
# 2. Rust ターゲットのインストール (thumbv7em-none-eabihf)
# ============================================================================
section "Rust ターゲットインストール"

TARGET="thumbv7em-none-eabihf"
info "ターゲット '${TARGET}' をインストール中..."
rustup target add "${TARGET}"
info "ターゲット '${TARGET}' のインストール完了"

# Windowsホストテスト用ターゲット (blinky_test.sh で使用)
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
  info "Windows 用テストターゲット 'x86_64-pc-windows-msvc' をインストール中..."
  rustup target add x86_64-pc-windows-msvc 2>/dev/null || true
fi

# ============================================================================
# 3. Rustup コンポーネントのインストール
# ============================================================================
section "Rustup コンポーネントインストール"

COMPONENTS=("rustfmt" "clippy" "llvm-tools-preview")
for component in "${COMPONENTS[@]}"; do
  info "コンポーネント '${component}' をインストール中..."
  rustup component add "${component}" 2>/dev/null || warn "'${component}' は既にインストール済みまたは利用不可"
done

# ============================================================================
# 4. Cargo ツールのインストール
# ============================================================================
section "Cargo ツールインストール"

# cargo-binutils: cargo size コマンド用
info "cargo-binutils をインストール中..."
cargo install cargo-binutils --quiet 2>/dev/null || warn "cargo-binutils のインストールに失敗しました"

# cargo-bloat: バイナリー肥大化解析用
info "cargo-bloat をインストール中..."
cargo install cargo-bloat --quiet 2>/dev/null || warn "cargo-bloat のインストールに失敗しました"

# cargo-llvm-cov: テストカバレッジ計測用
info "cargo-llvm-cov をインストール中..."
cargo install cargo-llvm-cov --quiet 2>/dev/null || warn "cargo-llvm-cov のインストールに失敗しました"

# cargo-nextest: 高速テストランナー
info "cargo-nextest をインストール中..."
cargo install cargo-nextest --locked --quiet 2>/dev/null || warn "cargo-nextest のインストールに失敗しました"

# cargo-deny: セキュリティスキャン用
info "cargo-deny をインストール中..."
cargo install cargo-deny --quiet 2>/dev/null || warn "cargo-deny のインストールに失敗しました"

# ============================================================================
# 5. probe-rs のインストール (ターゲットへの書き込み・デバッグ用)
# ============================================================================
section "probe-rs インストール"

if command -v probe-rs &>/dev/null; then
  info "probe-rs は既にインストールされています: $(probe-rs --version)"
else
  info "probe-rs をインストール中..."
  # probe-rs 公式インストール方法
  if command -v cargo &>/dev/null; then
    cargo install probe-rs-tools --quiet 2>/dev/null || {
      warn "probe-rs-tools のインストールに失敗しました"
      warn "手動でインストールしてください: https://probe.rs/docs/getting-started/installation/"
    }
  fi
fi

# ============================================================================
# 6. オプション: Node.js ツール (Markdownリント用)
# ============================================================================
section "オプションツール確認"

if command -v npm &>/dev/null; then
  info "npm が見つかりました。markdownlint-cli2 をインストール中..."
  npm install -g markdownlint-cli2 2>/dev/null || warn "markdownlint-cli2 のインストールに失敗しました"
else
  warn "npm が見つかりません。markdownlint-cli2 はスキップします。"
  warn "Markdown リントを使用する場合は Node.js をインストールしてください。"
fi

# ============================================================================
# 7. インストール確認
# ============================================================================
section "インストール確認"

echo ""
echo "${BOLD}インストール済みツール:${RESET}"
echo "  - rustc:        $(rustc --version 2>/dev/null || echo 'Not found')"
echo "  - cargo:        $(cargo --version 2>/dev/null || echo 'Not found')"
echo "  - rustfmt:      $(rustfmt --version 2>/dev/null || echo 'Not found')"
echo "  - clippy:       $(cargo clippy --version 2>/dev/null || echo 'Not found')"
echo "  - cargo-size:   $(cargo size --version 2>/dev/null || echo 'Not found')"
echo "  - cargo-bloat:  $(cargo bloat --version 2>/dev/null || echo 'Not found')"
echo "  - cargo-nextest:$(cargo nextest --version 2>/dev/null || echo 'Not found')"
echo "  - probe-rs:     $(probe-rs --version 2>/dev/null || echo 'Not found')"
echo ""

echo "${BOLD}インストール済みターゲット:${RESET}"
rustup target list --installed | grep -E "(thumbv7em|windows)" | while read -r target; do
  echo "  - ${target}"
done
echo ""

# ============================================================================
# 完了メッセージ
# ============================================================================
section "インストール完了"
info "すべての依存関係がインストールされました。"
