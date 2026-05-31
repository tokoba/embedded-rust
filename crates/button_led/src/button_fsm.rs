//! Button FSM（有限状態機械）
//!
//! このモジュールは、ボタン入力のチャタリング除去と長押し検出を行う純粋な同期 FSM を提供します。
//! Embassy や HAL に依存しないため、ホスト環境でのテストが可能です。
//!
//! # 設計方針
//!
//! - タイマー管理は外部で行い、FSM は `PhysicalEvent::Timeout` をイベントとして受け取る
//! - 長押し判定は「長押し閾値を超えたタイミング」で `ButtonEvent::LongPress` を発行
//! - 短押し判定は「離しが安定したタイミング」で `ButtonEvent::ShortPress` を発行
//! - `ButtonEvent::Released` は `ButtonState` でカバーするため、FSM からは発行しない

/// チャタリング除去用の待ち時間 [msec]
pub const DEBOUNCE_MS: u64 = 30;
/// ボタン長押し判定時間 [msec]
pub const LONG_PRESS_MS: u64 = 1000;

/// ボタン押下系のイベント
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonEvent {
  /// ボタンの短い押下
  ShortPress,
  /// ボタンの長い押下
  LongPress,
  /// ボタンのリリース
  Released,
}

/// ボタンの状態
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonState {
  /// 押されていない状態
  Released,
  /// 押されている状態
  Pressed,
}

/// FSM 外部からの入力イベント
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PhysicalEvent {
  /// ボタン押下（立ち上がりエッジ）
  RisingEdge,
  /// ボタンリリース（立ち下がりエッジ）
  FallingEdge,
  /// タイマー満了
  ///
  /// 現在の状態によって意味が異なる:
  /// - `DebouncingPress` 中: 押下デバウンス完了
  /// - `Pressed` 中: 長押し閾値到達
  /// - `DebouncingRelease` 中: 離しデバウンス完了
  Timeout,
}

/// FSM の内部状態
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonFsmState {
  /// ボタン待機（安定離脱状態）
  Idle,
  /// 押下デバウンス中
  DebouncingPress,
  /// 安定押下中（長押し判定待ちも含む）
  Pressed,
  /// 長押し検出済み
  LongPressDetected,
  /// 離しデバウンス中
  DebouncingRelease,
}

/// FSM 構造体（内部状態を保持）
#[derive(Debug)]
pub struct ButtonFsm {
  /// FSM の現在の状態
  pub state: ButtonFsmState,
  /// 押下開始時刻 [msec]
  pub press_start_ms: Option<u64>,
}

impl ButtonFsm {
  /// 新しい FSM を作成する
  ///
  /// # 引数
  ///
  /// * `initial_state` - 初期状態
  pub fn new(initial_state: ButtonFsmState) -> Self {
    Self {
      state: initial_state,
      press_start_ms: None,
    }
  }

