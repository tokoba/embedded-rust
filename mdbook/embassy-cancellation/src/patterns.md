# 実践パターン

この章では、Embassy でタスク制御を実装する際の代表的なパターンを紹介します。

## パターン A: supervisor + workers + state watch

最も **汎用的** な構成です。複雑な装置制御の基本形として推奨します。

```text
supervisor_task
 ├─ 外部 command を受信 (UART, Button, Channel)
 ├─ SystemState を Watch で全 task に配信
 ├─ worker へ個別 Command を送信
 └─ Status / ACK を待ち、次の遷移を判断

worker_task
 ├─ Command::Start を待つ
 ├─ work future と cancel future を select
 ├─ cleanup (PWM off, DMA stop, CS deassert)
 └─ Status を supervisor に返す
```

### 実装例（STM32F767ZI）

```rust
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::Watch;
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Timer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum Command { Start, Cancel, Stop }

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum Status { Started, Completed, Cancelled, Stopped, Fault }

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum SystemState { Boot, Running, StopRequested, Fault }

static CMD_CH: Channel<CriticalSectionRawMutex, Command, 4> = Channel::new();
static STATUS_CH: Channel<CriticalSectionRawMutex, Status, 4> = Channel::new();
static SYS_STATE: Watch<CriticalSectionRawMutex, SystemState, 4> = Watch::new();

#[embassy_executor::task]
async fn supervisor_task() {
    SYS_STATE.sender().send(SystemState::Running);

    loop {
        match STATUS_CH.receive().await {
            Status::Started => {
                info!("supervisor: worker started");
                // 2 秒後にキャンセル（デモ用）
                Timer::after(Duration::from_secs(2)).await;
                CMD_CH.send(Command::Cancel).await;
            }
            Status::Completed => {
                info!("supervisor: work completed normally");
            }
            Status::Cancelled => {
                info!("supervisor: worker cancelled successfully");
            }
            Status::Stopped => {
                info!("supervisor: worker stopped");
                SYS_STATE.sender().send(SystemState::StopRequested);
                break;
            }
            Status::Fault => {
                error!("supervisor: worker fault!");
                SYS_STATE.sender().send(SystemState::Fault);
            }
        }
    }
}

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
                        safe_cleanup().await;
                        STATUS_CH.send(Status::Cancelled).await;
                    }
                    Either::Second(Command::Stop) => {
                        safe_cleanup().await;
                        STATUS_CH.send(Status::Stopped).await;
                        return;
                    }
                    _ => {}
                }
            }
            Command::Cancel => {
                warn!("stale Cancel ignored in idle state");
            }
            Command::Stop => {
                STATUS_CH.send(Status::Stopped).await;
                return;
            }
        }
    }
}

async fn wait_cancel_or_stop() -> Command {
    loop {
        let cmd = CMD_CH.receive().await;
        if matches!(cmd, Command::Cancel | Command::Stop) {
            return cmd;
        }
    }
}

async fn measurement_sequence() {
    for step in 0..30u8 {
        info!("measurement step {}", step);
        Timer::after(Duration::from_millis(100)).await;
    }
}

async fn safe_cleanup() {
    info!("cleanup: peripherals returned to safe state");
    // PWM duty = 0, CS deassert, DMA stop, motor off など
    Timer::after(Duration::from_millis(10)).await;
}
```

**向く用途**: モード遷移が複雑、フェールセーフが必要、複数 peripheral の協調制御

## パターン B: device owner task

共有 peripheral を複数 task から直接触らず、**owner task に閉じ込める** パターンです。

```text
application_task
  → Channel<DeviceCommand>
      → device_owner_task
          → peripheral 操作
          → Channel<DeviceResult>
```

### 実装例（SPI owner）

```rust
#[derive(Clone, Copy)]
pub enum SpiCommand {
    ReadSensor,
    WriteConfig(u8),
    Cancel,
}

#[derive(Clone, Copy)]
pub enum SpiResult {
    SensorData([u8; 4]),
    WriteOk,
    Cancelled,
    Error,
}

static SPI_CMD: Channel<CriticalSectionRawMutex, SpiCommand, 4> = Channel::new();
static SPI_RESULT: Channel<CriticalSectionRawMutex, SpiResult, 4> = Channel::new();

#[embassy_executor::task]
async fn spi_owner_task(
    mut spi: Spi<'static, Async>,
    mut cs: Output<'static>,
) {
    loop {
        match SPI_CMD.receive().await {
            SpiCommand::ReadSensor => {
                cs.set_low();
                let mut buf = [0u8; 4];
                match spi.transfer_in_place(&mut buf).await {
                    Ok(()) => {
                        cs.set_high();
                        SPI_RESULT.send(SpiResult::SensorData(buf)).await;
                    }
                    Err(_) => {
                        cs.set_high();
                        SPI_RESULT.send(SpiResult::Error).await;
                    }
                }
            }
            SpiCommand::WriteConfig(val) => {
                cs.set_low();
                let result = spi.write(&[0x80, val]).await;
                cs.set_high();
                match result {
                    Ok(()) => SPI_RESULT.send(SpiResult::WriteOk).await,
                    Err(_) => SPI_RESULT.send(SpiResult::Error).await,
                }
            }
            SpiCommand::Cancel => {
                cs.set_high(); // 安全状態
                SPI_RESULT.send(SpiResult::Cancelled).await;
            }
        }
    }
}
```

