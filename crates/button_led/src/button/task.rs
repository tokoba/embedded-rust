//! Button Task

use defmt::info;

use crate::button::{
  events::{ButtonEvent, PhysicalEvent},
  fsm::{ButtonFsm, ButtonFsmState, WaitSpec},
  states::ButtonState,
};

use embassy_executor;
use embassy_futures::select::{Either, select};
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::mode::Async;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};

// タスク間通信(LED task <-> Button task)用のチャンネル
const BUTTON_EVENT_BUFFER_SIZE: usize = 3;
/// ボタン状態変更通知チャンネル
pub static BUTTON_CH: Channel<CriticalSectionRawMutex, ButtonEvent, BUTTON_EVENT_BUFFER_SIZE> =
  Channel::new();

/// 汎用ボタン監視タスク
/// Embassy タスクは 'static ライフタイムの引数のみ受け付けるため、
/// 必要なペリフェラルピンを個別に受け取る
///
/// FSM の next_wait() が返す WaitSpec に従い待機→結果をフィードバックする
/// 単純なイベントループ。タイミング知識は全て FSM 側が保持する。
#[embassy_executor::task]
pub async fn button_watcher_task(mut button: ExtiInput<'static, Async>, name: &'static str) {
  // 初期状態の判定と通知
  let initial_state = if button.is_low() {
    ButtonState::Released
  } else {
    ButtonState::Pressed
  };

  // fsm を初期状態に合わせて更新
  let mut fsm = match initial_state {
    ButtonState::Released => ButtonFsm::new(ButtonFsmState::Idle),
    ButtonState::Pressed => ButtonFsm::new(ButtonFsmState::DebouncingPress),
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

/// FSMにイベントを送信し、ButtonEventが発生した場合はチャンネルに転送する
pub async fn process_fsm_event(fsm: &mut ButtonFsm, event: PhysicalEvent, now_ms: u64) {
  if let Some(button_event) = fsm.on_event(event, now_ms) {
    BUTTON_CH.send(button_event).await;
    info!("[Button] FSM emitted: {}", button_event);
  }
}
