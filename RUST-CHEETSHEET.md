# 🦀 Rust チートシート（日本語版）

## 目次

- [基本ツール・初期設定](#基本ツール初期設定)
- [プロジェクト作成・ビルド・実行](#プロジェクト作成ビルド実行)
- [依存関係管理](#依存関係管理)
- [コード品質・静的解析](#コード品質静的解析)
- [テスト・カバレッジ](#テストカバレッジ)
- [デバッグ](#デバッグ)
- [バイナリ調査](#バイナリ調査)
- [クロスコンパイル/移植性](#クロスコンパイル移植性)
- [便利ツールまとめ](#便利ツールまとめ)
- [Hello World例](#hello-world例)
- [型・変数・コレクション](#型変数コレクション)
- [エラーハンドリング](#エラーハンドリング)
- [構造体・列挙型・トレイト](#構造体列挙型トレイト)
- [関数・クロージャ](#関数クロージャ)
- [ループ・イテレータ](#ループイテレータ)
- [ファイル操作/システム操作](#ファイル操作システム操作)
- [メモリ管理](#メモリ管理)

---

## 基本ツール・初期設定

```sh
rustup update         # ツールチェーンの更新
rustup default stable # デフォルトをstableに設定
rustup component add rustfmt clippy # コード整形＆Lint用コンポーネント追加
```

### コード整形

```sh
cargo fmt            # Rustコードを整形
cargo tomlfmt        # Cargo.tomlも整形（cargo-tomlfmtが必要）
```

---

## プロジェクト作成・ビルド・実行

```sh
cargo new myapp           # 新しいバイナリプロジェクト作成
cargo new --lib mylib     # 新しいライブラリプロジェクト作成
cargo build               # デバッグビルド
cargo build --release     # 最適化ビルド（リリース用）
cargo run                 # ビルド＆実行
cargo test                # テスト実行
cargo check               # 型チェックのみ（高速）
```

### ドキュメント生成

```sh
cargo doc --open          # ドキュメント生成＆ブラウザ表示
```

---

## 依存関係管理

```sh
cargo update                 # 依存ライブラリを最新版に更新
cargo add serde              # クレート追加（cargo-editが必要）
cargo rm serde               # クレート削除（cargo-editが必要）
cargo tree                   # 依存関係ツリー表示（cargo-treeが必要）
cargo udeps                  # 未使用クレート検出（cargo-udepsが必要）
cargo outdated               # 古い依存クレート確認（cargo-outdatedが必要）
cargo audit                  # 脆弱性チェック（cargo-auditが必要）
```

---

## コード品質・静的解析

```sh
cargo clippy                 # 静的解析/Lint実行
cargo clippy --fix           # 自動修正（可能な場合）
RUSTFLAGS="-D warnings" cargo build  # 警告をエラー扱いでビルド停止

# ファイル保存時に自動LintやCheckしたい場合（cargo-watch利用）
cargo watch -x check         # 保存時に型チェック自動化
cargo watch -x clippy        # 保存時にLint自動化
```

---

## テスト・カバレッジ

```sh
cargo test -- --nocapture    # 標準出力も表示してテスト実行
```

#### 非同期テスト例（tokio利用）

```rust
// main.rs
#[tokio::test]
async fn 非同期テスト() {
    // 非同期処理のテストを書く
}
```

#### カバレッジ計測(grcov例)

```sh
# カバレッジ取得例。詳細は公式: https://github.com/mozilla/grcov を参照
```

---

## デバッグ

```rust
// main.rs
println!("{:?}", val);    // デバッグ出力用プリント文
dbg!(&val);               // dbg!マクロで即時デバッグ出力

// 型推論結果を知りたい場合（エラー出力で型が分かる）
let _: () = some_func();
```

#### 型名を文字列として取得

```rust
// main.rs
fn type_of<T>(_: T) -> &'static str {
    std::any::type_name::<T>()
}
```

#### マクロ展開の確認 (cargo-expand必要)

```sh
cargo expand           # マクロ展開結果を見る
```

---

## バイナリ調査

```sh
cargo nm -- --demangle ./target/debug/myapp      # シンボル名一覧表示（人間向け名）
cargo objdump -- -disassemble ./target/debug/myapp | less   # アセンブリダンプ表示

ldd ./target/debug/myapp                         # 動的リンク情報確認 (Linux標準)
readelf -a ./target/debug/myapp | less           # ELF詳細情報(全表示)
strings ./target/debug/myapp | less              # バイナリ内文字列抽出

strace -tt -T -f ./target/debug/myapp            # システムコールトレース(Linux)
```

---

## クロスコンパイル移植性

```sh
rustup target add x86_64-unknown-linux-musl      # ターゲット追加(MUSL静的リンク用)

# zig cc + cargo-zigbuild の利用でglibc問題対策も可能:
# https://github.com/rust-cross/cargo-zigbuild

docker run --rm -ti \
  -v ${PWD}:/workspace -w /workspace rust:latest \
  cargo build --release     # Docker上でビルドも可能
```

---

## 便利ツールまとめ

| ツール        | 用途                         | 導入方法                       |
| ------------- | ---------------------------- | ------------------------------ |
| rust-analyzer | LSPベース補完/ナビゲーション | VSCode拡張 or rustup component |
| clippy        | Lint                         | rustup component add clippy    |
| fmt           | 整形                         | rustup component add rustfmt   |
| watch         | 自動再ビルド等               | cargo install cargo-watch      |
| expand        | マクロ展開                   | cargo install cargo-expand     |
| udeps         | 未使用依存検出               | cargo install cargo-udeps      |
| outdated      | 古い依存検出                 | cargo install cargo-outdated   |
| audit         | 脆弱性チェック               | cargo install cargo-audit      |
| binutils      | シンボル/逆アセンブル等      | cargo install cargo-binutils   |

---

## Hello World例

```rust
// main.rs
fn main() {
    println!("Hello World"); // 標準出力へHello Worldと表示
}
```

```sh
$ rustc main.rs     # コンパイル
$ ./main            # 実行
Hello World         # 出力結果
```

---

## 型・変数・コレクション

### 基本型と宣言例

```rust
// main.rs
let x: bool = false;      // 論理値型bool
let y: char = '上';       // 4バイトの文字型
let a: i8 = -2;           // 8ビット符号付き整数
let b: u8 = 200;          // 8ビット符号なし整数
let n: f32 = 0.45;        // 単精度浮動小数点数
let mut arr = [1,2,3,4];  // 配列
let mut v = vec![3,4,5];  // ベクタ(可変長配列)
let s = String::from("上善若水"); // ヒープ確保された文字列
let s2 = "水善利萬物而不爭";       // &strリテラル(スタティック)
```

### スライスと部分参照例

```rust
// main.rs
let s1 = &arr[0..2];   // 配列の部分参照(スライス)
let s2 = &mut arr[1..]; // 可変スライス
```

### ベクタ操作例

```rust
// main.rs
v.push(2);             // 要素追加
v.pop();               // 最後の要素を削除して返す
v.contains(&3);        // 指定値が含まれるか判定
v.remove(1);           // n番目要素削除
v.extend([6,7]);       // イテレータから拡張
v.resize(10,0);        // 長さ10まで0で埋める
v.fill(9);             // 全要素9で埋める(Rust1.50以降)
let len = v.len();     // 要素数取得
```

### タプルと分割代入例

```rust
// main.rs
let (a,b,c) = (4,5,6);    // 複数変数への同時代入(タプル分割)
let t = (3,"abc",true);   // 異種混在可能なタプル
println!("{}", t.1);      // .n でアクセス可
```

### ハッシュマップ例

```rust
// main.rs
use std::collections::HashMap;
let mut m = HashMap::new();
m.insert('a',1);
if let Some(v) = m.get(&'a') { println!("{}", v); }
m.entry('b').or_insert(42); // 存在しない場合のみ挿入するメソッド
```

---

## エラーハンドリング

### panic!による即時異常終了

```rust
// main.rs
panic!("エラー発生！"); // 即座にプログラム終了＆スタックトレース出力
```

### Option型によるnull安全な値取り扱い

```rust
// main.rs
let v = vec![3,4,5];
match v.get(12) {
    Some(val) => println!("{}", val),
    None => println!("値なし")
}
let e = v.get(0).unwrap();     // 値存在時のみunwrap可能
let f = v.get(5).unwrap_or(&0);// Noneの場合はデフォルト値0
```

### Result型によるエラー伝播

```rust
// main.rs
use std::fs::File;
match File::open("test.txt") {
    Ok(file) => println!("ファイルオープン成功"),
    Err(e) => println!("失敗: {}", e),
}
if let Ok(v) = std::env::var("SHLVL") { println!("{}", v);}
```

---

## 構造体・列挙型・トレイト

### 構造体定義とメソッド実装例

```rust
// main.rs
struct Wheel { r: i8, s: i8 }
impl Wheel {
    fn new(r: i8) -> Self { Self { r, s: 4 } }
    fn dump(&self) { println!("半径:{} スポーク数:{}", self.r, self.s);}
}
let mut w = Wheel::new(5);
w.dump();
```

### 列挙型(enum)定義とmatch文利用例

```rust
// main.rs
enum Fruit { Apple, Banana, Pear }
let f = Fruit::Apple;
match f {
    Fruit::Apple => println!("りんご"),
    Fruit::Banana => println!("バナナ"),
    Fruit::Pear => println!("洋梨"),
}
```

### トレイト実装による多態性例

```rust
// main.rs
trait Animal { fn speak(&self);}
struct Dog;
impl Animal for Dog {
    fn speak(&self) { println!("ワン！"); }
}
let d = Dog;
d.speak();
```

---

## 関数・クロージャ

### 基本的な関数宣言と戻り値タプル例

```rust
// main.rs
fn add(a: i32, b: i32) -> i32 { a + b }
fn multi_ret(x:i32)->(i32,i32){ (x,x*2) }
let (a,b) = multi_ret(5);
println!("{},{}",a,b);
```

### クロージャ（無名関数）使用例

```rust
// main.rs
let plus_one = |x| x+1;
println!("{}", plus_one(10));   // 結果:11
vec![1,2,3].iter().for_each(|x| print!("{}", x));   // 各要素表示
```

---

## ループ・イテレータ

### 基本的なfor/while/loop文例とイテレータ利用法

```rust
// main.rs
for i in 0..5 { print!("{},",i);}              // 範囲for文:0～4出力
while let Some(x) = v.pop() { print!("{},",x);}     // while letによる反復
loop { if 条件{break;} }                        // 無限ループ+breakで脱出
vec![1,2,3].iter().map(|x| x*2).for_each(|y| print!("{}",y));   // map+for_each活用
```

---

## ファイル操作/システム操作

### ファイル読み書き基本例（エラー処理付き）

```rust
// main.rs
use std::fs::File;
use std::io::{Read, Write};
let mut s = String::new();
match File::open("test.txt") {
    Ok(mut f) => { f.read_to_string(&mut s).unwrap(); },
    Err(e) => println!("ファイルオープン失敗: {}", e),
}
// 書き込み例:
File::create("output.txt").unwrap().write_all(s.as_bytes()).unwrap();
```

---

### コマンドライン引数取得／環境変数取得例

```rust
// main.rs
for arg in std::env::args() {
    println!("{}", arg);      // コマンドライン引数一覧表示
}
if let Ok(val) = std::env::var("ENV_VAR_NAME") {
    println!("{}", val);      // 環境変数ENV_VAR_NAMEの値を表示
}
```

```rs
if let Ok(val) = std::env::var("ENV_VAR_NAME") {
println!("{}", val); // 環境変数ENV_VAR_NAMEの値を表示
}
```

## メモリ管理

### プログラムにおけるメモリの領域の定義

- `text`: コード領域
- `.data/.bss`: 静的領域
- `heap`: ヒープ領域
- `stack`: スタック領域

#### `text`: コード領域

実行する命令が設置されている。
コンパイル言語であればコンパイルされたバイナリーが配置されている。

#### `.data/.bss`: 静的領域

static 変数, global 変数, static 初期化子などが配置されている。
プログラム全体のライフタイムで変数が生存し保持される。

#### `heap`: ヒープ領域

プログラム実行中に動的に確保・解放される領域。
OSやランタイムが管理する。

#### `stack`: スタック領域

関数呼び出しのフレームが積まれている。
ローカル変数や戻りアドレスなどが配置される。
関数呼び出しの深さ分だけ確保される。

```text
高アドレス
+----------------------+  ← スタックの始点
|        Stack         |  関数呼び出しのたびに増える/減る
+----------------------+
|        Heap          |  動的確保（Box, Vec, String などの中身）
+----------------------+
|   BSS / Data / Text  |  グローバル, static, コード
+----------------------+  ← 低アドレス
```

#### Rust における stack, heap の使用例

- stack: ライフタイムが関数のスコープ・ブロックスコープと一致する。スコープを抜けるとドロップされるため一般的にはライフタイムが短い。
- 大きな配列データなどは，スタックに確保するよりもヒープに確保する方が一般的。
- heap: `Box`, `Vec`, `String`, `HashMap`, `HashSet`, `BTreeMap`, `BTreeSet`, などで変数がヒープ領域に確保される。これらはサイズがコンパイル時に決められないケースも多い。ユーザー入力や外部からの入力によってサイズが変わるケースが多い。
- ヒープ領域に確保するデータはスタックよりもライフタイムが長くなることがある。
- 複数のスレッド間で共有するデータはヒープ領域に確保する方が一般的。(`Arc`, `Mutex`, `RwLock`, など)
- ライフタイムの長いヒープオブジェクトが多いとメモリを圧迫する。

#### std スタンダードライブラリー

- stdは以下の3層構造で構成される
- `std::core`: 基本的な型やトレイとのみで構成される。ヒープは使用しない
- `std::alloc`: ヒープ割り当てが必要な型(`Box`, `Vec`, `String`, `HashMap`, `HashSet`, `BTreeMap`, `BTreeSet`, など)を管理する
- `std`: OS関連の`fs`, `thread`, `process`, `net`, `os`などと連携する機能

#### 組み込み Rust のメモリ管理

- #![no_std] でスタンダードライブラリを不使用にすることで、スタック領域を節約することができる。
- embassy-rs, [embassy-executor](https://docs.rs/embassy-executor/latest/embassy_executor/) を使用する場合は `alloc`, `heap` は不要。各 task は static に確保される。
- heap を使用するとフラグメントが発生するため組み込みでは非推奨。
- 組み込み環境では [heapless](https://docs.rs/heapless/latest/heapless/) を使用することができる。static friendly なデータ構造を構築でき，`heapless::Vec` のような固定長の配列を構築することができる。動的にメモリ確保する必要がない場合に最適。
