# はじめに

このガイドは、Embassy-rs を使って複雑な組み込みタスク制御を設計するための実践資料です。
元資料の主題である **Thread-mode Executor / Interrupt-mode Executor / CancellationToken 相当の設計**を、Web 公開しやすい章立てに再構成しました。

対象読者は、C / NORTi / FreeRTOS などでタスク設計を経験しており、Rust + Embassy の async/await モデルを実機制御に導入したい開発者です。

## この資料で扱うこと

- Embassy executor の基本モデル
- `main -> spawn -> tasks` の thread-mode 設計
- `InterruptExecutor` による優先度分離
- `Signal` / `Channel` / `Watch` / `select` を使う協調キャンセル
- STM32F767ZI / NUCLEO-F767ZI 向けの実装例
- FreeRTOS / NORTi 的な設計から Embassy へ移すときの変換観点

## 重要な前提

Embassy では、Tokio や C# のような専用 `CancellationToken` 型を中心にするより、次のような組み合わせで設計します。

```text
CancellationToken
  ≒ Watch<SystemState> / Signal<Cancel> / Channel<Command>

cancelled().await
  ≒ select(work_future, cancel_future).await

TaskTracker.wait()
  ≒ ACK Channel + Supervisor state machine
```

つまり、**spawn 済み task を外から kill する設計ではなく、長寿命 task が上位 command / state を受け取り、`.await` 境界で協調的に停止する設計**が基本です。
