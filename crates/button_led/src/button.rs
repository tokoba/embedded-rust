//! STM32F767ZI NUCLEO-144 ボード用 button 制御モジュール
use defmt::Format;

pub const DEBOUNCE_MS: u64 = 30;
pub const LONG_PRESS_MS: u64 = 1000;

#[derive(Clone, Copy, Debug, Format, PartialEq, Eq)]
pub enum ButtonEvent { ShortPress, LongPress, Released }

#[derive(Clone, Copy, Debug, Format, PartialEq, Eq)]
pub enum ButtonState { Released, Pressed }
