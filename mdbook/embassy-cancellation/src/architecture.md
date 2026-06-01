# 設計全体像

Embassy による複雑なタスク制御は、次の **4 つの層** に分解すると理解しやすくなります。

## 4 層モデル

| 層 | 役割 | Embassy での主な要素 |
|---|---|---|
| **Executor 層** | task を poll し、sleep / wake を管理する | `Executor`, `InterruptExecutor` |
| **Task 層** | 通信・制御・監視などの責務単位 | `#[embassy_executor::task]` |
| **同期層** | task 間・ISR → task の接続 | `Signal`, `Channel`, `Watch`, `Mutex` |
| **制御層** | 状態遷移・停止・異常処理 | `enum State`, `enum Command`, supervisor task |

各層は独立した判断軸を持ちます。Executor 層で高優先度化しなくても同期層と制御層だけで十分な場合が多く、
逆に同期層が整っていなければ Executor を増やしても安全なキャンセルは実現できません。

## 推奨構成

```text
main
 ├─ board 初期化 (GPIO, Clock, Peripheral)
 ├─ channel / signal / watch 生成 (static)
 ├─ supervisor_task spawn
 ├─ device/service worker spawn
 └─ watchdog / heartbeat loop

supervisor_task
 ├─ 外部 command を受信 (Channel / UART / Button)
 ├─ SystemState を Watch で全 task に配信
 ├─ worker へ個別 Command を送信
 └─ ACK / Status を待ち、次の遷移を判断

worker_task
 ├─ Command を待つ (Channel::receive)
 ├─ work future と cancel future を select
 ├─ cleanup (PWM off, DMA stop, CS deassert)
 └─ Status を supervisor に返す
```

### NUCLEO-F767ZI での具体例

```rust
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::Watch;

// --- 制御層: enum で状態・命令を定義 ---
#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum Command {
    Start,
    Cancel,
    Stop,
    ResetFault,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum SystemState {
    Boot,
    Running,
    StopRequested,
    Fault,
}

// --- 同期層: static で channel / watch を生成 ---
static CMD_CH: Channel<CriticalSectionRawMutex, Command, 4> = Channel::new();
static STATUS_CH: Channel<CriticalSectionRawMutex, Status, 4> = Channel::new();
static SYS_STATE: Watch<CriticalSectionRawMutex, SystemState, 4> = Watch::new();
```

## 設計原則

### 1. イベントごとに task を spawn しない

Embassy の task は **静的割り当て** です。動的なイベントバーストに対して `pool_size` を増やすのではなく、
**常駐 worker + command loop** に寄せます。

```rust
// ✗ イベントのたびに spawn（pool_size 超過でパニック）
#[embassy_executor::task(pool_size = 10)]
async fn handle_event(data: u32) { /* ... */ }

// ✓ 常駐 worker が command を待つ
#[embassy_executor::task]
async fn worker_task() {
    loop {
        let cmd = CMD_CH.receive().await;
        // cmd に応じた処理
    }
}
```

### 2. キャンセル要求と停止完了を分ける

`Cancel` を送るだけでは安全停止完了を意味しません。worker から supervisor へ **ACK を返す経路** を必ず作ります。

```text
Supervisor → Worker: Command::Cancel
Worker → Supervisor: Status::Cancelled  ← これが「止まった」という事実
```

### 3. 長い処理には await 点を作る

Embassy は **協調型** です。`.await` しない長いループは他 task の進行もキャンセル反応も止めます。

```rust
// ✗ 他 task が動けない
for i in 0..10000 {
    heavy_calculation(i);
}

// ✓ ステップごとに yield
for i in 0..10000 {
    heavy_calculation(i);
    if i % 100 == 0 {
        embassy_futures::yield_now().await;
    }
}
```

### 4. ISR では通知だけにする

ISR では `try_send`, `signal`, atomic flag set などに留め、実処理は task 側へ逃がします。

```rust
// ISR 内（InterruptExecutor ではなく、HAL の割り込みハンドラ内）
// ✗ await は使えない
// ✗ blocking な send は使えない
// ✓ try_send / signal / atomic
if CMD_CH.try_send(Command::Start).is_err() {
    // overflow 処理
}
```

### 5. InterruptExecutor は少数の高優先度 task に限定する

便利ですが複雑です。通常は thread-mode executor を基本にし、**実測でレイテンシが不足したとき** に初めて追加を検討します。
