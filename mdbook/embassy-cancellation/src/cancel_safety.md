# キャンセル安全性とハードウェア

`select` で負けた future は **drop** されます。
その future が DMA / peripheral / protocol state を保持していた場合、
drop 時にハードウェアが安全に停止できるかを確認する必要があります。

これを **cancel-safe（キャンセル安全）** と呼びます。

## Drop と cancel-safe の関係

Rust では、値が scope を抜けると `Drop::drop()` が呼ばれます。
`select` で負けた future も同様に drop されるため、drop 時の動作が安全性の鍵になります。

```text
select(dma_transfer(), cancel.wait())
  → cancel.wait() が先に完了
  → dma_transfer() の future が drop される
  → Drop::drop() が呼ばれる
  → DMA は停止する？ バッファは有効？ ピンは安全状態？
```

## embassy-stm32 HAL の cancel-safe 性

embassy-stm32 の多くの非同期 API は、**Drop 時に転送を中止するよう実装されています**。
ただし、すべてのドライバが完全に cancel-safe であるとは保証されていません。

### 一般的に安全なパターン

| ペリフェラル | 操作 | cancel-safe | 備考 |
|---|---|---|---|
| GPIO | Output set/clear | ✅ | 非同期ではないため drop の影響なし |
| Timer | `Timer::after` | ✅ | drop 時に即座にキャンセル |
| EXTI | `wait_for_*_edge` | ✅ | drop 時に待機解除 |
| UART (polling) | `read`, `write` | ⚠️ | 途中 drop でバッファが中途半端になりうる |
| SPI + DMA | `transfer` | ⚠️ | Drop 実装が DMA を停止するが、CS ピンの管理は hand |
| I2C | `read`, `write` | ⚠️ | プロトコル途中での drop は bus を不定状態にしうる |

### 注意が必要なケース

#### DMA 転送

```rust
// ⚠️ DMA 転送中に drop される可能性がある
match select(spi.transfer(&mut buf), cancel.wait()).await {
    Either::First(Ok(())) => { /* 転送完了 */ }
    Either::Second(_) => {
        // spi.transfer の future が drop される
        // → DMA は HAL が停止するが、buf の内容は不定
        // → CS ピンは手動で deassert が必要
        cs_pin.set_high();
    }
}
```

#### I2C プロトコル

I2C は START → ADDRESS → DATA → STOP というプロトコルシーケンスがあります。
途中で drop されると、STOP condition が送信されず、bus が busy のままになることがあります。

```rust
// ⚠️ I2C は途中 drop に弱い
// → owner task パターンを使い、直接 select に入れない
```

## 安全な設計パターン

### パターン 1: owner task に閉じ込める

cancel-safe 不明のドライバは、**専用 owner task** に閉じ込めて、
`Channel` でコマンドを送る構成にします。

```rust
#[embassy_executor::task]
async fn spi_owner_task(
    mut spi: Spi<'static, Async>,
    mut cs: Output<'static>,
) {
    loop {
        match SPI_CMD_CH.receive().await {
            SpiCommand::Transfer(data) => {
                cs.set_low();
                let result = spi.transfer_in_place(&mut data).await;
                cs.set_high(); // ← 必ず deassert
                SPI_STATUS_CH.send(SpiResult::Done(result)).await;
            }
            SpiCommand::Cancel => {
                cs.set_high(); // 安全状態
                SPI_STATUS_CH.send(SpiResult::Cancelled).await;
            }
        }
    }
}

// 呼び出し側は Channel 経由で指示
// → spi の future を直接 select に入れないので cancel-safe
match select(SPI_STATUS_CH.receive(), global_cancel.wait()).await {
    Either::First(result) => { /* SPI 完了 */ }
    Either::Second(_) => {
        SPI_CMD_CH.send(SpiCommand::Cancel).await;
        // owner task が安全に CS deassert してくれる
    }
}
```

### パターン 2: cleanup 関数で後処理

Drop 後に明示的に cleanup を呼ぶ構成です。

```rust
async fn safe_motor_operation() {
    let result = select(motor_sequence(), CANCEL.wait()).await;
    // select 後は必ず cleanup
    motor_pwm_duty_zero().await;
    motor_brake_engage().await;

    match result {
        Either::First(()) => info!("motor sequence complete"),
        Either::Second(_) => info!("motor sequence cancelled"),
    }
}
```

### パターン 3: ステップ分割で cancel ポイントを作る

長い操作を小さなステップに分割し、各ステップ間で cancel をチェックします。
これにより、DMA 転送中に drop される事態自体を避けられます。

```rust
async fn multi_step_measurement(cancel: &Signal<CriticalSectionRawMutex, ()>) -> Result<Data, ()> {
    // ステップ 1: ADC サンプリング（短い、完了を待つ）
    let adc_val = adc.read(&mut ch).await;

    // cancel チェックポイント
    if cancel.signaled() { return Err(()); }

    // ステップ 2: SPI でセンサ読み取り（短い、完了を待つ）
    cs.set_low();
    spi.transfer_in_place(&mut buf).await.ok();
    cs.set_high();

    // cancel チェックポイント
    if cancel.signaled() { return Err(()); }

    // ステップ 3: 結果を処理
    let data = parse_sensor_data(&buf, adc_val);
    Ok(data)
}
```

## ドライバの cancel-safe 性を確認する方法

1. **ソースコードで `Drop` impl を確認する**
   - `impl Drop for Transfer<'_>` 等を探し、DMA 停止処理があるか確認

2. **Embassy の GitHub Issues を検索する**
   - `cancel-safe` や `select drop` で検索

3. **不明なら owner task に閉じ込める**
   - 最も安全な選択肢

4. **テストする**
   - `select(driver_op(), Timer::after_millis(1))` で意図的に途中 drop を発生させ、
     ペリフェラルが正常に再利用できるか確認

## チェックリスト

- [ ] `select` に直接入れるドライバの Drop 実装を確認した
- [ ] DMA 転送が途中 drop されてもバッファが安全である
- [ ] CS / チップセレクトが必ず deassert される
- [ ] I2C の STOP condition が保証される
- [ ] cancel-safe 不明のドライバは owner task に閉じ込めている
- [ ] cleanup 関数が peripheral を安全状態に戻している