  /// イベント処理（内部状態を更新）
  ///
  /// # 引数
  ///
  /// * `event` - 入力イベント
  /// * `now_ms` - 現在時刻 [msec]
  /// * `_debounce_ms` - デバウンス時間 [msec]（現在は未使用）
  /// * `_long_press_ms` - 長押し判定時間 [msec]（現在は未使用）
  ///
  /// # 戻り値
  ///
  /// 発行するボタンイベント（あれば）
  pub fn on_event(
    &mut self,
    event: PhysicalEvent,
    now_ms: u64,
    _debounce_ms: u64,
    _long_press_ms: u64,
  ) -> Option<ButtonEvent> {
    match (&self.state, event) {
      // ===== Idle 状態 =====
      (ButtonFsmState::Idle, PhysicalEvent::RisingEdge) => {
        // 押下開始: デバウンス期間へ
        self.state = ButtonFsmState::DebouncingPress;
        None
      }
      (ButtonFsmState::Idle, PhysicalEvent::FallingEdge | PhysicalEvent::Timeout) => {
        // Idle 状態では FallingEdge と Timeout は無視
        None
      }

      // ===== DebouncingPress 状態 =====
      (ButtonFsmState::DebouncingPress, PhysicalEvent::RisingEdge) => {
        // チャタリング継続: デバウンス期間をリセット
        // （タイマーを再設定する必要がある）
        None
      }
      (ButtonFsmState::DebouncingPress, PhysicalEvent::FallingEdge) => {
        // チャタリング扱い: Idle に戻る
        self.state = ButtonFsmState::Idle;
        None
      }
      (ButtonFsmState::DebouncingPress, PhysicalEvent::Timeout) => {
        // デバウンス完了: 安定押下へ
        self.state = ButtonFsmState::Pressed;
        self.press_start_ms = Some(now_ms);
        None
        // 注意: 外部で長押しタイマーを設定する必要がある
      }

      // ===== Pressed 状態 =====
      (ButtonFsmState::Pressed, PhysicalEvent::RisingEdge) => {
        // ノイズとして無視
        None
      }
      (ButtonFsmState::Pressed, PhysicalEvent::FallingEdge) => {
        // 離し開始: デバウンス期間へ
        self.state = ButtonFsmState::DebouncingRelease;
        None
        // 注意: 外部で長押しタイマーをキャンセルし、離しデバウンスタイマーを設定する必要がある
      }
      (ButtonFsmState::Pressed, PhysicalEvent::Timeout) => {
        // 長押し閾値到達
        self.state = ButtonFsmState::LongPressDetected;
        Some(ButtonEvent::LongPress)
      }

      // ===== LongPressDetected 状態 =====
      (ButtonFsmState::LongPressDetected, PhysicalEvent::RisingEdge) => {
        // ノイズとして無視
        None
      }
      (ButtonFsmState::LongPressDetected, PhysicalEvent::FallingEdge) => {
        // 離し開始: デバウンス期間へ
        self.state = ButtonFsmState::DebouncingRelease;
        // 長押し検出済みなので、press_start_ms をクリアして ShortPress を発行しないようにする
        self.press_start_ms = None;
        None
      }
      (ButtonFsmState::LongPressDetected, PhysicalEvent::Timeout) => {
        // 既に長押し検出済み: Timeout は無視
        None
      }

      // ===== DebouncingRelease 状態 =====
      (ButtonFsmState::DebouncingRelease, PhysicalEvent::RisingEdge) => {
        // 離しデバウンス中に再押下: 安定押下へ戻る
        self.state = ButtonFsmState::Pressed;
        // press_start_ms は既に設定されているので維持
        None
        // 注意: 外部で離しデバウンスタイマーをキャンセルし、長押しタイマーを再設定する必要がある
      }
      (ButtonFsmState::DebouncingRelease, PhysicalEvent::FallingEdge) => {
        // チャタリング継続: デバウンス期間をリセット
        // （タイマーを再設定する必要がある）
        None
      }
      (ButtonFsmState::DebouncingRelease, PhysicalEvent::Timeout) => {
        // デバウンス完了: Idle に戻る
        self.state = ButtonFsmState::Idle;

        // 短押し判定
        // 長押し検出済みでない場合のみ短押しを発行
        if self.press_start_ms.is_some() {
          // 離しデバウンスが完了したので、ここで短押しを発行
          // （長押しの場合は既に LongPress が発行されている）
          self.press_start_ms = None;
          Some(ButtonEvent::ShortPress)
        } else {
          None
        }
      }
    }
  }
}

// ============================================================================
// ホスト単体テスト
// ============================================================================

#[cfg(all(test, not(target_os = "none")))]
mod tests {
  use super::*;

  const DEBOUNCE_MS: u64 = 30;
  const LONG_PRESS_MS: u64 = 1000;

  // ========================================================================
  // Happy Path テスト
  // ========================================================================

  #[test]
  fn short_press_emits_events() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();

    // t=0: 押下開始
    if let Some(e) = fsm.on_event(PhysicalEvent::RisingEdge, 0, DEBOUNCE_MS, LONG_PRESS_MS) {
      events.push(e);
    }

    // t=DEBOUNCE_MS: デバウンス完了
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    // t=DEBOUNCE_MS + 100: 離し開始
    if let Some(e) = fsm.on_event(
      PhysicalEvent::FallingEdge,
      DEBOUNCE_MS + 100,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    // t=DEBOUNCE_MS + 100 + DEBOUNCE_MS: デバウンス完了
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS + 100 + DEBOUNCE_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    assert_eq!(events, vec![ButtonEvent::ShortPress]);
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }

  #[test]
  fn long_press_emits_events() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();

    // t=0: 押下開始
    if let Some(e) = fsm.on_event(PhysicalEvent::RisingEdge, 0, DEBOUNCE_MS, LONG_PRESS_MS) {
      events.push(e);
    }

