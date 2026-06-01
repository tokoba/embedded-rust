# キャンセル設計

Embassy でのキャンセルは、基本的に **協調的キャンセル** です。
外から task を強制 kill する API は存在せず、task 自身が停止要求を受け取り、安全な `.await` 境界で抜けます。

## なぜ「強制 kill」がないのか

Embassy の task は static メモリ上の状態機械です。
強制 kill すると、DMA 転送中のバッファ、SPI の CS ピン、プロトコルの途中状態などが不定になります。
安全な組み込みシステムでは、**task 自身がリソースを片付けてから停止する** ことが不可欠です。

## 基本形: `select(work, cancel)`

`embassy_futures::select` は 2 つの future を同時に待ち、先に完了した方を返します。
負けた future は **drop** されます。

```rust
use embassy_futures::select::{select, Either};

match select(work_future(), cancel_future()).await {
    Either::First(result) => {
        // 通常完了
        handle_result(result);
    }
    Either::Second(_) => {
        // キャンセル要求を受けた
        cleanup().await;
    }
}
```

### select3 / select4

3 つ以上の future を同時に待つ場合は `select3`, `select4` を使います。

```rust
use embassy_futures::select::{select3, Either3};

match select3(work_future(), cancel_signal.wait(), timeout_timer()).await {
    Either3::First(result) => { /* 通常完了 */ }
    Either3::Second(_)     => { /* キャンセル */ cleanup().await; }
    Either3::Third(_)      => { /* タイムアウト */ cleanup().await; }
}
```

## 手段 A: Signal による個別キャンセル

`Signal` は **単一 consumer** への最新値通知です。キャンセルの最もシンプルな形です。

```rust
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

static CANCEL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

// worker 側
#[embassy_executor::task]
async fn worker_task() {
    loop {
        // 開始を待つ（ここでは別途 START signal を使う想定）
        match select(long_measurement(), CANCEL.wait()).await {
            Either::First(data) => {
                // 正常完了
                info!("measurement done: {}", data);
            }
            Either::Second(_) => {
                // キャンセルされた
                safe_cleanup().await;
                info!("measurement cancelled");
            }
        }
    }
}

// supervisor / button handler 側
fn request_cancel() {
    CANCEL.signal(());
}
```

**向くケース**: 単一 worker の停止、最新の停止要求だけ見ればよい場合

**向かないケース**: 複数 task への broadcast、順序付き command の送信

### Signal の注意点

- `Signal::wait()` は最後に signal された値を **1 回だけ** 返す
- signal 後に wait が呼ばれる前に再度 signal すると、前の値は **上書き** される
- **古い signal が次の operation に誤適用されないよう**、operation 開始前に `Signal::reset()` を呼ぶこと

```rust
// operation 開始前にリセット
CANCEL.reset();
match select(next_operation(), CANCEL.wait()).await {
    // ...
}
```

## 手段 B: Channel による command loop

`Start`, `Cancel`, `Stop`, `Reconfigure` などが混在する場合は、
`Channel<Command, N>` で **順序付きコマンドキュー** を構成するのが扱いやすくなります。

```rust
use embassy_sync::channel::Channel;

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum Command {
    Start,
    Cancel,
    Stop,
    ResetFault,
}

static CMD_CH: Channel<CriticalSectionRawMutex, Command, 4> = Channel::new();

#[embassy_executor::task]
async fn worker_task() {
    loop {
        match CMD_CH.receive().await {
            Command::Start => {
                STATUS_CH.send(Status::Started).await;

                match select(do_work(), wait_cancel_or_stop()).await {
                    Either::First(()) => {
                        safe_cleanup().await;
                        STATUS_CH.send(Status::Completed).await;
                    }
                    Either::Second(Command::Cancel) => {
                        safe_cleanup().await;
                        STATUS_CH.send(Status::Cancelled).await;
                    }
                    Either::Second(Command::Stop) => {
                        safe_cleanup().await;
                        STATUS_CH.send(Status::Stopped).await;
                        return; // task 終了
                    }
                    _ => {}
                }
            }
            Command::Cancel => {
                // Idle 中の Cancel は古い停止要求として破棄
                warn!("stale Cancel ignored in idle state");
            }
            Command::Stop => {
                STATUS_CH.send(Status::Stopped).await;
                return;
            }
            Command::ResetFault => {
                // fault recovery logic
            }
        }
    }
}

/// Cancel または Stop を待つヘルパー
async fn wait_cancel_or_stop() -> Command {
    loop {
        let cmd = CMD_CH.receive().await;
        if matches!(cmd, Command::Cancel | Command::Stop) {
            return cmd;
        }
        // Start / ResetFault はここでは無視
    }
}
```

