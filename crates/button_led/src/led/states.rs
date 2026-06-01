//! Led 状態定義

// ✅ defmt は ARM ターゲットでのみ import
#[cfg(all(target_arch = "arm", target_os = "none"))]
use defmt::Format;

/// LEDの表示名
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, Format)]
pub enum LedDisplayName {
  Green,
  Blue,
  Red,
}

/// LEDのポート状態
#[allow(missing_docs)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Format)]
pub enum LedPortState {
  Off,
  On,
}

/// LED制御状態
#[allow(missing_docs)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Format)]
pub enum LedControlState {
  Off,
  On,
  Blink,
}
