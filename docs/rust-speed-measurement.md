# Rustでの実行速度・CPU負荷率・メモリ占有率の測定ガイド 🦀

本ドキュメントは、Rustで書かれたプログラムについて、各シーケンス・各関数の**実行時間**, **CPU使用率**, **メモリ占有率**を測定・可視化するための方法を網羅的にまとめたものです。  
対象は以下です。

- コード内の軽量計測（`std::time::Instant`、RAIIタイマーなど）
- 統計的に信頼できるマイクロベンチ（**Criterion**）
- ホットスポット・命令数・キャッシュミス等の低ノイズ計測（**perf**, **cargo-flamegraph**, **Valgrind(Callgrind/Massif)**, **Heaptrack**）
- ランタイムでの**CPU負荷率**・**メモリ使用量**計測（**sysinfo** 等）
- OSプロファイラ活用（Linux: **perf**, Windows: **WPR/WPA**, macOS: **Instruments/DTrace**）
- 非同期タスクの可観測性（**tokio-console**）

計測は環境依存性が高いため、再現性を高めるためのベストプラクティスも併記します。

---

## 0. 前提・基本方針

- 基本は**Releaseビルド**で測定（`cargo build --release`）。
- **入出力（println!・ログ）**は測定対象外にするか最小化。
- **データ規模・乱数種**は固定し、**複数回測定**して分布を見る。
- プロファイラのコールグラフ品質向上のため、Linux/WSL2では**フレームポインタ有効化**を推奨。

例:

```bash
cargo build --release
```

```bash
set RUSTFLAGS=-C force-frame-pointers=yes
cargo build --release
```

追加の再現性向上:

- **CPUスケーリング固定**（Linuxなら `sudo cpupower frequency-set -g performance` など）
- **タスクピニング**（Linux: `taskset`, Windows: `start /affinity`, NUMAなら `numactl`）
- **バックグラウンドを止める**, **ネットワーク切断**, **電源プラン高パフォーマンス**

---

## 1. コード内での軽量な実行時間計測

### 1.1 `std::time::Instant` による計測

最も簡単な経路。最適化の影響を減らすため `std::hint::black_box` の併用が有効です。

```rust
use std::hint::black_box;
use std::time::Instant;

fn heavy_computation(n: u64) -> u64 {
    let mut acc = 0;
    for i in 0..n {
        acc = acc.wrapping_add(black_box(i));
    }
    acc
}

fn main() {
    let start = Instant::now();
    let result = heavy_computation(10_000_000);
    let elapsed = start.elapsed();

    println!("result={result}, elapsed={:?}", elapsed);
}
```

### 1.2 RAIIで関数スコープの所要時間を自動ログ

関数・スコープごとの計測を簡便化できます。

```rust
use std::time::{Duration, Instant};

pub struct ScopeTimer<'a> {
    label: &'a str,
    start: Instant,
}

impl<'a> ScopeTimer<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, start: Instant::now() }
    }
}

impl<'a> Drop for ScopeTimer<'a> {
    fn drop(&mut self) {
        let dt = self.start.elapsed();
        println!("[{}] elapsed = {:?}", self.label, dt);
    }
}

fn some_work() {
    let _t = ScopeTimer::new("some_work");
    // 計測対象の処理
}
```

### 1.3 関数ごとのユーティリティ

戻り値と所要時間をペアで返すヘルパー。

```rust
use std::time::{Duration, Instant};

pub fn measure<F, R>(label: &str, mut f: F) -> (R, Duration)
where
    F: FnMut() -> R,
{
    let t0 = Instant::now();
    let r = f();
    let dt = t0.elapsed();
    println!("[{}] elapsed = {:?}", label, dt);
    (r, dt)
}
```

---

## 2. ベンチマークフレームワーク（Criterion）によるマイクロベンチ

**Criterion** は安定版Rustで動作し、ウォームアップ・統計解析を含む高信頼なベンチマークを提供します。

### 2.1 Cargo設定（例）

```toml
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "my_bench"
harness = false
```

### 2.2 ベンチファイル（`benches/my_bench.rs`）

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

fn sum_loop(n: u64) -> u64 {
    (0..n).fold(0u64, |acc, x| acc.wrapping_add(black_box(x)))
}

fn bench_sum(c: &mut Criterion) {
    let mut group = c.benchmark_group("sum");
    group.throughput(Throughput::Elements(10_000_000));
    group.bench_function("sum_loop_10M", |b| {
        b.iter(|| sum_loop(10_000_000));
    });
    group.finish();
}

