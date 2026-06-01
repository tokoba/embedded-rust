# STM32F767ZI 実装例

この章では、NUCLEO-F767ZI を想定した **完全なビルド可能サンプル** を示します。
前章までのパターンを統合した、supervisor + worker + button + LED の構成です。

## 概要

| 要素 | 内容 |
|---|---|
| ボード | NUCLEO-F767ZI |
| LED (LD1) | PB0 — 点滅で動作確認 |
| Button (B1) | PC13 — falling edge で `Start` 送信 |
| supervisor | 2 秒後に `Cancel` を送信（デモ用） |
| worker | `select(measurement, cancel)` で協調キャンセル |
| status | `Channel<Status>` で supervisor に ACK |

> ボードリビジョンや BSP 設定により LED / Button のピンが異なる場合があります。
> その場合は `main.rs` の `PB0`, `PC13`, `EXTI13` を実機に合わせて変更してください。

## ディレクトリ構成

```text
examples/nucleo-f767zi/
├── Cargo.toml
├── memory.x
├── .cargo/
│   └── config.toml
└── src/
    └── main.rs
```

## Cargo.toml

```toml
[package]
name = "nucleo-f767zi"
version = "0.1.0"
edition = "2021"

[dependencies]
embassy-executor = { version = "0.7", features = ["arch-cortex-m", "executor-thread", "integrated-timers"] }
embassy-time = { version = "0.4", features = ["tick-hz-32_768"] }
embassy-stm32 = { version = "0.3", features = ["stm32f767zi", "time-driver-any", "exti", "memory-x"] }
embassy-sync = "0.6"
embassy-futures = "0.1"

defmt = "0.3"
defmt-rtt = "0.4"
panic-probe = { version = "0.3", features = ["print-defmt"] }

cortex-m = { version = "0.7", features = ["inline-asm", "critical-section-single-core"] }
cortex-m-rt = "0.7"

[profile.release]
debug = 2
lto = true
opt-level = "s"
```

> **注意**: バージョン番号は最新の Embassy リリースに合わせて調整してください。
> `features` の `"stm32f767zi"` が対象チップを指定しています。

## memory.x

```ld
MEMORY
{
  /* STM32F767ZI: 2MB Flash, 512KB SRAM */
  FLASH : ORIGIN = 0x08000000, LENGTH = 2048K
  RAM   : ORIGIN = 0x20000000, LENGTH = 512K
}
```

## .cargo/config.toml

```toml
[target.'cfg(all(target_arch = "arm", target_os = "none"))']
runner = "probe-rs run --chip STM32F767ZITx"

[target.thumbv7em-none-eabihf]
rustflags = [
  "-C", "link-arg=-Tlink.x",
  "-C", "link-arg=-Tdefmt.x",
]

[build]
target = "thumbv7em-none-eabihf"

[env]
DEFMT_LOG = "trace"
```

## src/main.rs

