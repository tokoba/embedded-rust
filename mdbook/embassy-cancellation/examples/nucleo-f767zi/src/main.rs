#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer};
use panic_probe as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
enum Command {
    Start,
    Cancel,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
enum Status {
    Started,
    Step(u8),
    Completed,
    Cancelled,
    Stopped,
}

static CMD_CH: Channel<CriticalSectionRawMutex, Command, 4> = Channel::new();
static STATUS_CH: Channel<CriticalSectionRawMutex, Status, 4> = Channel::new();

#[embassy_executor::task]
async fn led_task(mut led: Output<'static>) {
    loop {
        led.set_high();
        Timer::after(Duration::from_millis(100)).await;
        led.set_low();
        Timer::after(Duration::from_millis(900)).await;
    }
}

#[embassy_executor::task]
async fn button_task(mut button: ExtiInput<'static>) {
    loop {
        button.wait_for_falling_edge().await;
        info!("button: Start command");
        if CMD_CH.try_send(Command::Start).is_err() {
            warn!("command queue full: Start dropped");
        }
        Timer::after(Duration::from_millis(250)).await;
    }
}

#[embassy_executor::task]
async fn supervisor_task() {
    loop {
        match STATUS_CH.receive().await {
            Status::Started => {
                info!("supervisor: worker started; cancel after 2 sec");
                Timer::after(Duration::from_secs(2)).await;
                if CMD_CH.try_send(Command::Cancel).is_err() {
                    warn!("command queue full: Cancel dropped");
                }
            }
            s => info!("supervisor: status={:?}", s),
        }
    }
}

#[embassy_executor::task]
async fn worker_task() {
    loop {
        match CMD_CH.receive().await {
            Command::Start => {
                STATUS_CH.send(Status::Started).await;

                match select(measurement_sequence(), wait_cancel_or_stop()).await {
                    Either::First(()) => {
                        safe_cleanup().await;
                        STATUS_CH.send(Status::Completed).await;
                    }
                    Either::Second(Command::Cancel) => {
                        warn!("worker: cancelled");
                        safe_cleanup().await;
                        STATUS_CH.send(Status::Cancelled).await;
                    }
                    Either::Second(Command::Stop) => {
                        warn!("worker: stopped");
                        safe_cleanup().await;
                        STATUS_CH.send(Status::Stopped).await;
                        break;
                    }
                    Either::Second(Command::Start) => {
                        // Start while running is treated as a replace/restart request.
                        // This sample ignores it to keep the state machine explicit.
                        warn!("worker: Start received while running; ignored");
                    }
                }
            }
            Command::Cancel => {
                // Idle 中の Cancel は古い停止要求として破棄する。
                warn!("worker: stale Cancel ignored in idle state");
            }
            Command::Stop => {
                STATUS_CH.send(Status::Stopped).await;
                break;
            }
        }
    }
}

async fn wait_cancel_or_stop() -> Command {
    loop {
        let cmd = CMD_CH.receive().await;
        if matches!(cmd, Command::Cancel | Command::Stop) {
            return cmd;
        }
    }
}

async fn measurement_sequence() {
    for step in 0..30u8 {
        // 実際のセンサ読取り・制御ステップをここへ置く。
        // 100ms ごとに await するため、キャンセル反応性が保たれる。
        STATUS_CH.send(Status::Step(step)).await;
        Timer::after(Duration::from_millis(100)).await;
    }
}

async fn safe_cleanup() {
    // ここで PWM duty=0, CS deassert, DMA stop, motor off などを行う。
    // このサンプルではログのみ。
    info!("cleanup: peripherals returned to safe state");
    Timer::after(Duration::from_millis(10)).await;
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    // NUCLEO-F767ZI: LD1 は一般に PB0、B1 は PC13。
    // ボードリビジョン差がある場合は該当ピンを変更する。
    let led = Output::new(p.PB0, Level::Low, Speed::Low);
    let button_input = Input::new(p.PC13, Pull::Up);
    let button = ExtiInput::new(button_input, p.EXTI13);

    spawner.spawn(led_task(led)).unwrap();
    spawner.spawn(button_task(button)).unwrap();
    spawner.spawn(supervisor_task()).unwrap();
    spawner.spawn(worker_task()).unwrap();

    info!("NUCLEO-F767ZI Embassy task-control sample started");

    loop {
        Timer::after(Duration::from_secs(10)).await;
        info!("main: heartbeat");
    }
}