criterion_group!(benches, bench_sum);
criterion_main!(benches);
```

### 2.3 実行

```bash
cargo bench
```

補助ツール: **cargo-criterion**（きれいなレポートHTML生成）

```bash
cargo install cargo-criterion
cargo criterion
```

---

## 3. CPU使用率・メモリ占有率のランタイム取得

### 3.1 `sysinfo` クレート

プロセスのCPU・メモリ使用量を周期的に取得できます。

Cargo.toml:

```toml
[dependencies]
sysinfo = "0.30"
```

コード例:

```rust
use sysinfo::{System, SystemExt, CpuRefreshKind, RefreshKind, ProcessExt, Pid};

fn main() {
    // CPUとプロセス情報をリフレッシュする設定
    let refresh_kind = RefreshKind::new()
        .with_cpu(CpuRefreshKind::everything())
        .with_processes();
    let mut sys = System::new_with_specifics(refresh_kind);

    let pid = Pid::from_u32(std::process::id());
    for _ in 0..10 {
        sys.refresh_all();

        if let Some(p) = sys.process(pid) {
            // CPU使用率（%）。前回refreshとの差分ベースで、コア数によって合計%が増えることがあります。
            let cpu = p.cpu_usage();
            // 物理メモリ（kB; プラットフォーム依存）
            let mem_kb = p.memory();
            // 仮想メモリ（kB）
            let virt_kb = p.virtual_memory();
            println!("CPU: {:.2}% | Mem: {} kB | Virt: {} kB", cpu, mem_kb, virt_kb);
        }

        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}
```

### 3.2 Linuxのピークメモリ（`/proc/self/status`）

```rust
use std::fs;

fn print_vmpeak() {
    if let Ok(s) = fs::read_to_string("/proc/self/status") {
        for line in s.lines() {
            if line.starts_with("VmPeak:") {
                println!("{}", line); // 例: "VmPeak:   123456 kB"
                break;
            }
        }
    }
}
```

### 3.3 `getrusage` による最大常駐集合サイズ（ru_maxrss）

プラットフォーム差に注意（LinuxはkB、macOSはバイト等）。

```rust
#[cfg(unix)]
fn print_ru_maxrss() {
    use libc::{getrusage, rusage, RUSAGE_SELF};
    unsafe {
        let mut usage: rusage = std::mem::zeroed();
        if getrusage(RUSAGE_SELF, &mut usage) == 0 {
            println!("ru_maxrss = {}", usage.ru_maxrss);
        }
    }
}
```

---

## 4. Linuxでのプロファイリング（perf）

### 4.1 インストール（Ubuntu/WSL2例）

```bash
sudo apt update
sudo apt install linux-tools-common
sudo apt install linux-tools-$(uname -r)
# または: sudo apt install linux-tools-generic
```

### 4.2 基本統計（`perf stat`）

```bash
cargo build --release
perf stat -r 5 ./target/release/your_binary
# 代表イベント指定
perf stat -e cycles,instructions,cache-references,cache-misses,branch-misses -r 5 -- ./target/release/your_binary
```

主な指標:

- **cycles / instructions**（IPC算出可）
- **cache-misses**, **branch-misses**
- **task-clock**（CPU時間）など

### 4.3 ホットスポット解析（`perf record` → `perf report`）

```bash
# フレームポインタ有効時（推奨）
perf record -F 999 -g -- ./target/release/your_binary
perf report

# フレームポインタ無効なら、DWARFベースのコールグラフ
perf record --call-graph dwarf -F 999 -- ./target/release/your_binary
perf report
```

### 4.4 アセンブリレベル注釈（`perf annotate`）

```bash
perf annotate
```

---

## 5. フレームグラフ（cargo-flamegraph）

Rustでは **cargo-flamegraph** により perf/DTrace をラップして簡便にフレームグラフ（SVG）を作成できます。

インストール:

```bash
cargo install flamegraph
```

実行（バイナリ）:

```bash
cargo flamegraph --bin your_binary
# 出力: flamegraph.svg
```

ベンチ対象:

```bash
cargo flamegraph --bench my_bench
```

---

## 6. メモリプロファイリング

### 6.1 Valgrind Massif（ヒープ使用量の推移）

```bash
sudo apt install valgrind
cargo build --release
valgrind --tool=massif ./target/release/your_binary
ms_print massif.out.*
```

### 6.2 Callgrind（関数ごとの命令数など）

```bash
valgrind --tool=callgrind ./target/release/your_binary
sudo apt install kcachegrind
kcachegrind callgrind.out.*
```

### 6.3 Heaptrack（割当ホットスポット）

```bash
sudo apt install heaptrack
heaptrack ./target/release/your_binary
heaptrack_gui heaptrack.your_binary.*.gz
```

---

## 7. Windowsでの計測

### 7.1 WPR/WPA（Windows Performance Recorder/Analyzer）

収集:

```bash
wpr -start GeneralProfile
wpr -stop trace.etl
```

解析:

```bash
wpa trace.etl
```

### 7.2 軽量監視（CPU・メモリ）

```bash
typeperf "\Processor(_Total)\% Processor Time" -sc 10
typeperf "\Process(your_binary)\Working Set - Private" -sc 10
```

### 7.3 コア固定（アフィニティ）

```bash
start /affinity 1 target\release\your_binary.exe
```

Perfが必要なら **WSL2** 上での利用が現実的です。

---

## 8. macOSでの計測

- **Instruments**（Time Profiler）でホットスポット解析
- **DTrace** によるサンプリング（権限が必要な場合あり）
- `cargo flamegraph` は macOS では DTrace を用いてフレームグラフ生成が可能

---

## 9. 非同期タスクの可観測性（tokio-console）

非同期（`tokio`）のタスクスケジューリングや待機時間を可視化できます。

Cargo.toml（例）:

```toml
[dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
console-subscriber = "0.2"
```

コード（初期化）:

```rust
#[tokio::main]
async fn main() {
    console_subscriber::init(); // tokio-console が接続可能になる
    // あとは通常の非同期コード
    for i in 0..5 {
        tokio::spawn(async move {
            // 擬似負荷
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            println!("task {i} done");
        });
    }
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
}
```

ビューア:

```bash
cargo install tokio-console
tokio-console
```

---

## 10. PGO（Profile-Guided Optimization）の概要

実行プロファイルに基づいて最適化を適用するLLVMの仕組みです。

概略ステップ:

1. プロファイル生成ビルド（`-C profile-generate`）でビルドし、実行して `.profraw` を得る
2. `llvm-profdata` で集約（`.profdata` に変換）
3. プロファイル適用ビルド（`-C profile-use=path/to.profdata`）

PGOはワークロード代表性が重要。CIに組み込み、プロファイル更新のタイミングを管理しましょう。

---

## 11. ベストプラクティス ✅

- **Releaseビルド**で測定、I/Oは最小化
- **ウォームアップ**と**複数回測定**でばらつきを抑える
- **フレームポインタ有効化**でコールスタック品質改善
- **データ・環境の固定**（CPU governor、アフィニティ、NUMA配置）
- **Criterion**で関数単位ベンチ、**Flamegraph/Perf**でホットパス把握
- **sysinfo/procfs/getrusage**でランタイムのCPU/メモリ把握を補助
- メモリは **Massif/Heaptrack** でピークやリークを可視化
- **target-cpu=native** を検討（`RUSTFLAGS=-C target-cpu=native`）※移植性に注意

---

## 12. 参考コマンド集（再掲）

- ビルド＆perf基本統計（Linux）

```bash
cargo build --release
perf stat -e cycles,instructions,cache-misses,branch-misses -r 5 -- ./target/release/your_binary
```

- ホットスポット解析

```bash
perf record -F 999 -g -- ./target/release/your_binary
perf report
```

- フレームグラフ生成

```bash
cargo install flamegraph
cargo flamegraph --bin your_binary
```

- ベンチ（Criterion）

```bash
cargo bench
```

- メモリ（Massif）

```bash
valgrind --tool=massif ./target/release/your_binary
ms_print massif.out.*
```

---

## 13. 追加ツール（参考）

- **cargo-llvm-lines**: LLVM IR行数で関数の最適化対象を把握

  ```bash
  cargo install cargo-llvm-lines
  cargo llvm-lines --bin your_binary
  ```

- **cargo-bloat**: バイナリサイズの関数別内訳

  ```bash
  cargo install cargo-bloat
  cargo bloat --release --bin your_binary
  ```

---

## 14. Windows 11 + Git Bash 環境で使える計測手法まとめ

Windows 11 上の **Git Bash**（MSYS2/bash）から実行可能な計測手法を抜粋して整理します。  
Git BashはUNIX風のシェルですが、**Windowsネイティブコマンド**（`typeperf`, `wpr`, `wpa`, `powershell.exe`, `cmd.exe`）も呼び出せます。  
必要に応じて **WSL2** を併用すると、`perf` や `cargo flamegraph` も利用可能です。

### 14.1 手軽な計測（Rust側）

- **Releaseビルド**

  ```bash
  cargo build --release
  ```

- **壁時計時間（Git Bashの time）**

  ```bash
  time ./target/release/your_binary.exe
  ```

- **マイクロベンチ（Criterion）**

  ```bash
  cargo bench
  cargo install cargo-criterion
  cargo criterion
  ```

- **フレームポインタ有効化（Git Bashでは export を使用）**

  ```bash
  export RUSTFLAGS="-C force-frame-pointers=yes"
  cargo build --release
  ```

### 14.2 ランタイム監視（プロセスのCPU/メモリ）

- **sysinfo クレート**はクロスプラットフォームで動作（既存の例を参照）。  
  Git Bashから実行すれば、CPU%／物理メモリ／仮想メモリを周期的に取得できます。

### 14.3 Windowsの外部計測（Git Bashから直接呼び出し）

- **typeperf（CPU・メモリ）**

  ```bash
  typeperf "\Processor(_Total)\% Processor Time" -sc 10
  typeperf "\Process(your_binary)\Working Set - Private" -sc 10
  typeperf "\Processor(_Total)\% Processor Time" -si 1 -sc 10 -f CSV -o perf.csv
  ```

  - `-sc` はサンプル回数、`-si` は間隔秒、`-f CSV` と `-o` で保存。

- **PowerShellの Get-Counter / Get-Process をGit Bash経由で呼ぶ**

  ```bash
  powershell.exe -NoProfile -Command "Get-Counter '\Processor(_Total)\% Processor Time' -SampleInterval 1 -MaxSamples 10"
  powershell.exe -NoProfile -Command "Get-Process | Where-Object { $_.ProcessName -eq 'your_binary' } | Select-Object ProcessName,CPU,WS,PM"
  ```

  - 引数のクォートは PowerShell 側のルールに従います。Git Bashでは全体を`"`で包むと扱いやすいです。

### 14.4 コア固定（アフィニティ指定）

- `start` は **cmd.exe のビルトイン**のため、Git Bashからは `cmd.exe /C` 経由で実行します。

  ```bash
  cmd.exe /C start "" /affinity 1 target\release\your_binary.exe
  ```

  - `1` はCPU0を意味します（ビットマスク）。例: CPU0とCPU1なら `/affinity 3`。

### 14.5 WPR/WPA（Windows Performance Recorder/Analyzer）

- Git Bashから直接呼び出し可能です。

  ```bash
  wpr -start GeneralProfile
  wpr -stop trace.etl
  wpa trace.etl
  ```

### 14.6 WSL2での perf / flamegraph 併用

- Windowsネイティブ環境では `perf` が使えないため、**WSL2** 上で計測します。Git BashからWSLを起動してコマンドを渡せます。

  ```bash
  wsl -e bash -lc "cd /mnt/c/path/to/your/repo && cargo build --release && perf stat -r 5 ./target/release/your_binary"
  wsl -e bash -lc "cd /mnt/c/path/to/your/repo && cargo install flamegraph && cargo flamegraph --bin your_binary"
  ```

  - リポジトリのパスは `C:\...` → `/mnt/c/...` に読み替えてください。
  - コールグラフ品質を高めるには、WSL側でも `export RUSTFLAGS="-C force-frame-pointers=yes"` を設定。

### 14.7 Git Bash利用時のTips

- **環境変数**: Git Bashでは `export NAME=VALUE` を使用。`set` は `cmd.exe` 用。
- **パス表記**: Windowsの `C:\path\to\file` は Git Bash で `C:/path/to/file` と書けます。WSL内では `/mnt/c/path/to/file`。
- **Windowsビルトインの呼び出し**: `cmd.exe /C ...` や `powershell.exe -Command ...` で橋渡し可能。
- **I/Oノイズ低減**: `println!` や詳細ログはベンチ中に無効化する（`RUST_LOG=off` など）。

---

以上。📈🛠️  
このガイドを基に、目的（高速化、ボトルネック発見、メモリ改善）に応じて適切な計測手法を選択・併用してください。
