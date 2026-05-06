# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## Rust Workspace Boilerplate — Claude Code ガイド 🦀

このリポジトリは、複数クレートを一括管理するためのマルチクレート・ワークスペース用ボイラープレートです。日本語ドキュメント・日本語コメントを前提に、Rust 2024 Edition（rustc 1.93）を採用しています。Claude Code（claude.ai/code）での作業効率を高めるため、よく使うコマンドや構造、拡張方法、環境特有の注意点をまとめています。

- 開発言語とドキュメント: 日本語
- Rust: **Edition 2024**, **rustc 1.93**
- ワークスペースレベルの設定・依存関係・Lintを統一
- MCP: **shimai mcp (chat, gpt-5)** 利用可能（補助的なコード生成・レビューに活用）

---

### ✅ クイックリファレンス（よく使うコマンド）

- 全ワークスペースのビルド（デバッグ/リリース）

```bash
cargo build --workspace
cargo build --workspace --release
```

- 特定バイナリの実行（例: hello）

```bash
cargo run --bin hello
# またはパッケージ指定
cargo run -p hello
```

- フォーマット（チェック/修正）

```bash
cargo fmt
cargo fmt --all
cargo fmt-check    # .cargo/config.toml の alias
```

- Lint（通常/厳格）

```bash
cargo lint         # clippy --workspace --all-targets --all-features
cargo lint-strict  # -D warnings で警告をエラー扱い
./clippy.sh        # 厳格lint後に fmt を実行
```

- テスト（標準/nextest）

```bash
cargo test-all            # test --workspace --all-features (alias)
cargo test --workspace

# nextest（高速並列テスト + doctest対応）
./nextest.sh                           # 通常のnextestテスト
./nextest.sh --with-doctests           # nextest + doctest
./nextest.sh --doctests-only          # doctest のみ
./nextest.sh --with-ignored           # ignored テストも含めて実行
./nextest.sh --verbose                # 詳細出力

# 直接コマンド使用
cargo nextest run
cargo nextest run -- --ignored
# 絞り込み例（.cargo/nextest.tomlのコメント参照）
cargo nextest run -E 'package(hello) and test(/calc/)'
cargo nextest run -E 'not test(/slow/)'
```

- ドキュメント生成（全体）

```bash
cargo doc-all           # doc --workspace --no-deps --all-features (alias)
cargo doc --workspace --open
```

- 依存関係のセキュリティ/ライセンスチェック

```bash
cargo deny check
```

- カバレッジ（要: cargo-llvm-cov + nextest統合）

```bash
# 基本カバレッジ（nextest自動検出）
./coverage.sh                    # HTML + LCOV両方生成
./coverage.sh --html-only        # HTMLレポートのみ
./coverage.sh --lcov-only        # LCOVレポートのみ
./coverage.sh --with-doctests    # doctestを含めてカバレッジ収集

# 従来のコマンド（互換性維持）
cargo cov        # --lcov 出力 lcov.info
cargo cov-html   # HTMLレポート生成
```

- ビルドタイミングの可視化（HTML）

```bash
./scripts/build_timings.sh
# 出力: target/cargo-timings/index.html
```

- Markdownのlint（要: markdownlint-cli2）

```bash
./mdlint.sh
```

- 大きいRustファイルの確認（上位30件）

```bash
./largefiles.sh
```

---

## 高レベルアーキテクチャと構造

- ワークスペースは `crates/*` を **members** として検出し、テンプレート系は `crates/_template-*` を **exclude**。
- 現在はサンプルとして **hello クレート** を含む構成（ライブラリ＋バイナリ構成）。
  - ライブラリは **greet** と **calc**（add/subtract/multiply）を提供。
  - バイナリは `src/main.rs` のエントリポイントで最小動作を確認可能。
- ルート `Cargo.toml` の **[workspace.package]** でメタ情報（version, authors, license, edition, rust-version など）を一括管理。
- **[workspace.dependencies]** に共通依存を集約し、各クレート側では `dependency = { workspace = true }` で参照。
  例: `serde = { workspace = true }`、`tokio = { workspace = true }`。
- **[workspace.lints.*]** により、Rust/clippy/rustdoc のLint設定をワークスペース全体で統一。
- **resolver = "3"**、**edition = "2024"**、**rust-version = "1.93.0"** を採用（MSRVの目安としても機能）。

---

## マルチクレート・ワークスペースの特徴（運用ポイント）

- 依存のバージョン統一を前提とするため、追加依存はまず**ルート**の `[workspace.dependencies]` に定義し、各クレートでは `workspace = true` を使うのが基本。
- **LintとDoc方針**はワークスペースで統一されるため、各クレートでの逸脱を避けられます（CI/ローカルで `cargo lint-strict` を活用）。
- **nextest** を同梱設定（`.cargo/nextest.toml`）で高速並列テストを標準化。フィルタ式（`-E`）で柔軟に選別可能。
- **cargo alias**（`.cargo/config.toml`）を積極活用:
  - `fmt-check` / `lint` / `lint-strict` / `test-all` / `doc-all` / `cov` / `cov-html`
