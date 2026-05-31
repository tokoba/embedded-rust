//! 割り込みを用いたボタンの押下検出
#![no_std]
#![no_main]
#![cfg(all(target_arch = "arm", target_os = "none"))]

use core::default::Default;

use button_led::button_fsm::{
  ButtonEvent, ButtonFsm, ButtonFsmState, ButtonState, PhysicalEvent, WaitSpec,
};
use button_led::led::*;
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_stm32::exti::{self, ExtiInput};
use embassy_stm32::gpio::Pull;
use embassy_stm32::mode::Async;
use embassy_stm32::peripherals::{PB0, PB7, PB14};
use embassy_stm32::{Peri, bind_interrupts, interrupt};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex; /* 排他制御用 */
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};
use {defmt_rtt as _, panic_probe as _};

// タスク間通信(LED task <-> Button task)用のチャンネル
const BUTTON_EVENT_BUFFER_SIZE: usize = 3;
static BUTTON_CH: Channel<CriticalSectionRawMutex, ButtonEvent, BUTTON_EVENT_BUFFER_SIZE> =
  Channel::new();

// 割り込み検出部分
bind_interrupts!(
  /// 割り込みハンドラ定義
    pub struct Irqs{
        EXTI15_10 => exti::InterruptHandler<interrupt::typelevel::EXTI15_10>;
});

/// メインの制御ループ
#[embassy_executor::main]
async fn main(spawner: Spawner) {
  let config = embassy_stm32::Config::default();
  let p = embassy_stm32::init(config);
  info!("Hello World!");

  // STM32F767ZI標準状態ではユーザーボタンは PC13 に割り当てられている
  // (SB17: ON, SB18: OFF)の半田付け状態であるため
  // embassy-stm32 0.5.0 では ExtiInput は既に型消去されているので
  // そのままタスクに渡せる
  let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down, Irqs);

  // ボタン監視タスクに関し処理を委譲する
  // 必要なペリフェラルピンを個別に渡す（Embassy タスクは 'static ライフタイムが必要）
  spawner.spawn(button_watcher_task(button, "USER").unwrap());
  spawner.spawn(led_task(p.PB0, p.PB7, p.PB14).unwrap());
}

/// LED制御タスク
#[embassy_executor::task]
async fn led_task(pb0: Peri<'static, PB0>, pb7: Peri<'static, PB7>, pb14: Peri<'static, PB14>) {
  // タスク開始時に LedControl を初期化（loop の外で一度だけ）
  let mut led_green = LedControl::new(pb0, LedControlState::Off, LED_GREEN_BLINK_PERIOD_MS, 0);
  let mut led_blue = LedControl::new(pb7, LedControlState::Off, LED_BLUE_BLINK_PERIOD_MS, 0);
  let mut led_red = LedControl::new(pb14, LedControlState::Off, LED_RED_BLINK_PERIOD_MS, 0);

  loop {
    let event = BUTTON_CH.receive().await;
    match event {
      ButtonEvent::ShortPress => {
        led_green.on();
        led_blue.off();
        led_red.off();
      }
      ButtonEvent::LongPress => {
        led_green.off();
        led_blue.on();
        led_red.off();
      }
      ButtonEvent::Released => {
        led_green.off();
        led_blue.off();
        led_red.on();
      }
    }
  }
}

/// FSMにイベントを送信し、ButtonEventが発生した場合はチャンネルに転送する
async fn process_fsm_event(fsm: &mut ButtonFsm, event: PhysicalEvent, now_ms: u64) {
  if let Some(button_event) = fsm.on_event(event, now_ms) {
    BUTTON_CH.send(button_event).await;
    info!("[Button] FSM emitted: {}", button_event);
  }
}

/// 汎用ボタン監視タスク
/// Embassy タスクは 'static ライフタイムの引数のみ受け付けるため、
/// 必要なペリフェラルピンを個別に受け取る
///
/// FSM の next_wait() が返す WaitSpec に従い待機→結果をフィードバックする
/// 単純なイベントループ。タイミング知識は全て FSM 側が保持する。
#[embassy_executor::task]
async fn button_watcher_task(mut button: ExtiInput<'static, Async>, name: &'static str) {
  let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);

  // 初期状態の判定と通知
  let initial_state = if button.is_low() {
    ButtonState::Released
  } else {
    ButtonState::Pressed
  };

  if initial_state == ButtonState::Released {
    BUTTON_CH.send(ButtonEvent::Released).await;
  } else {
    BUTTON_CH.send(ButtonEvent::ShortPress).await;
  }
  info!("[Button] Initial: {}", initial_state);
  info!("Press the {} button...", name);

  // FSM駆動のイベントループ
  loop {
    let now = Instant::now().as_millis();
    match fsm.next_wait(now) {
      // --- ボタン押下待ち ---
      WaitSpec::RisingEdge => {
        button.wait_for_rising_edge().await;
        info!("[Button] RisingEdge detected");
        let now = Instant::now().as_millis();
        process_fsm_event(&mut fsm, PhysicalEvent::RisingEdge, now).await;
      }

      // --- ディバウンス待ち（プレス確認 / リリース確認 共通） ---
      WaitSpec::Debounce(ms) => {
        Timer::after(Duration::from_millis(ms)).await;
        let now = Instant::now().as_millis();
        let event = if button.is_high() {
          PhysicalEvent::DebouncedHigh
        } else {
          PhysicalEvent::DebouncedLow
        };
        info!("[Button] Debounce result: {}", event);
        process_fsm_event(&mut fsm, event, now).await;
      }

      // --- 長押し判定レース（FallingEdge vs Timeout） ---
      WaitSpec::FallingEdgeOrTimeout(ms) => {
        match select(
          button.wait_for_falling_edge(),
          Timer::after(Duration::from_millis(ms)),
        )
        .await
        {
          Either::First(_) => {
            info!("[Button] FallingEdge during press");
            let now = Instant::now().as_millis();
            process_fsm_event(&mut fsm, PhysicalEvent::FallingEdge, now).await;
          }
          Either::Second(_) => {
            info!("[Button] Long press timeout");
            let now = Instant::now().as_millis();
            process_fsm_event(&mut fsm, PhysicalEvent::Timeout, now).await;
          }
        }
      }

      // --- 長押し後のリリース待ち ---
      WaitSpec::FallingEdge => {
        button.wait_for_falling_edge().await;
        info!("[Button] FallingEdge after long press");
        let now = Instant::now().as_millis();
        process_fsm_event(&mut fsm, PhysicalEvent::FallingEdge, now).await;
      }
    }
  }
}
