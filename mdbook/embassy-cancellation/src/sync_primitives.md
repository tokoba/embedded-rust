# 同期プリミティブ選定

Embassy は `no_std` 環境向けに、複数の同期プリミティブを提供しています。
用途に応じた選択がキャンセル設計の要です。

## 一覧と比較

| プリミティブ | 主用途 | 向くケース | 注意点 |
|---|---|---|---|
| `Signal<M, T>` | 単一 consumer への最新値通知 | 個別 cancel、単発 wake | 複数 task broadcast には不向き |
| `Channel<M, T, N>` | bounded queue | command、event、ACK | overflow 方針が必要 |
| `Watch<M, T, N>` | 最新状態の配信 | global state、shutdown | 全イベント履歴は残らない |
| `PubSubChannel<M, T, CAP, SUBS, PUBS>` | 1 対多 event 配信 | 複数 task への event broadcast | subscriber 数と capacity 設計が必要 |
| `Mutex<M, T>` | 共有資源保護 | I2C/SPI bus 共有、設定値共有 | cancel 通知には不適 |
| `AtomicBool` / `AtomicU8` | 最軽量 flag | ISR からの emergency flag | `.await` 中は反応できない |

## 選定フローチャート

```text
停止要求を 1 task に送りたい？
  → Yes → Signal<M, ()>

命令を順序付きで処理したい？
  → Yes → Channel<M, Command, N>

複数 task に「現在状態」を共有したい？
  → Yes → Watch<M, State, N>

複数 task にイベントを「配信」したい？
  → Yes → PubSubChannel<M, Event, CAP, SUBS, PUBS>

共有 peripheral を排他制御したい？
  → Yes → Mutex<M, T> または owner task パターン

ISR から即座にフラグを立てたい？
  → Yes → AtomicBool + task 側ポーリング
```

## 各プリミティブの詳細

### Signal

```rust
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

static CANCEL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

// 送信側（ISR や他の task から）
CANCEL.signal(());

// 受信側
CANCEL.wait().await;  // signal されるまで待機
```

- `wait()` は最後に signal された値を **1 回だけ** 返す
- signal 後に wait 前に再度 signal すると、前の値は上書きされる
- `reset()` で古い signal をクリアできる
- **1 producer : 1 consumer** のケースに最適

### Channel

```rust
use embassy_sync::channel::Channel;

static CMD_CH: Channel<CriticalSectionRawMutex, Command, 4> = Channel::new();

// 送信側
CMD_CH.send(Command::Start).await;      // 空きが出るまで待つ
CMD_CH.try_send(Command::Start);         // 即時（ISR 向け）

// 受信側
let cmd = CMD_CH.receive().await;
let cmd = CMD_CH.try_receive();           // 即時（ポーリング）
```

- bounded queue（キャパシティ `N` をコンパイル時に指定）
- FIFO 順序保証
- `send` は空きが出るまで await、`try_send` は即時
- command / event / ACK すべてに使える汎用性の高いプリミティブ

### Watch

```rust
use embassy_sync::watch::Watch;

static SYS_STATE: Watch<CriticalSectionRawMutex, SystemState, 4> = Watch::new();

// 送信側（supervisor）
SYS_STATE.sender().send(SystemState::StopRequested);

// 受信側（各 worker）
let mut rcv = SYS_STATE.receiver().unwrap();
let state = rcv.changed().await;  // 値が変わるまで待機
```

- 複数 receiver が **最新値** を読める
- イベント履歴は残らない（latest-value semantics）
- receiver 数は `N` で制限される
- global state / shutdown 通知に最適

### PubSubChannel

```rust
use embassy_sync::pubsub::PubSubChannel;

static EVENTS: PubSubChannel<CriticalSectionRawMutex, Event, 8, 4, 2> = PubSubChannel::new();

// publisher
let pub0 = EVENTS.publisher().unwrap();
pub0.publish(Event::SensorReady).await;

// subscriber（複数 task で独立に subscribe）
let mut sub = EVENTS.subscriber().unwrap();
let event = sub.next_message_pure().await;
```

- 1 対多のイベント配信
- 各 subscriber が独立にメッセージを消費
- capacity / subscriber 数 / publisher 数をコンパイル時に指定

### Mutex

```rust
use embassy_sync::mutex::Mutex;

static CONFIG: Mutex<CriticalSectionRawMutex, RefCell<Config>> = Mutex::new(RefCell::new(Config::default()));

// task A
{
    let guard = CONFIG.lock().await;
    guard.borrow_mut().threshold = 42;
}

// task B
{
    let guard = CONFIG.lock().await;
    let val = guard.borrow().threshold;
}
```

- 共有データの排他制御
- **キャンセル通知には使わない**（`lock()` は cancel ではなく排他のため）
- peripheral を複数 task で共有する場合は、Mutex より **owner task パターン** を推奨

## ISR から送る場合

ISR からは **`await` できません**。即時に戻る操作だけを使います。

| 操作 | ISR 互換 | 備考 |
|---|---|---|
| `Channel::try_send()` | ✅ | 満杯時は `Err` を返す |
| `Signal::signal()` | ✅ | 常に成功（上書き） |
| `AtomicBool::store()` | ✅ | 最軽量 |
| `Channel::send().await` | ❌ | await 不可 |
| `Mutex::lock().await` | ❌ | await 不可 |

### overflow（満杯）方針

ISR からの `try_send` が失敗した場合の方針を設計で明示します。

| 方針 | 適用場面 | 実装例 |
|---|---|---|
| **drop** | 高頻度イベントで最新処理だけ必要 | `let _ = ch.try_send(evt);` |
| **overwrite** | 最新値だけ意味がある | `Signal` を使う |
| **error count** | 取りこぼしを診断したい | `OVERFLOW_COUNT.fetch_add(1, Ordering::Relaxed);` |
| **emergency fault** | 取りこぼしが安全上許されない | `EMERGENCY_STOP.store(true, ...);` |

```rust
// ISR 内の overflow 処理例
fn on_sensor_irq() {
    if SENSOR_CH.try_send(SensorEvent::DataReady).is_err() {
        // 方針: error count + 最新値で上書き
        OVERFLOW_COUNT.fetch_add(1, Ordering::Relaxed);
        // Signal なら上書きされるので取りこぼしなし
        SENSOR_SIGNAL.signal(());
    }
}
```

## RawMutex の選択

| RawMutex 型 | 用途 |
|---|---|
| `CriticalSectionRawMutex` | ISR ↔ task 間で共有する場合（最も安全） |
| `NoopRawMutex` | 単一 executor・単一 task 内でのみ使う場合 |
| `ThreadModeRawMutex` | thread-mode task 間のみ（ISR からは使わない） |

組み込みでは **`CriticalSectionRawMutex` が最も汎用的** で、迷ったらこれを選びます。