- **cargo-deny** により、許容ライセンスの範囲と脆弱性監視を一元化。
  重複バージョンの禁止や特定crateの拒否（例: openssl系）を設定済み。

---

## ボイラープレートの目的と使い方（拡張の定石） 🎯

- 目的: 複数クレートへ拡張可能な「安全な初期土台」を提供し、品質基準・ビルド/テスト/ドキュメント/セキュリティチェックを最短で整える。
- 新規クレートの追加はスクリプトで統一:

```bash
# ライブラリクレートを追加（Editionはデフォルト2024）
./scripts/new_crate.sh my_lib

# バイナリクレートを追加
./scripts/new_crate.sh --bin my_cli

# Editionや配置先を明示
./scripts/new_crate.sh --lib --edition 2024 --path ./packages my_lib
```

- スクリプトは `Cargo.toml` をワークスペース継承前提で生成し、共通依存を `workspace = true` で宣言。
- 追加後、`cargo build --workspace` が通ることを前提に最小コードとテストを含む雛形が用意されます。

- 依存追加の流れ（例: `clap` を使う場合）

```toml
# 1) ルート Cargo.toml の [workspace.dependencies] に追加
clap = { version = "4", features = ["derive"] }

# 2) 各クレートの Cargo.toml で参照
[dependencies]
clap = { workspace = true }
```

- ドキュメント/テスト/セキュリティまで一気通貫で確認:

```bash
cargo fmt-check
cargo lint-strict
cargo test-all
cargo doc-all
cargo deny check
```

---

## 開発環境特有の注意点 ⚙️

- ツールチェーンは `rust-toolchain.toml` により **stable** に固定。補助コンポーネントとして **rustfmt / clippy** が指定済み。
- **Windows** 環境では、`*.sh` スクリプトの実行に **Git Bash** や **WSL** を推奨。PowerShell利用時は等価コマンドを手動実行してください。
- **cargo-nextest** と **cargo-llvm-cov** は別途インストールが必要です（例: `cargo install cargo-nextest`, `cargo install cargo-llvm-cov`）。
- ルート `Cargo.toml` の **workspace.package.publish = false** が継承されるため、公開を意図するクレートがある場合は、そのクレート側で個別に上書き設定してください（現状は公開前提ではありません）。
- **cargo-deny** の設定で重複バージョンを原則禁止しています（例外は `windows-sys` など）。依存追加時は解決方針（統一/置換）を優先してください。
- **aliases** に依存した運用を前提としているため、Claude Code内からも alias 呼び出し（例: `cargo lint-strict`）を優先すると意図した検査が漏れません。
- MCPの活用: **shimai mcp (chat, gpt-5)** を補助的に使い、テスト補完・ドキュメント改善・単純なコード生成を支援に回すのが有効です。生成コードは必ず `cargo lint-strict` と `cargo test-all` で検証してください。

---

## Claude Code への具体的指針（このリポジトリでの作業手順） 🧭

- まずはワークスペース全体の健全性チェック

```bash
cargo fmt-check && cargo lint-strict && cargo test-all && cargo deny check
```

- 既存クレートの変更時は、Doc/Doctestの整合も確認

```bash
cargo doc-all
cargo test --doc --workspace
```

- 新規クレート追加・小規模API拡張の際は、次の順で進める
  1. `./scripts/new_crate.sh` で雛形作成（lib/binの選択とEdition指定）
  2. 依存はルートに追加してから `workspace = true` で参照
  3. 最小テストを用意し、`nextest` で並列検証
  4. Lint/Doc/deny/coverageまで一括チェック

- バイナリの動作確認（例: hello）

```bash
cargo run --bin hello
```

- カバレッジレポートが必要な場合

```bash
cargo cov && cargo cov-html
# lcov.info と HTMLレポートをCIアーティファクト化する運用を想定
```

---

## プロジェクト概要（README要約） 📖

- 名称: **Rust Workspace Boilerplate**
- 目的: **拡張容易なマルチクレート構成**を前提に、品質（フォーマット/Lint/テスト/ドキュメント/セキュリティ）を標準化するテンプレート。
- 現状: `hello` クレートを含む最小構成（ライブラリ＋バイナリ）。計算モジュールと挨拶APIを提供。
- 想定拡張: `core`/`hello`/`cli` の分割、観測性（tracing）、CLI導入、ベンチ、カバレッジ計測の常設化。

---

## 付記（構成・設定の要点） 🧩

- **.cargo/config.toml**
  - alias: `fmt-check`, `lint`, `lint-strict`, `test-all`, `doc-all`, `cov`, `cov-html`
  - devプロファイル: コンパイル速度優先の軽量最適化（`opt-level = 1`, `codegen-units = 256`）
- **.cargo/nextest.toml**
  - デフォルトで全テストを並列実行、CIプロフィールも同梱
- **deny.toml**
  - 許容/禁止ライセンス、危険crateの拒否、重複バージョン禁止などを設定

---

必要なタスクの都度、このガイドのコマンド群と運用指針に沿って作業してください。Claude Codeからの操作でも、ワークスペースの方針（依存の統一・Lintの厳格化・ドキュメントとテストの同時整備）を外さないようにすることで、拡張時の不整合やCI落ちを防げます。
