#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Level, Output, Pull, Speed};
use embassy_stm32::mode::Async;
use embassy_stm32::{bind_interrupts, exti, interrupt};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer};
use panic_probe as _;

// ────────────────────────────────────────────
// 制御層: Command / Status enum
// ────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
#[allow(dead_code)]
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
// 割り込み制御
// ────────────────────────────────────────────
bind_interrupts!(
  /// 割り込みハンドラ定義
    pub struct Irqs{
        EXTI15_10 => exti::InterruptHandler<interrupt::typelevel::EXTI15_10>;
});

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

/// PeriMode は Async に設定しておく
#[embassy_executor::task]
async fn button_task(mut button: ExtiInput<'static, Async>) {
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
            warn!("worker: Start received while running; ignored");
          }
        }
      }
      Command::Cancel => {
        // Idle 中の Cancel は古い停止要求として破棄する。
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
/// 100ms ごとに await するため、キャンセル反応性が保たれる。
async fn measurement_sequence() {
  for step in 0..30u8 {
    STATUS_CH.send(Status::Step(step)).await;
    Timer::after(Duration::from_millis(100)).await;
  }
}

/// ペリフェラルを安全状態に戻す
/// ここで PWM duty=0, CS deassert, DMA stop, motor off などを行う。
/// このサンプルではログのみ。
async fn safe_cleanup() {
  info!("cleanup: peripherals returned to safe state");
  Timer::after(Duration::from_millis(10)).await;
}

// ────────────────────────────────────────────
// main: 初期化 + task spawn
// ────────────────────────────────────────────

#[embassy_executor::main]
async fn main(spawner: Spawner) {
  let p = embassy_stm32::init(Default::default());

  // NUCLEO-F767ZI: LD1 は PB0、B1 は PC13。
  // ボードリビジョン差がある場合は該当ピンを変更する。
  let led = Output::new(p.PB0, Level::Low, Speed::Low);
  let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down, Irqs);

  spawner.spawn(unwrap!(led_task(led)));
  spawner.spawn(unwrap!(button_task(button)));
  spawner.spawn(unwrap!(supervisor_task()));
  spawner.spawn(unwrap!(worker_task()));

  info!("NUCLEO-F767ZI Embassy task-control sample started");

  // main は heartbeat のみ
  loop {
    Timer::after(Duration::from_secs(10)).await;
    info!("main: heartbeat");
  }
}
