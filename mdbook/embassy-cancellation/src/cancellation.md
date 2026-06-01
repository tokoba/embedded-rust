# キャンセル設計

Embassy でのキャンセルは、基本的に **協調的キャンセル**です。
外から task を強制 kill する API に依存するのではなく、task 自身が停止要求を受け取り、安全な境界で抜けます。

## 基本形: `select(work, cancel)`

```rust
use embassy_futures::select::{select, Either};

match select(work_future(), cancel_future()).await {
    Either::First(result) => {
        // 通常完了
    }
    Either::Second(_) => {
        // キャンセル要求
        cleanup().await;
    }
}
```

## Signal による個別キャンセル

```rust
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

static CANCEL: Signal<CriticalSectionRawMutex, ()> = Signal::new();
```

- 単一 worker の停止に向く
- 最新の停止要求だけ見ればよい場合に向く
- 複数 task への broadcast には向かない

## Channel による command loop

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Start,
    Cancel,
    Stop,
    ResetFault,
}
```

`Start`, `Cancel`, `Stop`, `Reconfigure` などが混在するなら、`Channel<Command, N>` が扱いやすくなります。

## Watch による global state 配信

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemState {
    Boot,
    Running,
    StopRequested,
    Fault,
}
```

複数 task に一斉に停止やモード変更を伝える場合は、`Watch<SystemState>` が適しています。

## ACK を必ず返す

停止要求は「止めろ」という要求であり、「止まった」という事実ではありません。
機械制御・通信停止・DMA 停止では、worker から supervisor へ ACK を返します。

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Started,
    Completed,
    Cancelled,
    Stopped,
    Fault,
}
```

## キャンセル安全性

`select` で負けた future は drop されます。
その future が DMA / peripheral / protocol state を持っている場合、drop 時に安全に停止できるかを必ず確認します。

安全性が不明な driver は、直接 `select` の work future に入れず、専用 owner task に閉じ込めて `Channel` で指示する構成にします。