**向く用途**: UART / SPI / I2C / CAN / USB、トランザクション順序保証、cancel-safe 不明のドライバ

## パターン C: per-operation cancel

長い単発処理を停止する **最小構成** です。

```rust
use embassy_sync::signal::Signal;
use embassy_futures::select::{select, Either};

static CANCEL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

async fn cancellable_calibration() -> Result<CalibrationResult, ()> {
    CANCEL.reset(); // 古い signal をクリア

    match select(calibration_sequence(), CANCEL.wait()).await {
        Either::First(result) => Ok(result),
        Either::Second(_) => {
            calibration_cleanup().await;
            Err(())
        }
    }
}
```

**向く用途**: homing、calibration、firmware update、長時間 measurement

## パターン D: timer task を spawn しない

`Timer::after` のためだけに task を都度 spawn しないでください。
既存 worker 内で timer future と cancel future を `select` します。

```rust
// ✗ timer のためだけに task を spawn
#[embassy_executor::task]
async fn timeout_task() {
    Timer::after_secs(10).await;
    // timeout 処理
}

// ✓ 既存 worker 内で select
match select(Timer::after_secs(10), CANCEL.wait()).await {
    Either::First(_) => {
        // タイムアウト発生
        timeout_handler().await;
    }
    Either::Second(_) => {
        // タイムアウト前にキャンセル
        cancel_handler().await;
    }
}
```

## パターン E: enum state machine

複雑な制御では、`bool` flag の組み合わせよりも **enum state machine** が安全です。

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
enum MotorState {
    Idle,
    Homing,
    Running { speed: u16 },
    Stopping,
    Fault { code: u8 },
}

#[embassy_executor::task]
async fn motor_task() {
    let mut state = MotorState::Idle;

    loop {
        state = match state {
            MotorState::Idle => {
                match CMD_CH.receive().await {
                    Command::Start => {
                        motor_enable().await;
                        MotorState::Homing
                    }
                    _ => MotorState::Idle,
                }
            }
            MotorState::Homing => {
                match select(homing_sequence(), wait_cancel()).await {
                    Either::First(Ok(())) => MotorState::Running { speed: 1000 },
                    Either::First(Err(_)) => MotorState::Fault { code: 0x01 },
                    Either::Second(_) => MotorState::Stopping,
                }
            }
            MotorState::Running { speed } => {
                match select(run_motor(speed), wait_cancel()).await {
                    Either::First(()) => MotorState::Idle,
                    Either::Second(_) => MotorState::Stopping,
                }
            }
            MotorState::Stopping => {
                motor_safe_stop().await;
                STATUS_CH.send(Status::Stopped).await;
                MotorState::Idle
            }
            MotorState::Fault { code } => {
                error!("motor fault: {:#x}", code);
                motor_safe_stop().await;
                STATUS_CH.send(Status::Fault).await;
                // ResetFault を待つ
                loop {
                    if let Command::ResetFault = CMD_CH.receive().await {
                        break;
                    }
                }
                MotorState::Idle
            }
        };
    }
}
```

**利点**: 状態遷移が明示的で、不正な状態組み合わせがコンパイル時に防げる

## パターン F: button debounce + command 送信

NUCLEO-F767ZI のボタン（PC13）を使った実践例です。

```rust
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Pull};

#[embassy_executor::task]
async fn button_task(mut button: ExtiInput<'static>) {
    loop {
        button.wait_for_falling_edge().await;
        info!("button pressed");

        if CMD_CH.try_send(Command::Start).is_err() {
            warn!("command queue full");
        }

        // デバウンス: 250ms 間の追加プレスを無視
        Timer::after(Duration::from_millis(250)).await;
    }
}

// main での初期化
// let button_input = Input::new(p.PC13, Pull::Up);
// let button = ExtiInput::new(button_input, p.EXTI13);
// spawner.spawn(button_task(button)).unwrap();
```

**ポイント**: EXTI を使うことで、ポーリングなしで省電力にボタン入力を検出できます。
ISR で重い処理をせず、`try_send` で command を送るだけにします。
