# Interrupt-mode Executor

`InterruptExecutor` は、task を割り込みコンテキストで poll するための executor です。
これにより thread-mode executor より高い優先度で async task を動かせます。

## 誤解しやすい点

`irq -> task called` と考えると少し危険です。
実際には、通常の peripheral IRQ が task 本体を直接呼ぶわけではありません。

```text
task が async I/O を開始
  -> peripheral IRQ 発生
  -> HAL が waker を起こす
  -> executor が ready task を poll
```

一方、`InterruptExecutor` は **executor 自体を専用 IRQ / SWI で駆動する**構造です。
EXTI / USART / DMA などの本物の peripheral IRQ を executor IRQ と兼用する設計は、原則として避けます。

## 使うべき場面

- 低優先度 task を確実に中断したい
- 数十 µs〜サブ ms レベルの応答性が必要
- 高優先度 async pipeline を通常系から分離したい

## 避けるべき場面

- 単なるボタン入力
- UART 受信イベントの通常処理
- 長い計算処理
- ログ出力
- 複雑な共有状態を多数触る処理

## 設計上の注意

- `InterruptExecutor::start()` 前に IRQ 優先度を設定する
- high-priority task は短くする
- 共有データは `CriticalSectionRawMutex` や atomic を慎重に使う
- thread-mode で十分なら thread-mode を選ぶ

## STM32 での扱い

STM32 では未使用 IRQ / software interrupt 相当の選択が board / HAL の対応状況に依存します。
このガイドの実装例では、まず thread-mode executor だけで安全な supervisor + worker 構成を示します。
InterruptExecutor は、実測で thread-mode のレイテンシが不足したときに追加する選択肢とします。
