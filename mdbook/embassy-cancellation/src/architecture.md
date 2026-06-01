# 設計全体像

Embassy による複雑なタスク制御は、次の 4 つに分解すると理解しやすくなります。

| 層 | 役割 | Embassy での主な要素 |
|---|---|---|
| Executor 層 | task を poll し、sleep / wake を管理する | `Executor`, `InterruptExecutor` |
| Task 層 | 通信・制御・監視などの責務単位 | `#[embassy_executor::task]` |
| 同期層 | task 間・ISR→task の接続 | `Signal`, `Channel`, `Watch`, `Mutex` |
| 制御層 | 状態遷移・停止・異常処理 | `enum State`, `enum Command`, supervisor task |

## 推奨構成

```text
main
 ├─ board 初期化
 ├─ channel / signal / watch 初期化
 ├─ supervisor_task spawn
 ├─ device/service worker spawn
 └─ watchdog / heartbeat loop

supervisor_task
 ├─ 外部 command を受信
 ├─ SystemState を Watch で配信
 ├─ worker へ個別 Command を送信
 └─ ACK / Status を待つ

worker_task
 ├─ Command を待つ
 ├─ work future と cancel future を select
 ├─ cleanup
 └─ Status を返す
```

## 設計原則

1. **イベントごとに task を spawn しない**  
   Embassy task は静的割り当てです。イベント burst に対して `pool_size` を増やすより、常駐 worker + command loop に寄せます。

2. **キャンセル要求と停止完了を分ける**  
   `Cancel` を送るだけでは安全停止完了を意味しません。`Status::Stopped` や `Status::Cancelled` を返す ACK 経路を作ります。

3. **長い処理には await 点を作る**  
   Embassy は協調型です。`.await` しない長いループは他 task の進行もキャンセル反応も止めます。

4. **ISR では通知だけにする**  
   ISR では `try_send`, `signal`, atomic flag などに留め、実処理は task 側へ逃がします。

5. **InterruptExecutor は少数の高優先度 task に限定する**  
   便利ですが複雑です。通常は thread-mode executor を基本にします。
