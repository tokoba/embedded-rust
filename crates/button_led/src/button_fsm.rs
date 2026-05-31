//! Button FSM（有限状態機械）

pub const DEBOUNCE_MS: u64 = 30;
pub const LONG_PRESS_MS: u64 = 1000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonEvent {
  ShortPress,
  LongPress,
  Released,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonState {
  Released,
  Pressed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PhysicalEvent {
  RisingEdge,
  FallingEdge,
  Timeout,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonFsmState {
  Idle,
  DebouncingPress,
  Pressed,
  LongPressDetected,
  DebouncingRelease,
}

#[derive(Debug)]
pub struct ButtonFsm {
  pub state: ButtonFsmState,
  pub press_start_ms: Option<u64>,
}

impl ButtonFsm {
  pub fn new(initial_state: ButtonFsmState) -> Self {
    Self { state: initial_state, press_start_ms: None }
  }

  pub fn on_event(
    &mut self,
    event: PhysicalEvent,
    now_ms: u64,
    _debounce_ms: u64,
    _long_press_ms: u64,
  ) -> Option<ButtonEvent> {
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
      if let Some(e) = fsm.on_event(event, time, DEBOUNCE_MS, LONG_PRESS_MS) { events.push(e); }
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
      (PhysicalEvent::FallingEdge, DEBOUNCE_MS + LONG_PRESS_MS + 100),
      (PhysicalEvent::Timeout, DEBOUNCE_MS + LONG_PRESS_MS + 100 + DEBOUNCE_MS),
    ] {
      if let Some(e) = fsm.on_event(event, time, DEBOUNCE_MS, LONG_PRESS_MS) { events.push(e); }
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
      if let Some(e) = fsm.on_event(event, time, DEBOUNCE_MS, LONG_PRESS_MS) { events.push(e); }
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
      if let Some(e) = fsm.on_event(event, time, DEBOUNCE_MS, LONG_PRESS_MS) { events.push(e); }
    }
    assert_eq!(events, vec![ButtonEvent::LongPress]);
    assert_eq!(fsm.state, ButtonFsmState::LongPressDetected);
  }
}
