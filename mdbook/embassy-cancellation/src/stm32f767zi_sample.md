# STM32F767ZI/Nucleo-F767ZI 実装例

この章では、NUCLEO-F767ZI を想定した Embassy サンプルを示します。

- LD1: PB0 を LED として使用
- B1: PC13 をユーザーボタンとして使用
- Button falling edge で worker に `Start` を送信
- 2 秒後に supervisor が `Cancel` を送信
- worker は `select(work, cancel)` で協調キャンセル
- LED 点滅で状態を可視化

> ボードリビジョンや BSP 設定により LED / Button のピンが異なる場合があります。その場合は `src/main.rs` の `PB0`, `PC13`, `EXTI13` を実機配線に合わせて変更してください。

## ディレクトリ構成

```text
examples/nucleo-f767zi/
├── Cargo.toml
├── memory.x
├── .cargo/config.toml
└── src/main.rs
```

## ビルド例

```bash
cd examples/nucleo-f767zi
cargo build --release
```

## 書き込み例

```bash
probe-rs run --chip STM32F767ZITx target/thumbv7em-none-eabihf/release/nucleo-f767zi
```

## 実装のポイント

- command は `Channel<CriticalSectionRawMutex, Command, 4>` で送る
- cancel は command の一種として扱う
- status は `Channel<CriticalSectionRawMutex, Status, 4>` で supervisor に返す
- button task は EXTI を使い、ISR で重い処理をしない
- worker の長い処理は 100 ms step に分割し、cancel を受けられるようにする

完全なコードは zip 内の `examples/nucleo-f767zi/src/main.rs` を参照してください。
