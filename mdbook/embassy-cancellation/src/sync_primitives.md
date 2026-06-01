# 同期プリミティブ選定

| プリミティブ | 主用途 | 向くケース | 注意点 |
|---|---|---|---|
| `Signal<M, T>` | 単一 consumer への最新値通知 | 個別 cancel、単発 wake | 複数 task broadcast には不向き |
| `Channel<M, T, N>` | bounded queue | command、event、ACK | overflow 方針が必要 |
| `Watch<M, T, N>` | 最新状態の配信 | global state、shutdown | 全イベント履歴は残らない |
| `PubSubChannel` | 1 対多 event 配信 | 複数 task への event broadcast | subscriber 数と capacity 設計が必要 |
| `Mutex<M, T>` | 共有資源保護 | I2C/SPI bus 共有、設定値共有 | cancel 通知には過剰な場合あり |
| `AtomicBool` | 最軽量 flag | ISR からの emergency flag | `.await` 中は反応できない |

## 選定フロー

```text
停止要求を 1 task に送りたい？
  -> Signal

命令を順序付きで処理したい？
  -> Channel<Command>

複数 task に現在状態を共有したい？
  -> Watch<State>

複数 task にイベントを配信したい？
  -> PubSubChannel<Event>

共有 peripheral を守りたい？
  -> Mutex / owner task
```

## ISR から送る場合

ISR からは `await` できません。

- `try_send`
- `signal`
- atomic flag set

など、即時に戻る操作だけにします。
満杯時の方針は設計で明示します。

| 方針 | 例 |
|---|---|
| drop | 高頻度イベントで最新処理だけ必要 |
| overwrite | 最新値だけ意味がある |
| error count | 取りこぼしを診断したい |
| emergency fault | 取りこぼしが安全上許されない |
