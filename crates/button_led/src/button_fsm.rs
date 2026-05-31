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
  /// 短押しイベント（ディバウンス確認後に即時発行）
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
  /// プレス確定状態（ShortPress発行済み、長押し判定中）
  Pressed,
  /// 長押し検出状態
  LongPressDetected,
  /// プレス状態からのリリースディバウンス中
  DebouncingReleaseFromPress,
  /// 長押し状態からのリリースディバウンス中
  DebouncingReleaseFromLongPress,
}

/// ボタンFSM
#[derive(Debug)]
pub struct ButtonFsm {
  /// FSMの内部状態
  pub state: ButtonFsmState,
  /// プレス開始時間（テスト・外部参照用）
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
  ///
  /// 戻り値: 外部に通知すべきイベント（存在する場合）
  pub fn on_event(&mut self, event: PhysicalEvent, now_ms: u64) -> Option<ButtonEvent> {
    match (self.state, event) {
      // --- Idle ---
      (ButtonFsmState::Idle, PhysicalEvent::RisingEdge) => {
        self.state = ButtonFsmState::DebouncingPress;
        None
      }
      (ButtonFsmState::Idle, PhysicalEvent::FallingEdge | PhysicalEvent::Timeout) => None,

      // --- DebouncingPress ---
      (ButtonFsmState::DebouncingPress, PhysicalEvent::RisingEdge) => None,
      (ButtonFsmState::DebouncingPress, PhysicalEvent::FallingEdge) => {
        // ディバウンス中にFallingEdge → ノイズと判定
        self.state = ButtonFsmState::Idle;
        None
      }
      (ButtonFsmState::DebouncingPress, PhysicalEvent::Timeout) => {
        // ディバウンス完了 → プレス確定、ShortPress即時発行
        self.state = ButtonFsmState::Pressed;
        self.press_start_ms = Some(now_ms);
        Some(ButtonEvent::ShortPress)
      }

      // --- Pressed（長押し判定中） ---
      (ButtonFsmState::Pressed, PhysicalEvent::RisingEdge) => None,
      (ButtonFsmState::Pressed, PhysicalEvent::FallingEdge) => {
        self.state = ButtonFsmState::DebouncingReleaseFromPress;
        None
      }
      (ButtonFsmState::Pressed, PhysicalEvent::Timeout) => {
        // 長押し判定時間経過 → 長押し確定
        self.state = ButtonFsmState::LongPressDetected;
        Some(ButtonEvent::LongPress)
      }

      // --- LongPressDetected ---
      (ButtonFsmState::LongPressDetected, PhysicalEvent::RisingEdge) => None,
      (ButtonFsmState::LongPressDetected, PhysicalEvent::FallingEdge) => {
        self.state = ButtonFsmState::DebouncingReleaseFromLongPress;
        None
      }
      (ButtonFsmState::LongPressDetected, PhysicalEvent::Timeout) => None,

      // --- DebouncingReleaseFromPress ---
      (ButtonFsmState::DebouncingReleaseFromPress, PhysicalEvent::RisingEdge) => {
        // チャタリング → プレス状態に戻る（長押し判定継続）
        self.state = ButtonFsmState::Pressed;
        None
      }
      (ButtonFsmState::DebouncingReleaseFromPress, PhysicalEvent::FallingEdge) => None,
      (ButtonFsmState::DebouncingReleaseFromPress, PhysicalEvent::Timeout) => {
        // リリース確定（短押し後のリリース）
        self.state = ButtonFsmState::Idle;
        self.press_start_ms = None;
        Some(ButtonEvent::Released)
      }

      // --- DebouncingReleaseFromLongPress ---
      (ButtonFsmState::DebouncingReleaseFromLongPress, PhysicalEvent::RisingEdge) => {
        // チャタリング → 長押し状態に戻る
        self.state = ButtonFsmState::LongPressDetected;
        None
      }
      (ButtonFsmState::DebouncingReleaseFromLongPress, PhysicalEvent::FallingEdge) => None,
      (ButtonFsmState::DebouncingReleaseFromLongPress, PhysicalEvent::Timeout) => {
        // リリース確定（長押し後のリリース）
        self.state = ButtonFsmState::Idle;
        self.press_start_ms = None;
        Some(ButtonEvent::Released)
      }
    }
  }
}

#[cfg(all(test, not(target_os = "none")))]
mod tests {
  use super::*;

  /// 短押し: ShortPress → Released
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
    assert_eq!(events, vec![ButtonEvent::ShortPress, ButtonEvent::Released]);
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }

  /// 長押し: ShortPress → LongPress → Released
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
    assert_eq!(
      events,
      vec![
        ButtonEvent::ShortPress,
        ButtonEvent::LongPress,
        ButtonEvent::Released
      ]
    );
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }

  /// ディバウンス中のノイズ除去
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

  /// 長押し境界値: ShortPress → LongPress
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
    assert_eq!(
      events,
      vec![ButtonEvent::ShortPress, ButtonEvent::LongPress]
    );
    assert_eq!(fsm.state, ButtonFsmState::LongPressDetected);
  }

  /// 短押しリリース中のチャタリング耐性
  #[test]
  fn chattering_during_release_from_press() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();
    for (event, time) in [
      (PhysicalEvent::RisingEdge, 0),
      (PhysicalEvent::Timeout, DEBOUNCE_MS), // → Pressed, ShortPress
      (PhysicalEvent::FallingEdge, DEBOUNCE_MS + 100), // → DebouncingReleaseFromPress
      (PhysicalEvent::RisingEdge, DEBOUNCE_MS + 105), // チャタリング → Pressed に復帰
      (PhysicalEvent::FallingEdge, DEBOUNCE_MS + 200), // → DebouncingReleaseFromPress
      (PhysicalEvent::Timeout, DEBOUNCE_MS + 200 + DEBOUNCE_MS), // → Idle, Released
    ] {
      if let Some(e) = fsm.on_event(event, time) {
        events.push(e);
      }
    }
    assert_eq!(events, vec![ButtonEvent::ShortPress, ButtonEvent::Released]);
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }

  /// 長押しリリース中のチャタリング耐性
  #[test]
  fn chattering_during_release_from_long_press() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();
    for (event, time) in [
      (PhysicalEvent::RisingEdge, 0),
      (PhysicalEvent::Timeout, DEBOUNCE_MS), // → Pressed, ShortPress
      (PhysicalEvent::Timeout, DEBOUNCE_MS + LONG_PRESS_MS), // → LongPressDetected, LongPress
      (
        PhysicalEvent::FallingEdge,
        DEBOUNCE_MS + LONG_PRESS_MS + 100,
      ), // → DebouncingReleaseFromLongPress
      (PhysicalEvent::RisingEdge, DEBOUNCE_MS + LONG_PRESS_MS + 105), // チャタリング → LongPressDetected に復帰
      (
        PhysicalEvent::FallingEdge,
        DEBOUNCE_MS + LONG_PRESS_MS + 200,
      ), // → DebouncingReleaseFromLongPress
      (
        PhysicalEvent::Timeout,
        DEBOUNCE_MS + LONG_PRESS_MS + 200 + DEBOUNCE_MS,
      ), // → Idle, Released
    ] {
      if let Some(e) = fsm.on_event(event, time) {
        events.push(e);
      }
    }
    assert_eq!(
      events,
      vec![
        ButtonEvent::ShortPress,
        ButtonEvent::LongPress,
        ButtonEvent::Released
      ]
    );
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }
}
