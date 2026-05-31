//! embassy-rs STM32 向けのテンプレート
#![no_std]
#![no_main]
#![cfg(all(target_arch = "arm", target_os = "none"))]

use defmt::*;
use embassy_executor::Spawner;
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
  let config = embassy_stm32::Config::default();
  let _p = embassy_stm32::init(config);
  info!("template crate started");
}
