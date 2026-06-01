# FreeRTOS / NORTi / C# / Tokio からの移行

Embassy は RTOS ではありませんが、RTOS 的な設計経験は非常に有効です。
ただし、対応関係を誤ると task 数が増えすぎたり、キャンセル設計が不明瞭になります。

## FreeRTOS / NORTi → Embassy 対応表

| RTOS / NORTi 的概念 | Embassy での対応 | 設計上の注意 |
|---|---|---|
| task 生成 | `#[embassy_executor::task]` | task は **静的割り当て**。都度生成ではなく常駐化 |
| mailbox | `Channel<M, T, N>` | bounded capacity と overflow 方針を決める |
| event flag | `Signal` / `Watch` / `PubSubChannel` | 1 対 1 か 1 対多かで選ぶ |
| cyclic handler | `Timer::after` loop | handler ではなく async task の周期 loop にする |
| alarm | `Timer::after` / `with_timeout` | cancel と `select` しやすい |
| mutex | `Mutex<M, T>` / owner task | peripheral は owner task 化が安全 |
| rel_wai / forced release | cancel command + ACK | **強制解除ではなく協調停止** |
| memory pool | static buffer / bounded channel | heap ではなく容量を型と static で見積もる |
| task priority | `InterruptExecutor` | thread-mode では全 task が同一優先度 |
| semaphore | `Channel<M, (), N>` | Channel をセマフォとして使える |
| software timer | `Timer::after` + `select` | 専用の timer callback 機構はない |

## C# / Tokio → Embassy 対応表

C# の `CancellationToken` や Tokio の `tokio_util::sync::CancellationToken` に馴染みのある方向けの対応表です。

| C# / Tokio | Embassy | 補足 |
|---|---|---|
| `CancellationToken` | `Signal<M, ()>` / `Watch<M, State, N>` | 個別 cancel → Signal、global → Watch |
| `CancellationTokenSource.Cancel()` | `CANCEL.signal(())` / `SYS_STATE.sender().send(StopRequested)` | |
| `token.IsCancellationRequested` | `CANCEL.signaled()` / `AtomicBool::load()` | ポーリング的な確認 |
| `token.ThrowIfCancellationRequested()` | `.await` 境界で `select` + `match` | 例外ではなく enum 分岐 |
| `Task.WhenAny` | `select(a, b)` / `select3(a, b, c)` | 先に完了した future を返す |
| `Task.WhenAll` | `join(a, b)` | 両方の完了を待つ |
| `TaskTracker.wait()` | ACK `Channel` + supervisor state machine | 全 worker の停止完了を supervisor が追跡 |
| `linked token` | `Watch` + 個別 `Signal` の 2 段構成 | global stop → Watch、個別 → Signal |
| `IHostedService.StopAsync` | `Command::Stop` + `Status::Stopped` ACK | 段階的な graceful shutdown |
| `try { } catch (OperationCancelledException)` | `Either::Second(_)` 分岐 | 例外ではなく分岐 |

### C# との重要な違い

1. **例外ではなく enum 分岐**: C# は `OperationCancelledException` を throw しますが、Embassy では `select` の `Either::Second` で処理します
2. **GC がない**: C# は GC がリソースを解放しますが、Embassy では `Drop` と明示的 cleanup が必要です
3. **静的割り当て**: C# は `Task.Run` で動的生成しますが、Embassy の task は static です
4. **Single-threaded**: Embassy は協調的シングルスレッドです。C# の `Task` のような並列実行はありません

## 移行時の実践ポイント

### 1. タスク起動同期

NORTi で `StaTaskSync` のように起動完了を待っていた場合、Embassy では `Status::Initialized` を ACK channel で返す構成にします。

```rust
#[embassy_executor::task]
async fn device_task() {
    // 初期化処理
    init_device().await;
    STATUS_CH.send(Status::Initialized).await;  // ← 起動完了 ACK

    // メインループ
    loop {
        let cmd = CMD_CH.receive().await;
        // ...
    }
}

// supervisor 側
async fn start_device_and_wait() {
    spawner.spawn(device_task()).unwrap();
    // 起動完了を待つ
    loop {
        if let Status::Initialized = STATUS_CH.receive().await {
            info!("device task initialized");
            break;
        }
    }
}
```

### 2. メッセージ解放

Embassy の `Channel` は値を **move** します。C の `rel_blk` / `rel_msg` のようなメモリプール解放は不要です。
ただし、大きなバッファは static pool / heapless buffer / owner task で扱います。

```rust
// ✗ 大きなデータを Channel で毎回コピー
static DATA_CH: Channel<CriticalSectionRawMutex, [u8; 4096], 2> = Channel::new();

// ✓ index / reference を渡し、データは static buffer に置く
static BUFFERS: [Mutex<CriticalSectionRawMutex, RefCell<[u8; 4096]>>; 2] = /* ... */;
static BUF_IDX_CH: Channel<CriticalSectionRawMutex, usize, 2> = Channel::new();
```

### 3. 周期タイマ

周期ハンドラに処理を置くのではなく、task 内で次のように書きます。

```rust
#[embassy_executor::task]
async fn periodic_task() {
    let mut ticker = embassy_time::Ticker::every(Duration::from_millis(10));
    loop {
        do_periodic_step().await;
        ticker.next().await;  // 一定周期を保つ
    }
}
```

`Timer::after` と違い、`Ticker` は処理時間のドリフトを補正します。

### 4. 停止同期（graceful shutdown）

停止要求と停止完了は **必ず分けます**。

```text
Supervisor → Worker: Command::Stop
Worker:              cleanup (PWM off, DMA stop, ...)
Worker → Supervisor: Status::Stopped    ← これが「止まった」事実
Supervisor:          次の遷移 or shutdown
```
