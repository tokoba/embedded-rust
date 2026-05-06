#!/bin/bash
# Enhanced nextest runner with doctest support and better options

set -euo pipefail

# Default options
INCLUDE_DOCTESTS=true
INCLUDE_IGNORED=false
RUN_DOCTESTS_ONLY=false
VERBOSE=false

# Parse command line arguments
for arg in "$@"; do
  case $arg in
    --with-doctests)
      INCLUDE_DOCTESTS=true
      echo "📚 doctest も実行します"
      ;;
    --without-doctests)
      INCLUDE_DOCTESTS=false
      echo "⏩ doctest をスキップします"
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
      echo "  --without-doctests  doctest をスキップ (デフォルトは実行)"
      echo "  --doctests-only  doctest のみ実行（nextestは実行しない）"
      echo "  --with-ignored    ignored テストも実行"
      echo "  --verbose, -v     詳細出力"
      echo "  --help, -h        このヘルプを表示"
      echo ""
      echo "例:"
      echo "  $0                           # 通常のnextestテスト + doctest"
      echo "  $0 --without-doctests        # nextestのみ"
      echo "  $0 --doctests-only          # doctest のみ"
      echo "  $0 --with-ignored           # ignored テストも含めて実行"
      exit 0
      ;;
  esac
done

echo "========================================="
echo "Running tests with nextest"
echo "========================================="

# Tool check
if ! cargo nextest --version >/dev/null 2>&1; then
  echo "❌ cargo-nextest が見つかりません。以下でインストールしてください:"
  echo "   cargo install cargo-nextest"
  exit 1
fi

# Run tests based on options
if [ "$RUN_DOCTESTS_ONLY" = true ]; then
  echo ""
  echo "📚 Running doctests only..."
  if [ "$VERBOSE" = true ]; then
    cargo test --doc --workspace --all-features -- --nocapture
  else
    cargo test --doc --workspace --all-features
  fi
else
  # Run nextest tests
  NEXTEST_CMD="cargo nextest run --all-features --all-targets"
  
  if [ "$INCLUDE_IGNORED" = true ]; then
    NEXTEST_CMD="$NEXTEST_CMD && cargo nextest run --all-features --all-targets -- --ignored"
  fi
  
  echo ""
  echo "🚀 Running nextest tests..."
  if [ "$VERBOSE" = true ]; then
    eval "$NEXTEST_CMD --nocapture"
  else
    eval "$NEXTEST_CMD"
  fi
  
  # Run doctests if requested
  if [ "$INCLUDE_DOCTESTS" = true ]; then
    echo ""
    echo "📚 Running doctests..."
    if [ "$VERBOSE" = true ]; then
      cargo test --doc --workspace --all-features -- --nocapture
    else
      cargo test --doc --workspace --all-features
    fi
  fi
fi

echo ""
echo "========================================="
echo "✅ All tests completed successfully!"
echo "========================================="

