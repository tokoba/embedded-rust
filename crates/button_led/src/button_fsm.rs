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

/// FSMが呼び出し元に指示する次の待機動作
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(all(target_arch = "arm", target_os = "none"), derive(Format))]
pub enum WaitSpec {
  /// 立ち上がりエッジを待つ（ボタン押下待ち）
  RisingEdge,
  /// 指定ミリ秒のディバウンス待ち後にピン状態を読み取る
  Debounce(u64),
  /// 立ち下がりエッジまたは指定ミリ秒タイムアウトのレース
  FallingEdgeOrTimeout(u64),
  /// 立ち下がりエッジを待つ（長押し後のリリース待ち）
  FallingEdge,
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
  /// プレス開始時間（長押し判定用）
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

  /// 現在の状態に基づき、呼び出し元が次に実行すべき待機動作を返す
  pub fn next_wait(&self, now_ms: u64) -> WaitSpec {
    match self.state {
      ButtonFsmState::Idle => WaitSpec::RisingEdge,
      ButtonFsmState::DebouncingPress => WaitSpec::Debounce(DEBOUNCE_MS),
      ButtonFsmState::Pressed => {
        let press_start = self.press_start_ms.unwrap_or(now_ms);
        let elapsed = now_ms.saturating_sub(press_start);
        let remaining = LONG_PRESS_MS.saturating_sub(elapsed);
        WaitSpec::FallingEdgeOrTimeout(remaining)
      }
      ButtonFsmState::LongPressDetected => WaitSpec::FallingEdge,
      ButtonFsmState::DebouncingReleaseFromPress
      | ButtonFsmState::DebouncingReleaseFromLongPress => WaitSpec::Debounce(DEBOUNCE_MS),
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

      // --- DebouncingPress ---
      (ButtonFsmState::DebouncingPress, PhysicalEvent::DebouncedHigh) => {
        // ディバウンス完了、ピンHigh → プレス確定、ShortPress即時発行
        self.state = ButtonFsmState::Pressed;
        self.press_start_ms = Some(now_ms);
        Some(ButtonEvent::ShortPress)
      }
      (ButtonFsmState::DebouncingPress, PhysicalEvent::DebouncedLow) => {
        // ディバウンス完了、ピンLow → ノイズと判定
        self.state = ButtonFsmState::Idle;
        None
      }

      // --- Pressed（長押し判定中） ---
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
      (ButtonFsmState::LongPressDetected, PhysicalEvent::FallingEdge) => {
        self.state = ButtonFsmState::DebouncingReleaseFromLongPress;
        None
      }

      // --- DebouncingReleaseFromPress ---
      (ButtonFsmState::DebouncingReleaseFromPress, PhysicalEvent::DebouncedLow) => {
        // リリース確定（短押し後）
        self.state = ButtonFsmState::Idle;
        self.press_start_ms = None;
        Some(ButtonEvent::Released)
      }
      (ButtonFsmState::DebouncingReleaseFromPress, PhysicalEvent::DebouncedHigh) => {
        // チャタリング → プレス状態に戻る（長押し判定継続）
        self.state = ButtonFsmState::Pressed;
        None
      }

      // --- DebouncingReleaseFromLongPress ---
      (ButtonFsmState::DebouncingReleaseFromLongPress, PhysicalEvent::DebouncedLow) => {
        // リリース確定（長押し後）
        self.state = ButtonFsmState::Idle;
        self.press_start_ms = None;
        Some(ButtonEvent::Released)
      }
      (ButtonFsmState::DebouncingReleaseFromLongPress, PhysicalEvent::DebouncedHigh) => {
        // チャタリング → 長押し状態に戻る
        self.state = ButtonFsmState::LongPressDetected;
        None
      }

      // --- その他: 予期しないイベントは無視 ---
      _ => None,
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
      (PhysicalEvent::DebouncedHigh, DEBOUNCE_MS),
      (PhysicalEvent::FallingEdge, DEBOUNCE_MS + 100),
      (PhysicalEvent::DebouncedLow, DEBOUNCE_MS + 100 + DEBOUNCE_MS),
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
      (PhysicalEvent::DebouncedHigh, DEBOUNCE_MS),
      (PhysicalEvent::Timeout, DEBOUNCE_MS + LONG_PRESS_MS),
      (
        PhysicalEvent::FallingEdge,
        DEBOUNCE_MS + LONG_PRESS_MS + 100,
      ),
      (
        PhysicalEvent::DebouncedLow,
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
      (PhysicalEvent::DebouncedLow, DEBOUNCE_MS), // ピンLow → ノイズ
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
      (PhysicalEvent::DebouncedHigh, DEBOUNCE_MS),
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
      (PhysicalEvent::DebouncedHigh, DEBOUNCE_MS),
      (PhysicalEvent::FallingEdge, DEBOUNCE_MS + 100),
      (
        PhysicalEvent::DebouncedHigh,
        DEBOUNCE_MS + 100 + DEBOUNCE_MS,
      ), // チャタリング → Pressed に復帰
      (PhysicalEvent::FallingEdge, DEBOUNCE_MS + 200),
      (PhysicalEvent::DebouncedLow, DEBOUNCE_MS + 200 + DEBOUNCE_MS), // → Idle, Released
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
      (PhysicalEvent::DebouncedHigh, DEBOUNCE_MS),
      (PhysicalEvent::Timeout, DEBOUNCE_MS + LONG_PRESS_MS),
      (
        PhysicalEvent::FallingEdge,
        DEBOUNCE_MS + LONG_PRESS_MS + 100,
      ),
      (
        PhysicalEvent::DebouncedHigh,
        DEBOUNCE_MS + LONG_PRESS_MS + 100 + DEBOUNCE_MS,
      ), // チャタリング → LongPressDetected に復帰
      (
        PhysicalEvent::FallingEdge,
        DEBOUNCE_MS + LONG_PRESS_MS + 200,
      ),
      (
        PhysicalEvent::DebouncedLow,
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

  /// 連続短押し: FSMがIdleに正しく復帰し再利用可能であることを確認
  #[test]
  fn multiple_short_presses() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();

    // 1回目の短押し
    for (event, time) in [
      (PhysicalEvent::RisingEdge, 0),
      (PhysicalEvent::DebouncedHigh, DEBOUNCE_MS),
      (PhysicalEvent::FallingEdge, DEBOUNCE_MS + 100),
      (PhysicalEvent::DebouncedLow, DEBOUNCE_MS + 100 + DEBOUNCE_MS),
    ] {
      if let Some(e) = fsm.on_event(event, time) {
        events.push(e);
      }
    }

    // 2回目の短押し
    let base = 500;
    for (event, time) in [
      (PhysicalEvent::RisingEdge, base),
      (PhysicalEvent::DebouncedHigh, base + DEBOUNCE_MS),
      (PhysicalEvent::FallingEdge, base + DEBOUNCE_MS + 50),
      (
        PhysicalEvent::DebouncedLow,
        base + DEBOUNCE_MS + 50 + DEBOUNCE_MS,
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
        ButtonEvent::Released,
        ButtonEvent::ShortPress,
        ButtonEvent::Released,
      ]
    );
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }

  /// next_wait が各状態で正しい WaitSpec を返すことを確認
  #[test]
  fn next_wait_returns_correct_spec() {
    // Idle → RisingEdge
    let fsm = ButtonFsm::new(ButtonFsmState::Idle);
    assert_eq!(fsm.next_wait(0), WaitSpec::RisingEdge);

    // DebouncingPress → Debounce(DEBOUNCE_MS)
    let fsm = ButtonFsm::new(ButtonFsmState::DebouncingPress);
    assert_eq!(fsm.next_wait(0), WaitSpec::Debounce(DEBOUNCE_MS));

    // Pressed → FallingEdgeOrTimeout(remaining)
    let mut fsm = ButtonFsm::new(ButtonFsmState::Pressed);
    fsm.press_start_ms = Some(100);
    assert_eq!(
      fsm.next_wait(100),
      WaitSpec::FallingEdgeOrTimeout(LONG_PRESS_MS)
    );
    assert_eq!(
      fsm.next_wait(600),
      WaitSpec::FallingEdgeOrTimeout(LONG_PRESS_MS - 500)
    );
    // 超過時は saturating_sub で 0
    assert_eq!(fsm.next_wait(1200), WaitSpec::FallingEdgeOrTimeout(0));

    // LongPressDetected → FallingEdge
    let fsm = ButtonFsm::new(ButtonFsmState::LongPressDetected);
    assert_eq!(fsm.next_wait(0), WaitSpec::FallingEdge);

    // DebouncingReleaseFromPress → Debounce(DEBOUNCE_MS)
    let fsm = ButtonFsm::new(ButtonFsmState::DebouncingReleaseFromPress);
    assert_eq!(fsm.next_wait(0), WaitSpec::Debounce(DEBOUNCE_MS));

    // DebouncingReleaseFromLongPress → Debounce(DEBOUNCE_MS)
    let fsm = ButtonFsm::new(ButtonFsmState::DebouncingReleaseFromLongPress);
    assert_eq!(fsm.next_wait(0), WaitSpec::Debounce(DEBOUNCE_MS));
  }

  /// 予期しないイベントは無視されることを確認
  #[test]
  fn unexpected_events_are_ignored() {
    // Idle で FallingEdge は無視
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    assert_eq!(fsm.on_event(PhysicalEvent::FallingEdge, 0), None);
    assert_eq!(fsm.state, ButtonFsmState::Idle);

    // Idle で Timeout は無視
    assert_eq!(fsm.on_event(PhysicalEvent::Timeout, 0), None);
    assert_eq!(fsm.state, ButtonFsmState::Idle);

    // Idle で DebouncedHigh は無視
    assert_eq!(fsm.on_event(PhysicalEvent::DebouncedHigh, 0), None);
    assert_eq!(fsm.state, ButtonFsmState::Idle);

    // Idle で DebouncedLow は無視
    assert_eq!(fsm.on_event(PhysicalEvent::DebouncedLow, 0), None);
    assert_eq!(fsm.state, ButtonFsmState::Idle);

    // Pressed で RisingEdge は無視
    let mut fsm = ButtonFsm::new(ButtonFsmState::Pressed);
    fsm.press_start_ms = Some(0);
    assert_eq!(fsm.on_event(PhysicalEvent::RisingEdge, 100), None);
    assert_eq!(fsm.state, ButtonFsmState::Pressed);

    // LongPressDetected で Timeout は無視
    let mut fsm = ButtonFsm::new(ButtonFsmState::LongPressDetected);
    assert_eq!(fsm.on_event(PhysicalEvent::Timeout, 0), None);
    assert_eq!(fsm.state, ButtonFsmState::LongPressDetected);
  }
}
