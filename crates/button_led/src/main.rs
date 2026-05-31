//! 割り込みを用いたボタンの押下検出
#![no_std]
#![no_main]
#![cfg(all(target_arch = "arm", target_os = "none"))]

use core::default::Default;

use button_led::button_fsm::{
  ButtonEvent, ButtonFsm, ButtonFsmState, ButtonState, DEBOUNCE_MS, LONG_PRESS_MS, PhysicalEvent,
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
use embassy_time::{Duration, Instant, Timer}; /* チャタリング防止機能用の共有チャンネル */
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
    info!("[Button] FSM event: {}", button_event);
  }
}

/// 汎用ボタン監視タスク
/// Embassy タスクは 'static ライフタイムの引数のみ受け付けるため、
/// 必要なペリフェラルピンを個別に受け取る
#[embassy_executor::task]
async fn button_watcher_task(mut button: ExtiInput<'static, Async>, name: &'static str) {
  let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);

  let mut button_state = if button.is_low() {
    ButtonState::Released
  } else {
    ButtonState::Pressed
  };

  if button_state == ButtonState::Released {
    BUTTON_CH.send(ButtonEvent::Released).await;
  } else {
    BUTTON_CH.send(ButtonEvent::ShortPress).await;
  }
  info!("[Button] Current: {}", button_state);
  info!("Press the {} button...", name);

  loop {
    if button_state == ButtonState::Released {
      // ボタンが押されていないときは，押下イベントを待つ
      /* Active High (ボタン押下時にHigh) */
      button.wait_for_rising_edge().await;
      button_state = ButtonState::Pressed;
      info!("[Button] Current: {}", button_state);

      info!("Checking button press type: Short Press or Long Press");
    } else {
      // ボタンがすでに押されているときは，リリースイベントを待つ
      // 最初からボタンが押された状態スタートとする
      info!("{}: Started with Pressed state!", name); /* ボタン押下は確定 */
    }

    /* FSM: 立ち上がりエッジ → DebouncingPress */
    let now = Instant::now().as_millis();
    process_fsm_event(&mut fsm, PhysicalEvent::RisingEdge, now).await;

    /* チャタリング対策の待ち */
    Timer::after(Duration::from_millis(DEBOUNCE_MS)).await;

    /* 待ち時間後にI/O再確認(ここは割り込みではなくタスクでReadする) */
    if button.is_high() {
      /* 現在も押されている状態継続と判断。
      FSM: タイムアウト → Pressed, ShortPress発行 */
      let now = Instant::now().as_millis();
      process_fsm_event(&mut fsm, PhysicalEvent::Timeout, now).await;
      info!("[Button] Short Pressed");
    } else {
      /* 一定時間後にlowになっているのでチャタリングと判断(ノイズ扱い) */
      /* FSM: 立ち下がりエッジ → Idle（ノイズ除去） */
      let now = Instant::now().as_millis();
      process_fsm_event(&mut fsm, PhysicalEvent::FallingEdge, now).await;
      button_state = ButtonState::Released;
      info!("[Button] Current: {}", button_state);
      continue; /* 無視する */
    }

    info!("{}: Pressed!", name); /* ボタン押下は確定 */
    let button_press_start_time = Instant::now(); /* ボタン押下開始判定時刻 */
    let mut long_press_detected = false;

    loop {
      let elapsed = Instant::now() - button_press_start_time; /* 経過時間 */
      if elapsed > Duration::from_millis(LONG_PRESS_MS) {
        long_press_detected = true;
        /* FSM: タイムアウト → LongPressDetected, LongPress発行 */
        let now = Instant::now().as_millis();
        process_fsm_event(&mut fsm, PhysicalEvent::Timeout, now).await;
        info!("[Button] Long Pressed");
        break; /* 長押し確定 */
        /* この後も長押し状態からのリリース待ち状態に遷移する */
      }

      let remaining_for_long_press = Duration::from_millis(LONG_PRESS_MS) - elapsed; /* すでに経過している時間を差し引いてレースさせる */

      /* 長押し判定 or ボタンリリース判定のレース開始。
      一定時間以上，押下継続であれば長押しと判定する。
      先にボタンリリースされた場合は判定終了。 */
      match select(
        button.wait_for_falling_edge(),         /* ボタンリリース待ち */
        Timer::after(remaining_for_long_press), /* 長押し判定 */
      )
      .await
      {
        /* リリース or 長押しのどちらかのイベントは必ず発生する */
        /* リリースが先の場合の条件分岐 */
        Either::First(_) => {
          /* FSM: 立ち下がりエッジ → DebouncingReleaseFromPress */
          let now = Instant::now().as_millis();
          process_fsm_event(&mut fsm, PhysicalEvent::FallingEdge, now).await;

          /* リリース時も機械的なチャタリング対策待ち */
          Timer::after(Duration::from_millis(DEBOUNCE_MS)).await;
          if button.is_low() {
            /* ボタンリリース継続のため，ボタンリリース確定とする */
            /* FSM: タイムアウト → Idle, Released発行 */
            let now = Instant::now().as_millis();
            process_fsm_event(&mut fsm, PhysicalEvent::Timeout, now).await;
            button_state = ButtonState::Released;
            info!("[Button] Current: {}", button_state);
            info!("[Button] Released After Short Pressed");
            break;
          } else {
            /* リリース判定は誤検知で単なるチャタリングと認定 */
            /* FSM: 立ち上がりエッジ → Pressed に復帰 */
            let now = Instant::now().as_millis();
            process_fsm_event(&mut fsm, PhysicalEvent::RisingEdge, now).await;
            continue;
          }
        }
        Either::Second(_) => {
          /* 長押しの場合，一発で確定してよい */
          long_press_detected = true;
          /* FSM: タイムアウト → LongPressDetected, LongPress発行 */
          let now = Instant::now().as_millis();
          process_fsm_event(&mut fsm, PhysicalEvent::Timeout, now).await;
          info!("[Button] Long Pressed");
          break; /* 長押し確定でループを抜け、リリース待ちに遷移する */
        }
      }
    }

    if !long_press_detected {
      /* 短い押下確定の場合は処理なし(すでにリリースされているため) */
      continue;
    }

    /* 長押しの場合はさらに継続してリリースを確認する */
    loop {
      button.wait_for_falling_edge().await;
      /* FSM: 立ち下がりエッジ → DebouncingReleaseFromLongPress */
      let now = Instant::now().as_millis();
      process_fsm_event(&mut fsm, PhysicalEvent::FallingEdge, now).await;

      Timer::after(Duration::from_millis(DEBOUNCE_MS)).await;
      if button.is_low() {
        /* ボタンリリース確定 */
        /* FSM: タイムアウト → Idle, Released発行 */
        let now = Instant::now().as_millis();
        process_fsm_event(&mut fsm, PhysicalEvent::Timeout, now).await;
        button_state = ButtonState::Released;
        info!("[Button] Current: {}", button_state);
        info!("[Button] Released After Long Pressed");
        break; /* 長押し後のリリース確定でループを抜け、最初の押下待ちに戻る */
      } else {
        /* チャタリングと判定。リリースは誤検知なので無視 */
        /* FSM: 立ち上がりエッジ → LongPressDetected に復帰 */
        let now = Instant::now().as_millis();
        process_fsm_event(&mut fsm, PhysicalEvent::RisingEdge, now).await;
        continue; /* 長押しの後のリリース待ち継続 */
      }
    }
  }
}
