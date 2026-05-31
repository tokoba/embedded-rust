//! button_led library.
#![cfg_attr(not(test), no_std)]

#[cfg(all(test, not(target_os = "none")))]
extern crate std;

/// button_fsm は常に公開（純粋ロジックのため）
pub mod button_fsm;

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub mod led;

/// システムエラー
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
