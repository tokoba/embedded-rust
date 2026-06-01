# 参考資料

## Embassy 公式・一次情報

- [Embassy Book](https://embassy.dev/book/) — Embassy の公式ガイドブック
- [Embassy STM32 HAL](https://docs.embassy.dev/embassy-stm32/git/stm32f767zi/index.html) — STM32F767ZI 向け HAL ドキュメント
- [embassy-executor docs](https://docs.rs/embassy-executor/latest/embassy_executor/) — Executor の API リファレンス
- [InterruptExecutor docs](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html) — InterruptExecutor の詳細
- [embassy-sync docs](https://docs.embassy.dev/embassy-sync/git/default/index.html) — Signal / Channel / Watch / Mutex の API
- [embassy-futures select docs](https://docs.embassy.dev/embassy-futures/git/default/select/fn.select.html) — select combinator のリファレンス
- [embassy-time docs](https://docs.rs/embassy-time/latest/embassy_time/) — Timer / Duration / Ticker の API

## Embassy サンプルコード

- [Embassy examples (GitHub)](https://github.com/embassy-rs/embassy/tree/main/examples) — 公式サンプル集
- [multiprio.rs (nRF52840)](https://github.com/embassy-rs/embassy/blob/main/examples/nrf52840/src/bin/multiprio.rs) — Multi-Priority executor の決定版サンプル
- [DeepWiki: Embassy examples](https://deepwiki.com/embassy-rs/embassy/6.3-examples) — サンプルの解説

## STM32F767ZI / NUCLEO-F767ZI

- [STM32F767ZI product page (ST)](https://www.st.com/ja/microcontrollers-microprocessors/stm32f767zi.html) — MCU 仕様
- [NUCLEO-F767ZI product page (ST)](https://www.st.com/ja/evaluation-tools/nucleo-f767zi.html) — 開発ボード仕様
- [STM32F767ZI Reference Manual (RM0410)](https://www.st.com/resource/en/reference_manual/rm0410-stm32f76xxx-and-stm32f77xxx-advanced-armbased-32bit-mcus-stmicroelectronics.pdf) — ペリフェラル詳細

## Rust async / キャンセル

- [Comprehensive Rust: Cancellation](https://google.github.io/comprehensive-rust/concurrency/async-pitfalls/cancellation.html) — Rust async のキャンセルの落とし穴
- [Tokio: Graceful Shutdown](https://tokio.rs/tokio/topics/shutdown) — Tokio での graceful shutdown パターン
- [The Async Book: Cancellation](https://rust-lang.github.io/async-book/) — Rust 公式の async 解説

## 実践記事

- [Practical Embedded Rust Development Tips with Embassy](https://acalustra.com/embedded-rust-development-tips-with-embassy.html) — 実践的な開発 Tips
- [Embassy Trouble Documentation](https://docs.embassy.dev/) — トラブルシューティング

## ツール

- [probe-rs](https://probe.rs/) — Rust 製デバッグプローブツール
- [defmt](https://defmt.ferrous-systems.com/) — 組み込み向け高効率ログフレームワーク
- [cargo-embed](https://probe.rs/docs/tools/cargo-embed/) — 組み込み開発用の cargo サブコマンド
- [mdBook](https://rust-lang.github.io/mdBook/) — Rust 製の静的サイトジェネレータ（本書の生成に使用）
