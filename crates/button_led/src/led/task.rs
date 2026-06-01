//! Led Task

use crate::button::events::ButtonEvent;
use crate::button::task::BUTTON_CH;
use crate::led::config::{
  LED_BLUE_BLINK_PERIOD_MS, LED_GREEN_BLINK_PERIOD_MS, LED_RED_BLINK_PERIOD_MS,
};
use crate::led::{LedControl, LedControlState};

use embassy_stm32::Peri;
use embassy_stm32::peripherals::{PB0, PB7, PB14};

/// LED制御タスク
#[embassy_executor::task]
pub async fn led_task(pb0: Peri<'static, PB0>, pb7: Peri<'static, PB7>, pb14: Peri<'static, PB14>) {
  // タスク開始時に LedControl を初期化（loop の外で一度だけ）
  let mut led_green = LedControl::new(pb0, LedControlState::Off, LED_GREEN_BLINK_PERIOD_MS, 0);
  let mut led_blue = LedControl::new(pb7, LedControlState::Off, LED_BLUE_BLINK_PERIOD_MS, 0);
  let mut led_red = LedControl::new(pb14, LedControlState::Off, LED_RED_BLINK_PERIOD_MS, 0);

  loop {
    let event = BUTTON_CH.receive().await;
    match event {
      ButtonEvent::ShortPress => {
        led_green.on();
        (led_blue).off();
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
