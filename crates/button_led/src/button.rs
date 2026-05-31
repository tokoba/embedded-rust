//! button.rs
//! STM32F767ZI NUCLEO-144 ボード用 button 制御モジュール
use defmt::*;

/// チャタリング除去用の待ち時間
pub const DEBOUNCE_MS: u64 = 30; /* チャタリング除去用の待ち時間[msec] */
/// ボタン長押し判定時間
pub const LONG_PRESS_MS: u64 = 1000; /* ボタン長押し判定時間[msec] */

/// ボタン押下系のイベント
#[derive(Clone, Copy, Debug, Format, PartialEq)]
pub enum ButtonEvent {
  /// ボタンの短い押下
  ShortPress, /* 短い押下 < 長押し判定時間未満 */
  /// ボタンの長い押下
  LongPress, /* 長押し > 長押し判定時間以上 */
  /// ボタンのリリース
  Released, /* ボタンリリース */
}

/// ボタンの状態
#[derive(Clone, Copy, Debug, Format, PartialEq)]
pub enum ButtonState {
  /// 押されていない状態
  Released, /* 押されていない状態 */
  /// 押されている状態
  Pressed, /* 押されている状態 */
}
