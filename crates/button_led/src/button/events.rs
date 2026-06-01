//! Button Events 定義
//!

// ✅ defmt は ARM ターゲットでのみ import
#[cfg(all(target_arch = "arm", target_os = "none"))]
use defmt::Format;

/// ボタンから外部に通知するイベント
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(all(target_arch = "arm", target_os = "none"), derive(Format))]
pub enum ButtonEvent {
  /// 短押しイベント（ディバウンス確認後に即時発行）
  ShortPress,
  /// 長押しイベント
  LongPress,
  /// リリースイベント
  Released,
}

/// GPIOから入力される物理的なイベント
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(all(target_arch = "arm", target_os = "none"), derive(Format))]
pub enum PhysicalEvent {
  /// 立ち上がりエッジ検出
  RisingEdge,
  /// 立ち下がりエッジ検出
  FallingEdge,
  /// ディバウンス待ち完了後、ピンがHigh（押下継続）
  DebouncedHigh,
  /// ディバウンス待ち完了後、ピンがLow（リリースまたはノイズ）
  DebouncedLow,
  /// タイムアウト（長押し判定用）
  Timeout,
}
