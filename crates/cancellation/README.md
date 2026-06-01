# Embassy-rs を用いたタスクの処理キャンセルの方法

# Embassy-rs における複雑なタスク制御の詳細ガイド

Embassy-rs のタスク制御について，Executor の種類，優先度制御，キャンセルパターンまで体系的に解説します。

***

## 1. Thread-based Executor（スレッドモード実行）

最も基本的な実行モデルで，`main` 関数からタスクを spawn する方式です。 [\[docs.rs\]](https://docs.rs/embassy-executor/latest/embassy_executor/struct.Executor.html)

```rust
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;
use defmt::info;

#[embassy_executor::task]
async fn sensor_task() {
    loop {
        info!("Reading sensor...");
        Timer::after_millis(500).await; // ← ここで他タスクに制御を譲る
    }
}

#[embassy_executor::task]
async fn led_task() {
    loop {
        info!("LED toggle");
        Timer::after_millis(1000).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let _p = embassy_stm32::init(Default::default());
    spawner.spawn(sensor_task()).unwrap();
    spawner.spawn(led_task()).unwrap();

    // main 自体も async task として動作
    loop {
        Timer::after_secs(10).await;
        info!("Watchdog alive");
    }
}
```

### 特徴

- **WFE/SEV** ベースでスリープ → ウェイクアップを実現（Cortex-M） [\[docs.rs\]](https://docs.rs/embassy-executor/latest/embassy_executor/struct.Executor.html)
- **協調的マルチタスク**: `.await` ポイントでのみコンテキストスイッチが発生
- タスクは **静的に割り当て** され，ヒープ不要 [\[docs.embassy.dev\]](https://docs.embassy.dev/struct.InterruptExecutor.html)
- **フェアネス保証**: 1つのタスクが連続 wake されても，他の全タスクが先に実行される [\[docs.embassy.dev\]](https://docs.embassy.dev/struct.InterruptExecutor.html)

***

## 2. Interrupt-based Executor（割り込みモード実行）

`InterruptExecutor` を使い，**割り込みコンテキスト**でタスクを実行します。これにより，スレッドモードよりも高い優先度でタスクを動かせます。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

### Multi-Priority 構成（multiprio パターン）

Embassy 公式の `multiprio.rs` サンプルがこの構成の決定版です： [\[github.com\]](https://github.com/embassy-rs/embassy/blob/main/examples/nrf52840/src/bin/multiprio.rs)

```rust
use embassy_executor::{Executor, InterruptExecutor};
use embassy_nrf::interrupt::{self, InterruptExt, Priority};
use static_cell::StaticCell;

// === 高優先度タスク ===
#[embassy_executor::task]
async fn run_high() {
    loop {
        info!("[high] tick!");
        Timer::after_ticks(27374).await;
    }
}

// === 中優先度タスク ===
#[embassy_executor::task]
async fn run_med() {
    loop {
        info!("[med] Starting long computation");
        embassy_time::block_for(embassy_time::Duration::from_secs(1));
        info!("[med] done");
        Timer::after_ticks(23421).await;
    }
}

// === 低優先度タスク ===
#[embassy_executor::task]
async fn run_low() {
    loop {
        info!("[low] Starting long computation");
        embassy_time::block_for(embassy_time::Duration::from_secs(2));
        info!("[low] done");
        Timer::after_ticks(32983).await;
    }
}

// Executor インスタンス（static）
static EXECUTOR_HIGH: InterruptExecutor = InterruptExecutor::new();
static EXECUTOR_MED: InterruptExecutor = InterruptExecutor::new();
static EXECUTOR_LOW: StaticCell<Executor> = StaticCell::new();

// 割り込みハンドラ
#[interrupt]
unsafe fn EGU1_SWI1() {
    unsafe { EXECUTOR_HIGH.on_interrupt() }
}

#[interrupt]
unsafe fn EGU0_SWI0() {
    unsafe { EXECUTOR_MED.on_interrupt() }
}

#[entry]
fn main() -> ! {
    let _p = embassy_nrf::init(Default::default());

    // 高優先度: Priority::P6（数値が小さいほど高優先度 on Cortex-M）
    interrupt::EGU1_SWI1.set_priority(Priority::P6);
    let spawner = EXECUTOR_HIGH.start(interrupt::EGU1_SWI1);
    spawner.spawn(run_high()).unwrap();

    // 中優先度: Priority::P7
    interrupt::EGU0_SWI0.set_priority(Priority::P7);
    let spawner = EXECUTOR_MED.start(interrupt::EGU0_SWI0);
    spawner.spawn(run_med()).unwrap();

    // 低優先度: スレッドモード（WFE/SEV）
    let executor = EXECUTOR_LOW.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(run_low()).unwrap();
    });
}
```

### 動作原理と設計上のポイント

| 項目             | Thread Executor | Interrupt Executor                  |
| -------------- | --------------- | ----------------------------------- |
| **実行コンテキスト**   | スレッドモード（最低優先度）  | 割り込みコンテキスト                          |
| **プリエンプション**   | 全ての割り込みに中断される   | 自身より高い優先度のみに中断される                   |
| **Spawner の型** | `Spawner`       | `SendSpawner`（異なるスレッドからの spawn を表す） |
| **使用する割り込み**   | 不要              | 未使用の SWI/EGU 等を選択                   |
| **スリープ**       | WFE/SEV         | 割り込み pend で起床                       |

 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html), [\[deepwiki.com\]](https://deepwiki.com/embassy-rs/embassy/2-async-execution-model)

> ⚠️ **重要**: `InterruptExecutor::start()` を呼ぶ**前に**割り込み優先度を設定すること。後から変更してはならない。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

***

## 3. タスクキャンセルパターン（CancellationToken 相当）

### 🔑 Embassy には C# の `CancellationToken` や tokio の `CancellationToken` に直接対応する機能はない

Embassy のタスクは **静的に割り当てられた Future** であり，外部から直接 abort/cancel する API は提供されていません（`SpawnToken` を drop すると panic する設計）。 [\[github.com\]](https://github.com/embassy-rs/embassy/blob/main/embassy-executor/src/spawner.rs)

代わりに，**協調的キャンセル** を実現するいくつかのイディオムがあります：

***

### パターン A: `select` + `Signal` による Cancellation Token 実装

**最も実用的で推奨されるパターンです。** `embassy_futures::select` で「本来の処理」と「キャンセルシグナル」を競合させます。 [\[acalustra.com\]](https://acalustra.com/embedded-rust-development-tips-with-embassy.html), [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/signal/struct.Signal.html)

```rust
use embassy_futures::select::{select, Either};
use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

// グローバルなキャンセルシグナル（= CancellationToken 相当）
static CANCEL_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[embassy_executor::task]
async fn cancellable_worker() {
    info!("Worker started");

    // 本来の長時間処理と，キャンセルシグナルを select で競合
    match select(
        long_running_operation(),
        CANCEL_SIGNAL.wait(),  // キャンセル待ち
    ).await {
        Either::First(result) => {
            info!("Work completed normally: {:?}", result);
        }
        Either::Second(_) => {
            info!("Work CANCELLED by upper layer");
            // クリーンアップ処理
            cleanup().await;
        }
    }

    info!("Worker exiting");
}

async fn long_running_operation() -> u32 {
    // 段階的な処理（各 await ポイントでキャンセル可能）
    Timer::after_secs(5).await;
    42
}

async fn cleanup() {
    // ペリフェラルの安全な停止など
}

// 上位タスクからキャンセルを発行
#[embassy_executor::task]
async fn supervisor_task() {
    Timer::after_secs(2).await;
    info!("Supervisor: issuing cancel!");
    CANCEL_SIGNAL.signal(()); // ← キャンセル発行
}
```

***

### パターン B: `Channel` によるコマンドベースキャンセル

複数のコマンド（Start/Stop/Cancel 等）を送信する場合に有効です。 [\[dev.to\]](https://dev.to/theembeddedrustacean/sharing-data-among-tasks-in-rust-embassy-synchronization-primitives-59hk)

```rust
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

#[derive(Clone)]
enum TaskCommand {
    Start,
    Stop,
    Cancel,
    UpdateParam(u32),
}

static CMD_CHANNEL: Channel<CriticalSectionRawMutex, TaskCommand, 4> = Channel::new();

#[embassy_executor::task]
async fn controlled_task() {
    loop {
        // コマンド待ちと処理を select で競合
        match select(
            do_periodic_work(),
            CMD_CHANNEL.receive(),
        ).await {
            Either::First(_) => {
                // 通常処理完了
            }
            Either::Second(cmd) => match cmd {
                TaskCommand::Cancel => {
                    info!("Received cancel command");
                    break; // タスク終了
                }
                TaskCommand::Stop => {
                    info!("Paused, waiting for Start...");
                    loop {
                        match CMD_CHANNEL.receive().await {
                            TaskCommand::Start => break,
                            TaskCommand::Cancel => return,
                            _ => {}
                        }
                    }
                }
                TaskCommand::UpdateParam(val) => {
                    info!("Param updated to {}", val);
                }
                _ => {}
            },
        }
    }
}
```

***

### パターン C: `AtomicBool` フラグによる軽量キャンセル

最も軽量で，ISR（割り込みサービスルーチン）からも安全に設定できます。

```rust
use core::sync::atomic::{AtomicBool, Ordering};

static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

#[embassy_executor::task]
async fn cancellable_loop_task() {
    loop {
        if CANCEL_FLAG.load(Ordering::Relaxed) {
            info!("Cancel detected, exiting");
            CANCEL_FLAG.store(false, Ordering::Relaxed); // reset
            break;
        }

        // 処理の各ステップ
        do_step_1().await;

        if CANCEL_FLAG.load(Ordering::Relaxed) {
            break;
        }

        do_step_2().await;
    }
}
```

> ⚠️ このパターンは `.await` ポイントの間でしかチェックされないため，長い await がある場合にレイテンシが生じます。即座のキャンセルが必要な場合は **パターン A** を使用してください。

***

### パターン D: ネストされたキャンセル伝播（C# の CancellationTokenSource チェーン相当）

```rust
use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

// 階層的キャンセル: parent → child1, child2
static PARENT_CANCEL: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static CHILD1_CANCEL: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static CHILD2_CANCEL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[embassy_executor::task]
async fn parent_monitor() {
    // 親のキャンセルを監視し，子に伝播
    PARENT_CANCEL.wait().await;
    info!("Parent cancel → propagating to children");
    CHILD1_CANCEL.signal(());
    CHILD2_CANCEL.signal(());
}

#[embassy_executor::task]
async fn child_task_1() {
    match select(actual_work_1(), CHILD1_CANCEL.wait()).await {
        Either::First(_) => info!("Child1 completed"),
        Either::Second(_) => info!("Child1 cancelled"),
    }
}

#[embassy_executor::task]
async fn child_task_2() {
    match select(actual_work_2(), CHILD2_CANCEL.wait()).await {
        Either::First(_) => info!("Child2 completed"),
        Either::Second(_) => info!("Child2 cancelled"),
    }
}
```

***

## 4. 同期プリミティブの使い分け早見表

| プリミティブ             | 用途          | キャンセル安全 | 備考                             |
| ------------------ | ----------- | :-----: | ------------------------------ |
| `Signal<M, T>`     | 最新値の1対1通知   |    ✅    | `wait()` は cancel-safe。値のロスを許容 |
| `Channel<M, T, N>` | MPMC キュー    |    ✅    | バッファ付き。コマンドパターンに最適             |
| `PubSubChannel`    | 1対多ブロードキャスト |    ✅    | 全 subscriber に配信               |
| `Watch<M, T, N>`   | 状態の共有・変更通知  |    ✅    | 複数 observer が最新状態を観測           |
| `Mutex<M, T>`      | 排他制御        |    ✅    | I2C 共有等に利用                     |

 [\[deepwiki.com\]](https://deepwiki.com/embassy-rs/embassy/2-async-execution-model), [\[dev.to\]](https://dev.to/theembeddedrustacean/sharing-data-among-tasks-in-rust-embassy-synchronization-primitives-59hk)

***

## 5. 推奨リソース・参考サイト

### 📚 公式ドキュメント・リポジトリ

- **[Embassy Book](https://embassy-rs.github.io/embassy-book/embassy/dev/index.html)** — 基本概念とアーキテクチャ [\[embassy-rs.github.io\]](https://embassy-rs.github.io/embassy-book/embassy/dev/index.html)
- **[embassy-executor API docs](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)** — InterruptExecutor の詳細 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)
- **[multiprio.rs サンプル](https://github.com/embassy-rs/embassy/blob/main/examples/nrf52840/src/bin/multiprio.rs)** — 複数優先度 Executor の実装例 [\[github.com\]](https://github.com/embassy-rs/embassy/blob/main/examples/nrf52840/src/bin/multiprio.rs)
- **[embassy-sync Signal](https://docs.embassy.dev/embassy-sync/git/default/signal/struct.Signal.html)** — cancel-safe な Signal の API [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/signal/struct.Signal.html)

### 📝 実践ガイド・チュートリアル

- **[Practical Embedded Rust Development Tips with Embassy](https://acalustra.com/embedded-rust-development-tips-with-embassy.html)** — select パターン，Channel，I2C 共有の実践例 [\[acalustra.com\]](https://acalustra.com/embedded-rust-development-tips-with-embassy.html)
- **[Sharing Data Among Tasks (DEV Community)](https://dev.to/theembeddedrustacean/sharing-data-among-tasks-in-rust-embassy-synchronization-primitives-59hk)** — embassy-sync プリミティブの詳細比較 [\[dev.to\]](https://dev.to/theembeddedrustacean/sharing-data-among-tasks-in-rust-embassy-synchronization-primitives-59hk)
- **[DeepWiki: Async Execution Model](https://deepwiki.com/embassy-rs/embassy/2-async-execution-model)** — タスクライフサイクル，スケジューリング戦略の詳解 [\[deepwiki.com\]](https://deepwiki.com/embassy-rs/embassy/2-async-execution-model)
- **[Async Programming in Embedded Rust with Embassy (ESP32)](https://esp32.implrust.com/embassy/index.html)** — ESP32 向けだが概念は共通 [\[esp32.implrust.com\]](https://esp32.implrust.com/embassy/index.html)

***

## 6. C#/tokio との比較まとめ

| 概念        | C#                        | tokio                                 | Embassy-rs                  |
| --------- | ------------------------- | ------------------------------------- | --------------------------- |
| キャンセルトークン | `CancellationToken`       | `tokio_util::sync::CancellationToken` | `Signal` + `select`         |
| キャンセル伝播   | `CreateLinkedTokenSource` | `child_token()`                       | 親 Signal → 子 Signal の手動伝播   |
| タスク abort | `Task.Cancel()`           | `JoinHandle::abort()`                 | **なし**（協調的キャンセルのみ）          |
| select    | `Task.WhenAny()`          | `tokio::select!`                      | `embassy_futures::select()` |
| join      | `Task.WhenAll()`          | `tokio::join!`                        | `embassy_futures::join()`   |
| Channel   | `Channel<T>`              | `mpsc::channel`                       | `embassy_sync::Channel`     |

Embassy は `no_std` + `no_alloc` 環境のため，**すべてが静的割り当て**であることが最大の違いです。tokio のように動的に JoinHandle を取得して abort するパターンは使えませんが，`select` + `Signal` の組み合わせで十分に実用的なキャンセル制御が可能です。

---

# 調査レポート：Rust embassy-rs で複雑なタスクを制御する設計方法と「上位からのキャンセル」実装パターン

## 1. 調査の目的とアプローチ

### 1.1 ユーザー要求の解釈

ご質問の核心は、**C# / Tokio の `CancellationToken` 的な“上位から下位タスクへキャンセルを伝播する設計”を、`no_std` 組み込み環境の Embassy でどう実用化するか**です。特に、次の2系統を比較しながら、実装ノウハウと参考コンテンツを探す依頼だと解釈しました。

1. **Thread-mode executor**
   - `main` / `#[embassy_executor::main]` から `Spawner` 経由で task を起動し、通常優先度・協調スケジューリングで動かす。
2. **Interrupt-mode executor**
   - NVIC 等の割り込みを executor の駆動源にして、より高い優先度の async task を動かす。
3. **キャンセル設計**
   - spawn 済み task を外部から強制停止するのではなく、`select`、`Channel`、`Signal`、`Watch` 等で「キャンセル通知」を伝え、task 側が `.await` 境界で協調的に抜ける設計が Embassy では中心になる。

Embassy 公式ドキュメントでは `embassy-executor` は「embedded usage 向け async/await executor」で、`no alloc`、heap 不要、task の静的割当、timer queue、割り込みや WFE/SEV による sleep/wake、複数 executor による優先度分離を説明しています。 また、GitHub issue では `Timer::after(...).await` を .NET の `CancellationToken` 的に早期復帰させたいという要望に対し、Embassy 側のメンテナが「async Rust では `select` が idiomatic」とし、全 async API へキャンセル機構を追加しない方針を述べています。 [\[docs.embassy.dev\]](https://docs.embassy.dev/) [\[github.com\]](https://github.com/embassy-rs/embassy/issues/3906)

### 1.2 調査すべき問いへの分解

MECE に近い形で、以下の6項目に分解しました。

1. **Embassy executor の基本モデル**\
   Embassy の task / executor / spawner / waker はどのように連携し、RTOS thread と何が違うのか？
2. **Thread-mode executor の設計**\
   `main -> spawn -> tasks` の構造で、複雑な状態制御・I/O 待ち・周期処理をどう組むべきか？
3. **Interrupt-mode executor の設計**\
   `irq -> task` と見える構造の実態は何で、どこまで ISR 的な即応性を async task に持たせられるのか？
4. **キャンセル伝播の設計**\
   Embassy で `CancellationToken` 相当をどう実装すべきか？ `Channel` / `Signal` / `Watch` / `AtomicBool` の使い分けは？
5. **spawn 済み task の寿命管理**\
   `pool_size`、`SpawnError::Busy`、task を大量再spawnしない設計、再利用可能な long-running task 化の勘所は？
6. **参考サイト・コンテンツ**\
   どの公式ドキュメント、issue、実践記事、Tokio資料を読むと体系的に理解できるか？

### 1.3 調査構造

Thread-mode executor で複雑な制御を書く場合、私は次の原則を推奨します。

- **main task は初期化と supervision に寄せる**\
  `main` にすべてのロジックを置くと、状態遷移・I/O・タイムアウト・キャンセルが絡んで肥大化します。`main` は HAL 初期化、channel / signal の構築、長寿命 task の spawn、必要なら supervisor loop に限定するのがよいです。
- **各 peripheral は owner task を作る**\
  たとえば motor task、sensor task、ui task、communication task のように分け、共有 peripheral を複数 task から直接触らない方が安全です。Embassy の実践記事でも、I2C を task 間で共有する場合に `embassy_sync::mutex::Mutex` と shared bus を使う例が紹介されています。 [\[acalustra.com\]](https://acalustra.com/embedded-rust-development-tips-with-embassy.html)
- **指示は bounded channel、状態通知は Watch / PubSub、単発通知は Signal**\
  `embassy-sync` は、MPMC `Channel`、broadcast 的な `PubSubChannel`、単一 consumer 向け `Signal`、複数 observer 向け `Watch`、async `Mutex` 等を提供します。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html)
- **長時間待ちには必ずキャンセル分岐を混ぜる**\
  `Timer::after(...)`、`uart.read(...)`、`channel.receive()` など、長く pending しうる `.await` は `select(cancel, work)` でキャンセル可能にしておくと、上位制御しやすくなります。`embassy_futures` は `select` を「複数 future のうち最初に完了したものを待つ」future combinator として提供しています。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-futures/git/default/index.html)

***

### 3.3 Interrupt-mode executor：`irq -> tasks called` ではなく「割り込み優先度で executor が task を poll」

ご質問では「interrupt based executor: irq(exti etc) -> tasks called」と表現されていますが、Embassy の `InterruptExecutor` は少し違う理解が必要です。公式 docs は、Interrupt-mode executor は task を interrupt mode で実行し、interrupt handler が task を poll するよう setup され、task が wake されると software から interrupt が pend される、と説明しています。 つまり、**EXTI ISR が async task 関数を直接呼ぶ**というより、**interrupt priority を持つ executor context で ready task が poll される**という構造です。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

`InterruptExecutor` の用途として、thread mode を非 async task 用に空けること、または thread-mode executor を低優先度 task 用、interrupt-mode executor を高優先度 task 用にして複数 executor を動かすことが挙げられています。高優先度 task は低優先度 task を preempt でき、さらに複数 interrupt-mode executor を異なる priority の interrupt に割り当てることも可能と説明されています。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

ただし、公式 docs は `InterruptExecutor` について、thread-mode executor より複雑で、use case が満たせるなら thread-mode executor を推奨すると述べています。 これは実務上かなり重要です。高優先度化は便利ですが、割り込み優先度、共有データ保護、critical section、処理時間、デバッグ性が難しくなります。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

`InterruptExecutor::start` は executor を初期化して interrupt を enable し、background で interrupt 経由で動き続け、戻り値として `SendSpawner` を返します。`SendSpawner` が返る理由は、executor が実質的に別 thread、つまり interrupt 側で動くため、そこへ task を spawn する操作は「送信」に相当するからだと docs は説明しています。 また、interrupt handler は自分で書き、`on_interrupt()` を呼ぶ必要があり、この method は interrupt handler からのみ呼ぶべきで、`start()` 前に呼んではならないとされています。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

典型構成は以下です。

設計上の勘所は、**本当に interrupt-mode executor が必要かを最初に疑う**ことです。単に GPIO EXTI でボタン入力を拾いたい、UART RX が来たら処理したい、程度であれば、HAL の async API や channel で thread-mode task を wake するだけで十分な場合が多いです。逆に、数十 µs〜サブ ms レベルの応答性、低優先度 task を確実に中断して処理したい制御、ソフトリアルタイムな high-priority pipeline があるなら、interrupt-mode executor を検討する価値があります。

***

### 3.4 Embassy における CancellationToken 相当：`select + 通知プリミティブ` が実用解

Tokio の graceful shutdown ドキュメントは、shutdown を「停止条件を見つける」「プログラムの各部分に shutdown を伝える」「shutdown 完了を待つ」の3段階で説明しています。 さらに、複数 task に shutdown を伝える方法として `CancellationToken` を示し、clone された token はどれか1つが cancel されると他の clone も cancel され、task は `cancelled()` を await して shutdown に反応できると説明しています。 [\[tokio.rs\]](https://tokio.rs/tokio/topics/shutdown)

Embassy には、調査した公式 docs 上では Tokio の `CancellationToken` と同等の専用 token は確認できませんでした。`embassy-sync` が提供するのは `Channel`、`PriorityChannel`、`PubSubChannel`、`Signal`、`Watch`、`Mutex`、`AtomicWaker` 等です。 そのため、Embassy では以下のような対応関係で考えるのが実用的です。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html)

| 目的               | Tokio / C# 的発想            | Embassy での候補                                              | 向いている場面             |
| ---------------- | ------------------------- | --------------------------------------------------------- | ------------------- |
| 全体 shutdown 通知   | `CancellationToken` clone | `Watch<bool>` / `PubSubChannel<Command>`                  | 複数 task に同じ停止状態を伝える |
| 1 task への cancel | child token               | `Signal<Cancel>` / dedicated `Channel<Command, 1>`        | worker task へ個別停止指示 |
| timer の早期復帰      | `Task.Delay(..., token)`  | `select(Timer::after(...), cancel.receive())`             | タイムアウト待ちを外部イベントで中断  |
| interrupt から通知   | token cancel from ISR     | `Channel<CriticalSectionRawMutex, Event, N>` の `try_send` | ISR から task へ event |
| 完了待ち             | `TaskTracker` / join      | ACK channel / state watch                                 | Embassy 側では自前設計が必要  |

根拠として特に重要なのが GitHub issue #3906 です。この issue では、投稿者が `.NET Task.Delay` の cancellation token 的に `Timer::after(time).await` を外部から早期復帰させたいと述べています。そこで contributor が、`next_pane_counter` に `embassy-sync` の `Channel::Receiver` を渡し、`select(Timer::after(time), rec.receive()).await` すれば、channel message が timer cancellation として働くと提案しています。 その後、`select` は `embassy_futures` のものだと案内され、メンテナが「async Rust の idiomatic way は上記のように `select` を使うこと」「全 async things に cancellation support は追加しない」と述べて issue を close しています。 [\[github.com\]](https://github.com/embassy-rs/embassy/issues/3906) [\[github.com\]](https://github.com/embassy-rs/embassy/issues/3906), [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-futures/git/default/index.html)

つまり Embassy での cancellation token 実装は、次のように書き換えるのが基本です。

```rust
use embassy_futures::select::{select, Either};
use embassy_time::{Timer, Duration};

// 擬似コード: 実際の RawMutex / Channel 型はターゲットに合わせて選ぶ
#[derive(Clone, Copy)]
enum Command {
    Start,
    Stop,
    Cancel,
}

#[embassy_executor::task]
async fn worker(/* receiver: Receiver<Command> */) {
    loop {
        // 1. 次の指示を待つ
        let cmd = /* receiver.receive().await */;
        match cmd {
            Command::Start => {
                // 2. 長い処理や timer は cancel と select する
                let work = async {
                    Timer::after(Duration::from_secs(10)).await;
                    // 実処理
                };

                let cancel = async {
                    loop {
                        let c = /* receiver.receive().await */;
                        if matches!(c, Command::Cancel | Command::Stop) {
                            break;
                        }
                    }
                };

                match select(work, cancel).await {
                    Either::First(_) => {
                        // work completed
                    }
                    Either::Second(_) => {
                        // cancelled cooperatively
                        // cleanup here
                    }
                }
            }
            Command::Stop | Command::Cancel => {
                // graceful stop
                break;
            }
        }
    }
}
```

このコードは設計例であり、引用元のコードそのものではありません。引用元が明示しているのは、`Channel::Receiver` と `select(Timer::after(time), rec.receive()).await` による timer cancellation パターンです。 [\[github.com\]](https://github.com/embassy-rs/embassy/issues/3906)

***

### 3.5 `spawn` 済み task のキャンセルではなく「長寿命 task + command loop」に寄せる

Embassy で複雑な制御を作る際にありがちな落とし穴は、イベントのたびに task を `spawn` し、キャンセルしたくなったら古い task を止めようとする設計です。`spawner.rs` では、`SpawnError::Busy` は「この task の instance がすでに多すぎる」場合に返り、デフォルトでは `#[embassy_executor::task]` の task は1 instance のみ、`pool_size` を指定すると複数 instance が可能だが RAM 使用量が増えると説明されています。 [\[github.com\]](https://github.com/embassy-rs/embassy/blob/main/embassy-executor/src/spawner.rs)

GitHub issue #3906 の背景でも、投稿者は timer task を spawn し、外部割り込みに反応して次の timer task を spawn すると、`pool_size` を超えたとき crash する、と述べています。 これは実務で非常に示唆的です。つまり、イベント頻度が外部要因に依存し、短時間に連打・連続割り込み・通信 burst が起こる可能性があるなら、**task を event ごとに生成する設計は pool 枯渇リスクを持つ**ということです。 [\[github.com\]](https://github.com/embassy-rs/embassy/issues/3906)

より安全なのは、以下のような構成です。

#### 実用パターン A：worker は一度だけ spawn

- 起動時に `worker_task` を1回だけ spawn
- `worker_task` は `loop { command.receive().await; ... }`
- 各 job は内部 state machine として処理
- cancel は channel / signal / watch で受ける
- 完了・キャンセル済みは `status` channel で supervisor へ ACK

#### 実用パターン B：timer task を spawn しない

`Timer::after` だけのために task を spawn するより、既存 worker 内で timer future と cancel future を `select` します。これは issue #3906 の提案と同じ考え方です。 [\[github.com\]](https://github.com/embassy-rs/embassy/issues/3906)

#### 実用パターン C：state machine を enum で明示する

複雑な制御では `AtomicBool` を増やすより、次のような enum state にまとめる方が可読性・検証性が上がります。

```rust
enum State {
    Idle,
    Running,
    Cancelling,
    Fault,
}

enum Command {
    Start,
    Cancel,
    ResetFault,
}

enum Status {
    Idle,
    Started,
    Cancelled,
    Completed,
    Fault,
}
```

この部分は実装提案です。Embassy 公式 source から直接この enum 設計が述べられているわけではありませんが、Embassy の `embassy-sync` が command / status 伝達に使える同期プリミティブを提供していること、`embassy_futures` が `select` を提供していること、task instance の `pool_size` 制約があることに基づく実務上の設計推奨です。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html), [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-futures/git/default/index.html), [\[github.com\]](https://github.com/embassy-rs/embassy/blob/main/embassy-executor/src/spawner.rs)

***

### 3.6 割り込み、Channel、RawMutex の使い分け

`embassy_sync::channel` の docs は、`Channel` を async task 間で値を送る queue と説明し、複数 producer / 複数 consumer の MPMC channel であると述べています。 また、この queue は mutex type を取るため、thread mode task 間でのみ message を渡すなら `ThreadModeMutex`、interrupt handler から task へ message を渡すなら `CriticalSectionMutex` を使う、と説明されています。 docs には `CriticalSectionRawMutex` の `Channel` を static に作り、interrupt handler から `try_send(42)` し、async task が `receiver.receive().await` で受け取る例が掲載されています。 [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/channel/index.html)

これは Embassy で割り込みを扱ううえで非常に重要です。ISR では以下を原則にします。

- **ISR では長い処理をしない**
- **await しない**
- **blocking send しない**
- **bounded channel が満杯なら drop / overwrite / error count など方針を決める**
- **実処理は task 側に寄せる**

Embassy では `AtomicWaker` も interrupt context から Waker を wake する utility として提供されています。 ただし通常の application レベルでは、まず `Channel` / `Signal` / `Watch` を検討し、低レベル future を自作する必要がある場合に `AtomicWaker` を検討する順番でよいと思います。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html)

`Signal`、`Channel`、`Watch` の使い分けは次のように考えると整理しやすいです。

| プリミティブ          | 公式説明上の性質                                                                                                                            | キャンセル用途での使い方                                     |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| `Channel`       | MPMC、各 message は1 consumer だけが受信。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html)              | command queue、ISR event queue、ACK queue          |
| `PubSubChannel` | publish-subscribe、各 message が全 subscriber に届く。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html) | broadcast stop / mode change                     |
| `Signal`        | latest value を単一 consumer に通知。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html)                 | 特定 worker への cancel pulse                        |
| `Watch`         | latest value を複数 receiver に通知。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html)                 | global shutdown flag / mode / generation counter |
| `Mutex`         | async task 間の状態同期。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html)                             | peripheral 共有、状態共有。ただし cancel 通知には過剰な場合あり        |

特に cancellation token 的な意味では、**boolean だけではなく generation counter を持つ Watch** が便利です。たとえば `CancelGen(u32)` を publish し、task は自分が処理開始時に見た generation と現在値を比較します。これにより「古い cancel signal を誤って消費する」「cancel 後に再 start したら前回 cancel が残っていた」といった問題を避けやすくなります。これは本調査から導いた設計提案であり、特定の公式 API が generation counter を提供すると述べているわけではありません。

***

## 4. 比較・評価

### 4.1 Thread-mode executor と Interrupt-mode executor の比較

| 観点           | Thread-mode executor                                                                                                                     | Interrupt-mode executor                                                                                                                              |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| 公式上の位置づけ     | 最も単純で一般的な executor。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.Executor.html)                  | interrupt mode で task を実行する executor。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)   |
| sleep / wake | Cortex-M では WFE で sleep、wake 時に SEV。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.Executor.html) | task wake 時に software interrupt を pend。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html) |
| 優先度          | 通常は低優先度・thread mode                                                                                                                      | thread mode より高優先度で task 実行可能。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)          |
| 複雑さ          | 低い                                                                                                                                       | 公式 docs は「より複雑」とし、可能なら thread-mode を推奨。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html) |
| Spawner      | `Spawner`                                                                                                                                | `start` は `SendSpawner` を返す。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)            |
| 主な用途         | 一般制御、通信、UI、sensor polling                                                                                                                | 低遅延制御、高優先度 event 処理、priority separation                                                                                                              |
| 注意点          | `.await` しない長い同期処理は他 task を詰まらせる                                                                                                         | ISR priority、critical section、処理時間、共有データ保護が難しい                                                                                                       |

### 4.2 キャンセル実装方式の比較

| 方式                   | 長所                                 | 短所                                        | 推奨度                        |
| -------------------- | ---------------------------------- | ----------------------------------------- | -------------------------- |
| `AtomicBool` polling | 非常に軽い                              | `.await` 中は反応できない。履歴・世代管理が弱い              | 補助用途                       |
| `Signal`             | 単一 task への通知が簡単                    | 複数 task broadcast には不向き                   | 個別 cancel に有効              |
| `Channel<Command>`   | start/cancel/stop/reset 等を型で表現しやすい | queue overflow 設計が必要                      | 最有力                        |
| `Watch<State>`       | 複数 observer が最新状態を見られる             | event queue ではなく state 通知                 | global mode / shutdown に有効 |
| `PubSubChannel`      | broadcast event 向き                 | subscriber / capacity 設計が必要               | 複数 task への通知に有効            |
| spawn し直し            | 実装が一見簡単                            | `pool_size` / `Busy` / RAM 増加 / cancel 困難 | 原則避ける                      |

***

## 5. 参考サイト・コンテンツ一覧：読む順と得られる知見

### 5.1 最優先：公式・一次情報

1. Embassy Book\
   Embassy の目的、async/await の考え方、executor / HAL / timer / DMA など全体像を把握する入口です。Embassy は async/await を embedded development の first-class option にする project であると説明しています。\
   URL: [Embassy Book](https://embassy.dev/book/index.html) [\[embassy.dev\]](https://embassy.dev/book/index.html)

2. embassy\_executor - Rust / docs.embassy.dev\
   `embassy-executor` の no alloc、static task allocation、timer queue、wake された task のみ poll、fairness、multiple executor、feature flags、custom platform の説明があります。\
   URL: [embassy\_executor docs](https://docs.embassy.dev/) [\[docs.embassy.dev\]](https://docs.embassy.dev/)

3. Executor in embassy\_executor\
   Thread-mode executor の WFE/SEV、`run`、`Spawner` の渡し方、executor instance の lifetime に関する説明があります。\
   URL: [Thread-mode Executor docs](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.Executor.html) [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.Executor.html)

4. InterruptExecutor in embassy\_executor\
   Interrupt-mode executor の本質、software interrupt による wake、priority separation、`SendSpawner`、interrupt handler で `on_interrupt()` を呼ぶ必要性、thread-mode 推奨の注意がまとまっています。\
   URL: [InterruptExecutor docs](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html) [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

5. embassy\_sync - Rust\
   `Channel`、`PriorityChannel`、`PubSubChannel`、`Signal`、`Watch`、`Mutex`、`AtomicWaker` など、キャンセル通知・状態通知・task 間通信に使う基盤 API の一覧があります。\
   URL: [embassy\_sync docs](https://docs.embassy.dev/embassy-sync/git/default/index.html) [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html)

6. embassy\_sync::channel - Rust\
   MPMC channel、bounded channel、interrupt handler から task への message passing、`CriticalSectionRawMutex` の例があり、ISR と async task の接続設計に特に有用です。\
   URL: [embassy\_sync::channel docs](https://docs.rs/embassy-sync/latest/embassy_sync/channel/index.html) [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/channel/index.html)

7. embassy\_futures - Rust\
   `no_std` / no alloc 対応の future utility で、`join`、`select`、`yield_now` 等を提供します。キャンセル設計の中心になる `select` の入口です。\
   URL: [embassy\_futures docs](https://docs.embassy.dev/embassy-futures/git/default/index.html) [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-futures/git/default/index.html)

### 5.2 キャンセル設計に直接効く GitHub issue

8. embassy-time: Timer with\_cancellation #3906\
   .NET `Task.Delay` + cancellation token 的な `Timer` cancel 要望に対して、`Channel::Receiver` と `select(Timer::after(...), rec.receive())` を使う具体案が提示され、メンテナが `select` を idiomatic と述べています。今回の主題に最も直結する資料です。\
   URL: [GitHub issue #3906](https://github.com/embassy-rs/embassy/issues/3906) [\[github.com\]](https://github.com/embassy-rs/embassy/issues/3906)

9. embassy/embassy-executor/src/spawner.rs\
   `SpawnToken`、`Spawner`、`SendSpawner`、`SpawnError::Busy`、`pool_size` の実際の source コメントが読めます。spawn を job queue と誤解しないために重要です。\
   URL: [spawner.rs](https://github.com/embassy-rs/embassy/blob/main/embassy-executor/src/spawner.rs) [\[github.com\]](https://github.com/embassy-rs/embassy/blob/main/embassy-executor/src/spawner.rs)

### 5.3 実践記事・周辺資料

10. Practical Embedded Rust Development Guide with Embassy\
    I2C 共有、`embassy_futures::select` による複数 event 待ち、Embassy channel による task 通信など、実務寄りの tip がまとまっています。\
    URL: [Practical Embedded Rust Development Guide with Embassy](https://acalustra.com/embedded-rust-development-tips-with-embassy.html) [\[acalustra.com\]](https://acalustra.com/embedded-rust-development-tips-with-embassy.html)

11. Async Execution Model | embassy-rs/embassy | DeepWiki\
    Embassy の cooperative scheduler、task lifecycle、timekeeping、synchronization primitives、tracing などを概観する二次資料です。一次情報ではありませんが、構造理解に役立ちます。\
    URL: [Async Execution Model | DeepWiki](https://deepwiki.com/embassy-rs/embassy/2-async-execution-model) [\[deepwiki.com\]](https://deepwiki.com/embassy-rs/embassy/2-async-execution-model)

12. Graceful Shutdown | Tokio\
    Embassy ではそのまま使えませんが、shutdown を「検知」「通知」「完了待ち」に分解する考え方、`CancellationToken` の clone / cancel / cancelled wait の設計は、Embassy の command channel / watch 設計へ翻訳できます。\
    URL: [Tokio Graceful Shutdown](https://tokio.rs/tokio/topics/shutdown) [\[tokio.rs\]](https://tokio.rs/tokio/topics/shutdown)

***

## 6. 実装ノウハウ：Embassy 版 CancellationToken をどう作るか

### 6.1 最小構成：単一 worker への cancel

単一 worker に対して「今の処理をやめて」と伝えるだけなら、`Signal` または capacity 1 の `Channel<Command>` で十分です。`embassy-sync` は `Signal` を「latest value を単一 consumer に通知」と説明しています。`Command` を enum にしておくと、`Cancel` 以外に `Start`、`Stop`、`Reconfigure` 等を追加しやすくなります。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html)

```rust
enum Command {
    Start,
    Cancel,
    Stop,
}

// worker 内では、長く待つ future と cancel receive を select する
```

ここで重要なのは、cancel を「割り込み」ではなく「task が `.await` 可能な future」として扱うことです。`embassy_futures` の `select` は「複数 future のうち最初に完了したものを待つ」ための combinator です。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-futures/git/default/index.html)

### 6.2 複数 task への global cancel

複数 task に「全体 shutdown」や「mode change」を伝えるなら、`Watch` または `PubSubChannel` が候補です。`embassy-sync` は `Watch` を「latest value を複数 receiver に通知」、`PubSubChannel` を「publish-subscribe channel、各 message が全 consumer に届く」と説明しています。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html)

使い分けは以下です。

- **状態が重要**：`Watch<SystemState>`\
  例：`Run`, `ShutdownRequested`, `Fault`, `Maintenance`
- **イベントが重要**：`PubSubChannel<SystemEvent>`\
  例：`EmergencyStop`, `NetworkLost`, `ButtonLongPress`

global cancel では、bool より enum を推奨します。`true/false` だけでは「停止要求」「停止中」「停止完了」「fault 停止」の区別が曖昧になりやすいためです。

### 6.3 完了待ち：Tokio の TaskTracker 相当は ACK channel で作る

Tokio の graceful shutdown docs は、shutdown を通知した後、`TaskTracker` で task 完了を待つ例を示しています。 調査した Embassy 公式 docs では `TaskTracker` 相当の標準機構は確認できませんでした。したがって Embassy では、worker ごとに `Status::Stopped` や `Ack::Cancelled` を supervisor に返す ACK channel を設計するのが現実的です。 [\[tokio.rs\]](https://tokio.rs/tokio/topics/shutdown)

この ACK 設計は提案ですが、Embassy の `Channel` が task 間 communication に使えること、bounded channel として設計できることは docs に示されています。 [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/channel/index.html)

### 6.4 cancel-safe な task の書き方

Embassy の cooperative async では、cancel は `.await` 境界で反応します。従って次のような設計が重要です。

1. **長い同期処理を避ける**\
   `.await` なしで長時間ループすると他 task も cancel も進みません。
2. **長時間 wait は必ず `select` する**\
   `Timer::after`、I/O receive、channel receive など。
3. **peripheral を安全状態に戻す cleanup を書く**\
   motor off、PWM duty 0、CS deassert、DMA stop など。
4. **キャンセル完了を ACK する**\
   上位 supervisor が「止まったつもり」にならないようにします。
5. **再 start と cancel の競合を state machine で扱う**\
   bool flag の羅列ではなく、`State` と `Command` の組み合わせを明示します。

***

## 7. 結論と提言

Embassy で複雑なタスクを制御する場合、最も重要な発想転換は、\*\*「spawn した task を外からキャンセルする」ではなく、「長寿命 task が上位 command を受け取り、`select` で協調的に現在処理を中断する」\*\*ことです。これは GitHub issue #3906 における実際のやり取り、`embassy_futures::select`、`embassy_sync::Channel` の設計と整合します。 [\[github.com\]](https://github.com/embassy-rs/embassy/issues/3906), [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-futures/git/default/index.html), [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/channel/index.html)

Thread-mode executor は、一般的な制御・通信・UI・sensor task の土台として最初に選ぶべき構成です。公式 docs でも最も単純で一般的な executor とされています。 Interrupt-mode executor は、より高い優先度で task を poll でき、低優先度 task を preempt できる強力な手段ですが、公式 docs はより複雑で、可能なら thread-mode を推奨すると述べています。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.Executor.html) [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

実装方針としては、以下を推奨します。

- **初期 task は起動時に一度だけ spawn**
- **イベントごとに task を spawn しない**
- **worker は command loop 化**
- **キャンセルは `select(work, cancel)`**
- **割り込みは `try_send` / `signal` だけに留める**
- **上位 supervisor は ACK を待つ**
- **状態は enum state machine で明示**
- **`pool_size` は最後の手段として慎重に使う**

最終的に、C# / Tokio の `CancellationToken` は Embassy では以下のように翻訳するとよいです。

```text
CancellationToken
  ≒ Watch<SystemState> / PubSubChannel<SystemEvent> / Channel<Command>
cancelled().await
  ≒ select(work_future, cancel_receiver.receive()).await
TaskTracker.wait()
  ≒ ACK Channel + Supervisor state machine
spawn per request
  ≒ long-running worker + command loop
```

この翻訳を採用すれば、Embassy の静的・有界・低消費電力・no\_std という強みを壊さず、上位からのキャンセル伝播を実用化できます。

***

## 8. 参考文献・情報源

- Embassy Book — Embassy の全体像、async/await、executor、HAL の入口。 [\[embassy.dev\]](https://embassy.dev/book/index.html)
- embassy\_executor - Rust — executor の特徴、static allocation、timer queue、multiple executor、feature flags。 [\[docs.embassy.dev\]](https://docs.embassy.dev/)
- Executor in embassy\_executor — Thread-mode executor、WFE/SEV、`run`、`Spawner`。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.Executor.html)
- InterruptExecutor in embassy\_executor — Interrupt-mode executor、software interrupt、priority separation、`SendSpawner`、注意点。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)
- embassy\_sync - Rust — `Channel`、`PubSubChannel`、`Signal`、`Watch`、`Mutex`、`AtomicWaker`。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/index.html)
- embassy\_sync::channel - Rust — MPMC bounded channel、interrupt handler から task への message passing。 [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/channel/index.html)
- embassy\_futures - Rust — `select` / `join` / `yield_now` など no\_std future utility。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-futures/git/default/index.html)
- embassy-time: Timer with\_cancellation #3906 — Embassy で cancellation token 的な timer cancel をどう考えるかの最重要 issue。 [\[github.com\]](https://github.com/embassy-rs/embassy/issues/3906)
- embassy/embassy-executor/src/spawner.rs — `SpawnToken`、`Spawner`、`SendSpawner`、`SpawnError::Busy`、`pool_size` の source コメント。 [\[github.com\]](https://github.com/embassy-rs/embassy/blob/main/embassy-executor/src/spawner.rs)
- Practical Embedded Rust Development Guide with Embassy — Embassy 実践 tips、`select`、channel、I2C 共有。 [\[acalustra.com\]](https://acalustra.com/embedded-rust-development-tips-with-embassy.html)
- Async Execution Model | embassy-rs/embassy | DeepWiki — Embassy の実行モデルを俯瞰する二次資料。 [\[deepwiki.com\]](https://deepwiki.com/embassy-rs/embassy/2-async-execution-model)
- Graceful Shutdown | Tokio — `CancellationToken`、shutdown 検知・通知・完了待ちの考え方。 [\[tokio.rs\]](https://tokio.rs/tokio/topics/shutdown)

以下、**Embassy/embassy-rs で複雑なタスク制御をどう設計するか**を、
**(A) executor の基本整理** → **(B) thread mode / interrupt mode の使い分け** → **(C) cancellation token 相当の実装パターン** → **(D) 実務ノウハウ** → **(E) 参考サイト**
の順に、かなり実務寄りで整理します。

***

# まず結論

Embassy では、**Tokio や C# のような「専用の CancellationToken 型」を中心に設計する**より、
**`Signal` / `Watch` / `Channel` / `select` / `with_timeout` を組み合わせて、「協調的キャンセル」を実装する**のが実務上の基本です。今回確認した公式ドキュメント群では、`Signal` は **single consumer 向け**、`Watch` は **multiple consumers 向け**、`Channel` は **MPMC キュー**、`select` は **先に完了した future を選び、もう片方は drop**、`with_timeout` は **タイムアウト付き実行**として説明されています。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/signal/struct.Signal.html), [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/watch/struct.Watch.html), [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/channel/index.html), [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-futures/git/default/select/fn.select.html), [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-time/0.1.5)

また、Embassy の executor は **協調型 (cooperative)** で、タスクは `.await` でのみ実行権を返します。したがって、**「上位からのキャンセルを即時伝播したい」なら、長い計算ループや長い I/O の途中で定期的に await 可能点を作る**ことが重要です。Embassy の runtime は、タスクが `.await` で `Poll::Pending` を返すと他のタスクに実行を回す仕組みであり、終了またはキャンセルされたタスクは再度 enqueue されない、と説明されています。 [\[embassy-rs.github.io\]](https://embassy-rs.github.io/embassy-book/embassy/dev/runtime.html), [\[deepwiki.com\]](https://deepwiki.com/embassy-rs/embassy/2-async-execution-model)

さらに重要なのは、**interrupt executor は「EXTI のたびにその IRQ が直接 task を呼ぶ」ものではない**、という点です。公式 docs では `InterruptExecutor` は **専用の割り込みで executor 自体を駆動する仕組み**として説明されており、使用する IRQ は **ハードが使わない IRQ / software interrupt (SWI)** を選ぶのが想定です。むしろ通常の peripheral IRQ（EXTI, DMA, USART など）は、HAL がその割り込みを処理して **待っている task を wake する**流れが基本です。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html), [\[embassy-rs.github.io\]](https://embassy-rs.github.io/embassy-book/embassy/dev/runtime.html)

***

# 1. Embassy のタスク制御の基本像

Embassy の executor は、**静的に確保された task storage** 上で async task を動かします。`embassy-executor` は **heap 不要 / alloc 不要**、各 task は静的に保持され、**wake された task だけを poll** し、**公平性 (fairness)** があると説明されています。さらに、複数 executor を作ることで **優先度の異なる task 群**を分離できます。 [\[docs.rs\]](https://docs.rs/embassy-executor/latest/embassy_executor/), [\[github.com\]](https://github.com/embassy-rs/embassy/blob/main/embassy-executor/README.md), [\[docs.rs\]](https://docs.rs/crate/embassy-executor/latest)

Embassy Book では、タスクは I/O で待つと future が yield し、executor が別の future を実行する、と説明されています。つまり RTOS のような「任意の地点でプリエンプト」ではなく、**await 協調型**です。これは、組み込みでありがちな「ドライバ待ち」「IRQ 完了待ち」「一定周期処理」と非常に相性が良いです。 [\[embassy.dev\]](https://embassy.dev/book/index.html), [\[embassy-rs.github.io\]](https://embassy-rs.github.io/embassy-book/embassy/dev/index.html)

実務上は、頭の中で次のように整理すると分かりやすいです。

- **タスク = 長寿命の責務単位**
  - 通信管理
  - 状態機械
  - センサー収集
  - コマンド処理
  - ログ出力
- **IRQ = イベント通知の発火点**
  - 完了
  - 入力変化
  - DMA 完了
  - タイマ
- **同期原語 = タスク間・IRQ→タスク間の接着剤**
  - `Signal`
  - `Watch`
  - `Channel`
  - `Mutex`
- **キャンセル = 強制 kill ではなく、協調停止**
  - `select(cancel.wait(), work_future)`
  - `with_timeout(...)`
  - 状態遷移による停止要求
  - `Watch` による「現在モード」伝播

***

# 2. 1) thread based executor: `main -> spawn -> tasks`

これは **最も基本・最も推奨される形**です。公式 docs でも thread-mode executor は **“the simplest and most common kind of executor”** と説明されています。Cortex-M 系では、thread mode executor は thread mode（最下位優先度）で動き、仕事がなければ `WFE` で sleep、wake 時に `SEV` で復帰する仕組みとして説明されています。 [\[docs.espressif.com\]](https://docs.espressif.com/projects/rust/esp-hal-embassy/0.7.0/src/esp_hal_embassy/executor/thread.rs.html), [\[tomo-wait-...nablog.com\]](https://tomo-wait-for-it-yuki.hatenablog.com/entry/2023/08/06/062234)

また、`#[embassy_executor::main]` を使うと、Embassy runtime は executor を作り、`main` を最初の task として spawn します。公式 runtime docs には、`#[embassy::main]` マクロを使うと executor を作成し、エントリポイントを最初の task として spawn する、と書かれています。 [\[embassy-rs.github.io\]](https://embassy-rs.github.io/embassy-book/embassy/dev/runtime.html)

### 典型構成

```rust
#[embassy_executor::task]
async fn sensor_task() {
    loop {
        // センサ読み取り
        // await point を必ず作る
        Timer::after_millis(10).await;
    }
}

#[embassy_executor::task]
async fn control_task() {
    loop {
        // 制御状態機械
        Timer::after_millis(1).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    spawner.spawn(sensor_task()).unwrap();
    spawner.spawn(control_task()).unwrap();

    loop {
        Timer::after_secs(1).await;
    }
}
```

この形のメリットは以下です。

- **構成が単純**
- **低消費電力と相性が良い**
- **タスク間設計に集中できる**
- HAL の interrupt + async driver の恩恵を最も素直に受けられる    [\[docs.rs\]](https://docs.rs/embassy-executor/latest/embassy_executor/), [\[embassy-rs.github.io\]](https://embassy-rs.github.io/embassy-book/embassy/dev/runtime.html), [\[github.com\]](https://github.com/embassy-rs/embassy/blob/main/embassy-executor/README.md)

### 実務上の向き不向き

**向いているもの**

- 複数通信の並列待ち
- センサー周期監視
- UI/状態機械
- 上位プロトコル処理
- 中低優先度の制御ループ

**注意点**

- 1つの task が長時間 `.await` せず回り続けると、**協調型なので他 task が遅延**します。これは Embassy runtime docs の「tasks must not block indefinitely」に対応します。 [\[embassy-rs.github.io\]](https://embassy-rs.github.io/embassy-book/embassy/dev/runtime.html)
- 「重い計算」は細切れ化するか、スケジューリングを意識して `yield_now` 相当の分割を考えるべきです。`embassy-futures` には `yield_now` があります。 [\[docs.rs\]](https://docs.rs/embassy-futures/latest/embassy_futures/)

***

# 3. 2) interrupt based executor: `irq -> tasks`

## 3.1 まず誤解しやすい点

ご質問の

> irq(exti etc) -> tasks called

は、**半分正しく、半分誤解されやすい**です。

Embassy の通常の async driver フローでは、
**peripheral IRQ が直接 “task 本体” を呼ぶ**というより、

1. task が peripheral 操作を開始して待つ
2. peripheral IRQ が発生
3. HAL が状態更新し waiting task を wake
4. executor が wake された task を poll

という流れです。これは runtime docs の “typical application flow” に書かれています。 [\[embassy-rs.github.io\]](https://embassy-rs.github.io/embassy-book/embassy/dev/runtime.html)

## 3.2 `InterruptExecutor` は何か

`InterruptExecutor` は、**executor 自体を割り込みコンテキストで動かす**仕組みです。公式 docs では、interrupt handler が task を poll し、task が wake されると IRQ を software pend するので、thread mode より高優先度で async task を動かせる、と説明されています。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

さらに docs では、使う IRQ は **hardware が使わない IRQ**、または **software interrupt (SWI)** を使うのが前提と明記されています。つまり、**EXTI や USART の“本物の peripheral IRQ”を interrupt executor の専用 IRQ と兼用する設計は基本的に避ける**べき、という理解が実務的に正しいです。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

## 3.3 どういうときに使うか

公式 docs では、InterruptExecutor の用途として、

- thread mode を non-async 処理用に空けておく
- 複数 executor を走らせ、**高優先度 executor が低優先度 executor を preempt** する

ことが挙げられています。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html), [\[docs.rs\]](https://docs.rs/embassy-executor/latest/embassy_executor/)

### 実務で向いている例

- 極低レイテンシが必要な async task
- `main` 系の通常 task 群とは切り離したい高優先度処理
- 高優先度制御と通常通信を executor 単位で分離したい場合

### 実務で避けたい例

- 何でも interrupt executor に載せる
- EXTI/DMA/USART 割り込みを使い回して executor を駆動する
- 長い処理を interrupt executor 上で回す
  （高優先度で他を止めやすい）

***

# 4. 複雑なタスク制御では「task を増やしすぎる」より「責務を分離する」

これはあなたの RTOS/NORTi/FreeRTOS 的な感覚に近いですが、Embassy でもかなり重要です。

社内検索でも、[コントローラータスク構成図.pdf](https://shimadzugroup-my.sharepoint.com/personal/okoba_tsutomu_wd_shimadzu_jp/Documents/Microsoft%20Teams%20%e3%83%81%e3%83%a3%e3%83%83%e3%83%88%20%e3%83%95%e3%82%a1%e3%82%a4%e3%83%ab/%e3%82%b3%e3%83%b3%e3%83%88%e3%83%ad%e3%83%bc%e3%83%a9%e3%83%bc%e3%82%bf%e3%82%b9%e3%82%af%e6%a7%8b%e6%88%90%e5%9b%b3.pdf?web=1\&EntityRepresentationId=409ce053-f9a4-4f6c-b47e-cd1ddf47d756) や [CDD制御タスク構成図.pdf](https://shimadzugroup-my.sharepoint.com/personal/okoba_tsutomu_wd_shimadzu_jp/Documents/Microsoft%20Teams%20%e3%83%81%e3%83%a3%e3%83%83%e3%83%88%20%e3%83%95%e3%82%a1%e3%82%a4%e3%83%ab/CDD%e5%88%b6%e5%be%a1%e3%82%bf%e3%82%b9%e3%82%af%e6%a7%8b%e6%88%90%e5%9b%b3.pdf?web=1\&EntityRepresentationId=17a1cb1a-57db-47ad-b9ed-67f604ff50f8) には、通信・分析・モニタ・エラー回復・時間管理などを責務ごとに分けた構成が見えています。Embassy に移しても、この「責務で切る」発想自体はかなり相性が良いです。 [\[コントローラータスク構成図 \| PDF\]](https://shimadzugroup-my.sharepoint.com/personal/okoba_tsutomu_wd_shimadzu_jp/Documents/Microsoft%20Teams%20%e3%83%81%e3%83%a3%e3%83%83%e3%83%88%20%e3%83%95%e3%82%a1%e3%82%a4%e3%83%ab/%e3%82%b3%e3%83%b3%e3%83%88%e3%83%ad%e3%83%bc%e3%83%a9%e3%83%bc%e3%82%bf%e3%82%b9%e3%82%af%e6%a7%8b%e6%88%90%e5%9b%b3.pdf?web=1), [\[CDD制御タスク構成図 \| PDF\]](https://shimadzugroup-my.sharepoint.com/personal/okoba_tsutomu_wd_shimadzu_jp/Documents/Microsoft%20Teams%20%e3%83%81%e3%83%a3%e3%83%83%e3%83%88%20%e3%83%95%e3%82%a1%e3%82%a4%e3%83%ab/CDD%e5%88%b6%e5%be%a1%e3%82%bf%e3%82%b9%e3%82%af%e6%a7%8b%e6%88%90%e5%9b%b3.pdf?web=1)

ただし Embassy では RTOS のような「タスク間メールボックスを何層も張る」より、次のように整理すると見通しが良くなります。

- **状態更新の配布** → `Watch`
- **1対1 の停止/命令** → `Signal`
- **キューイングが必要な要求** → `Channel`
- **共有資源保護** → `Mutex`
- **並列待ち・停止待ち** → `select`
- **時間制約** → `with_timeout`

***

# 5. CancellationToken 相当を Embassy でどう実現するか

ここが本題です。

## 5.1 Embassy でのキャンセルは「協調的キャンセル」

`embassy_futures::select` は、**複数 future のうち最初に完了したものを採用し、他方は drop** すると明記されています。したがって、Embassy でのキャンセルは典型的に

- `work_future`
- `cancel_future`

を `select` して、**cancel が先なら work を drop して抜ける**

という形になります。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-futures/git/default/select/fn.select.html), [\[github.com\]](https://github.com/embassy-rs/embassy/blob/main/embassy-futures/src/select.rs)

### 基本形

```rust
use embassy_futures::select::{select, Either};

async fn worker() {
    match select(do_work(), CANCEL_SIGNAL.wait()).await {
        Either::First(_) => {
            // work 完了
        }
        Either::Second(_) => {
            // cancel 指示で中断
        }
    }
}
```

これは Tokio/C# の cancellation token に最も近いです。

***

## 5.2 単一 task を止めるなら `Signal`

`Signal` は single-slot / single-consumer 向けで、**最新値だけ渡せばよい停止要求**に向いています。公式 docs では `wait()` は **cancel-safe** で、「poll を最後まで完了しなくても値は失われない」と明記されています。これは `select(cancel.wait(), something)` に非常に向いています。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/signal/struct.Signal.html)

### 典型パターン: 1 task 用 cancel token

```rust
use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_futures::select::{select, Either};

#[derive(Clone, Copy)]
enum CancelCmd {
    Cancel,
}

static CANCEL: Signal<CriticalSectionRawMutex, CancelCmd> = Signal::new();

async fn long_running_task() {
    loop {
        match select(step_once(), CANCEL.wait()).await {
            Either::First(_) => {
                // 1ステップ終わったので継続
            }
            Either::Second(CancelCmd::Cancel) => {
                // 後始末して終了
                cleanup().await;
                break;
            }
        }
    }
}
```

### 向いている用途

- 単一 worker の停止
- 再起動要求
- 「今すぐループを抜けて安全停止」の通知

### 注意

- `Signal` は **single consumer** なので、複数 task に一斉停止伝播したいなら不向きです。docs でも multiple consumers なら `Watch` を使うよう書かれています。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/signal/struct.Signal.html), [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/watch/struct.Watch.html)

***

## 5.3 複数 task へ上位からキャンセルを伝播するなら `Watch`

`Watch` は **single-slot / multiple receivers** の原語で、複数 receiver が最新値の変化を待てます。docs では、「最新 state を複数 task に知らせる」のに向く、と明記されています。 [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/watch/struct.Watch.html)

これは C# の「共有 cancellation state」にかなり近いです。

### 典型パターン: システムモード/停止状態

```rust
use embassy_sync::watch::Watch;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RunState {
    Running,
    StopRequested,
    Stopped,
}

static RUN_STATE: Watch<CriticalSectionRawMutex, RunState, 8> =
    Watch::new_with(RunState::Running);
```

各 task は、

- 通常処理中は `RUN_STATE` を適宜チェック
- ブロック点では `select(work, state_changed())`
- `StopRequested` を見たら安全停止

という構成にします。

### 向いている用途

- 複数 task に対する一斉停止
- 動作モード (`Idle / Running / Homing / Fault / Shutdown`)
- supervisor task からの状態配信

### 注意

docs にある通り `Watch` は**古い値を上書き**するので、**すべてのイベントを取りこぼさず処理したい用途には向きません**。停止や状態更新のように「最新値だけ見れば良い」用途に向きます。 [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/watch/struct.Watch.html)

***

## 5.4 “取りこぼし厳禁” なら `Channel`

`Channel` は MPMC キューで、各メッセージは 1 consumer が受け取ります。docs には IRQ handler から `try_send()`、task 側で `receive().await` する例もあります。 [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/channel/index.html)

### 向いている用途

- 停止要求だけでなく、**理由コード付きコマンド列**
- `Start`, `Stop`, `Reconfigure`, `Flush`, `Recover`
- command dispatcher → worker

### 典型パターン

```rust
enum Command {
    Start,
    Stop,
    Reconfigure(Config),
}

static CMD_CH: Channel<CriticalSectionRawMutex, Command, 8> = Channel::new();
```

### 注意

- 停止を「全員に一斉伝播」したいだけなら `Watch` のほうが自然
- 厳密なキュー性・順序性が必要なら `Channel`

***

## 5.5 タイムアウトキャンセルは `with_timeout`

`embassy_time` には **`with_timeout`** があり、一定時間で future を打ち切るパターンに使えます。docs で明記されています。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-time/0.1.5), [\[docs.rs\]](https://docs.rs/embassy-time/latest/embassy_time/)

```rust
use embassy_time::{with_timeout, Duration};

match with_timeout(Duration::from_millis(100), some_io()).await {
    Ok(v) => { /* 成功 */ }
    Err(_) => { /* timeout */ }
}
```

これは

- 上位からの停止
- タイムアウト
- 通常完了

の 3つを `select` と組み合わせて扱えます。

***

# 6. 実務上かなり重要なノウハウ

## 6.1 “キャンセルされる側” は await 点を十分に作る

Embassy は cooperative なので、以下のような task はキャンセル伝播が悪いです。

```rust
loop {
    // 重い計算を延々実行
}
```

これだと `Signal` も `Watch` も見に行けません。
したがって、

- 1ステップごとに await
- polling ループを小さく切る
- I/O 待ちを async HAL に寄せる
- 重計算はチャンク化して `yield_now` 的に割る

が重要です。`embassy-futures` には `yield_now` があります。 [\[docs.rs\]](https://docs.rs/embassy-futures/latest/embassy_futures/)

***

## 6.2 「停止要求」と「停止完了通知」は分ける

C# でも同じですが、**cancel request** と **stopped acknowledgment** は分けたほうが安全です。

おすすめは：

- 上位 → 下位: `Signal<Cancel>`
- 下位 → 上位: `Signal<Stopped>` or `Watch<RunState>`

停止要求だけ送って「止まったはず」と思い込むのは危険です。
特に機械制御・モータ制御・通信停止では、**安全停止完了を別経路で確認**したほうがよいです。

この発想は、社内の [EA7Z-0193-0002\_Platform\_Programming\_Reference.docx](https://shimadzugroup.sharepoint.com/sites/SCJ_GP1929/_layouts/15/Doc.aspx?sourcedoc=%7B54CE8C28-C39E-4E6C-AD0E-2292B2BA68C5%7D\&file=EA7Z-0193-0002_Platform_Programming_Reference.docx\&action=default\&mobileredirect=true\&DefaultItemOpen=1\&EntityRepresentationId=d7453311-9a60-4a79-93f9-a0477ab2ede2) でも、通知開始/停止、同期応答、通知解除の同期という形で近い考え方が見えています。 [\[EA7Z-0193-..._Reference \| Word\]](https://shimadzugroup.sharepoint.com/sites/SCJ_GP1929/_layouts/15/Doc.aspx?sourcedoc=%7B54CE8C28-C39E-4E6C-AD0E-2292B2BA68C5%7D&file=EA7Z-0193-0002_Platform_Programming_Reference.docx&action=default&mobileredirect=true&DefaultItemOpen=1)

***

## 6.3 子タスクを乱立させるより「常駐 task + command/state」のほうが堅い

Embassy の task は静的割当で、spawn/再spawn 自体は可能ですが、
**複雑制御では「都度 spawn / kill」を多用するより、長寿命 worker に指示を送る**ほうが設計が安定しやすいです。

理由は：

- 静的 task slot との整合が分かりやすい
- 中断時の後始末位置が固定できる
- デバッグしやすい
- 「どの task が今生きているか」が明快

つまり C# 的に

- 新しい Job を毎回 `Task.Run` する感覚

よりも、

- **専用 service task を立てて command/state で動かす**

ほうが Embassy には合っています。

***

## 6.4 `InterruptExecutor` は“高優先度の少数タスク”だけに限定する

これはかなり重要です。

`InterruptExecutor` は便利ですが、docs 上も **thread-mode executor のほうが simpler / recommended** に読める記述です。高優先度で async task を回せる反面、設計を誤るとシステム全体のレイテンシ分布が崩れやすいです。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

おすすめの設計は：

- **通常系**
  - thread-mode executor
  - UI、通信上位、状態監視、ロギング
- **高優先度系**
  - interrupt executor
  - 低レイテンシ要求の少数 task のみ

***

## 6.5 EXTI は「executor IRQ」ではなく「イベント源」と考える

ここは実務で混線しやすいです。

- **EXTI**
  - 入力変化のイベント源
  - HAL が pending 処理や wake に使う
- **InterruptExecutor の IRQ**
  - executor を駆動する専用 IRQ

この2つは別概念として考えたほうが安全です。
特に STM32 / nRF / RP 系で async HAL を使う場合、peripheral IRQ は **driver future の再開トリガ**として働く理解が基本です。 [\[embassy-rs.github.io\]](https://embassy-rs.github.io/embassy-book/embassy/dev/runtime.html), [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

***

# 7. おすすめ設計パターン（実務向け）

## パターンA: supervisor + workers + state watch

**最もおすすめ**です。

- `supervisor_task`
  - 全体状態を管理
  - `Watch<RunState>`
  - worker へ mode 配信
- `worker_task_X`
  - `select(work_step(), state_changed())`
  - `StopRequested` / `Fault` / `Reconfigure` を監視
- `command_task`
  - 外部入力を `Channel<Command>` で supervisor へ渡す

### 向いている用途

- モード遷移が複雑
- フェールセーフが必要
- 複数周辺機器を協調制御

***

## パターンB: command queue + dedicated device task

- device ごとに常駐 task
- 上位は `Channel` で命令送信
- device task 内で状態機械化
- 停止は `Stop` command か `Watch<RunState>`

### 向いている用途

- UART, SPI, I2C, CAN, USB など device service
- 逐次実行が必要なハードウェア

***

## パターンC: per-operation cancel signal

- 長い単発処理 1 個にだけ cancel を入れたい
- `Signal<Cancel>` + `select(work, cancel.wait())`

### 向いている用途

- firmware update
- 長い calibration
- 長い homing
- 長い measurement sequence

***

# 8. 参考サイト・コンテンツ（かなり有用だったもの）

以下は今回見つけた中で、特に有用度が高かったものです。

## 公式・準公式（最優先）

1. [Embassy Book](https://embassy.dev/book/index.html)
   Embassy 全体像の入口です。async の考え方、HAL、runtime 全体を俯瞰するのに最適です。 [\[embassy.dev\]](https://embassy.dev/book/index.html), [\[embassy-rs.github.io\]](https://embassy-rs.github.io/embassy-book/embassy/dev/index.html)

2. [Embassy runtime :: Embassy Docs](https://embassy-rs.github.io/embassy-book/embassy/dev/runtime.html)
   task の poll/yield の流れ、IRQ と task 再開の関係、`InterruptExecutor` の位置づけ理解に非常に重要です。 [\[embassy-rs.github.io\]](https://embassy-rs.github.io/embassy-book/embassy/dev/runtime.html)

3. [InterruptExecutor in embassy\_executor - Rust](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)
   interrupt executor を正しく理解するうえで最重要です。
   特に **unused IRQ / software interrupt を使う**点は要確認です。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

4. [embassy\_executor - Rust - Docs.rs](https://docs.rs/embassy-executor/latest/embassy_executor/)
   executor の特徴（heap 不要、wake された task だけ poll、公平性、複数 executor）を簡潔に確認できます。 [\[docs.rs\]](https://docs.rs/embassy-executor/latest/embassy_executor/), [\[github.com\]](https://github.com/embassy-rs/embassy/blob/main/embassy-executor/README.md)

5. [embassy\_futures::select - Rust](https://docs.embassy.dev/embassy-futures/git/default/select/fn.select.html)
   **「片方が終わるともう片方は drop」** が明確に書かれており、キャンセル設計の土台です。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-futures/git/default/select/fn.select.html), [\[github.com\]](https://github.com/embassy-rs/embassy/blob/main/embassy-futures/src/select.rs)

6. [Signal in embassy\_sync::signal - Rust](https://docs.embassy.dev/embassy-sync/git/default/signal/struct.Signal.html)
   `wait()` が **cancel-safe** と明記されており、cancellation token 的用途に非常に重要です。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-sync/git/default/signal/struct.Signal.html)

7. [Watch in embassy\_sync::watch - Rust](https://docs.rs/embassy-sync/latest/embassy_sync/watch/struct.Watch.html)
   複数 task への状態伝播に向いています。 [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/watch/struct.Watch.html)

8. [embassy\_sync::channel - Rust](https://docs.rs/embassy-sync/latest/embassy_sync/channel/index.html)
   IRQ→task や task 間 command queue の基本です。 [\[docs.rs\]](https://docs.rs/embassy-sync/latest/embassy_sync/channel/index.html)

9. [embassy\_time - Rust](https://docs.embassy.dev/embassy-time/0.1.5)
   `with_timeout` を使う設計の入口です。 [\[docs.embassy.dev\]](https://docs.embassy.dev/embassy-time/0.1.5), [\[docs.rs\]](https://docs.rs/embassy-time/latest/embassy_time/)

## 補助資料

10. [Executor and Task Scheduling | embassy-rs/embassy | DeepWiki](https://deepwiki.com/embassy-rs/embassy/2.1-executor-and-task-scheduling)
    公式ではありませんが、executor 内部構造の整理に便利です。 [\[deepwiki.com\]](https://deepwiki.com/embassy-rs/embassy/2.1-executor-and-task-scheduling), [\[deepwiki.com\]](https://deepwiki.com/embassy-rs/embassy/2-async-execution-model)

11. [Rust の組込み用非同期フレームワーク Embassy (3)](https://tomo-wait-for-it-yuki.hatenablog.com/entry/2023/08/06/062234)
    日本語で thread-mode executor を読むのに役立ちます。 [\[tomo-wait-...nablog.com\]](https://tomo-wait-for-it-yuki.hatenablog.com/entry/2023/08/06/062234)

12. [Sharing Data Among Tasks in Rust Embassy: Synchronization Primitives](https://dev.to/theembeddedrustacean/sharing-data-among-tasks-in-rust-embassy-synchronization-primitives-59hk)
    `Signal / Channel / PubSub / Mutex` の使い分けの感覚を掴むのに良いです。 [\[dev.to\]](https://dev.to/theembeddedrustacean/sharing-data-among-tasks-in-rust-embassy-synchronization-primitives-59hk), [\[docs.rs\]](https://docs.rs/crate/embassy-sync/latest)

***

# 9. 社内で関連しそうだった資料

社内検索では、Embassy そのものの直接資料は強く出ませんでしたが、
**タスク分割・通知・同期停止の発想**として、以下は参考になりそうでした。

- [コントローラータスク構成図.pdf](https://shimadzugroup-my.sharepoint.com/personal/okoba_tsutomu_wd_shimadzu_jp/Documents/Microsoft%20Teams%20%e3%83%81%e3%83%a3%e3%83%83%e3%83%88%20%e3%83%95%e3%82%a1%e3%82%a4%e3%83%ab/%e3%82%b3%e3%83%b3%e3%83%88%e3%83%ad%e3%83%bc%e3%83%a9%e3%83%bc%e3%82%bf%e3%82%b9%e3%82%af%e6%a7%8b%e6%88%90%e5%9b%b3.pdf?web=1\&EntityRepresentationId=409ce053-f9a4-4f6c-b47e-cd1ddf47d756)
  通信・分析・監視・エラー回復などの責務分割例。 [\[コントローラータスク構成図 \| PDF\]](https://shimadzugroup-my.sharepoint.com/personal/okoba_tsutomu_wd_shimadzu_jp/Documents/Microsoft%20Teams%20%e3%83%81%e3%83%a3%e3%83%83%e3%83%88%20%e3%83%95%e3%82%a1%e3%82%a4%e3%83%ab/%e3%82%b3%e3%83%b3%e3%83%88%e3%83%ad%e3%83%bc%e3%83%a9%e3%83%bc%e3%82%bf%e3%82%b9%e3%82%af%e6%a7%8b%e6%88%90%e5%9b%b3.pdf?web=1)
- [CDD制御タスク構成図.pdf](https://shimadzugroup-my.sharepoint.com/personal/okoba_tsutomu_wd_shimadzu_jp/Documents/Microsoft%20Teams%20%e3%83%81%e3%83%a3%e3%83%83%e3%83%88%20%e3%83%95%e3%82%a1%e3%82%a4%e3%83%ab/CDD%e5%88%b6%e5%be%a1%e3%82%bf%e3%82%b9%e3%82%af%e6%a7%8b%e6%88%90%e5%9b%b3.pdf?web=1\&EntityRepresentationId=17a1cb1a-57db-47ad-b9ed-67f604ff50f8)
  装置制御を複数タスクへ役割分離している構造。 [\[CDD制御タスク構成図 \| PDF\]](https://shimadzugroup-my.sharepoint.com/personal/okoba_tsutomu_wd_shimadzu_jp/Documents/Microsoft%20Teams%20%e3%83%81%e3%83%a3%e3%83%83%e3%83%88%20%e3%83%95%e3%82%a1%e3%82%a4%e3%83%ab/CDD%e5%88%b6%e5%be%a1%e3%82%bf%e3%82%b9%e3%82%af%e6%a7%8b%e6%88%90%e5%9b%b3.pdf?web=1)
- [EA7Z-0193-0002\_Platform\_Programming\_Reference.docx](https://shimadzugroup.sharepoint.com/sites/SCJ_GP1929/_layouts/15/Doc.aspx?sourcedoc=%7B54CE8C28-C39E-4E6C-AD0E-2292B2BA68C5%7D\&file=EA7Z-0193-0002_Platform_Programming_Reference.docx\&action=default\&mobileredirect=true\&DefaultItemOpen=1\&EntityRepresentationId=d7453311-9a60-4a79-93f9-a0477ab2ede2)
  メッセージ受信、通知開始/停止、同期解除などの記述があり、「停止要求と停止同期を分ける」設計の参考になります。 [\[EA7Z-0193-..._Reference \| Word\]](https://shimadzugroup.sharepoint.com/sites/SCJ_GP1929/_layouts/15/Doc.aspx?sourcedoc=%7B54CE8C28-C39E-4E6C-AD0E-2292B2BA68C5%7D&file=EA7Z-0193-0002_Platform_Programming_Reference.docx&action=default&mobileredirect=true&DefaultItemOpen=1)

***

# 10. 私の実務推奨（組み込み制御向け）

あなたのように **複雑な装置制御 / 状態機械 / 複数 task 協調**を意識しているなら、私は Embassy で次を推します。

## 推奨アーキテクチャ

- **thread-mode executor を基本**
- **supervisor task を 1 個置く**
- **各 device/service は常駐 worker task**
- **全体状態は `Watch<RunState>`**
- **個別停止は `Signal<Cancel>`**
- **逐次命令は `Channel<Command>`**
- **長い仕事は `select(work, cancel)`**
- **I/O は `with_timeout` を併用**
- **interrupt executor は本当に必要な高優先度 task のみ**

## 理由

- RTOS 的な「タスク爆発」を避けやすい
- 停止・エラー・再初期化の設計が明快
- フェールセーフが入れやすい
- デバッグしやすい
- task の生死管理が単純

***

必要なら次に続けて、あなた向けにさらに実務寄りのものをそのまま出せます。

1. **STM32 / RP2040 / nRF 向けの具体コード例**
2. **`CancellationToken` 風の自作ラッパ型 (`Signal`/`Watch` ベース) 実装例**
3. **「supervisor + workers」構成の完全サンプル**
4. **FreeRTOS/NORTi 設計を Embassy へ移植するときの設計変換表**

Embassy では、タスク制御は「スレッドモード executor」と「割り込みモード executor」を組み合わせつつ、キャンセルは Rust の「Future を drop するハードキャンセル」と、Signal / Channel / `select` を使った協調キャンセルの 2 段構えで設計するのが現実的です。 [docs](https://docs.rs/embassy-executor/latest/embassy_executor/struct.InterruptExecutor.html)
C#/Tokio の CancellationToken 相当をそのまま提供する公式 API はありませんが、`embassy_sync::Signal` や `Channel` と `embassy_futures::select` を使うことでほぼ同じモデルを自作できます。 [lib](https://lib.rs/crates/min-cancel-token)

***

## Embassy executor の基本構造

Embassy の executor は「プラットフォーム側が main ループと wakeup メカニズムを定義し、その上で static に配置したタスクをポーリングする」形の `no_std` 向け async executor です。 [docs](https://docs.rs/embassy-executor/latest/embassy_executor/)
タスクは `#[embassy_executor::task]` でマークされた `async fn` で、呼び出すと `SpawnToken` が返り、それを `Spawner::spawn()` などで executor に登録して実行開始します。 [docs.embassy](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.SpawnToken.html)

- スレッドモード executor
  - 通常の `Executor` はスレッドモード（Cortex-M の thread mode）でタスクを実行し、main ループからポーリングされます。 [docs](https://docs.rs/embassy-executor/latest/embassy_executor/)
  - `Spawner` は Send ではないため、その executor と同じスレッド内からのみ spawn できますが、`SendSpawner` に変換することで別スレッドからの spawn も可能です。 [docs](https://docs.rs/embassy-executor/latest/embassy_executor/struct.Spawner.html)

- 割り込みモード `InterruptExecutor`
  - `InterruptExecutor` は指定した IRQ ハンドラ内で executor をポーリングする仕組みで、タスクは割り込みコンテキストで実行されます。 [docs.embassy](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)
  - `start(irq)` で executor を初期化し、対応する IRQ を有効化すると、タスクの wake 時にソフトウェアで IRQ を pending にして executor を回す、という動作になります。 [docs](https://docs.rs/embassy-executor/latest/embassy_executor/struct.InterruptExecutor.html)

***

## 典型的なタスク構成パターン

### 1. スレッドベース executor: main → spawn → tasks

スレッドモード側では、`#[embassy_executor::main]` でマークしたエントリポイントが 1 個の executor を起動し、そこから子タスクを spawn する形が典型です。 [docs](https://docs.rs/embassy-executor/latest/embassy_executor/struct.Spawner.html)
I/O 共有やイベント伝搬には `embassy_sync::Channel` や `Mutex` を使うのが一般的で、I2C バス共有やメッセージパッシングの例が実践記事でも紹介されています。 [acalustra](https://acalustra.com/embedded-rust-development-tips-with-embassy.html)

例:

- main タスク: ボード初期化 → 各デバイス用タスクを spawn
- 各デバイスタスク: `Channel` 経由でコマンド/イベントを受信しつつ、ドライバを操作する。 [docs.embassy](https://docs.embassy.dev/embassy-sync/git/default/channel/index.html)

この構成では「タスクのライフサイクルとキャンセル」を main/上位タスクが管理しやすく、C#/Tokio でいう「親タスクが CancellationToken を配る」のに相当する設計が取りやすいです。

### 2. 割り込みベース executor: IRQ → high-prio tasks

`InterruptExecutor` を用いると、特定のタスク群を「割り込み優先度」で動かせます。 [docs](https://docs.rs/embassy-executor/0.7.0/embassy_executor/struct.InterruptExecutor.html)
使い方の要点は以下です。

- 未使用の IRQ 番号（あるいは SWI 相当）を 1 つ選び、そこに `InterruptExecutor` をぶら下げる。 [docs.embassy](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)
- `executor.start(irq)` の戻り値として `SendSpawner` を得て、その spawner で high-priority タスクを spawn する。 [docs](https://docs.rs/embassy-executor/latest/embassy_executor/struct.InterruptExecutor.html)
- IRQ ハンドラ内から `InterruptExecutor::on_interrupt()` を呼び出して、タスク群をポーリングする。 [docs](https://docs.rs/embassy-executor/0.7.0/embassy_executor/struct.InterruptExecutor.html)

構成例:

- Thread executor: ログ出力・低レート通信・UI 等の低優先度タスク
- Interrupt executor: 高レートの ADC サンプリングやタイムクリティカルなプロトコル処理

この場合も、タスク間の制御やキャンセル通知自体は `Channel` や `Signal` 経由で行うのが自然です。 [docs.embassy](https://docs.embassy.dev/embassy-sync/0.7.0/default/signal/struct.Signal.html)

***

## Rust async におけるキャンセルの前提（Embassy でも同じ）

Rust の async では「Future を drop した時点で、その非同期処理は二度とポーリングされず、そこでキャンセルされたものとみなす」というモデルになっています。 [google.github](https://google.github.io/comprehensive-rust/concurrency/async-pitfalls/cancellation.html)
`select` 的なプリミティブでは「完了しなかった側の Future は drop される」ため、内部状態の持ち方を誤ると「キャンセル・セーフでない」コードになり得ます。 [acs.pages.rwth-aachen](https://acs.pages.rwth-aachen.de/public/teaching/legos/legos-rs/embassy_futures/select/fn.select.html)

- `select` の winner 以外の Future は drop される → その Future が内部バッファやプロトコル状態を所有していると、途中まで処理済みのデータが失われる可能性がある。 [users.rust-lang](https://users.rust-lang.org/t/cancel-safety-in-async-and-tokio-select/92381)
- キャンセルセーフにするには、「状態は外側の構造体やチャネル側に置き、Future 自体は状態を借用するだけ」にするのが推奨されています。 [google.github](https://google.github.io/comprehensive-rust/concurrency/async-pitfalls/cancellation.html)

Embassy も Tokio もこのモデルの上に乗っているので、「Future の drop = ハードキャンセル」であり、これを前提に cooperative cancellation を積み上げる、という設計になります。 [docs](https://docs.rs/embassy-executor/latest/embassy_executor/)

***

## CancellationToken の概念と Rust 実装の例

Tokio では `tokio_util::sync::CancellationToken` が広く使われており、`cancel()` でキャンセルを発行し、タスク側は `cancelled().await` でキャンセル通知を待つ協調キャンセルモデルが実装されています。 [docs](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html)
また汎用インタフェースとして `min-cancel-token` クレートは「CancellationToken を埋め込み用途（各タスクに static に割り当てるなど）にも使えるようにする」ことを想定した trait ベースの設計を提案しています。 [lib](https://lib.rs/crates/min-cancel-token)

- 特徴
  - 親 → 子への一方向なキャンセル伝播（親が子をキャンセルする）。 [docs](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html)
  - 子トークン（child token）を作って階層的にキャンセルを伝搬できる。 [lib](https://lib.rs/crates/min-cancel-token)

Embassy 固有の CancellationToken 型はありませんが、同じパターンを `Signal` や `Channel`、`select` を組み合わせて表現できます。 [docs](https://docs.rs/embassy-futures/latest/embassy_futures/select/index.html)

***

## Embassy で CancellationToken 相当を構成する実用パターン

### 1. `Signal` を使った one-shot キャンセル

`embassy_sync::signal::Signal` は「単一コンシューマ・単一スロットのシグナリング primitive」で、`signal(value)` で値をセットし、`wait()` で値を取り出す Future を返します。 [docs.embassy](https://docs.embassy.dev/embassy-sync/git/default/signal/struct.Signal.html)
「上位から下位タスクへの一方向キャンセル通知」には、`Signal<()>` を使うと概念的に CancellationToken に非常に近い動作になります。

典型パターン:

- 親タスク側
  - `static CANCEL: Signal<..., ()> = Signal::new();` を用意
  - 子タスクに `&CANCEL` を渡して spawn
  - キャンセルしたいタイミングで `CANCEL.signal(())`

- 子タスク側
  - メインループで `embassy_futures::select` により「通常処理」と「キャンセル通知」を競合させる。 [acs.pages.rwth-aachen](https://acs.pages.rwth-aachen.de/public/teaching/legos/legos-rs/embassy_futures/select/fn.select.html)

概念コード（簡略化）:

```rust
static CANCEL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[embassy_executor::task]
async fn worker(cancel: &'static Signal<CriticalSectionRawMutex, ()>) {
    loop {
        use embassy_futures::select::{select, Either};

        let work_fut = do_some_work();         // 通常処理 Future
        let cancel_fut = cancel.wait();        // キャンセル通知 Future

        match select(work_fut, cancel_fut).await {
            Either::First(_) => {
                // 通常処理 1 サイクル完了
            }
            Either::Second(_) => {
                // キャンセル要求を受信 → 後始末して return
                cleanup().await;
                return;
            }
        }
    }
}
```

- `select` でキャンセル側が勝ったときにのみ、クリーンアップを行って `return` することで cooperative cancellation を実現できます。 [docs](https://docs.rs/embassy-futures/latest/embassy_futures/select/index.html)
- `Signal` は「フルのときに `signal()` すると上書き」なので、キャンセル通知の多重発行も特に問題にはなりません（一度でも受け取れれば十分）。 [docs.embassy](https://docs.embassy.dev/embassy-sync/0.7.0/default/signal/struct.Signal.html)

### 2. `Channel` を使ったコマンドパターン型キャンセル

マルチプロデューサ・マルチコンシューマで使える `embassy_sync::channel::Channel` を使うと、「コマンド列の中に Stop/Cancel メッセージを流す」形でもキャンセルできます。 [docs.embassy](https://docs.embassy.dev/embassy-sync/git/default/channel/index.html)

- 親タスク → 子タスクへのコマンドを enum で表現し、その中に `Stop`/`Shutdown` variant を用意する。
- 子タスクは `receiver.receive().await` でコマンドを待ち、`Stop` を受け取ったら後始末して return。

この方式だと「キャンセル = 単なるコマンドの一種」になるため、状態遷移管理がしやすい反面、ブロードキャスト向きではありません（1 つの `Stop` メッセージは 1 コンシューマだけが取り出す）。 [docs.embassy](https://docs.embassy.dev/embassy-sync/git/default/channel/index.html)
複数タスクへの一斉停止には `Signal` を共有する方がシンプルです。 [docs.embassy](https://docs.embassy.dev/embassy-sync/git/default/signal/struct.Signal.html)

### 3. `Signal` の階層化で擬似 child token

Tokio の `child_token()` のように階層的にキャンセルを伝播させたい場合、以下のようなパターンが取れます。

- 親タスクがグローバルな `GLOBAL_CANCEL: Signal` を持つ。 [docs.embassy](https://docs.embassy.dev/embassy-sync/0.7.0/default/signal/struct.Signal.html)
- 子タスクグループごとに `group_cancel: Signal` を別途用意し、子タスクには `(GLOBAL_CANCEL, group_cancel)` のどちらか（または両方）を渡す。
- 子タスク側では `select` で「自グループの cancel」と「グローバル cancel」を両方監視し、どちらかが来たら終了。

疑似的ではありますが、「システム全体のキャンセル」と「サブシステム単位のキャンセル」を分けて扱えるので、実運用では十分柔軟なモデルになります。

***

## DMA やペリフェラルを伴うタスクのキャンセル設計

Rust の async におけるハードキャンセルは「任意の `await` ポイントで Future が drop され得る」ため、途中状態のまま停止するとペリフェラルやソケットが中途半端な状態になる可能性があります。 [trio.discourse](https://trio.discourse.group/t/structured-concurrency-in-rust/73/15)

- IO 操作や DMA など「同期キャンセル不能な操作」は「最後まで完了させる」か「専用の abort API で明示的に中断して再初期化する」方が安全です。 [trio.discourse](https://trio.discourse.group/t/structured-concurrency-in-rust/73/15)
- そのため、上で紹介したような cooperative cancellation（`Signal` + `select`）では「1 回のトランザクション（たとえば 1 フレーム受信や 1 ブロック送信）が終わるまで待ってから終了する」ように設計するのが望ましいです。 [acs.pages.rwth-aachen](https://acs.pages.rwth-aachen.de/public/teaching/legos/legos-rs/embassy_futures/select/fn.select.html)

Embassy ドライバや `embedded-hal` async 実装が cancellation-safe かどうかは API ごとのドキュメントを確認する必要があり、Google の「Cancellation」章でも「コンパイラはキャンセルセーフティを保障してくれないので、設計者が意識する必要がある」と警告されています。 [google.github](https://google.github.io/comprehensive-rust/concurrency/async-pitfalls/cancellation.html)

***

## interrupt executor とキャンセル伝播

`InterruptExecutor` 上で動かしている high-priority タスクに対しても、キャンセルモデルは同じで構いません。 [docs.embassy](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.InterruptExecutor.html)

- high-priority タスクが `Signal` をポーリングしつつ短い臨界セクションで処理を行う。
- キャンセル要求が来たら、処理サイクルの合間にペリフェラルを停止・再初期化し、return する。

注意点:

- 割り込み優先度が高いタスクは「長時間ループし続ける」とシステム全体をスタベーションさせるので、キャンセルチェックを適切な粒度で入れる必要があります。
- 低優先度側（thread executor）から高優先度側にキャンセルを伝える場合も、`Signal` は static で共有できるので、設計はシンプルです。 [docs.embassy](https://docs.embassy.dev/embassy-sync/git/default/signal/struct.Signal.html)

***

## 参考になるコンテンツ（キャンセル設計のヒント）

Embassy + async cancellation を直接組み合わせて詳説している記事は現時点ではあまり多くなく、以下のように「ピースごとの情報」を組み合わせる形になります。

### Embassy 関連

- `embassy_executor` ドキュメント（Executor/Spawner/SpawnToken の概要） [docs.embassy](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.SpawnToken.html)
- `InterruptExecutor` ドキュメント（割り込みモード executor、start/on_interrupt、マルチプライオリティ構成など） [docs](https://docs.rs/embassy-executor/0.7.0/embassy_executor/struct.InterruptExecutor.html)
- `embassy_sync::channel`（タスク間のメッセージパッシング、bounded MPMC channel） [docs.embassy](https://docs.embassy.dev/embassy-sync/git/default/channel/index.html)
- `embassy_sync::signal::Signal`（単一コンシューマの one-shot/overwrite 型シグナル） [docs.embassy](https://docs.embassy.dev/embassy-sync/0.7.0/default/signal/struct.Signal.html)
- Acalustra: “Embedded Rust Development Tips with Embassy” – `select` と Channel によるイベント処理やタスク間通信の実例。 [acalustra](https://acalustra.com/embedded-rust-development-tips-with-embassy.html)

### Rust async のキャンセル理論

- Google “Comprehensive Rust” の Cancellation 章 – Future drop = cancellation、キャンセルセーフな設計の考え方。 [google.github](https://google.github.io/comprehensive-rust/concurrency/async-pitfalls/cancellation.html)
- `embassy_futures::select` / `select3` – 「勝った Future 以外は drop される」ことを明示しているドキュメント。 [docs](https://docs.rs/embassy-futures/latest/embassy_futures/select/index.html)
- Rust Users Forum: `select!` と cancel safety に関する議論 – チャネルなど「状態は外側に置き、Future は借用する」スタイルが推奨されている。 [users.rust-lang](https://users.rust-lang.org/t/cancel-safety-in-async-and-tokio-select/92381)
- Structured concurrency と cancellation に関する議論（Trio Discourse） – cooperative vs hard cancellation の設計思想。 [trio.discourse](https://trio.discourse.group/t/structured-concurrency-in-rust/73/15)

### CancellationToken 実装例（Tokio/汎用）

- `tokio_util::sync::CancellationToken` – Rust 版 CancellationToken の代表的実装、親子トークンや cancel の API など。 [docs](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html)
- `min-cancel-token` クレート – embedded を含む様々な環境向けに「CancellationToken をどうインタフェースとして定義するか」の指針を示す crate。 [lib](https://lib.rs/crates/min-cancel-token)

### 日本語の参考（Embassy 全般 + Cancellation のイメージ）

- Qiita: Rust で組み込み: Embassy-rs 入門 1/3 – Embassy の基本概念やタスク間共有の方法（Channel/Mutex など）。 [qiita](https://qiita.com/5C6F2F/items/7864e7ee75f0ae9f8c6e)
- Qiita: C# の async/await における CancellationToken まとめ – 「トークンを引数でバケツリレーする」「下流まで渡せるところには全部渡す」というキャンセル設計の基本が整理されており、Embassy での設計にもそのまま応用できます。 [qiita](https://qiita.com/toRisouP/items/60673e4a39319e69fbc0)

***

## 実務での設計指針まとめ（Embassy + CancellationToken 的モデル）

- 「タスクを drop して止めるハードキャンセル」は最後の手段にして、基本は `Signal`/`Channel` + `select` を用いた cooperative cancellation を採用する。
- 各 long-running task のメインループには「キャンセル通知待ち Future」と「本来の処理 Future」を `select` で競合させるポイントを必ず 1 箇所以上置く。これにより、上位からのキャンセル指示を安全に受け取れる。 [acs.pages.rwth-aachen](https://acs.pages.rwth-aachen.de/public/teaching/legos/legos-rs/embassy_futures/select/fn.select.html)
- DMA や複雑なプロトコルを扱うタスクでは、キャンセル分岐で「トランザクションを中断し、ペリフェラルを既知の初期状態に戻す」処理を必ず入れる。これは Rust async の cancellability モデル（任意の `await` で drop され得る）を安全に扱うために必須。 [trio.discourse](https://trio.discourse.group/t/structured-concurrency-in-rust/73/15)
- 階層化されたシステムでは、「グローバルキャンセル用 `Signal`」と「サブシステム単位の `Signal`」を分けて設計し、擬似的な parent/child token のように扱う。
- 割り込み executor を使う場合も同じ cancel パターンをそのまま適用できるが、優先度と実行時間をよりシビアに管理する必要がある。 [docs](https://docs.rs/embassy-executor/latest/embassy_executor/struct.InterruptExecutor.html)

---

# **Embassy-RSにおける高度なタスク制御と非同期アーキテクチャの徹底考察**

近年の組み込みシステム開発において、Rust言語の非同期処理（async/await）を活用したパラダイムは、ファームウェアのアーキテクチャ設計に根本的な変革をもたらしている。従来の組み込み開発では、決定論的でシンプルな「スーパー・ループ」アーキテクチャと、完全なプリエンプティブ・マルチタスクを提供する「リアルタイムOS（RTOS）」のいずれかを選択する必要があった。しかし、embassy-rsフレームワークの登場により、エグゼキュータベースの協調的マルチタスクという第三の選択肢が確立された。この手法は、コンパイル時にタスクのステートマシンを静的に確保するため、動的メモリ割り当て（ヒープ）を一切必要とせず、タスクごとのスタックサイズ調整や重いカーネルコンテキストスイッチを排除し、単一スタックでの極めて高効率な実行モデルを実現している 1。
一方で、システムの複雑性が増大するにつれ、ファームウェアには高度なタスク制御機構が要求される。長時間実行される計算タスクのプリエンプティブな割り込み、上位レイヤーからのタスクの強制キャンセル、そして決定論的な状態遷移のオーケストレーションなどを実現するには、エグゼキュータの基盤となるアーキテクチャに対する深い理解が不可欠である。さらに、C\#やRustの一般的な非同期ランタイム（Tokioなど）から移行してきた開発者にとって、タスクキャンセルという概念のパラダイムシフトは最大の障壁となる。エンタープライズソフトウェアで一般的な明示的なCancellationTokenモデルは、組み込みRustにおいてはアンチパターンとみなされ、代わりにDropトレイトと所有権セマンティクスによる暗黙的かつ強制的なメカニズムに置き換わる 2。本報告書では、embassy-rsにおけるスレッドベースおよび割り込みベースのエグゼキュータの構造的二面性を詳細に解析し、上位タスクからのキャンセル手法の実装ノウハウ、およびハードウェア周辺機器に対するキャンセル安全性の致命的な影響について、網羅的かつ専門的な分析を提供する。

## **1\. エグゼキュータの基盤設計とハードウェアの相互作用**

embassy-rsエコシステムの中核を成すのは、マイクロコントローラ環境に特化して極限まで最適化されたembassy-executorである。汎用OS上で動作するTokioなどのランタイムとは異なり、Embassyのエグゼキュータは動的メモリ割り当て（allocクレート）を前提とせず、完全に静的な環境で動作するよう設計されている。タスクはコンパイル時に静的に割り当てられ、リンカによってRAMに収まるかどうかが事前に検証されるため、実行時のメモリ枯渇によるパニックは数学的に発生し得ない 4。この静的な性質が、タスクのスケジューリング、スポーン（生成）、および終了のメカニズムに独自の制約と強力な機能をもたらしている。
エグゼキュータが機能するためには、実行可能なタスクが存在しないときにCPUをスリープさせ、外部イベントが発生した際にCPUをウェイクアップさせるためのプラットフォーム固有のハードウェア機構が必要となる。フレームワークはこの要求を満たすため、スレッドベース・エグゼキュータと割り込みベース・エグゼキュータという2つの主要なアーキテクチャを提供している 4。

### **1.1 スレッドベース・エグゼキュータ（協調的マルチタスク）**

スレッドベース・エグゼキュータは、Embassyアプリケーションの標準的なエントリポイントであり、通常は\#\[embassy\_executor::main\]属性マクロを介して初期化される 4。このモデルにおいて、エグゼキュータはプロセッサのメイン実行コンテキスト（ARM Cortex-Mアーキテクチャにおける「スレッドモード」）内で動作する。初期化プロセスでは、main関数にSpawnerが渡され、そこから初期タスクがspawnされる構造をとる。
このエグゼキュータのライフサイクルは、タスクのポーリングとスリープの連続的なループである。エグゼキュータが起動すると、内部のキューで「実行可能（Ready）」としてフラグが立てられているすべてのタスクがポーリングされる。各タスクはRustコンパイラによって生成された非同期ステートマシンであり、エグゼキュータは各ステートマシンのpollメソッドを評価する。タスクがPoll::Pendingを返した場合、それはタスクがタイマーの満了、DMA転送の完了、ネットワークパケットの到着などの外部イベントを待機していることを意味する 5。
すべてのアクティブなタスクがPoll::Pendingを返すと、エグゼキュータは即座に実行すべき処理がないと判断する。ここで電力を浪費するビジー・ループ処理を行うのではなく、エグゼキュータはハードウェア固有のスリープ命令を利用する。ARM Cortex-Mアーキテクチャの場合、これはWFE（Wait For Event）命令によって実現される 4。この命令により、CPUは命令実行を完全に停止し、低電力状態へと移行する。
その後、非同期処理を行っていた周辺機器（ペリフェラル）が操作を完了すると、ハードウェア割り込みが発生する。この際、ペリフェラルに紐づく極めて軽量な割り込みサービスルーチン（ISR）が実行され、そこに登録されていたWakerが呼び出される。Wakerはエグゼキュータの内部キューにアクセスし、スリープしていたタスクを実行可能状態としてマーキングした上で、SEV（Send Event）命令を実行する。このSEV命令によってCPUはWFEスリープ状態から復帰し、エグゼキュータのメインループが再開され、新たに目覚めたタスクが再びポーリングされるというサイクルが形成される 4。
このスレッドベース・エグゼキュータは、I/Oバウンドなアプリケーションに対して極めて高い効率性を誇る。しかし、その実行モデルは完全に協調的（Cooperative）であるという決定的な制限を持つ。もし単一のタスクが暗号化ハッシュの計算や複雑な浮動小数点演算などの長時間の同期処理を実行した場合、そのタスクはawaitによって明示的に制御を譲らない限り、エグゼキュータ全体をブロックしてしまう 8。厳密なリアルタイムのデッドラインが要求されるシステムにおいて、この協調的な制限はシステムの応答性を著しく低下させるため、より高度なプリエンプティブ・アーキテクチャが必要となる。

### **1.2 割り込みベース・エグゼキュータ（真のプリエンプティブ非同期）**

純粋な協調的マルチタスクの限界を克服するため、embassy-rsはInterruptExecutorという独自の概念を提供する 10。これは従来のRTOS設計から大きく逸脱した革命的なアプローチであり、非同期エグゼキュータ全体を特定の割り込みハンドラ（ISR）の*内部*で動作させることを可能にする。現代のマイクロコントローラに搭載されているネスト型ベクタ割り込みコントローラ（NVIC）のハードウェア機能を直接活用することで、開発者は非同期Rustのパラダイムに留まったまま、真のプリエンプティブ・マルチタスク（タスクの強制割り込み）を実現できる。
InterruptExecutorは、スレッドベース・エグゼキュータのようなWFE/SEVによるスリープ・ループを利用しない。その代わり、その実行は特定のハードウェア割り込みベクタに完全にバインドされる 9。これを展開するには、ファームウェア・アーキテクトはハードウェアによって使用されていない割り込みラインを選択する必要がある。多くのチップにはこの目的のための専用のソフトウェア割り込み（SWI）が用意されているが、未使用であればUARTやSPIなどの周辺機器の割り込みベクタをハイジャックして利用することも可能である 10。
初期化および実行シーケンスは極めて厳密な手順を要求する。まず、InterruptExecutorはメモリ上に静的に割り当てられなければならない 9。次に、選択した割り込みに対してハードウェアの優先度レベル（NVIC Priority）が設定される。この優先度設定こそが、システムにおけるプリエンプション（割り込み実行）の階層を定義する中核的なメカニズムとなる 9。続いて、対象の割り込み番号を引数としてエグゼキュータのstartメソッドが呼び出され、これによってNVICレベルで割り込みが有効化されると同時に、メインスレッドから割り込みコンテキストへとタスクをスポーンするためのSendSpawnerが返却される 10。最後に、開発者は選択したハードウェアベクタに対する割り込みサービスルーチン（ISR）を手動で実装し、その内部でEXECUTOR.on\_interrupt()を呼び出す 5。
InterruptExecutor内に存在するタスクがWakerによって起床される際、エグゼキュータはタスクを即座にポーリングするわけではない。Wakerは、プロセッサレベルでバインドされたハードウェア割り込みの「保留（Pending）」フラグを強制的にセットする 5。ハードウェア割り込みコントローラ（NVIC）はこの保留フラグを検知すると、CPUが現在実行しているコード（メインスレッドベース・エグゼキュータや低優先度の割り込み）の優先度を比較する。もしトリガーされた割り込みの方が優先度が高ければ、CPUは即座に現在の操作を一時停止し、プロセッサのレジスタ状態をスタックに退避（プッシュ）して、ISRへとベクタリング（ジャンプ）する。ISR内でon\_interrupt()が呼び出されると、エグゼキュータはその特定のInterruptExecutor内で実行可能なすべてのタスクを、すべてがPoll::Pendingを返すまでポーリングし続ける。処理が完了するとISRは終了し、CPU状態がスタックから復元（ポップ）され、プリエンプトされていた低優先度のコードが中断された正確な位置から実行を再開する 5。
このメカニズムの恩恵により、開発者は極めて複雑な多層システムを容易に構築できる。例えば、低優先度のネットワークロギング処理をスレッドモードのエグゼキュータで維持し、シリアルコマンドの解析処理を中優先度のInterruptExecutorで実行し、リアルタイム性が要求されるモータ制御のフィードバックループを高優先度のInterruptExecutorで処理するといったトポロジーが可能となる 9。この設計下では、モータ制御のイベントが発生した場合、ネットワークロギングのタスクがどれだけ重い同期処理を実行中であっても、ハードウェアレベルで即座にプリエンプトされ、モータ制御の非同期処理が遅延なく実行された後に元の状態に復帰する。

| 特性 / エグゼキュータモデル | スレッドベース・エグゼキュータ (Executor) | 割り込みベース・エグゼキュータ (InterruptExecutor) |
| :---- | :---- | :---- |
| **実行コンテキスト** | メインスレッド（スレッドモード） | 割り込みサービスルーチン（ハンドラモード） |
| **スリープ・待機機構** | ハードウェア命令 (WFE / WFI) | ISRからリターンして処理を譲る。NVICの保留フラグで起動 |
| **プリエンプション（割り込み）** | 協調的（他のタスクを強制停止できない） | プリエンプティブ（優先度に応じて他のエグゼキュータを中断可能） |
| **マルチコア・サポート** | サポートあり（コアを跨いだウェイクアップが可能） | 現状、一部のアーキテクチャでは制限あり（マルチコアでは未動作の事例あり） 12 |
| **最適なワークロード** | バックグラウンド処理、標準的なI/O操作、HTTP通信 | 厳密なリアルタイム処理、モータ制御、高周波数のDSP演算 |

## **2\. キャンセルトークン・パラダイムの脱構築**

複雑なファームウェアにおいて極めて重要な要件の一つが、上位タスクからの指示に基づき、実行中の子タスクを任意のタイミングでキャンセル（中断）する機能である。C\#などの高級言語や、Rustの標準的な非同期ランタイムであるTokioなどを用いた環境では、この制御は通常、明示的なCancellationTokenオブジェクトを用いて管理される 2。TokioやC\#のパラダイムでは、キャンセルトークンが複製（クローン）されてスポーンされたタスク内に渡される。タスク内部のロジックは、定期的にトークンの状態をポーリングするか（例：token.IsCancellationRequestedの確認）、あるいはブロックを伴うI/O APIにトークンを渡し、キャンセルが発生した際にエラーとして処理を中断する責任を負う 13。
Rustの広範なエコシステム内には、tokio-utilやsmol-cancellation-tokenといったサードパーティ製のキャンセルトークン・クレートが存在するが 14、これらをembassy-rsや組み込み環境で利用することは、明確なアンチパターンとして認識されている。Rustの非同期モデルの根底にある哲学は、明示的なトークンの監視によるキャンセルではなく、所有権セマンティクスとDropトレイトのライフサイクルを活用した暗黙的かつ強制的なキャンセルに基づいている 3。

### **2.1 組み込みRustにおけるCancellationTokenの誤謬**

組み込みRustにおいて明示的なキャンセルトークンが推奨されない最大の理由は、Rustのフューチャー（Future）の構造そのものに由来する。Rustでは、async fnはコンパイラによって巨大なステートマシン（状態遷移機械）へとコンパイルされる。このステートマシンは、エグゼキュータによって自身のpollメソッドがアクティブに呼び出されない限り、一切処理を進行させることができない。エグゼキュータ、あるいはオーケストレーション用のマクロが、そのフューチャーのポーリングを単に停止し、スコープから外して破棄（ドロップ）した場合、そのフューチャーは即座にキャンセルされたものとして扱われる 15。
フューチャーがドロップされると、Rustの強力なメモリ管理保証により、そのステートマシンのローカルスコープ内にキャプチャされていたすべての変数やリソースのDrop実装が再帰的に実行される。このメカニズムは、メモリが確実に解放され、ネットワークソケットが適切に閉じられ、共有ロックが安全に解除されることを言語レベルで保証する 3。
明示的なキャンセルトークンの支持者は、この「フューチャーのドロップ」によるキャンセルモデルをしばしば批判する。彼らはこれをオペレーティングシステムのプロセスに対するkill \-9（強制終了）に例え、タスクが「無防備な状態」で突然終了させられ、リソースを安全にクリーンアップするためのファイナライズ処理を実行する機会が奪われると主張する 3。しかし、組み込みRustの世界において、この批判はRAII（Resource Acquisition Is Initialization）パターンの誤解に基づいている。Rustにおいてフューチャーをドロップすることは、言語レベルで完全に安全な操作である。クリーンアップロジックは、カスタムのキャンセルハンドラやトークン監視ループの中に記述されるべきではなく、個々のリソース自身が持つDrop実装の中にカプセル化されるべきだからである 15。トークンを引き回すことは、ヒープ割り当て（Arcなど）を必要とする場合が多く、ノーアロケーションを前提とするembassy-rsの思想と根本的に衝突する。

### **2.2 select\!マクロによるトップダウン・オーケストレーション**

embassy-rsにおいて、上位レイヤーからの制御によってタスクをキャンセルするためには、トークンを渡すのではなく、並行マルチプレクサ・プリミティブ、特にselectパターンを活用する 17。embassy\_futures::select::select関数（および同等のマクロ）は、2つ以上のフューチャーを引数に取り、それらを同時並行的にポーリングする。selectの最も決定的なメカニズムはその終了挙動にある。すなわち、渡された複数のフューチャーのうち、いずれか一つが最初にPoll::Readyを返して完了した瞬間、selectブロック全体の処理が完了し、完了しなかった残りの保留中のフューチャーは即座に強制的にドロップされる 19。
このメカニズムこそが、Embassyにおけるタスクキャンセルの基盤である。タスクをスポーンしてキャンセルトークンを渡すのではなく、上位のオーケストレーション・タスクが、メインの重いワークロード処理を一つのブランチに置き、キャンセルイベント（シグナル待機など）をもう一つのブランチに置いたselectブロックをawaitする。もしキャンセルイベントが先に解決（発火）した場合、メインのワークロードのフューチャーは言語レベルで瞬時にドロップされ、実行は完全に停止する。この一連の動作により、ワークロードを実行しているタスク自身がキャンセルをチェックするロジックを一切持つことなく、上位からの完全なタスクキャンセルが実現される 20。

## **3\. 実践的タスクキャンセル手法と同期プリミティブの実装ノウハウ**

selectブロックのキャンセル用ブランチをトリガーするために、embassy-rsのエコシステムはembassy-syncクレート内に、動的メモリ割り当てを一切必要としない複数の同期プリミティブを提供している。システムのアーキテクチャに応じて最適なプリミティブを選択することが、キャンセルロジックのトポロジーを決定づける。

### **3.1 Signalによるユニキャスト・キャンセル**

embassy\_sync::signal::Signalは、単一のコンシューマ（受信者）向けに設計されたシングルスロットのシグナリング・プリミティブである 22。これはバッファサイズが1のチャネルに似ているが、極めて重要な違いが存在する。プロデューサ（送信側）が、すでに値が保持されている満杯のSignalに対して新たな値を送信しようとした場合、プロデューサをブロックしたりエラーを返したりするのではなく、古い値を上書きして最新の値を保持する動作を行う 22。
この「上書き」の挙動により、Signalはキャンセル要求や最新の状態更新を伝達するための理想的な構造体となる。上位のマネージャタスクは、キューが満杯になってデッドロックに陥るリスクを考慮することなく、システム状態の変更（例：SystemState::Cancel、SystemState::Restart）をSignalに継続的にプッシュすることができる 21。
実践的な実装パターンは、静的に割り当てられたSignalを利用する：

1. グローバルなシグナルを宣言する：static CANCEL\_SIGNAL: Signal\<CriticalSectionRawMutex, ()\> \= Signal::new(); 21。
2. ワーカタスクはマルチプレクサ内で実行を待機する：let result \= select(heavy\_workload(), CANCEL\_SIGNAL.wait()).await; 23。
3. wait()メソッドは、このSignalがシグナル化（値がセット）されたときにのみ完了するフューチャーを返す 25。このフューチャーは明示的に「キャンセルセーフ（Cancel-safe）」としてドキュメント化されている。これは、もしheavy\_workloadが先に完了してwait()フューチャーが途中でドロップされたとしても、シグナル内部の状態が破損したり、送信されたシグナルトリガーが不注意に失われたりすることがないことを保証している 22。
4. 上位タスクがCANCEL\_SIGNAL.signal(())を呼び出すと、selectブロック内のwait()ブランチが完了し、selectブロックが解決されることで、実行中であったheavy\_workloadのフューチャーは突然ドロップされ、キャンセルされる。

### **3.2 Watchによるブロードキャスト・キャンセル**

複数の独立したタスクを同時にキャンセルする必要があるアーキテクチャにおいて、Signalは単一の受信者しかサポートしないため不十分である。もし複数のタスクが同じSignalに対してwait()を呼び出した場合、最初にポーリングされたタスクのみがそのイベントを消費してキャンセルされ、他のタスクは実行を継続してしまう。
ブロードキャストによるグローバルなタスクキャンセルには、embassy\_sync::watch::Watchプリミティブが採用される 26。Watchは内部に共有値を保持し、その値が更新されるたびに任意の数のサブスクライバー（購読者）に対して通知を送る機構を提供する 26。システム状態を表す列挙型（Enum）をWatchチャネルに埋め込むことで、監視タスク（スーパーバイザー）が状態をState::Shutdownに更新すると、それぞれのローカルワークロードをWatch::changed()フューチャーとともにselectで多重化している全ての子タスクが同時に起床し、新しい状態を観測し、それぞれのワークロードのフューチャーを一斉にドロップして終了する 26。

### **3.3 決定論的キャンセルのためのselect\_biased\!**

キャンセルのオーケストレーションにおいて、ポーリング・マルチプレクサの決定論的（Deterministic）な挙動は極めて重要である。標準的なselect\!の実装は、タスクの飢餓（スターベーション）を防ぐため、内部のフューチャーを擬似ランダムやラウンドロビン方式で公平にポーリングする。しかし、組み込み制御システムにおいて、上位からのキャンセルシグナルは一般的なワークロードよりも本質的に高い優先度を持たなければならない。
もし、キャンセルシグナルの発火とワークロードのフューチャーの起床が同時に発生した場合（例：ハードウェアタイマーの完了と外部からの割り込みトリガーがミリ秒単位で完全に一致した場合）、公平なselectはワークロードのフューチャーを先にポーリングしてしまう可能性がある。これにより、本来であれば即座にキャンセルされるべき操作が最後まで実行されてしまう競合状態（レースコンディション）が生じる。
キャンセル要求が他のいかなる作業よりも先に処理されることを保証するために、開発者はselect\_biased\!マクロを利用する 28。公平なselectとは異なり、select\_biased\!はコード上に定義された引数のレキシカル（語彙的）な順序に厳密に従って、上から順にフューチャーをポーリングする 28。
select\_biased\!マクロの最初のブランチにSignal::wait()フューチャーを配置することで、エグゼキュータはワークロードのステートマシンを進行させる前に、必ずキャンセル条件を評価することが強制される。これにより、同時に複数のウェイクアップが発生した場合でも、厳密で決定論的なキャンセル挙動が担保される 28。

### **3.4 「Replace Spawn」パターンのアーキテクチャ構築**

タスク制御における一般的な要件として、既存のタスクを破棄し、新たなパラメータで初期化されたタスクに置き換える「リプレース・スポーン（Replace Spawn）」が存在する。例えば、ネットワーク送信タスクがアクティブにペイロードを送信しようと試みている最中に、より優先度の高い緊急ペイロードが到着した場合、上位マネージャは進行中の送信タスクを中断させ、新しいデータを伴う新規タスクをスポーンさせなければならない。
embassy-rsのコミュニティリポジトリでは、エグゼキュータのプールサイズが重複タスクを制限している状況下でロジックを簡素化するために、エグゼキュータレベルでのreplace\_spawn機能の追加が要望されたケースがある 29。しかし、現状のSpawnerは既存タスクを強制的に立ち退かせるネイティブなreplace\_spawnプリミティブを提供していないため、このロジックはアプリケーションのアーキテクチャによって構成する必要がある。
embassy-rsの既存プリミティブを用いて「リプレース・スポーン」と同等の機能を実現するためには、タスク自体の内部に永続的な「スーパーバイザ・ループ」を実装し、新しいパラメータを運ぶSignalと組み合わせる手法が定石となる：

1. 新しい引数を運ぶためのSignal\<Mutex, Payload\>をグローバルに定義する。
2. スポーンされるタスク内部は、一番外側に無限ループ（loop {... }）を持つ構成とする。
3. ループ内部で、まずタスクはSignalをwait()して、最初のペイロードを受信する。
4. 受信後、タスクはネットワーク送信フューチャー（実際のワークロード）と、再度Signal::wait()を呼び出すフューチャーを内包したselect\_biased\!ブロックを実行する。
5. ネットワーク送信が先に完了した場合、selectブロックは通常通り解決され、ループが先頭に戻り、次のペイロードがシグナルに届くのを待機する。
6. もしネットワーク送信がアクティブな間に、上位マネージャによって新しいペイロードがSignalにプッシュされた場合、Signal::wait()が先に解決される。select\_biased\!の挙動により、実行中だったネットワーク送信のフューチャーは即座にドロップ（キャンセル）され、新しいペイロードが返却される。その後ループが先頭に戻り、その新しいパラメータを用いて即座に新しい送信フューチャーが再起動する。

このアーキテクチャ・パターンは、タスクの置き換えという責任をエグゼキュータの内部キュー管理からアプリケーションの非同期制御フローへと効果的に移行させ、フレームワーク本体への変更を必要とせずに堅牢なタスク・リプレースを実現する。

| 同期プリミティブ | コンシューマ・トポロジー | オーバーフロー時の挙動 | 最適なユースケース |
| :---- | :---- | :---- | :---- |
| **Channel** | 複数プロデューサ, 複数コンシューマ | ブロックするかエラーを返す | キューを通じた順序付きの、損失が許されないイベント処理 26。 |
| **Signal** | 複数プロデューサ, 単一コンシューマ | 最も古い値を静かに上書きする | 特定の単一タスクに対するユニキャスト・キャンセルや状態更新 22。 |
| **Watch** | 複数プロデューサ, 複数コンシューマ | 最も古い値を静かに上書きする | 全システムに跨るブロードキャスト・キャンセルやグローバル状態の共有 26。 |
| **Mutex** | 共有状態の排他制御 | アクセス取得までブロックする | ペリフェラルや一般的なデータ構造の安全な共有保護 26。 |

## **4\. ハードウェア委譲の危険性：キャンセル安全性とDMA**

ここまではソフトウェアのメモリ管理の観点から、タスクキャンセルにおいてDropトレイトに依存するRustのモデルがいかに数学的に堅牢であるかを論じてきた。しかし、純粋なソフトウェアのステートマシンと、独立して動作するハードウェア・コントローラとの間をブリッジする際、この「フューチャーのドロップ」という暗黙のキャンセル機構は、致命的な脆弱性を引き起こす可能性がある。組み込みシステムにおいて、これはDirect Memory Access（DMA）トランザクションの実行中に最も顕著に現れる 18。「キャンセル安全性（Cancellation Safety）」の概念は、割り込み可能なタスクを設計する上でEmbassyのファームウェアエンジニアが最も注意を払わなければならない最大の障壁である 31。

### **4.1 非同期キャンセルによるメモリ破壊（DMA Corruption）のメカニズム**

この脆弱性の本質を理解するためには、非同期DMA転送の一般的な実行フローを追う必要がある：

1. ファームウェアのタスクが、バイト配列をSPIペリフェラル経由で送信する必要があるとする。CPUの使用率を最適化するため、タスクはこのデータ転送作業をマイクロコントローラのDMAハードウェアに委譲する。
2. タスクは自身のローカル・スタックフレーム上にバッファを割り当て、データを充填し、そのバッファの物理メモリアドレスを示すポインタをDMAコントローラのレジスタに渡す。
3. タスクはDMAコントローラに転送の開始を指示し、その後、転送の完了を知らせるハードウェア割り込みをawaitすることでCPUの制御をエグゼキュータに譲る（Yield） 33。
4. CPUはスリープ状態に入るか、他のタスクの実行を開始する。一方、完全に自律的なDMAハードウェアは、指定されたローカルバッファのRAMアドレスから1バイトずつ読み出し、SPIペリフェラルへと継続的に転送し始める 32。

もし、この一連のシーケンスを実行しているタスクがselect\!ブロック内にカプセル化されていた場合、非同期キャンセルの影響を直接的に受けることになる。DMA転送がアクティブに進行している最中に、別のブランチでキャンセルシグナルがトリガーされると、select\!ブロックはSPI転送を待機しているフューチャーを不当に終了させ、即座にドロップする 20。
ソフトウェアのレベルでは、フューチャーがドロップされると、タスクのスタックフレームがデアロケート（解放）される。ローカルバッファを保持していたメモリアドレスはフリーになったと見なされ、後続の関数呼び出しや別のタスクの変数割り当てのために再利用される状態となる。
しかし、*ハードウェアのDMAコントローラは、ソフトウェア側でタスクがキャンセルされたという事実を一切関知しない*。DMAは自身のレジスタにプログラムされたトランザクションを愚直に実行し続け、すでに解放されたバッファの物理メモリアドレスに対して読み書きを継続する 32。
もしDMA転送が「読み出し」（メモリからペリフェラルへの転送）であった場合、すでに別のタスクによってそのメモリ空間に割り当てられていた任意の、あるいは機密性の高いメモリデータを気付かずに送信し続けることになる。さらに致命的なのが「書き込み」（ペリフェラルからメモリへの受信転送）の場合である。DMAコントローラは、現在他の実行中ソフトウェアが利用しているメモリ空間に対して、ハードウェアレベルでサイレントに上書きを実行してしまう。これにより、決定論的なメモリ破壊、データのサイレントな改ざん、あるいは原因不明のハードフォールトといった壊滅的な事態が引き起こされる 15。

### **4.2 ハードウェアの停止とDropトレイトによる安全性の担保**

Rustのコンパイラは、物理ハードウェアの自律的な実行状態を推論することはできない。そのため、キャンセル安全性を担保する責任は、ハードウェア抽象化レイヤー（HAL）の作成者に重くのしかかる。embassy-rsエコシステムにおいて、HALのメンテナはソフトウェアのライフサイクルイベントとハードウェアの物理状態の間のギャップを埋めるための高度なセーフガードを実装しなければならない。
DMAのキャンセル脆弱性を解決するために、HALは生のDMA転送処理を不透明なTransferオブジェクトとしてカプセル化し抽象化する 33。非同期関数がDMA転送を開始すると、このTransferオブジェクトが内部的に作成され、awaitポイントを越えてタスク内に保持される。
ここでの決定的なセーフガードは、このTransferオブジェクト自体に対するDropトレイトの実装である 34。上位のselect\!ブロックがタスクをキャンセルした場合、ローカル変数のスタックが解放される直前に、このTransferオブジェクトのDropメソッドが実行される。
DMA転送のためのDrop実装は、メソッドからリターンする前に以下の同期的かつ命令的なアクションを実行しなければならない：

1. DMAコントローラの制御レジスタに停止用のビットマスクを書き込み、アクティブな転送を物理的に即座に停止させる 34。
2. ステータスレジスタをポーリングすることで、ハードウェアが停止状態を完全に認識したことを確認する。これにより、ペリフェラルが完全に静止し、これ以上のメモリアクセスが絶対に発生しないことを保証する。
3. 中断された転送に関連する保留中の割り込みフラグをクリアし、InterruptExecutorやスレッドループ内でスプリアス・ウェイクアップ（誤検知による起床）が発生するのを防ぐ。

この極めてアグレッシブなハードウェア停止シーケンスをDropロジック内に直接組み込むことで、Transferオブジェクトは厳密な意味で「キャンセルセーフ」となる 34。embassy-stm32やembassy-rpなどの最新のEmbassy HALを利用する開発者は、ローカルのスタックバッファを安全にDMAフューチャーに渡し、select\!マクロによって任意のタイミングでそれらをキャンセルすることができる。ドロップされたバッファが占有していたメモリ領域が、実行中の転送によって破壊されないことがアーキテクチャレベルで保証されているからである 34。

### **4.3 サードパーティ製ドライバの監査と安全性の防衛**

公式のEmbassy HALは設計段階からキャンセル安全性を強制しているが、ファームウェア・エンジニアがサードパーティ製の非同期ドライバを統合する場合や、手動でレジスタ操作を行う非同期ラッパーを記述する場合は、細心の注意を払う必要がある 35。ドライバのキャンセル安全性を監査する際、アーキテクトは以下のポイントを評価しなければならない：

1. **ドライバがawaitポイントを跨いで一時的な状態を保持していないか？** もしフューチャーが状態をミューテート（変更）している最中にドロップされた場合、その不完全な状態変更がドライバを破壊するようであれば、そのドライバはキャンセルセーフではない 20。
2. **ドライバが外部ハードウェアに所有権を委譲していないか？** DMAコントローラ、暗号化コプロセッサ、コア間のFIFOなどにポインタを渡すドライバはすべて、カスタムのDropブロック内でそのハードウェア操作を安全に回収または強制停止しなければならない 36。
3. **キャンセルによってペリフェラルが永続的に破損した状態に残されないか？** 例えば、I2Cドライバがトランザクションを開始してデバイスアドレスを書き込み、レジスタのペイロードを書き込む直前にキャンセルされたとする。この場合、物理的なI2CバスはクロックラインをLowに保ったまま「ハング」した状態になる可能性がある。堅牢なDrop実装は、後続のタスクがバスを正常に取得できるよう、キャンセル時にペリフェラル・ハードウェアをリセットするか通信を適切に終了させる必要がある 37。

もし、あるドライバのキャンセル安全性が保証できないと判断された場合、システムはそのドライバを非同期キャンセルの影響から保護しなければならない。これは、安全性が未確認のドライバコードをselect\!ブロックの外側で明示的に実行するか、あるいはそのドライバ専用の永続的なタスクとしてカプセル化し、上位タスクとは安全なChannelプリミティブを通じて通信させることで達成される。これにより、上位のロジックフローがどのようにキャンセルされようとも、ドライバの内部ステートマシンは常に最後まで実行されることが担保される 3。

## **5\. 複雑なタスク・オーケストレーションの統合的設計**

エグゼキュータの挙動モデル、同期プリミティブの特性、およびハードウェアのキャンセル安全性という理論的基盤を統合することで、ファームウェア・アーキテクトはembassy-rsを用いて極めて堅牢で深い複雑性を持つ組み込みシステムを構築することができる。以下は、高信頼性ファームウェア・アーキテクチャを設計するための統合的なアプローチである。

### **5.1 ハイブリッド・エグゼキュータのトポロジー展開**

現代のベアメタル・アプリケーションにおいて、システム全体が純粋なスレッドモード、あるいは純粋な割り込みモードのみに最適に収まるケースは稀である。厳密なリアルタイム制約を満たしつつCPU効率を最大化するには、ハイブリッドなアプローチが必要不可欠である 18。
標準的なデプロイメント・アーキテクチャでは、まずスレッドモードに主要なExecutorを確立する。このエグゼキュータは、多くのコンテキストを必要とするが厳密なデッドラインを持たない、低速でI/Oバウンドなタスク群を管理する。SDカードへのロギング、USB CDC-ACMシリアル接続の管理、イーサネットインターフェース上のTCPソケットの維持などがこれに該当する 39。これらのタスクは本質的に協調的であるため、長時間のI/O待機中（Poll::Pending時）には自発的に制御を譲り、スレッドベースのCPUスリープ（WFE）に貢献する。
同時に、ファームウェアは決定論的なハードウェア・イベントに遅延なく応答しなければならない。BLDCモータ・ドライバのPIDループ計算や、厳密なマイクロ秒間隔でのADCサンプリングなどが挙げられる。これらのクリティカルなタスクは、1つまたは複数のInterruptExecutorインスタンスに割り当てられる 10。使用されていないハードウェア割り込みを慎重に選択し、それらに対して段階的なNVIC優先度レベル（例えば、センサーサンプリング用にPriority::P6、モータの転流制御用にさらに高いPriority::P7など）を設定する。このアーキテクチャにより、クリティカルな実行パスはスレッドモードのエグゼキュータだけでなく、より低い優先度の割り込みエグゼキュータをもプリエンプト（強制割り込み）可能となり、完全に非同期なコードベース内でありながら、サブマイクロ秒レベルのレイテンシ応答がハードウェア的に保証される 9。

### **5.2 グローバル状態遷移とグレースフル・シャットダウン**

システムの運用状態間（例：「通常稼働」から「ディープスリープ」、あるいは致命的フォルト発生時の「安全停止」など）の混沌とした遷移を管理するためには、厳密なトップダウン型のキャンセルトポロジーが要求される。
分散したタスク内で個別の条件チェックに依存するのではなく、アーキテクチャはグローバルなシステム状態を表現する中央集権的なembassy\_sync::watch::Watch構造体を採用すべきである 26。スレッドモードであれ割り込みモードであれ、すべてのエグゼキュータ上でスポーンされる長時間稼働タスクは、その主要な無限ループをselect\_biased\!ブロックでラップし、Watch::changed()ブランチを最も高い優先度として評価するよう設計する 27。
フォルト・ハンドラやスーパーバイザ・タスクがグローバルなWatchを終了（Terminal）状態に更新した瞬間、その通知はシステム全体に瞬時にブロードキャストされる。すべてのエグゼキュータを跨いで、各タスクのselect\_biased\!ブロックが一斉に解決され、稼働中のワークロード・フューチャーがドロップされる。システムのすべてのハードウェア対話がキャンセルセーフなHALコンストラクトを利用しているため、このアグレッシブなタスク終了は、即座にすべてのDMA転送をハードウェアレベルで停止させ、PWM出力を安全なデフォルト状態に沈黙させ、通信バスのトランザクションをリセットする。その後、各タスクはグレースフルに（安全かつ秩序立って）終了するか、低電力スリープモードへと移行する。これは、Rustの非同期所有権モデルとDropセマンティクスによって完全にオーケストレーションされた、協調的かつ決定論的なシャットダウンの極致である。
結論として、embassy-rsにおいて高度なタスク制御を習得することは、C言語やRTOSで培われた従来のマルチスレッド的な思考パラダイムから脱却することを意味する。ハードウェアに直結したエグゼキュータの特殊な挙動モデルを理解し、キャンセルトークンの代わりにDropトレイトを強力なキャンセル機構として受け入れ、ハードウェアレベルのキャンセル安全性を厳格に強制することでのみ、ファームウェア・エンジニアは重厚なRTOSカーネルに依存することなく、かつて不可能と思われていた水準の安全性、効率性、そしてリアルタイムのプリエンプションを達成することができるのである。

#### **引用文献**

1. GitHub \- embassy-rs/embassy: Modern embedded framework, using Rust and async., 6月 1, 2026にアクセス、 <https://github.com/embassy-rs/embassy>
2. Arc Troopers: Adventures in Async Rust \- Bit Bashing, 6月 1, 2026にアクセス、 <https://bitbashing.io/async-arc.html>
3. Is cancelling Futures by dropping them a fundamentally terrible idea? : r/rust \- Reddit, 6月 1, 2026にアクセス、 [https://www.reddit.com/r/rust/comments/1hj1eg9/is\_cancelling\_futures\_by\_dropping\_them\_a/](https://www.reddit.com/r/rust/comments/1hj1eg9/is_cancelling_futures_by_dropping_them_a/)
4. embassy\_executor \- Rust \- Docs.rs, 6月 1, 2026にアクセス、 [https://docs.rs/embassy-executor/latest/embassy\_executor/](https://docs.rs/embassy-executor/latest/embassy_executor/)
5. embassy\_rs interrupts for the RP2040 : r/rust \- Reddit, 6月 1, 2026にアクセス、 [https://www.reddit.com/r/rust/comments/1haqrtz/embassy\_rs\_interrupts\_for\_the\_rp2040/](https://www.reddit.com/r/rust/comments/1haqrtz/embassy_rs_interrupts_for_the_rp2040/)
6. Measuring Cpu usage with rust Embassy \- Giacomo Caironi, 6月 1, 2026にアクセス、 <https://www.giacomocaironi.dev/posts/measuring-cpu-usage-with-rust-embassy/>
7. Feature flags of embassy-executor crate // Lib.rs, 6月 1, 2026にアクセス、 <https://lib.rs/crates/embassy-executor/features>
8. Overview of Embedded Rust Operating Systems and Frameworks \- PMC, 6月 1, 2026にアクセス、 <https://pmc.ncbi.nlm.nih.gov/articles/PMC11398098/>
9. embassy/examples/nrf52840/src/bin/multiprio.rs at main \- GitHub, 6月 1, 2026にアクセス、 <https://github.com/embassy-rs/embassy/blob/master/examples/nrf52840/src/bin/multiprio.rs>
10. InterruptExecutor in embassy\_executor \- Rust \- Docs.rs, 6月 1, 2026にアクセス、 [https://docs.rs/embassy-executor/latest/embassy\_executor/struct.InterruptExecutor.html](https://docs.rs/embassy-executor/latest/embassy_executor/struct.InterruptExecutor.html)
11. InterruptExecutor in embassy\_executor \- Rust \- embassy-executor, 6月 1, 2026にアクセス、 <https://docs.embassy.dev/embassy-executor/0.9.1/cortex-m/struct.InterruptExecutor.html>
12. RP 2x chips: Embassy Executor and two cores · Issue \#3854 \- GitHub, 6月 1, 2026にアクセス、 <https://github.com/embassy-rs/embassy/issues/3854>
13. min-cancel-token \- Lib.rs, 6月 1, 2026にアクセス、 <https://lib.rs/crates/min-cancel-token>
14. smol-cancellation-token — async Rust library // Lib.rs, 6月 1, 2026にアクセス、 <https://lib.rs/crates/smol-cancellation-token>
15. Futurelock: A subtle risk in async Rust \- Hacker News, 6月 1, 2026にアクセス、 <https://news.ycombinator.com/item?id=45774086>
16. Async Isn't Real & Cannot Hurt You \- No Boilerplate : r/rust \- Reddit, 6月 1, 2026にアクセス、 [https://www.reddit.com/r/rust/comments/1m1kimp/async\_isnt\_real\_cannot\_hurt\_you\_no\_boilerplate/](https://www.reddit.com/r/rust/comments/1m1kimp/async_isnt_real_cannot_hurt_you_no_boilerplate/)
17. embassy\_futures::select \- Rust \- Docs.rs, 6月 1, 2026にアクセス、 [https://docs.rs/embassy-futures/latest/embassy\_futures/select/index.html](https://docs.rs/embassy-futures/latest/embassy_futures/select/index.html)
18. Embassy Book, 6月 1, 2026にアクセス、 <https://embassy.dev/book/>
19. \[UNSOUND\] \`select\_slice\` is unsound, miri · Issue \#3320 · embassy-rs/embassy \- GitHub, 6月 1, 2026にアクセス、 <https://github.com/embassy-rs/embassy/issues/3320>
20. Cancel safety in async and tokio::select\!{} \- help \- The Rust Programming Language Forum, 6月 1, 2026にアクセス、 <https://users.rust-lang.org/t/cancel-safety-in-async-and-tokio-select/92381>
21. When the ESP32-S3 sends messages on the CAN bus and there is no receiver, the software reset may fail · Issue \#4683 · esp-rs/esp-hal \- GitHub, 6月 1, 2026にアクセス、 <https://github.com/esp-rs/esp-hal/issues/4683>
22. signal.rs \- source \- Docs.rs, 6月 1, 2026にアクセス、 [https://docs.rs/embassy-sync/latest/src/embassy\_sync/signal.rs.html](https://docs.rs/embassy-sync/latest/src/embassy_sync/signal.rs.html)
23. Is embassy\_sync Signal cancel-safe? · Issue \#5599 \- GitHub, 6月 1, 2026にアクセス、 <https://github.com/embassy-rs/embassy/issues/5599>
24. Trouble Documentation \- Embassy, 6月 1, 2026にアクセス、 <https://embassy.dev/trouble/>
25. Signal in embassy\_sync::signal \- Rust \- embassy-executor, 6月 1, 2026にアクセス、 <https://docs.embassy.dev/embassy-sync/git/default/signal/struct.Signal.html>
26. embassy\_sync \- Rust \- embassy-executor, 6月 1, 2026にアクセス、 <https://docs.embassy.dev/embassy-sync>
27. Concurrency — list of Rust libraries/crates // Lib.rs, 6月 1, 2026にアクセス、 <https://lib.rs/concurrency>
28. select\_biased in futures \- Rust \- Docs.rs, 6月 1, 2026にアクセス、 [https://docs.rs/futures/latest/futures/macro.select\_biased.html](https://docs.rs/futures/latest/futures/macro.select_biased.html)
29. Feature Request: Cancel embassy executor task / \`replace\_spawn\` · Issue \#3197 \- GitHub, 6月 1, 2026にアクセス、 <https://github.com/embassy-rs/embassy/issues/3197>
30. RFD 400 Dealing with cancel safety in async Rust \- Oxide RFD, 6月 1, 2026にアクセス、 <https://rfd.shared.oxide.computer/rfd/0400>
31. comprehensive-rust.pdf \- Google, 6月 1, 2026にアクセス、 <https://google.github.io/comprehensive-rust/comprehensive-rust.pdf>
32. Io\_uring, kTLS and Rust for zero syscall HTTPS server | Hacker News, 6月 1, 2026にアクセス、 <https://news.ycombinator.com/item?id=44980865>
33. Timer in embassy\_stm32::timer::low\_level \- Rust \- embassy-executor, 6月 1, 2026にアクセス、 [https://docs.embassy.dev/embassy-stm32/git/stm32g041y8/timer/low\_level/struct.Timer.html](https://docs.embassy.dev/embassy-stm32/git/stm32g041y8/timer/low_level/struct.Timer.html)
34. atsamd\_hal::sercom::spi \- Rust \- Docs.rs, 6月 1, 2026にアクセス、 [https://docs.rs/atsamd-hal/latest/atsamd\_hal/sercom/spi/index.html](https://docs.rs/atsamd-hal/latest/atsamd_hal/sercom/spi/index.html)
35. Cancelling async Rust : r/rust \- Reddit, 6月 1, 2026にアクセス、 [https://www.reddit.com/r/rust/comments/1nx4df1/cancelling\_async\_rust/](https://www.reddit.com/r/rust/comments/1nx4df1/cancelling_async_rust/)
36. Issues · OpenDevicePartnership/embassy-mcxa \- GitHub, 6月 1, 2026にアクセス、 <https://github.com/OpenDevicePartnership/embassy-mcxa/issues>
37. When using I2C \+ shared bus \+ embassy, if there is no slave device at the accessed address, subsequent I2C operations will send the same signal repeatedly. · Issue \#1790 · esp-rs/esp-hal \- GitHub, 6月 1, 2026にアクセス、 <https://github.com/esp-rs/esp-hal/issues/1790>
38. Understanding async cancelation \- help \- The Rust Programming Language Forum, 6月 1, 2026にアクセス、 <https://users.rust-lang.org/t/understanding-async-cancelation/134510>
39. embassy/embassy-executor/Cargo.toml at main · embassy-rs/embassy \- GitHub, 6月 1, 2026にアクセス、 <https://github.com/embassy-rs/embassy/blob/main/embassy-executor/Cargo.toml>
40. Mutable/Imutable error \- embedded \- The Rust Programming Language Forum, 6月 1, 2026にアクセス、 <https://users.rust-lang.org/t/mutable-imutable-error/120634>
