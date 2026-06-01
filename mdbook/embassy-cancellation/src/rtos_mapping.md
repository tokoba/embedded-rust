# FreeRTOS/NORTi からの移行観点

Embassy は RTOS ではありませんが、RTOS 的な設計経験は非常に有効です。
ただし、対応関係を誤ると task 数が増えすぎたり、キャンセル設計が不明瞭になります。

| RTOS / NORTi 的概念 | Embassy での対応 | 設計上の注意 |
|---|---|---|
| task | `#[embassy_executor::task]` | task は静的割り当て。都度生成ではなく常駐化 |
| mailbox | `Channel` | bounded capacity と overflow 方針を決める |
| event flag | `Signal` / `Watch` / `PubSubChannel` | 1対1か1対多かで選ぶ |
| cyclic handler | `Timer::after` loop | handler ではなく async task の周期 loop にする |
| alarm | `Timer::after` / `with_timeout` | cancel と `select` しやすい |
| mutex | `Mutex` / owner task | peripheral は owner task 化が安全 |
| rel_wai / forced release | cancel command + ACK | 強制解除ではなく協調停止 |
| memory pool | static buffer / bounded channel | heap ではなく容量を型と static で見積もる |

## 移行時の考え方

### 1. タスク起動同期

NORTi で `StaTaskSync` のように起動完了を待っていた場合、Embassy では `Status::Initialized` を ACK channel で返す構成にします。

### 2. メッセージ解放

Embassy の `Channel` は値を move します。メモリプールを明示的に解放する設計とは異なりますが、大きいバッファは static pool / heapless buffer / owner task で扱います。

### 3. 周期タイマ

周期ハンドラに処理を置くのではなく、task 内で次のように書きます。

```rust
loop {
    do_periodic_step().await;
    Timer::after_millis(10).await;
}
```

### 4. 停止同期

停止要求と停止完了は分けます。

```text
Supervisor -> Worker: Command::Stop
Worker -> Supervisor: Status::Stopped
```
