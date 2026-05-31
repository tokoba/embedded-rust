#!/bin/bash
# Enhanced nextest runner with doctest support and better options
#
# ホストテスト可能なクレートのみをテストします。
# 各クレートの Cargo.toml に設定された metadata.ci.host-testable を自動判定します。

set -euo pipefail

#--------------------------------------
# 1. ツールチェック
#--------------------------------------
if ! command -v jq >/dev/null 2>&1; then
  echo "❌ jq が見つかりません。インストールしてください。"
  echo "   Windows: scoop install jq または choco install jq"
  exit 1
fi

if ! cargo nextest --version >/dev/null 2>&1; then
  echo "❌ cargo-nextest が見つかりません。以下でインストールしてください:"
  echo "   cargo install cargo-nextest"
  exit 1
fi

#--------------------------------------
# 2. ホストテスト不可パッケージの自動抽出
#   (workspace かつ metadata.ci.host-testable != true)
#--------------------------------------
echo "🔎 cargo metadata からホストテスト対象を自動判定します..."

# mapfile は MSYS2/Git Bash で動作しない場合があるため、代替方法を使用
# Windows環境ではキャリッジリターンが混入するため tr -d '\r' で削除
EXCLUDED_PACKAGES=()
while IFS= read -r line; do
  [ -n "$line" ] && EXCLUDED_PACKAGES+=("$line")
done < <(
  cargo metadata --no-deps --format-version 1 2>/dev/null |
    jq -r '
      .workspace_members as $members
      | .packages[]
      | select(.id as $id | any($members[]; . == $id))
      | select(.metadata.ci["host-testable"] != true)
      | .name
    ' | tr -d '\r' | sort -u
)

if [ ${#EXCLUDED_PACKAGES[@]} -eq 0 ]; then
  echo "ℹ️  除外されるパッケージはありません"
else
  echo "🚫 ホストテストから除外されるパッケージ: ${EXCLUDED_PACKAGES[*]}"
fi

EXCLUDE_ARGS=()
for pkg in "${EXCLUDED_PACKAGES[@]}"; do
  EXCLUDE_ARGS+=(--exclude "$pkg")
done

#--------------------------------------
# 3. オプション解析
#--------------------------------------
INCLUDE_DOCTESTS=false
INCLUDE_IGNORED=false
RUN_DOCTESTS_ONLY=false
VERBOSE=false

for arg in "$@"; do
  case $arg in
    --with-doctests)
      INCLUDE_DOCTESTS=true
      echo "📚 doctest も実行します"
      ;;
    --with-ignored)
      INCLUDE_IGNORED=true
      echo "🔄 ignored テストも実行します"
      ;;
    --doctests-only)
      RUN_DOCTESTS_ONLY=true
      echo "📚 doctest のみ実行します"
      ;;
    --verbose|-v)
      VERBOSE=true
      echo "🔍 詳細出力モード"
      ;;
    --help|-h)
      echo "使用方法: $0 [オプション]"
      echo "オプション:"
      echo "  --with-doctests   通常のnextestテストに加えてdoctestも実行"
      echo "  --doctests-only   doctest のみ実行（nextestは実行しない）"
      echo "  --with-ignored    ignored テストも実行"
      echo "  --verbose, -v     詳細出力"
      echo "  --help, -h        このヘルプを表示"
      echo ""
      echo "除外されるARM専用クレート(自動検出):"
      echo "  ${EXCLUDED_PACKAGES[*]:-(なし)}"
      echo ""
      echo "※ ホストテスト可能にするには、Cargo.toml に以下を追加:"
      echo "   [package.metadata.ci]"
      echo "   host-testable = true"
      exit 0
      ;;
  esac
done

echo "========================================="
echo "Running tests with nextest"
echo "========================================="

#--------------------------------------
# 4. テスト実行
#--------------------------------------
TARGET_ARGS=(--target x86_64-pc-windows-msvc)
FEATURE_ARGS=(--all-features --lib)

if [ "$RUN_DOCTESTS_ONLY" = true ]; then
  echo ""
  echo "📚 Running doctests only..."
  if [ "$VERBOSE" = true ]; then
    cargo test --doc --workspace "${EXCLUDE_ARGS[@]}" "${FEATURE_ARGS[@]}" "${TARGET_ARGS[@]}" -- --nocapture
  else
    cargo test --doc --workspace "${EXCLUDE_ARGS[@]}" "${FEATURE_ARGS[@]}" "${TARGET_ARGS[@]}"
  fi
else
  echo ""
  echo "🚀 Running nextest tests..."
  if [ "$VERBOSE" = true ]; then
    cargo nextest run --workspace "${EXCLUDE_ARGS[@]}" "${FEATURE_ARGS[@]}" "${TARGET_ARGS[@]}" --nocapture
  else
    cargo nextest run --workspace "${EXCLUDE_ARGS[@]}" "${FEATURE_ARGS[@]}" "${TARGET_ARGS[@]}"
  fi

  if [ "$INCLUDE_IGNORED" = true ]; then
    echo ""
    echo "🔄 Running ignored tests..."
    if [ "$VERBOSE" = true ]; then
      cargo nextest run --workspace "${EXCLUDE_ARGS[@]}" "${FEATURE_ARGS[@]}" "${TARGET_ARGS[@]}" -- --ignored --nocapture
    else
      cargo nextest run --workspace "${EXCLUDE_ARGS[@]}" "${FEATURE_ARGS[@]}" "${TARGET_ARGS[@]}" -- --ignored
    fi
  fi

  if [ "$INCLUDE_DOCTESTS" = true ]; then
    echo ""
    echo "📚 Running doctests..."
    if [ "$VERBOSE" = true ]; then
      cargo test --doc --workspace "${EXCLUDE_ARGS[@]}" "${FEATURE_ARGS[@]}" "${TARGET_ARGS[@]}" -- --nocapture
    else
      cargo test --doc --workspace "${EXCLUDE_ARGS[@]}" "${FEATURE_ARGS[@]}" "${TARGET_ARGS[@]}"
    fi
  fi
fi

echo ""
echo "========================================="
echo "✅ All tests completed successfully!"
echo "========================================="
