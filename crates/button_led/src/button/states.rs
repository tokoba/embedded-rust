//! Button 状態モジュール

// ✅ defmt は ARM ターゲットでのみ import
#[cfg(all(target_arch = "arm", target_os = "none"))]
use defmt::Format;

/// ボタンの状態
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(all(target_arch = "arm", target_os = "none"), derive(Format))]
pub enum ButtonState {
  /// リリース状態
  Released,
  /// プレス状態
  Pressed,
}
