# 実践パターン

## パターン A: supervisor + workers + state watch

最も汎用的な構成です。

```text
supervisor_task
 ├─ command を受信
 ├─ SystemState を更新
 ├─ worker へ Command を送る
 └─ Status / ACK を待つ

worker_task
 ├─ Command::Start を待つ
 ├─ work と cancel を select
 ├─ cleanup
 └─ Status を返す
```

向く用途:

- モード遷移が複雑
- フェールセーフが必要
- 複数 peripheral を協調制御する

## パターン B: device owner task

共有 peripheral を複数 task から直接触らず、owner task へ閉じ込めます。

```text
application task
  -> Channel<DeviceCommand>
      -> device_owner_task
          -> peripheral 操作
          -> Channel<DeviceStatus>
```

向く用途:

- UART / SPI / I2C / CAN / USB
- トランザクション順序を保証したい処理
- キャンセル安全性が不明な driver

## パターン C: per-operation cancel

長い単発処理を止めたい場合の最小構成です。

```rust
match select(long_operation(), CANCEL.wait()).await {
    Either::First(_) => {}
    Either::Second(_) => cleanup().await,
}
```

向く用途:

- homing
- calibration
- firmware update
- 長時間 measurement sequence

## パターン D: timer task を spawn しない

`Timer::after` のためだけに task を都度 spawn しないでください。
既存 worker 内で timer future と cancel future を `select` します。

```rust
match select(Timer::after_secs(10), cancel.wait()).await {
    Either::First(_) => timeout_handler().await,
    Either::Second(_) => cancel_handler().await,
}
```

## パターン E: bool ではなく enum state

複雑な制御では、flag の組み合わせより enum state machine が安全です。

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Idle,
    Running,
    Cancelling,
    Fault,
}
```
