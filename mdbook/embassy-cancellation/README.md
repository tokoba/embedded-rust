# Embassy-rs 実践タスク制御ガイド

この zip は、Embassy-rs のタスク制御・キャンセル設計を mdBook で公開するためのドキュメント一式です。

## 使い方

```bash
cargo install mdbook
mdbook serve
```

## STM32F767ZI サンプル

```bash
cd examples/nucleo-f767zi
cargo build --release
probe-rs run --chip STM32F767ZITx target/thumbv7em-none-eabihf/release/nucleo-f767zi
```

## 構成

- `src/` mdBook 本文
- `examples/nucleo-f767zi/` NUCLEO-F767ZI 向け Embassy サンプル
- `book.toml` mdBook 設定