    // t=DEBOUNCE_MS: デバウンス完了
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    // t=DEBOUNCE_MS + LONG_PRESS_MS: 長押し検出
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS + LONG_PRESS_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    // t=DEBOUNCE_MS + LONG_PRESS_MS + 100: 離し開始
    if let Some(e) = fsm.on_event(
      PhysicalEvent::FallingEdge,
      DEBOUNCE_MS + LONG_PRESS_MS + 100,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    // t=DEBOUNCE_MS + LONG_PRESS_MS + 100 + DEBOUNCE_MS: デバウンス完了
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS + LONG_PRESS_MS + 100 + DEBOUNCE_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    assert_eq!(events, vec![ButtonEvent::LongPress]);
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }

  // ========================================================================
  // 異常系テスト
  // ========================================================================

  #[test]
  fn debounce_rejects_short_noise() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();

    // t=0: 押下開始
    if let Some(e) = fsm.on_event(PhysicalEvent::RisingEdge, 0, DEBOUNCE_MS, LONG_PRESS_MS) {
      events.push(e);
    }

    // t=10: チャタリングで離す
    if let Some(e) = fsm.on_event(PhysicalEvent::FallingEdge, 10, DEBOUNCE_MS, LONG_PRESS_MS) {
      events.push(e);
    }

    // t=DEBOUNCE_MS: デバウンス完了タイミングだが、既に Idle に戻っている
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    assert_eq!(events, vec![] as Vec<ButtonEvent>);
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }

  // ========================================================================
  // 境界値テスト
  // ========================================================================

  #[test]
  fn long_press_at_exact_boundary_emits_long_press() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();

    // t=0: 押下開始
    if let Some(e) = fsm.on_event(PhysicalEvent::RisingEdge, 0, DEBOUNCE_MS, LONG_PRESS_MS) {
      events.push(e);
    }

    // t=DEBOUNCE_MS: デバウンス完了
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    // t=DEBOUNCE_MS + LONG_PRESS_MS: 厳密に長押し閾値
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS + LONG_PRESS_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    assert_eq!(events, vec![ButtonEvent::LongPress]);
    assert_eq!(fsm.state, ButtonFsmState::LongPressDetected);
  }

  #[test]
  fn consecutive_short_presses() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();

    // 1 回目の短押し
    if let Some(e) = fsm.on_event(PhysicalEvent::RisingEdge, 0, DEBOUNCE_MS, LONG_PRESS_MS) {
      events.push(e);
    }
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }
    if let Some(e) = fsm.on_event(
      PhysicalEvent::FallingEdge,
      DEBOUNCE_MS + 100,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS + 100 + DEBOUNCE_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    // 2 回目の短押し
    if let Some(e) = fsm.on_event(
      PhysicalEvent::RisingEdge,
      DEBOUNCE_MS + 200,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS + 200 + DEBOUNCE_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }
    if let Some(e) = fsm.on_event(
      PhysicalEvent::FallingEdge,
      DEBOUNCE_MS + 300,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS + 300 + DEBOUNCE_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    assert_eq!(
      events,
      vec![ButtonEvent::ShortPress, ButtonEvent::ShortPress]
    );
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }

  #[test]
  fn release_after_long_press_emits_single_released() {
    let mut fsm = ButtonFsm::new(ButtonFsmState::Idle);
    let mut events = Vec::new();

    // 長押しシーケンス
    if let Some(e) = fsm.on_event(PhysicalEvent::RisingEdge, 0, DEBOUNCE_MS, LONG_PRESS_MS) {
      events.push(e);
    }
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS + LONG_PRESS_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }
    if let Some(e) = fsm.on_event(
      PhysicalEvent::FallingEdge,
      DEBOUNCE_MS + LONG_PRESS_MS + 100,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }
    if let Some(e) = fsm.on_event(
      PhysicalEvent::Timeout,
      DEBOUNCE_MS + LONG_PRESS_MS + 100 + DEBOUNCE_MS,
      DEBOUNCE_MS,
      LONG_PRESS_MS,
    ) {
      events.push(e);
    }

    // 長押し後の離しでは ShortPress は発行されない
    assert_eq!(events, vec![ButtonEvent::LongPress]);
    assert_eq!(fsm.state, ButtonFsmState::Idle);
  }
}
