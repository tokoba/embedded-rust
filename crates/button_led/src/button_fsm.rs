//! Button FSM（有限状態機械）

// ✅ defmt は ARM ターゲットでのみ import
#[cfg(all(target_arch = "arm", target_os = "none"))]
use defmt::Format;

/// ディバウンス時間(一般的に10msec-20msec)
pub const DEBOUNCE_MS: u64 = 20;
/// 長押し判定時間
pub const LONG_PRESS_MS: u64 = 1000;

/// ボタンから外部に通知するイベント
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(all(target_arch = "arm", target_os = "none"), derive(Format))]
pub enum ButtonEvent {
  /// 短押しイベント
  ShortPress,
  /// 長押しイベント
  LongPress,
  /// リリースイベント
  Released,
}

/// ボタンの状態
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(all(target_arch = "arm", target_os = "none"), derive(Format))]
pub enum ButtonState {
  /// リリース状態
  Released,
  /// プレス状態
  Pressed,
}

/// GPIOから入力される物理的なイベント
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(all(target_arch = "arm", target_os = "none"), derive(Format))]
pub enum PhysicalEvent {
  /// 立ち上がりエッジ
  RisingEdge,
  /// 立ち下がりエッジ
  FallingEdge,
  /// タイムアウト
  Timeout,
}

/// FSMの内部状態
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(all(target_arch = "arm", target_os = "none"), derive(Format))]
pub enum ButtonFsmState {
  /// アイドル状態
  Idle,
  /// プレスディバウンス中
  DebouncingPress,
  /// プレス状態
  Pressed,
  /// 長押し検出状態
  LongPressDetected,
  /// リリースディバウンス中
  DebouncingRelease,
}

/// FSMの内部状態
#[derive(Debug)]
pub struct ButtonFsm {
  /// FSMの内部状態
  pub state: ButtonFsmState,
  /// プレス開始時間
  pub press_start_ms: Option<u64>,
}

impl ButtonFsm {
  /// FSMの初期化
  pub fn new(initial_state: ButtonFsmState) -> Self {
    Self {
      state: initial_state,
      press_start_ms: None,
    }
  }

  /// 物理的なイベントを処理する
  pub fn on_event(&mut self, event: PhysicalEvent, now_ms: u64) -> Option<ButtonEvent> {
    match (self.state, event) {
      (ButtonFsmState::Idle, PhysicalEvent::RisingEdge) => {
        self.state = ButtonFsmState::DebouncingPress;
        None
      }
      (ButtonFsmState::Idle, PhysicalEvent::FallingEdge | PhysicalEvent::Timeout) => None,
      (ButtonFsmState::DebouncingPress, PhysicalEvent::RisingEdge) => None,
      (ButtonFsmState::DebouncingPress, PhysicalEvent::FallingEdge) => {
        self.state = ButtonFsmState::Idle;
        None
      }
      (ButtonFsmState::DebouncingPress, PhysicalEvent::Timeout) => {
        self.state = ButtonFsmState::Pressed;
        self.press_start_ms = Some(now_ms);
        None
      }
      (ButtonFsmState::Pressed, PhysicalEvent::RisingEdge) => None,
      (ButtonFsmState::Pressed, PhysicalEvent::FallingEdge) => {
        self.state = ButtonFsmState::DebouncingRelease;
        None
      }
      (ButtonFsmState::Pressed, PhysicalEvent::Timeout) => {
        self.state = ButtonFsmState::LongPressDetected;
        Some(ButtonEvent::LongPress)
      }
      (ButtonFsmState::LongPressDetected, PhysicalEvent::RisingEdge) => None,
      (ButtonFsmState::LongPressDetected, PhysicalEvent::FallingEdge) => {
        self.state = ButtonFsmState::DebouncingRelease;
        self.press_start_ms = None;
        None
      }
      (ButtonFsmState::LongPressDetected, PhysicalEvent::Timeout) => None,
      (ButtonFsmState::DebouncingRelease, PhysicalEvent::RisingEdge) => {
        self.state = ButtonFsmState::Pressed;
        None
      }
      (ButtonFsmState::DebouncingRelease, PhysicalEvent::FallingEdge) => None,
      (ButtonFsmState::DebouncingRelease, PhysicalEvent::Timeout) => {
        self.state = ButtonFsmState::Idle;
        if self.press_start_ms.is_some() {
          self.press_start_ms = None;
          Some(ButtonEvent::ShortPress)
        } else {
          None
        }
      }
    }
  }
}

#[cfg(all(test, not(target_os = "none")))]
mod tests {
  use super::*;

  #[test]
  fn short_press_emits_events() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();
    for (event, time) in [
      (PhysicalEvent::RisingEdge, 0),
      (PhysicalEvent::Timeout, DEBOUNCE_MS),
      (PhysicalEvent::FallingEdge, DEBOUNCE_MS + 100),
      (PhysicalEvent::Timeout, DEBOUNCE_MS + 100 + DEBOUNCE_MS),
    ] {
      if let Some(e) = fsm.on_event(event, time) {
        events.push(e);
      }
    }
    assert_eq!(events, vec![ButtonEvent::ShortPress]);
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }

  #[test]
  fn long_press_emits_events() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();
    for (event, time) in [
      (PhysicalEvent::RisingEdge, 0),
      (PhysicalEvent::Timeout, DEBOUNCE_MS),
      (PhysicalEvent::Timeout, DEBOUNCE_MS + LONG_PRESS_MS),
      (
        PhysicalEvent::FallingEdge,
        DEBOUNCE_MS + LONG_PRESS_MS + 100,
      ),
      (
        PhysicalEvent::Timeout,
        DEBOUNCE_MS + LONG_PRESS_MS + 100 + DEBOUNCE_MS,
      ),
    ] {
      if let Some(e) = fsm.on_event(event, time) {
        events.push(e);
      }
    }
    assert_eq!(events, vec![ButtonEvent::LongPress]);
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }

  #[test]
  fn debounce_rejects_short_noise() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();
    for (event, time) in [
      (PhysicalEvent::RisingEdge, 0),
      (PhysicalEvent::FallingEdge, 10),
      (PhysicalEvent::Timeout, DEBOUNCE_MS),
    ] {
      if let Some(e) = fsm.on_event(event, time) {
        events.push(e);
      }
    }
    assert_eq!(events, Vec::<ButtonEvent>::new());
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }

  #[test]
  fn long_press_at_exact_boundary_emits_long_press() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();
    for (event, time) in [
      (PhysicalEvent::RisingEdge, 0),
      (PhysicalEvent::Timeout, DEBOUNCE_MS),
      (PhysicalEvent::Timeout, DEBOUNCE_MS + LONG_PRESS_MS),
    ] {
      if let Some(e) = fsm.on_event(event, time) {
        events.push(e);
      }
    }
    assert_eq!(events, vec![ButtonEvent::LongPress]);
    assert_eq!(fsm.state, ButtonFsmState::LongPressDetected);
  }
}