```rust
#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer};
use panic_probe as _;

// ────────────────────────────────────────────
// 制御層: Command / Status enum
// ────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
enum Command {
    Start,
    Cancel,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
enum Status {
    Started,
    Step(u8),
    Completed,
    Cancelled,
    Stopped,
}

// ────────────────────────────────────────────
// 同期層: static Channel
// ────────────────────────────────────────────

static CMD_CH: Channel<CriticalSectionRawMutex, Command, 4> = Channel::new();
static STATUS_CH: Channel<CriticalSectionRawMutex, Status, 4> = Channel::new();

// ────────────────────────────────────────────
// Task: LED 点滅（動作確認用）
// ────────────────────────────────────────────

#[embassy_executor::task]
async fn led_task(mut led: Output<'static>) {
    loop {
        led.set_high();
        Timer::after(Duration::from_millis(100)).await;
        led.set_low();
        Timer::after(Duration::from_millis(900)).await;
    }
}

// ────────────────────────────────────────────
// Task: ボタン入力 → Command 送信
// ────────────────────────────────────────────

#[embassy_executor::task]
async fn button_task(mut button: ExtiInput<'static>) {
    loop {
        button.wait_for_falling_edge().await;
        info!("button: Start command");
        if CMD_CH.try_send(Command::Start).is_err() {
            warn!("command queue full: Start dropped");
        }
        // デバウンス
        Timer::after(Duration::from_millis(250)).await;
    }
}

// ────────────────────────────────────────────
// Task: Supervisor（状態監視・指示）
// ────────────────────────────────────────────

#[embassy_executor::task]
async fn supervisor_task() {
    loop {
        match STATUS_CH.receive().await {
            Status::Started => {
                info!("supervisor: worker started; cancel after 2 sec");
                Timer::after(Duration::from_secs(2)).await;
                if CMD_CH.try_send(Command::Cancel).is_err() {
                    warn!("command queue full: Cancel dropped");
                }
            }
            s => info!("supervisor: status={:?}", s),
        }
    }
}

// ────────────────────────────────────────────
// Task: Worker（計測シーケンス）
// ────────────────────────────────────────────

#[embassy_executor::task]
async fn worker_task() {
    loop {
        match CMD_CH.receive().await {
            Command::Start => {
                STATUS_CH.send(Status::Started).await;

                match select(measurement_sequence(), wait_cancel_or_stop()).await {
                    Either::First(()) => {
                        safe_cleanup().await;
                        STATUS_CH.send(Status::Completed).await;
                    }
                    Either::Second(Command::Cancel) => {
                        warn!("worker: cancelled");
                        safe_cleanup().await;
                        STATUS_CH.send(Status::Cancelled).await;
                    }
                    Either::Second(Command::Stop) => {
                        warn!("worker: stopped");
                        safe_cleanup().await;
                        STATUS_CH.send(Status::Stopped).await;
                        break;
                    }
                    Either::Second(Command::Start) => {
                        warn!("worker: Start while running; ignored");
                    }
                }
            }
            Command::Cancel => {
                warn!("worker: stale Cancel ignored in idle state");
            }
            Command::Stop => {
                STATUS_CH.send(Status::Stopped).await;
                break;
            }
        }
    }
}

// ────────────────────────────────────────────
// ヘルパー関数
// ────────────────────────────────────────────

/// Cancel または Stop コマンドを待つ
async fn wait_cancel_or_stop() -> Command {
    loop {
        let cmd = CMD_CH.receive().await;
        if matches!(cmd, Command::Cancel | Command::Stop) {
            return cmd;
        }
    }
}

/// 計測シーケンス（30 ステップ × 100ms = 約 3 秒）
async fn measurement_sequence() {
    for step in 0..30u8 {
        STATUS_CH.send(Status::Step(step)).await;
        Timer::after(Duration::from_millis(100)).await;
    }
}

/// ペリフェラルを安全状態に戻す
async fn safe_cleanup() {
    // PWM duty=0, CS deassert, DMA stop, motor off など
    info!("cleanup: peripherals returned to safe state");
    Timer::after(Duration::from_millis(10)).await;
}

// ────────────────────────────────────────────
// main: 初期化 + task spawn
// ────────────────────────────────────────────

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    // NUCLEO-F767ZI: LD1 = PB0, B1 = PC13
    let led = Output::new(p.PB0, Level::Low, Speed::Low);
    let button_input = Input::new(p.PC13, Pull::Up);
    let button = ExtiInput::new(button_input, p.EXTI13);

    spawner.spawn(led_task(led)).unwrap();
    spawner.spawn(button_task(button)).unwrap();
    spawner.spawn(supervisor_task()).unwrap();
    spawner.spawn(worker_task()).unwrap();

    info!("NUCLEO-F767ZI Embassy task-control sample started");

    // main は heartbeat のみ
    loop {
        Timer::after(Duration::from_secs(10)).await;
        info!("main: heartbeat");
    }
}
```

## ビルドと書き込み

### ビルド

```bash
cd examples/nucleo-f767zi
cargo build --release
```

### 書き込み（probe-rs）

```bash
probe-rs run --chip STM32F767ZITx target/thumbv7em-none-eabihf/release/nucleo-f767zi
```

### デバッグログの確認

defmt-rtt を使用しているため、probe-rs の RTT 機能でログを確認できます。

```bash
# probe-rs がログを自動表示
# または別ターミナルで:
probe-rs attach --chip STM32F767ZITx
```

## 動作フロー

```text
1. 起動 → LED 点滅開始、全 task が idle
2. ボタン押下 → button_task が Command::Start を送信
3. worker_task が measurement_sequence を開始
4. supervisor_task が Status::Started を受信し、2 秒タイマ開始
5. 2 秒後 → supervisor が Command::Cancel を送信
6. worker が select で Cancel を検出
7. safe_cleanup() を実行
8. Status::Cancelled を supervisor に返送
9. Idle に戻り、次のボタン押下を待つ
```

## 実装のポイント

| ポイント | 解説 |
|---|---|
| command は `Channel<Command, 4>` | 複数 command の順序を保持 |
| cancel は command の一種 | 専用の Signal を使わず統一 |
| status は `Channel<Status, 4>` | supervisor への ACK 経路 |
| button task は EXTI | ISR で重い処理をしない |
| 計測は 100ms ステップ | cancel 反応性を保つ（最大 100ms 遅延） |
| cleanup は明示的 | drop に頼らず安全状態を保証 |
