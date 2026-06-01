# 安全性チェックリスト

Embassy で複雑な装置制御を行う場合のレビュー観点です。

## Executor / task 設計

- [ ] thread-mode executor で足りる処理を interrupt executor に載せていない
- [ ] interrupt executor 上の task は短く、共有資源アクセスが限定されている
- [ ] `.await` なしの長時間ループがない
- [ ] task をイベントごとに大量 spawn していない
- [ ] `pool_size` 増加に頼る前に常駐 worker 化を検討している

## キャンセル設計

- [ ] 停止要求と停止完了 ACK を分けている
- [ ] 長時間待ちの future は cancel future と `select` している
- [ ] 再 start と cancel の競合を state machine で扱っている
- [ ] 古い cancel signal が次の operation に誤適用されない設計になっている
- [ ] cleanup で peripheral を安全状態へ戻している

## ISR / peripheral

- [ ] ISR で `await` していない
- [ ] ISR で blocking send していない
- [ ] `try_send` 失敗時の方針がある
- [ ] DMA / SPI / I2C / UART などの driver が cancel-safe か確認している
- [ ] cancel-safe 不明の driver は owner task に閉じ込めている

## Channel / buffer

- [ ] Channel capacity を burst 条件から見積もっている
- [ ] overflow 時の drop / overwrite / fault 方針が明記されている
- [ ] 大きな payload を Channel で頻繁にコピーしていない
- [ ] static buffer の lifetime と所有権が明確

## 実機確認

- [ ] ボタン連打で command overflow しない
- [ ] cancel 直後に再 start しても状態が壊れない
- [ ] WDT / heartbeat が止まらない
- [ ] release build でタイミングを確認した
- [ ] defmt log だけに依存せず LED / GPIO などでも異常を観測できる
