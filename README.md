# Rust Multi-Member Workspace Boilerplate

複数のRustクレートを含むMonorepoスタイルのワークスペースのためのボイラープレートです。`crates/*` の各ディレクトリが独立したクレートとして自動的に認識されます。

## 🚀 クイックスタート

```bash
# 1. このリポジトリをクローン
git clone <repository-url>
cd Rust_Workspace_Boilerplate

# 2. 新規ライブラリクレートを作成
./scripts/new_crate.sh utils

# 3. 新規バイナリクレートを作成
./scripts/new_crate.sh --bin my_app

# 4. 全クレートを一度にビルド
cargo build --workspace

# 5. テスト実行
./nextest.sh
```

## 📁 ディレクトリ構成

### 完全なディレクトリ構成例

```text
Rust_Workspace_Boilerplate/
├── .cargo/                    # Cargo設定ファイル
│   ├── config.toml           # cargo alias、target-dir設定
│   └── nextest.toml          # cargo-nextest設定
├── crates/                    # 🔥 メンバークレート群（自動検出）
│   └── hello/                # サンプルライブラリ＋バイナリクレート
│       ├── Cargo.toml        # workspace依存を継承
│       └── src/
│           ├── lib.rs        # ライブラリエントリ（greet/calcモジュール）
│           ├── main.rs       # バイナリエントリ
│           ├── calc/         # 計算モジュール
│           │   ├── mod.rs
│           │   ├── add.rs
│           │   ├── subtract.rs
│           │   └── multiply.rs
│           └── tests/        # 統合テスト
│               ├── hello_tests.rs
│               └── test_calc.rs
├── scripts/                   # 🔧 開発補助スクリプト集
│   ├── new_crate.sh          # 🆕 新規クレート作成（推奨）
│   └── build_timings.sh      # ⏱️ ビルド時間計測
├── docs/                     # 📚 プロジェクトドキュメント
│   ├── project-brief.md       # プロジェクト概要
│   └── review/              # レビュー結果
│       ├── analysis-result.md
│       ├── review-req.md
│       └── *.md
├── target/                   # 🔨 ビルド出力（.gitignore）
├── .git/                     # Gitリポジトリ情報
├── .claude/                  # Claude Code設定
├── .claude_dev/              # Claude Code開発設定
├── .codanna/                 # コードインテックス情報
├── .vscode/                  # VSCode設定
├── Cargo.toml               # ⚙️ ワークスペース定義ファイル
├── rust-toolchain.toml      # 🦀 Rustバージョン固定
├── deny.toml                # 🚫 cargo-denyセキュリティ設定
├── rustfmt.toml             # 🎨 コードフォーマット設定
├── check.sh                 # 総合チェックスクリプト
├── clippy.sh                # Clippy実行スクリプト
├── coda.sh                  # コード解析スクリプト
├── coverage.sh              # カバレッジ測定スクリプト
├── largefiles.sh           # 大容量ファイル検出スクリプト
├── mdlint.sh               # Markdown Lintスクリプト
├── nextest.sh             # Nextest実行スクリプト
├── AGENTS.md               # AIエージェント設定
├── CLAUDE.md              # Claude Codeガイド
├── GEMINI.md              # Gemini AIガイド
├── RUST-CHEETSHEET.md      # Rustチートシート
├── .gitignore             # Git除外設定
├── .markdownlint.json      # Markdown Lint設定
├── .mcp.json.example      # MCP設定例
├── .prettierignore        # Prettier除外設定
└── README.md              # 📖 このファイル
```

### 新規クレート作成後の構成例

```bash
# ライブラリクレートを作成した場合
./scripts/new_crate.sh http_client

 crates/
 ├── hello/                  # 既存クレート
 │   └── ...
 └── http_client/           # 新規ライブラリクレート
     ├── Cargo.toml        # ✅ 自動生成 + workspace継承設定
     └── src/
         ├── lib.rs       # ✅ サンプルコード付き
         └── tests/       # ✅ テストサンプル付き

# バイナリクレートを作成した場合
./scripts/new_crate.sh --bin my_app

 crates/
 ├── hello/                  # 既存クレート
 │   └── ...
 ├── http_client/           # ライブラリクレート
 │   └── ...
 └── my_app/              # 新規バイナリクレート
     ├── Cargo.toml        # ✅ 自動生成 + workspace継承設定
     └── src/
         └── main.rs      # ✅ サンプルコード付き
```

## 🛠️ 開発環境セットアップ

### 必須ツール

```bash
# Rustツールチェーン（rust-toolchain.tomlで自動）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 推奨コンポーネント
rustup component add rustfmt clippy llvm-tools-preview
```

### 推奨ツール（開発体験向上）

```bash
# カバレッジ測定
cargo install cargo-llvm-cov

# セキュリティスキャン
cargo install cargo-deny

# 高速テストランナー
cargo install cargo-nextest

# Markdownリント（Node.js必要）
npm install -g markdownlint-cli2
```

## 🏗️ クレートの種類と使い分け

| タイプ | 用途 | 作成コマンド | 主な用途 |
|------|------|-------------|---------|
| **ライブラリ** | 再利用可能な機能 | `./scripts/new_crate.sh crate_name` | 共通ライブラリ、内部API |
| **バイナリ** | 実行可能なアプリケーション | `./scripts/new_crate.sh --bin app_name` | CLIツール、Webサーバー |

### クレート間の依存関係例

