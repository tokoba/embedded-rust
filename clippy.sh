#!/usr/bin/env bash
set -euo pipefail

# 実機ターゲット（STM32F767ZI: Cortex-M7F）
TARGET=thumbv7em-none-eabihf

# ==============================================================================
# 0. フォーマット実行
# ==============================================================================
echo "==> Running cargo fmt..."
cargo fmt || true

# ==============================================================================
# 1. 実機ターゲット用ファームウェアの Clippy（メインのチェック）
# ==============================================================================
# --target: 実際に焼くターゲットを指定（cfg(target_arch = "arm") の依存が有効になる）
# --no-default-features: 実際のビルドと同じ状態にする
# --bins: バイナリクレートのみ対象（tests は no_std ターゲットでは壊れやすい）
echo ""
echo "==> Running clippy for firmware (target = ${TARGET})..."
cargo clippy \
  --workspace \
  --target "$TARGET" \
  --no-default-features \
  --bins \
  -- -D warnings

# ==============================================================================
# 2. ホスト PC 向け Clippy（ライブラリクレートのテストを含む）
# ==============================================================================
# ホストターゲットを明示的に指定（.cargo/config.toml のデフォルトターゲットを上書き）
# --lib --tests: ライブラリとテストのみ対象（バイナリは no_std でエラーになるため除外）
echo ""
echo "==> Running clippy for library crates with host tests..."

# jqでhost-testableなクレートを抽出（コマンド置換方式）
# Git Bash (MINGW64) でも堅牢に動作するシンプルな実装
HOST_TESTABLE_CRATES=(
  $(
    cargo metadata --no-deps --format-version 1 2>/dev/null |
      jq -r '
        .workspace_members as $members
        | .packages[]
        | select(.id as $id | any($members[]; . == $id))
        | select(.metadata.ci["host-testable"] == true)
        | .name
      ' | tr -d '\r' | sort -u
  )
)

if [ ${#HOST_TESTABLE_CRATES[@]} -eq 0 ]; then
  echo "ℹ️  ホストテスト可能なクレートはありません"
else
  echo "📦 対象クレート: ${HOST_TESTABLE_CRATES[*]}"
  for crate in "${HOST_TESTABLE_CRATES[@]}"; do
    echo ""
    echo "  ==> Clipping $crate..."
    cargo clippy \
      -p "$crate" \
      --target x86_64-pc-windows-msvc \
      --lib --tests \
      -- -D warnings
  done
fi

# テストも実行してパスすることを確認
echo ""
echo "==> Running host tests for library crates..."
for crate in "${HOST_TESTABLE_CRATES[@]}"; do
  echo ""
  echo "  ==> Testing $crate..."
  cargo test -p "$crate" --lib --target x86_64-pc-windows-msvc
done

# ==============================================================================
# 3. Nextest 相当の Clippy は、このプロジェクトでは不要なのでスキップ
# ==============================================================================
echo ""
echo "==> Skipping clippy for nextest targets (no_std bin-only workspace)."
echo ""
echo "✅ All clippy checks passed!"
