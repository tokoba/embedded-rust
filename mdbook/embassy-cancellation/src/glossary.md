# 用語集

## Executor

async task を poll する実行器です。Embassy では thread-mode `Executor` と interrupt-mode `InterruptExecutor` が重要です。

## Spawner

task を executor に登録するための handle です。

## Thread-mode

Cortex-M の通常実行コンテキストです。Embassy の基本 executor はここで動きます。

## Interrupt-mode

割り込みコンテキストです。`InterruptExecutor` は task をこの文脈で poll します。

## Waker

待機中の future を再度 poll 可能にする通知機構です。

## Signal

単一 consumer 向けの最新値通知です。個別 cancel に向きます。

## Channel

bounded queue です。command / event / ACK に向きます。

## Watch

複数 receiver が最新 state を観測するための primitive です。

## 協調的キャンセル

task 側が cancel 要求を検知し、安全な境界で処理を抜ける設計です。

## cancel-safe

future が途中で drop されても、所有する状態・buffer・peripheral が破綻しない性質です。
