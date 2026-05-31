//! led.rs
//! STM32F767ZI NUCLEO-144 ボード用 LED 操作用モジュール
//! <https://www.st.com/resource/en/user_manual/um1974-stm32-nucleo144-boards-mb1137-stmicroelectronics.pdf> 参照
//! 標準出荷状態の仕様
//! SB119: Open(はんだ付けなし), SB120 : Close(抵抗はんだ付けあり)であり，
//! この状態では PB0 が LED_GREEN に割り当てられる。変更する場合は SB119/120 の抵抗を変更する必要がある。

/// LED(GREEN) の点滅周期 (ミリ秒)
pub const LED_GREEN_BLINK_PERIOD_MS: u64 = 300;
/// LED(BLUE) の点滅周期 (ミリ秒)
pub const LED_BLUE_BLINK_PERIOD_MS: u64 = 1000;
/// LED(RED) の点滅周期 (ミリ秒)
pub const LED_RED_BLINK_PERIOD_MS: u64 = 2000;

use defmt::Format;
use embassy_stm32::Peri;
use embassy_stm32::gpio::{self, Level, Output, Speed};

/// 表示用に色をenum化（defmt表示できるように）
#[derive(Copy, Clone, Debug, Format)]
pub enum LedDisplayName {
  /// 緑
  Green,
  /// 青
  Blue,
  /// 赤
  Red,
}

/// LEDの出力ポート制御状態の定義
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(not(test), derive(Format))]
pub enum LedPortState {
  /// 消灯
  Off,
  /// 点灯 (点滅は複合的な状態として取り扱うのでLED単体の状態には含めない)
  On,
}

/// LED 点灯・点滅等の制御状態の定義
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(not(test), derive(Format))]
pub enum LedControlState {
  /// 消灯
  Off,
  /// 点灯
  On,
  /// 点滅
  Blink,
}

/// LED制御状態の管理構造体
/// `Output<'d>` は `core::fmt::Debug` を実装していないため、自動導出は使用しない
pub struct LedControl<'d> {
  /// LED 出力ポートのピン(stm32::gpio)
  pin: Output<'d>,
  /// LED 出力ポート制御状態
  port_state: LedPortState,
  /// LED 制御状態
  control_state: LedControlState,
  /// LED 制御周期(ON+OFFの合計時間) (msec)
  blink_period_ms: u64,
  /// 最後にLED点滅のトグル実施した時間(msec)
  last_blink_toggle_time: u64,
}

impl<'d> core::fmt::Debug for LedControl<'d> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("LedControl")
      .field("port_state", &self.port_state)
      .field("control_state", &self.control_state)
      .field("blink_period_ms", &self.blink_period_ms)
      .field("last_blink_toggle_time", &self.last_blink_toggle_time)
      .finish_non_exhaustive()
  }
}

/// LED 制御の実装ブロック
impl<'d> LedControl<'d> {
  /// LEDの初期化
  /// 初期状態の制御状態は引数で定義する
  /// 各LEDの色はポートごとに決まっており変更不可，Predefinedであるため，new()でのみ設定可能
  /// ペリフェラル参照を受け取り、内部で Output として初期化する
  pub fn new(
    pin: Peri<'d, impl gpio::Pin>,
    control_state: LedControlState,
    blink_period_ms: u64,
    last_blink_toggle_time: u64,
  ) -> Self {
    // 内部で GPIO を出力モードとして初期化
    let pin = Output::new(pin, Level::Low, Speed::Low);
    Self {
      pin,
      // 初期状態のポート制御状態は制御状態に応じて設定
      port_state: match control_state {
        LedControlState::Off => LedPortState::Off,
        LedControlState::On => LedPortState::On,
        LedControlState::Blink => LedPortState::Off,
      },
      control_state,
      blink_period_ms,
      last_blink_toggle_time,
    }
  }

  /// 点灯
  pub fn on(&mut self) {
    self.pin.set_high();
    self.port_state = LedPortState::On;
    self.control_state = LedControlState::On;
  }

  /// 消灯
  pub fn off(&mut self) {
    self.pin.set_low();
    self.port_state = LedPortState::Off;
    self.control_state = LedControlState::Off;
  }

  /// 点滅(点滅モードに設定する)
  pub fn blink(&mut self) {
    self.pin.set_low();
    self.port_state = LedPortState::Off;
    self.control_state = LedControlState::Blink;
  }

  /// 点滅設定時のLEDのトグル(点滅モードに設定した状態での内部処理関数)
  /// 本関数は時間管理せず，外部で時間管理して強制的にtoggleする関数
  pub fn toggle(&mut self) -> LedPortState {
    // blink 設定のときだけ toggle を有効化する
    // off, on 設定のときは toggle ではなく on/off を使用すること
    if self.control_state == LedControlState::Blink {
      match self.port_state {
        LedPortState::Off => {
          self.pin.set_high();
          self.port_state = LedPortState::On;
        }
        LedPortState::On => {
          self.pin.set_low();
          self.port_state = LedPortState::Off;
        }
      }
    }
    self.port_state
  }

  /// 点滅設定時のLEDのトグル(点滅モードに設定した状態での内部処理関数)
  /// 最後にトグルした時刻と現在時刻の差分を比較し，経過時間が点滅周期の半分を超えた場合はトグルを行う
  pub fn toggle_if_required(&mut self, current_time: u64) -> LedPortState {
    // blink 設定のときだけ toggle を有効化する
    // off, on 設定のときは toggle ではなく on/off を使用すること
    if self.control_state == LedControlState::Blink
      && current_time - self.last_blink_toggle_time > self.blink_period_ms / 2
    {
      match self.port_state {
        LedPortState::Off => {
          self.pin.set_high();
          self.port_state = LedPortState::On;
          self.last_blink_toggle_time = current_time;
        }
        LedPortState::On => {
          self.pin.set_low();
          self.port_state = LedPortState::Off;
          self.last_blink_toggle_time = current_time;
        }
      }
    }
    self.port_state
  }

  /// 点滅周期の設定(ON+OFFの合計時間)
  pub fn set_blink_period(&mut self, blink_period_ms: u64) {
    self.blink_period_ms = blink_period_ms;
  }
}
