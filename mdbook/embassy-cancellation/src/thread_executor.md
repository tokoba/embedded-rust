# Thread-mode Executor

Thread-mode Executor は、Embassy の **最も基本的な実行モデル** です。
`#[embassy_executor::main]` マクロで起動し、`Spawner` で task を spawn します。

## 動作原理

Cortex-M の Thread-mode（通常実行コンテキスト）で動作します。

1. Executor が全 task の ready 状態を確認し、ready な task を poll する
2. 全 task が pending なら **WFE (Wait For Event)** で CPU をスリープさせる
3. 割り込み発生時に HAL が **Waker** を呼び、対応 task を ready にマークする
4. **SEV (Send Event)** で CPU が起床し、ready な task を poll する

```text
[全 task pending] → WFE sleep → IRQ → HAL waker → SEV → poll ready tasks → ...
```

### フェアネス保証

Embassy Executor は **フェアネスを保証** します。1 つの task が連続して wake されても、
他の全 ready task が先に poll されてから再び poll されます。これにより、高頻度 task が他の task を飢えさせることはありません。

## 基本コード（STM32F767ZI）

```rust
#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_time::{Duration, Timer};
use panic_probe as _;

#[embassy_executor::task]
async fn sensor_task() {
    loop {
        info!("sensor: reading");
        // 実際にはここで ADC / I2C / SPI のセンサ読み取りを行う
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::task]
async fn led_task(mut led: Output<'static>) {
    loop {
        led.toggle();
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    // NUCLEO-F767ZI: LD1 = PB0
    let led = Output::new(p.PB0, Level::Low, Speed::Low);

    spawner.spawn(sensor_task()).unwrap();
    spawner.spawn(led_task(led)).unwrap();

    // main 自体も async task として動作
    loop {
        Timer::after(Duration::from_secs(10)).await;
        info!("main: heartbeat");
    }
}
```

## Thread-mode が向く処理

| 用途 | 理由 |
|---|---|
| 通信上位層（UART パース、プロトコル処理） | 応答性は ms 単位で十分 |
| センサ周期処理 | Timer loop で自然に書ける |
| UI / LED / ボタン処理 | EXTI + async で低消費電力 |
| ロギング | defmt は軽量だが ISR 内では制約あり |
| 状態監視・supervisor | 複数 channel を待つだけの軽い処理 |
| command dispatcher | channel receive → 分岐 |

## 注意点

### `.await` なしの長時間処理を避ける

```rust
// ✗ 他 task を止める
loop {
    heavy_calculation();
}

// ✓ チャンクごとに yield
for chunk in data.chunks(64) {
    process(chunk);
    embassy_futures::yield_now().await;
}
```

Embassy は **協調型** マルチタスクです。`.await` ポイントでのみコンテキストスイッチが発生するため、
長時間 `.await` を呼ばないコードは他のすべての task をブロックします。

### main にロジックを集めない

`main` は **board 初期化と task 起動** に寄せ、制御の本体は supervisor / worker task に分けます。

```rust
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // ✓ 初期化だけ
    let p = embassy_stm32::init(Default::default());
    let led = Output::new(p.PB0, Level::Low, Speed::Low);

    // ✓ task 起動
    spawner.spawn(supervisor_task()).unwrap();
    spawner.spawn(worker_task()).unwrap();
    spawner.spawn(led_task(led)).unwrap();

    // ✓ main は heartbeat / watchdog だけ
    loop {
        Timer::after(Duration::from_secs(10)).await;
        info!("main: heartbeat");
    }
}
```

### static 割り当ての理解

Embassy の task は **コンパイル時に static メモリに配置** されます。
`pool_size` 属性で同一 task 関数のインスタンス数を増やせますが、動的な heap 割り当てでは *ありません*。

```rust
// 同じ関数から最大 3 つの task インスタンスを同時実行可能
#[embassy_executor::task(pool_size = 3)]
async fn uart_handler(id: u8) {
    info!("UART handler {} started", id);
    loop {
        // ...
        Timer::after(Duration::from_millis(100)).await;
    }
}
```
