//! lib.rs
//! blinky プロジェクトのライブラリー
#![cfg_attr(not(test), no_std)]

// テスト時にのみ std をリンク（MCU ターゲットでは無効）
#[cfg(all(test, not(target_os = "none")))]
extern crate std;

// ARM ターゲット（MCU）でのみ led, button モジュールをビルド
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub mod button;
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub mod button_fsm;
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub mod led;

// button_fsm はホストテスト可能
#[cfg(all(test, not(target_os = "none")))]
pub mod button_fsm;

/// システムエラー定義
#[derive(Debug, PartialEq, Eq)]
pub enum SystemError {
  /// 配列スライスが空
  EmptySlice,
}

/// 与えられた引数配列の数値の中の最小値を返す
/// ここでは u64 に限定する
pub fn min(values: &[u64]) -> Result<u64, SystemError> {
  values.iter().copied().min().ok_or(SystemError::EmptySlice)
}

// ホスト向けユニットテスト
#[cfg(all(test, not(target_os = "none")))]
mod tests {
  use super::*;

  #[test]
  fn test_min() {
    let numbers: [u64; 3] = [100, 50, 200];
    assert_eq!(min(&numbers), Ok(50));
  }

  #[test]
  fn test_min_empty_slice() {
    let numbers: [u64; 0] = [];
    assert_eq!(min(&numbers), Err(SystemError::EmptySlice));
  }
}
