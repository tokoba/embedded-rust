# cargo xtask + host-tuple による embedded/host テスト運用設計

## 目的

この変更は、組み込みターゲット（`thumbv7em-none-eabihf`）と host 環境（Windows/Linux）でのテスト・lint・ビルドを、OS 依存の shell script ではなく `cargo xtask` に集約するためのものです。

最終方針は次の通りです。

> **「どの package を host で回すか」は xtask が判定し、**  
> **「どの host triple で回すか」は Cargo の `host-tuple` に任せる。**

これにより、Windows では Windows host、Linux では Linux host に自然に向けた `cargo nextest` / `cargo clippy` が実行できます。

---

## 変更の全体像

| 項目 | 変更前 | 変更後 |
|---|---|---|
| host target | `x86_64-pc-windows-msvc` 固定 | `host-tuple` |
| host/embedded 判定 | `jq` + shell script | Rust 製 `xtask` + `cargo_metadata` |
| root の default target | `thumbv7em-none-eabihf` 固定 | default target は host に戻す |
| shell script | 実処理を持つ | `cargo xtask ...` への互換ラッパー |
| host test 対象 | `metadata.ci.host-testable` のみに依存 | lib target を基本候補、metadata で override |

---

## 重要な設計判断

### 1. root の `.cargo/config.toml` から `[build] target = ...` を外す

root に embedded target を固定すると、`cargo test` / `cargo clippy` / `cargo nextest` がすべて組み込み向けに寄ってしまい、host テストのたびに target override が必要になります。

そのため root は host default とし、embedded 向けコマンドでは xtask が明示的に `--target thumbv7em-none-eabihf` を渡します。

### 2. `host-tuple` を使う

host 側では固定 triple を使いません。

```bash
cargo nextest run --target host-tuple ...
cargo clippy --target host-tuple ...
```

これにより Windows / Linux の両方で、その実行環境の host triple に向いたビルドになります。

### 3. host testable package の判定ルール

xtask は `cargo metadata` を読み、以下の優先順位で host 対象 package を判定します。

1. `package.metadata.ci.host-testable = false` が明示されていれば除外
2. lib target を持つ package は host 候補
3. bin-only package は原則除外
4. ただし `package.metadata.ci.host-testable = true` が明示されていれば bin-only でも許可

このルールにより、`button_led` のような lib を持つ package は自然に host テスト対象になり、`template` のような ARM 専用 bin-only package は除外されます。

---

## 追加・変更ファイル

```text
Cargo.toml                         # workspace members に xtask を追加
.cargo/config.toml                 # build.target を削除し alias を追加
.cargo/nextest.toml                # host OS 別 override を追加
xtask/Cargo.toml                   # xtask crate を追加
xtask/src/main.rs                  # task runner 本体
clippy.sh                          # 互換 wrapper 化
nextest.sh                         # 互換 wrapper 化
crates/button_led/Cargo.toml       # metadata は true のまま
crates/button_led/src/lib.rs       # host test 対象 module の cfg を整理
crates/button_led/src/button_fsm.rs# no_std/host test しやすい純粋 FSM として維持
crates/template/Cargo.toml         # metadata は false のまま
```

---

## 使い方

### host test

```bash
cargo xtask test-host
```

内部では host 対象 package を抽出し、概ね次のように実行します。

```bash
cargo nextest run --workspace -p button_led --target host-tuple --lib
```

### host clippy

```bash
cargo xtask clippy-host
```

内部では次のように host の lib / tests を lint します。

```bash
cargo clippy -p button_led --target host-tuple --lib --tests -- -D warnings
```

### embedded build

```bash
cargo xtask build-embedded
```

内部では次を実行します。

```bash
cargo build --workspace --target thumbv7em-none-eabihf --bins
```

### embedded clippy

```bash
cargo xtask clippy-embedded
```

内部では次を実行します。

```bash
cargo clippy --workspace --target thumbv7em-none-eabihf --no-default-features --bins -- -D warnings
```

### CI 相当

```bash
cargo xtask ci
```

実行順序は次の通りです。

1. `cargo fmt --all -- --check`
2. `cargo xtask clippy-host`
3. `cargo xtask test-host`
4. `cargo xtask clippy-embedded`
5. `cargo xtask build-embedded`

---

## host 対象 package の調整方法

### lib を持つ crate を通常通り host test 対象にする

特別な設定は不要です。

### 明示的に host test 対象から外す

```toml
[package.metadata.ci]
host-testable = false
```

### bin-only crate を例外的に host test 対象にする

```toml
[package.metadata.ci]
host-testable = true
```

ただし bin-only crate を host test 対象にする場合、`main.rs` 側の `cfg` と依存関係が host で壊れないように分離されている必要があります。

---

## 注意点

- `host-tuple` は「現在実行している OS の host triple」を使うための値です。
- Linux 上で Windows binary を実行する仕組みではありません。
- cross target を host 上で実行したい場合は、別途 target runner（例: emulator / probe / Wine 等）の設計が必要です。
- root の `[build] target = "thumbv7em-none-eabihf"` を戻すと、host test の UX が再び悪化します。

---

## 推奨する日常操作

開発中に最も頻繁に使うコマンドは次の 2 つです。

```bash
cargo xtask test-host
cargo xtask clippy-host
```

firmware 側の確認では次を使います。

```bash
cargo xtask clippy-embedded
cargo xtask build-embedded
```

CI やリリース前確認では次を使います。

```bash
cargo xtask ci
```
