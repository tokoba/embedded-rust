# Thread-mode Executor

Thread-mode Executor は、Embassy の最も基本的な実行モデルです。
`#[embassy_executor::main]` で起動し、`Spawner` で task を spawn します。

## 基本コード

```rust
#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use panic_probe as _;

#[embassy_executor::task]
async fn sensor_task() {
    loop {
        info!("sensor tick");
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::task]
async fn led_task() {
    loop {
        info!("led tick");
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let _p = embassy_stm32::init(Default::default());

    spawner.spawn(sensor_task()).unwrap();
    spawner.spawn(led_task()).unwrap();

    loop {
        Timer::after(Duration::from_secs(10)).await;
        info!("main heartbeat");
    }
}
```

## Thread-mode が向く処理

- 通信上位層
- センサ周期処理
- UI / LED / ボタン処理
- ロギング
- 状態監視
- command dispatcher

## 注意点

### `.await` なしの長時間処理を避ける

```rust
// 悪い例: 他 task を止める
loop {
    heavy_calculation();
}
```

分割可能なら、チャンクごとに `Timer::after_micros(0).await` や `yield_now` 相当の待機点を挿入します。

### main にロジックを集めない

`main` は board 初期化と task 起動に寄せ、制御の本体は supervisor / worker に分けます。
