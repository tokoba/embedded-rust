//! button_led library.
#![cfg_attr(not(test), no_std)]

#[cfg(all(test, not(target_os = "none")))]
extern crate std;

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub mod button;

// button_fsm は HAL/Embassy 非依存の純粋ロジックなので host/embedded の両方で公開する。
pub mod button_fsm;

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub mod led;

#[derive(Debug, PartialEq, Eq)]
pub enum SystemError {
  EmptySlice,
}

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
