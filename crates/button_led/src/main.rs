//! 割り込みを用いたボタンの押下検出
#![no_std]
#![no_main]
#![cfg(all(target_arch = "arm", target_os = "none"))]

use core::default::Default;

use button_led::button::task::button_watcher_task;
use button_led::led::task::led_task;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::exti::{self, ExtiInput};
use embassy_stm32::gpio::Pull;
use embassy_stm32::{bind_interrupts, interrupt};
use {defmt_rtt as _, panic_probe as _};

// 割り込み検出部分
bind_interrupts!(
  /// 割り込みハンドラ定義
    pub struct Irqs{
        EXTI15_10 => exti::InterruptHandler<interrupt::typelevel::EXTI15_10>;
});

/// メインの制御ループ
#[embassy_executor::main]
async fn main(spawner: Spawner) {
  let config = embassy_stm32::Config::default();
  let p = embassy_stm32::init(config);
  info!("Hello World!");

  // STM32F767ZI標準状態ではユーザーボタンは PC13 に割り当てられている
  // (SB17: ON, SB18: OFF)の半田付け状態であるため
  // embassy-stm32 0.5.0 では ExtiInput は既に型消去されているので
  // そのままタスクに渡せる
  let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down, Irqs);

  // ボタン監視タスクに関し処理を委譲する
  // 必要なペリフェラルピンを個別に渡す（Embassy タスクは 'static ライフタイムが必要）
  spawner.spawn(unwrap!(button_watcher_task(button, "USER")));
  spawner.spawn(unwrap!(led_task(p.PB0, p.PB7, p.PB14)));
}
