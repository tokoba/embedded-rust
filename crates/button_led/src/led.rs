//! STM32F767ZI NUCLEO-144 ボード用 LED 操作用モジュール
use defmt::Format;
use embassy_stm32::gpio::{self, Level, Output, Speed};
use embassy_stm32::Peri;

pub const LED_GREEN_BLINK_PERIOD_MS: u64 = 300;
pub const LED_BLUE_BLINK_PERIOD_MS: u64 = 1000;
pub const LED_RED_BLINK_PERIOD_MS: u64 = 2000;

#[derive(Copy, Clone, Debug, Format)]
pub enum LedDisplayName { Green, Blue, Red }

#[derive(Clone, Copy, PartialEq, Eq, Debug, Format)]
pub enum LedPortState { Off, On }

#[derive(Clone, Copy, PartialEq, Eq, Debug, Format)]
pub enum LedControlState { Off, On, Blink }

pub struct LedControl<'d> {
  pin: Output<'d>,
  port_state: LedPortState,
  control_state: LedControlState,
  blink_period_ms: u64,
  last_blink_toggle_time: u64,
}

impl<'d> core::fmt::Debug for LedControl<'d> {
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
  pub fn new(pin: Peri<'d, impl gpio::Pin>, control_state: LedControlState, blink_period_ms: u64, last_blink_toggle_time: u64) -> Self {
    let pin = Output::new(pin, Level::Low, Speed::Low);
    Self { pin, port_state: if control_state == LedControlState::On { LedPortState::On } else { LedPortState::Off }, control_state, blink_period_ms, last_blink_toggle_time }
  }
  pub fn on(&mut self) { self.pin.set_high(); self.port_state = LedPortState::On; self.control_state = LedControlState::On; }
  pub fn off(&mut self) { self.pin.set_low(); self.port_state = LedPortState::Off; self.control_state = LedControlState::Off; }
  pub fn blink(&mut self) { self.pin.set_low(); self.port_state = LedPortState::Off; self.control_state = LedControlState::Blink; }
  pub fn toggle(&mut self) -> LedPortState {
    if self.control_state == LedControlState::Blink {
      match self.port_state { LedPortState::Off => { self.pin.set_high(); self.port_state = LedPortState::On; }, LedPortState::On => { self.pin.set_low(); self.port_state = LedPortState::Off; } }
    }
    self.port_state
  }
  pub fn toggle_if_required(&mut self, current_time: u64) -> LedPortState {
    if self.control_state == LedControlState::Blink && current_time.saturating_sub(self.last_blink_toggle_time) > self.blink_period_ms / 2 {
      self.toggle(); self.last_blink_toggle_time = current_time;
    }
    self.port_state
  }
  pub fn set_blink_period(&mut self, blink_period_ms: u64) { self.blink_period_ms = blink_period_ms; }
}