## 手段 C: Watch による global state 配信

複数 task に一斉にモード変更や停止を伝える場合は `Watch` が適しています。

```rust
use embassy_sync::watch::Watch;

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum SystemState {
    Boot,
    Running,
    StopRequested,
    Fault,
}

static SYS_STATE: Watch<CriticalSectionRawMutex, SystemState, 4> = Watch::new();

// supervisor 側
async fn request_global_stop() {
    SYS_STATE.sender().send(SystemState::StopRequested);
}

// 各 worker 側
#[embassy_executor::task]
async fn motor_task() {
    let mut watcher = SYS_STATE.receiver().unwrap();
    loop {
        // 現在の state を確認しつつ作業
        match select(motor_step(), watcher.changed()).await {
            Either::First(()) => { /* 1 ステップ完了 */ }
            Either::Second(state) => {
                if *state == SystemState::StopRequested {
                    motor_safe_stop().await;
                    return;
                }
            }
        }
    }
}
```

**向くケース**: 全 task への shutdown 通知、モード切替、fault broadcast

**向かないケース**: 個別 task への command、イベント履歴が必要な場合

## 手段 D: AtomicBool による最軽量フラグ

ISR から即座にフラグを立て、task がポーリングで検知する最小構成です。

```rust
use core::sync::atomic::{AtomicBool, Ordering};

static EMERGENCY_STOP: AtomicBool = AtomicBool::new(false);

// ISR 側（HAL の割り込みハンドラ内）
fn on_emergency_irq() {
    EMERGENCY_STOP.store(true, Ordering::Release);
}

// task 側（ステップごとにチェック）
async fn measurement_loop() {
    for step in 0..1000u16 {
        if EMERGENCY_STOP.load(Ordering::Acquire) {
            emergency_cleanup().await;
            return;
        }
        do_step(step).await;
    }
}
```

**注意**: `AtomicBool` は `.await` 中には反応できません。
ポーリング間隔が応答性の上限になります。即座の反応が必要なら `Signal` か `Channel` を使います。

## ACK を必ず返す

停止要求は「止めろ」という **要求** であり、「止まった」という **事実** ではありません。
機械制御・通信停止・DMA 停止では、worker から supervisor へ ACK を返します。

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum Status {
    Initialized,
    Started,
    Step(u8),
    Completed,
    Cancelled,
    Stopped,
    Fault,
}

static STATUS_CH: Channel<CriticalSectionRawMutex, Status, 4> = Channel::new();

// supervisor 側
async fn stop_worker_and_wait_ack() {
    CMD_CH.send(Command::Stop).await;
    loop {
        match STATUS_CH.receive().await {
            Status::Stopped => {
                info!("worker confirmed stop");
                break;
            }
            other => {
                info!("waiting for stop ACK, got {:?}", other);
            }
        }
    }
}
```

## キャンセルと再開の競合

`Cancel` と `Start` が短い間隔で連続する場合、以下の競合が発生します。

```text
時刻 t0: Supervisor → Cancel
時刻 t1: Supervisor → Start      ← Cancel 処理中に Start が来る
```

### 対策: state machine を明示する

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkerState {
    Idle,
    Running,
    Cancelling,
}

// worker 内
let mut state = WorkerState::Idle;
loop {
    match (state, CMD_CH.receive().await) {
        (WorkerState::Idle, Command::Start) => {
            state = WorkerState::Running;
            // ...
        }
        (WorkerState::Running, Command::Cancel) => {
            state = WorkerState::Cancelling;
            // cleanup → Idle
        }
        (WorkerState::Cancelling, Command::Start) => {
            // Cancel 完了まで Start を遅延
            warn!("Start deferred: cancelling in progress");
        }
        _ => {}
    }
}
```

## ネストした select（入れ子キャンセル）

外側のキャンセルと内側のタイムアウトを組み合わせる場合：

```rust
// 外側: global cancel
// 内側: 個別 operation のタイムアウト
match select(
    async {
        // 内側: operation + timeout
        match select(sensor_read(), Timer::after_secs(5)).await {
            Either::First(data) => Ok(data),
            Either::Second(_) => Err("timeout"),
        }
    },
    CANCEL.wait(),
).await {
    Either::First(result) => { /* operation 結果 */ }
    Either::Second(_) => { /* global cancel */ cleanup().await; }
}
```
