# はじめに

このガイドは、**Embassy-rs** を使って組み込みシステムにおける複雑なタスク制御を設計するための実践資料です。

元となる調査の主題は、**Thread-mode Executor / Interrupt-mode Executor / CancellationToken 相当の設計** でした。
本書はその成果を Web 公開しやすい章立てに再構成し、STM32F767ZI / NUCLEO-F767ZI で動作するサンプルコードを添えたものです。

## 対象読者

- C / NORTi / FreeRTOS などで組み込みタスク設計を経験している方
- Rust + Embassy の async/await モデルを **実機制御** に導入したい開発者
- C# (Tokio 含む) の `CancellationToken` パターンに馴染みがあり、`no_std` 環境での実現方法を探している方

## この資料で扱うこと

| 章 | 内容 |
|---|---|
| [設計全体像](./architecture.md) | 4 層モデル（Executor / Task / 同期 / 制御）による整理 |
| [Thread-mode Executor](./thread_executor.md) | 基本実行モデル、WFE/SEV、フェアネス |
| [Interrupt-mode Executor](./interrupt_executor.md) | 割り込み優先度による分離、multiprio パターン |
| [キャンセル設計](./cancellation.md) | `select` / `Signal` / `Channel` / `Watch` による協調キャンセル |
| [キャンセル安全性とハードウェア](./cancel_safety.md) | DMA / SPI / I2C の drop 安全性 |
| [同期プリミティブ選定](./sync_primitives.md) | 用途別プリミティブの選択指針 |
| [実践パターン](./patterns.md) | supervisor + worker、device owner、per-operation cancel 等 |
| [STM32F767ZI 実装例](./stm32f767zi_sample.md) | NUCLEO-F767ZI で動作するフルサンプル |
| [移行観点](./rtos_mapping.md) | FreeRTOS / NORTi / C# / Tokio との概念対応表 |
| [安全性チェックリスト](./safety_checklist.md) | レビュー時の確認項目 |
| [用語集](./glossary.md) | 本書で使用する用語の定義 |
| [参考資料](./references.md) | 公式ドキュメント・外部資料へのリンク |

## 重要な前提: Embassy には CancellationToken がない

Embassy では（Tokio や C# とは異なり）専用の `CancellationToken` 型は提供されていません。
代わりに、次の同期プリミティブの **組み合わせ** で同等の設計を実現します。

```text
CancellationToken
  ≒ Watch<SystemState> / Signal<Cancel> / Channel<Command>

cancelled().await
  ≒ select(work_future, cancel_future).await

TaskTracker.wait()
  ≒ ACK Channel + Supervisor state machine
```

つまり、**spawn 済み task を外から kill する設計ではなく、長寿命 task が上位 command / state を受け取り、`.await` 境界で協調的に停止する設計** が基本です。

## 本書の読み方

初めて Embassy に触れる方は、[設計全体像](./architecture.md) → [Thread-mode Executor](./thread_executor.md) → [キャンセル設計](./cancellation.md) → [STM32F767ZI 実装例](./stm32f767zi_sample.md) の順で読むのがおすすめです。

FreeRTOS / NORTi の経験者は、先に [移行観点](./rtos_mapping.md) をざっと見てから他章を読むと、概念の対応が掴みやすくなります。
