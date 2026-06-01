# 安全性チェックリスト

Embassy で複雑な装置制御を行う場合のレビュー観点です。
プロジェクトのコードレビュー時にこのリストを使用してください。

## Executor / Task 設計

- [ ] thread-mode executor で足りる処理を interrupt executor に載せていない
- [ ] interrupt executor 上の task は短く、共有資源アクセスが限定されている
- [ ] `.await` なしの長時間ループがない（目安: 1ms 以上連続する処理は分割）
- [ ] task をイベントごとに大量 spawn していない
- [ ] `pool_size` 増加に頼る前に常駐 worker 化を検討した
- [ ] main 関数は初期化と task spawn のみで、ロジックは task に分離している

## キャンセル設計

- [ ] 停止要求（`Command::Cancel`）と停止完了（`Status::Cancelled`）ACK を分けている
- [ ] 長時間待ちの future は cancel future と `select` している
- [ ] 再 Start と Cancel の競合を state machine で扱っている
- [ ] 古い cancel signal が次の operation に誤適用されない設計になっている（`Signal::reset()` 等）
- [ ] cleanup で peripheral を安全状態へ戻している（PWM off, CS deassert, motor brake）
- [ ] ネストした select の drop 順序を理解している

## キャンセル安全性（cancel-safe）

- [ ] `select` に直接入れるドライバの `Drop` 実装を確認した
- [ ] DMA 転送が途中 drop されてもバッファが安全である
- [ ] SPI の CS ピンが drop 後も必ず deassert される
- [ ] I2C のプロトコルシーケンスが途中 drop で STOP condition を保証する
- [ ] cancel-safe 不明のドライバは owner task に閉じ込めている
- [ ] cleanup 関数が DMA / peripheral を明示的に安全状態に戻している

## ISR / Peripheral

- [ ] ISR 内で `await` していない
- [ ] ISR 内で blocking send していない
- [ ] `try_send` 失敗時の方針がある（drop / overwrite / error count / fault）
- [ ] ISR 内の処理は最小限（flag set / try_send / signal のみ）
- [ ] peripheral の初期化順序が正しい（clock → GPIO → peripheral → interrupt enable）

## Channel / Buffer

- [ ] Channel capacity を burst 条件から見積もっている
- [ ] overflow 時の drop / overwrite / fault 方針が明記されている
- [ ] 大きな payload を Channel で頻繁にコピーしていない
- [ ] static buffer の lifetime と所有権が明確
- [ ] Channel の sender / receiver が正しい task に配置されている

## State Machine

- [ ] bool flag の組み合わせではなく enum state を使っている
- [ ] 全ての state 遷移パスが明示されている
- [ ] 不正な遷移（例: Idle → Cancelling）が型で防がれている
- [ ] fault state からの recovery パスがある
- [ ] state 変更時にログ出力している

## 実機確認

- [ ] ボタン連打で command overflow しない
- [ ] cancel 直後に再 start しても状態が壊れない
- [ ] WDT / heartbeat が止まらない
- [ ] release build でタイミングを確認した（debug と release で挙動が変わる）
- [ ] defmt log だけに依存せず LED / GPIO などでも異常を観測できる
- [ ] 電源投入直後（cold start）のエッジケースを確認した
- [ ] 長時間連続動作（数時間〜数日）での安定性を確認した
