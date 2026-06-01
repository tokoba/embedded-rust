//! STM32F767ZI NUCLEO-144 ボード用 LED 操作用モジュール
use embassy_stm32::Peri;
use embassy_stm32::gpio::{self, Level, Output, Speed};

pub mod config;
pub mod states;
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub mod task;

use crate::led::states::{LedControlState, LedPortState};

/// LED制御
pub struct LedControl<'d> {
  /// 出力ピン指定
  pin: Output<'d>,
  /// ポート状態
  port_state: LedPortState,
  /// 制御状態
  control_state: LedControlState,
  /// 点滅周期
  blink_period_ms: u64,
  /// 点滅周期更新時刻
  last_blink_toggle_time: u64,
}

impl<'d> core::fmt::Debug for LedControl<'d> {
  /// デバッグ情報の表示
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("LedControl")
      .field("port_state", &self.port_state)
      .field("control_state", &self.control_state)
      .field("blink_period_ms", &self.blink_period_ms)
      .field("last_blink_toggle_time", &self.last_blink_toggle_time)
      .finish_non_exhaustive()
  }
}

impl<'d> LedControl<'d> {
  /// LED制御の初期化
  pub fn new(
    pin: Peri<'d, impl gpio::Pin>,
    control_state: LedControlState,
    blink_period_ms: u64,
    last_blink_toggle_time: u64,
  ) -> Self {
    let pin = Output::new(pin, Level::Low, Speed::Low);
    Self {
      pin,
      port_state: if control_state == LedControlState::On {
        LedPortState::On
      } else {
        LedPortState::Off
      },
      control_state,
      blink_period_ms,
      last_blink_toggle_time,
    }
  }
  /// LEDを点灯させる
  pub fn on(&mut self) {
    self.pin.set_high();
    self.port_state = LedPortState::On;
    self.control_state = LedControlState::On;
  }
  /// LEDを消灯させる
  pub fn off(&mut self) {
    self.pin.set_low();
    self.port_state = LedPortState::Off;
    self.control_state = LedControlState::Off;
  }
  /// LEDを点滅させる
  pub fn blink(&mut self) {
    self.pin.set_low();
    self.port_state = LedPortState::Off;
    self.control_state = LedControlState::Blink;
  }
  /// LEDのポート状態を反転させる
  pub fn toggle(&mut self) -> LedPortState {
    if self.control_state == LedControlState::Blink {
      match self.port_state {
        LedPortState::Off => {
          self.pin.set_high();
          self.port_state = LedPortState::On;
        }
        LedPortState::On => {
          self.pin.set_low();
          self.port_state = LedPortState::Off;
        }
      }
    }
    self.port_state
  }
  /// 必要に応じてLEDのポート状態を反転させる
  pub fn toggle_if_required(&mut self, current_time: u64) -> LedPortState {
    if self.control_state == LedControlState::Blink
      && current_time.saturating_sub(self.last_blink_toggle_time) > self.blink_period_ms / 2
    {
      self.toggle();
      self.last_blink_toggle_time = current_time;
    }
    self.port_state
  }
  /// 点滅周期を設定する
  pub fn set_blink_period(&mut self, blink_period_ms: u64) {
    self.blink_period_ms = blink_period_ms;
  }
}
