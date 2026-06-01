# 用語集

## Executor

async task を poll する実行器です。Embassy では thread-mode `Executor` と interrupt-mode `InterruptExecutor` の 2 種類があります。

## Spawner

task を executor に登録するための handle です。`spawner.spawn(task()).unwrap()` のように使います。

## Thread-mode

Cortex-M の通常実行コンテキスト（Handler mode ではないコンテキスト）です。Embassy の基本 executor はここで動きます。最低優先度であり、すべての割り込みにプリエンプトされます。

## Interrupt-mode（Handler mode）

割り込みコンテキストです。`InterruptExecutor` は task をこの文脈で poll します。NVIC 優先度によりプリエンプションが制御されます。

## Waker

待機中の future を再度 poll 可能にする通知機構です。peripheral IRQ ハンドラが waker を呼ぶことで、対応する task が ready 状態になります。

## WFE / SEV

**Wait For Event** / **Send Event**。Cortex-M の命令です。Embassy の thread-mode executor は、全 task が pending のとき WFE で CPU をスリープさせ、waker が SEV で起床させます。

## Signal

`embassy_sync::signal::Signal<M, T>`。単一 consumer 向けの最新値通知プリミティブです。個別 cancel に向きます。最新の値で上書きされるため、全メッセージの受信は保証されません。

## Channel

`embassy_sync::channel::Channel<M, T, N>`。bounded queue（容量 N の FIFO キュー）です。command / event / ACK に向きます。`send` は空きが出るまで await し、`try_send` は即時に結果を返します。

## Watch

`embassy_sync::watch::Watch<M, T, N>`。複数 receiver が最新 state を観測するためのプリミティブです。値が変わるまで `changed().await` で待機します。全イベント履歴は残りません。

## PubSubChannel

`embassy_sync::pubsub::PubSubChannel`。1 対多のイベント配信プリミティブです。各 subscriber が独立にメッセージを消費します。

## 協調的キャンセル（Cooperative Cancellation）

task 側が cancel 要求を検知し、安全な `.await` 境界で処理を抜ける設計パターンです。強制 kill とは異なり、task 自身がリソースの後始末を行います。

## cancel-safe

future が途中で drop されても、所有する state / buffer / peripheral が破綻しない性質です。`select` で負けた future は drop されるため、cancel-safe でないドライバは直接 `select` に入れてはいけません。

## select

`embassy_futures::select::select(a, b)`。2 つの future を同時に待ち、先に完了した方を返す combinator です。負けた future は drop されます。

## Drop

Rust の trait で、値がスコープを抜けるときに呼ばれるデストラクタです。`select` で負けた future の `Drop::drop()` が呼ばれることが、キャンセル安全性の鍵になります。

## supervisor task

複数の worker task を統括し、command の分配・state 管理・fault 処理を担当する task です。

## worker task

実際の処理（計測・制御・通信）を行う task です。supervisor から command を受け取り、status を返します。

## owner task

特定の peripheral を排他的に所有し、Channel 経由でコマンドを受けて操作する task です。cancel-safe 不明のドライバを安全に扱うためのパターンです。

## ACK（Acknowledge）

worker が supervisor に返す完了通知です。停止要求は「要求」であり、ACK が「完了した事実」を伝えます。

## NVIC

**Nested Vectored Interrupt Controller**。Cortex-M の割り込みコントローラです。優先度設定により InterruptExecutor の実行優先度が決まります。

## defmt

Embassy エコシステムで標準的に使われる、組み込み向け高効率ログフレームワークです。RTT（Real-Time Transfer）経由でホスト PC にログを転送します。

## probe-rs

Rust 製のデバッグプローブツールです。書き込み・デバッグ・RTT ログ表示に使用します。