```toml
# crates/my_app/Cargo.toml
[dependencies]
# ワークスペース内の他クレートを利用
utils = { workspace = true }
http_client = { workspace = true }

# 外部クレートもworkspace経由で利用
tokio = { workspace = true }
```

## 📋 依存関係管理ポリシー

### ワークスペース依存の一覧（現在）

```toml
# Cargo.toml [workspace.dependencies]
thiserror = "1"                 # 自作エラー型
serde = { version = "1", features = ["derive"] }  # シリアライズ
serde_json = "1"                # JSON処理
tracing = "0.1"                 # ロギング
tracing-subscriber = "0.3"      # ログ出力
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }  # 非同期ランタイム
```

### 新規依存の追加手順

1. **ワークスペースに追加**:

   ```bash
   # ルートのCargo.tomlを編集
   [workspace.dependencies]
   chrono = "0.4"  # 新規追加
   ```

2. **クレートで利用**:

   ```toml
   # crates/my_lib/Cargo.toml
   [dependencies]
   chrono = { workspace = true }  # ✅ 正しい使い方
   # chrono = "0.4"         # ❌ 非推奨：重複定義
   ```

## 🔄 よく使うコマンド一覧

### 開発ワークフロー

```bash
# 🏗️ ビルド系
cargo build --workspace              # 全クレートをビルド
cargo run --bin my_app              # 特定バイナリを実行
cargo test --workspace              # 全クレートのテスト実行

# 🎨 フォーマット & Lint
cargo fmt --all                     # 全ファイルをフォーマット
cargo fmt --all -- --check          # フォーマットチェックのみ
cargo lint                          # Clippy警告レベル
cargo lint-strict                   # Clippy厳格レベル（CI用）

# 🧪 テスト & カバレッジ
./scripts/check.sh                  # ✓ 総合チェック（推奨）
cargo test-all                      # 全機能オンでテスト
./scripts/coverage.sh               # カバレッジHTML生成

# 📚 ドキュメント
cargo doc-all                       # 全クレートのドキュメント生成
cargo doc --open                    # ブラウザでドキュメント開く

# 🔍 依存関係調査
cargo tree                          # 依存ツリー表示
./scripts/inspect.sh tokio          # 特定クレートの依存を調査
```

### 管理系コマンド

```bash
# 🧹 クリーンアップ
./scripts/clean.sh                  # targetとロックファイル削除
cargo clean                         # targetのみ削除

# ⏱️ パフォーマンス計測
./scripts/build_timings.sh          # ビルド時間計測

# 📎 ファイル管理
./scripts/largefiles.sh             # 10MB以上のファイルを検出
./scripts/mdlint.sh                 # MarkdownファイルのLint
```

## 🎯 具体的な使用例

### 例1: Web APIサーバープロジェクトの構成

```bash
# プロジェクト初期化
git clone <this-boilerplate> my_web_project
cd my_web_project

# クレート群を作成
./scripts/new_crate.sh config           # 設定管理ライブラリ
./scripts/new_crate.sh database         # データベースアクセス層
./scripts/new_crate.sh auth             # 認証・認可ライブラリ
./scripts/new_crate.sh --bin api_server # Web APIサーバー本体

# 依存関係設定
# crates/api_server/Cargo.toml
[dependencies]
config = { workspace = true }
database = { workspace = true }
auth = { workspace = true }
tokio = { workspace = true }
axum = "0.7"  # APIサーバー固有の依存
```

### 例2: CLIツールの構成

```bash
# CLIツールプロジェクト
./scripts/new_crate.sh core             # コアロジック
./scripts/new_crate.sh --bin my_tool    # CLIインターフェース

# crates/my_tool/Cargo.toml
[dependencies]
core = { workspace = true }
clap = { version = "4", features = ["derive"] }  # CLI引数解析
```

## 🐛 トラブルシューティング

### よくある問題と解決策

| 問題 | 解決策 |
|------|--------|
| **「cargo: member not found」エラー** | `crates/` ディレクトリにクレートがあるか確認 |
| **依存バージョン競合** | `cargo tree -d` で競合を調査 |
| **ビルドが遅い** | `./scripts/build_timings.sh` で原因調査 |
| **テストが失敗** | `cargo test --workspace --all-features` で全機能テスト |
| **フォーマットエラー** | `cargo fmt --all` で修正 |

### デバッグコマンド

```bash
# 依存関係の可視化
cargo tree --duplicates --format "{p}"
cargo tree -e features -i serde

# ビルド詳細情報
RUST_LOG=debug cargo build --workspace
```

## 🚀 CI/CD連携

### GitHub Actions例

```yaml
- name: Cache cargo registry
  uses: Swatinem/rust-cache@v2

- name: Check code formatting
  run: cargo fmt --all -- --check

- name: Run clippy
  run: cargo lint-strict

- name: Run tests
  run: cargo test-all

- name: Generate coverage
  run: ./scripts/coverage.sh
```

## 📚 参考資料

- [Cargo Workspace公式ドキュメント](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [このプロジェクトのワークスペースガイド](./docs/cargo_workspace_guide.md)
- [Rustベストプラクティス](https://rust-lang.github.io/api-guidelines/)

## 📄 ライセンス

MIT License - 詳細は [LICENSE](LICENSE) ファイルを参照

---

💡 **ヒント**: 最初は `./scripts/new_crate.sh hello_lib` で簡単なライブラリを作り、ワークスペースの動作を試してみてください！
