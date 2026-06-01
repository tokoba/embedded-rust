# Interrupt-mode Executor

`InterruptExecutor` は、task を **割り込みコンテキスト** で poll するための executor です。
これにより thread-mode executor より **高い NVIC 優先度** で async task を動かせます。

## 動作原理

```text
通常の peripheral IRQ パス:
  task が async I/O を開始
    → peripheral IRQ 発生 (EXTI, USART, DMA 等)
    → HAL が waker を起こす
    → executor が ready task を poll

InterruptExecutor のパス:
  executor 自体が専用 IRQ / SWI で駆動される
    → IRQ 優先度 = executor 内 task の優先度
    → thread-mode task をプリエンプトできる
```

### よくある誤解

> 「IRQ → task 本体が直接呼ばれる」

これは **誤り** です。peripheral IRQ が task を直接呼ぶわけではありません。
peripheral IRQ は waker を起こし、**executor が poll することで** task が進みます。
`InterruptExecutor` の場合は、その executor 自体が割り込み優先度で動くため、
thread-mode の task より先に poll されるという仕組みです。

### EXTI / USART IRQ を executor IRQ と兼用しない

`InterruptExecutor` を駆動する IRQ には、**未使用の IRQ 番号** または **ソフトウェア割り込み相当** を割り当てます。
EXTI / USART / DMA などの本物の peripheral IRQ を executor IRQ と兼用する設計は、原則として避けます。

## Multi-Priority 構成（multiprio パターン）

Embassy 公式の `multiprio.rs` サンプルに基づく 3 段優先度の構成例です。

### STM32F767ZI での実装例

STM32F7 では未使用のソフトウェア割り込み枠として、使用していない peripheral IRQ
（例: `UART4`, `UART5` など、プロジェクトで未使用のもの）を executor 駆動用に転用できます。

```rust
#![no_std]
#![no_main]

use cortex_m_rt::entry;
use defmt::*;
use defmt_rtt as _;
use embassy_executor::{Executor, InterruptExecutor};
use embassy_stm32::interrupt;
use embassy_stm32::interrupt::{InterruptExt, Priority};
use embassy_time::{Duration, Timer};
use panic_probe as _;
use static_cell::StaticCell;

// 高優先度 task
#[embassy_executor::task]
async fn high_priority_task() {
    loop {
        info!("[HIGH] tick");
        Timer::after(Duration::from_millis(500)).await;
    }
}

// 中優先度 task
#[embassy_executor::task]
async fn med_priority_task() {
    loop {
        info!("[MED] processing");
        // 重い処理のシミュレーション（block_for は他 task をブロックする）
        embassy_time::block_for(Duration::from_millis(200));
        info!("[MED] done");
        Timer::after(Duration::from_millis(1000)).await;
    }
}

// 低優先度 task（thread-mode）
#[embassy_executor::task]
async fn low_priority_task() {
    loop {
        info!("[LOW] background work");
        embassy_time::block_for(Duration::from_millis(500));
        info!("[LOW] done");
        Timer::after(Duration::from_millis(2000)).await;
    }
}

// Executor インスタンス（static 配置必須）
static EXECUTOR_HIGH: InterruptExecutor = InterruptExecutor::new();
static EXECUTOR_MED: InterruptExecutor = InterruptExecutor::new();
static EXECUTOR_LOW: StaticCell<Executor> = StaticCell::new();

// 高優先度 executor の割り込みハンドラ
// ※ UART4 をプロジェクトで未使用と仮定して転用
#[interrupt]
unsafe fn UART4() {
    EXECUTOR_HIGH.on_interrupt()
}

// 中優先度 executor の割り込みハンドラ
#[interrupt]
unsafe fn UART5() {
    EXECUTOR_MED.on_interrupt()
}

#[entry]
fn main() -> ! {
    info!("multiprio example start");
    let _p = embassy_stm32::init(Default::default());

    // 高優先度 executor: Priority::P6（数値が小さいほど高優先度）
    let irq = interrupt::UART4;
    irq.set_priority(Priority::P6);
    let spawner = EXECUTOR_HIGH.start(irq);
    spawner.spawn(high_priority_task()).unwrap();

    // 中優先度 executor: Priority::P7
    let irq = interrupt::UART5;
    irq.set_priority(Priority::P7);
    let spawner = EXECUTOR_MED.start(irq);
    spawner.spawn(med_priority_task()).unwrap();

    // 低優先度 executor: thread-mode（最低優先度）
    let executor = EXECUTOR_LOW.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(low_priority_task()).unwrap();
    });
}
```

### 優先度の動作

```text
優先度高  ┌─────────────────────────────────┐
          │  EXECUTOR_HIGH (UART4, P6)      │ ← 他のすべてをプリエンプト可能
          │  high_priority_task             │
          ├─────────────────────────────────┤
          │  EXECUTOR_MED  (UART5, P7)      │ ← LOW をプリエンプト可能
          │  med_priority_task              │
          ├─────────────────────────────────┤
          │  EXECUTOR_LOW  (thread-mode)    │ ← 最低優先度
          │  low_priority_task              │
優先度低  └─────────────────────────────────┘
```

**重要**: 高優先度 task が `block_for` 等で長時間実行すると、低優先度 task が飢えます。
高優先度 executor 上の task は **短く、すぐに `.await` する** ことが鉄則です。

## 使うべき場面

| 場面 | 理由 |
|---|---|
| 低優先度 task を確実に中断したい | NVIC プリエンプションで実現 |
| 数十 µs〜サブ ms レベルの応答性 | thread-mode では他 task の実行待ちが発生 |
| 高優先度 async pipeline の分離 | モーター制御、安全停止、高速通信 |

## 避けるべき場面

| 場面 | 理由 |
|---|---|
| 単なるボタン入力 | EXTI + thread-mode で十分 |
| UART 受信の通常処理 | DMA + async で十分 |
| 長い計算処理 | 他の高優先度 task もブロックする |
| ログ出力 | ISR 文脈での defmt は制約あり |
| 複雑な共有状態を多数触る処理 | Mutex 競合が複雑化する |

## 設計上の注意

1. **`InterruptExecutor::start()` の前に IRQ 優先度を設定する**
2. **high-priority task は短くし、すぐに `.await` する**
3. **共有データは `CriticalSectionRawMutex` や atomic を慎重に使う**
4. **thread-mode で十分なら thread-mode を選ぶ**（YAGNI 原則）

## STM32 での IRQ 選択について

STM32 では未使用 IRQ の選択が HAL / board の対応状況に依存します。
本ガイドの実装例では、まず **thread-mode executor だけで安全な supervisor + worker 構成** を示します。
`InterruptExecutor` は、**実測で thread-mode のレイテンシが不足したときに追加する選択肢** とします。

> **推奨**: プロジェクトで使用していない UART / TIM 等の IRQ を executor 駆動用に転用する。
> `UART4`、`UART5`、`TIM6_DAC`、`TIM7` などが候補になります。
