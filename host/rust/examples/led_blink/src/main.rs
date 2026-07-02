//! GPIO LED 点滅 example（Rust / rppal）
//!
//! ホスト側（コンテナ外・Pi ネイティブ）で直接実行する。デーモン化はしない。
//! 対象は BCM(Broadcom) GPIO 番号。rppal は /dev/gpiomem 経由でアクセスするため、
//! gpio グループ所属のユーザーなら sudo なしで実行できる。
//!
//! 使い方:
//!   led_blink [GPIO番号] [周期ms]
//!   例) led_blink            # BCM17, 500ms
//!       led_blink 27 200     # BCM27, 200ms
//!
//! 配線例: GPIO17 -->|(LED)|-- [≈330Ω] -- GND
//! Ctrl-C で停止するとラインは入力に戻る（rppal の reset-on-drop）。

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rppal::gpio::Gpio;

const DEFAULT_GPIO: u8 = 17;
const DEFAULT_PERIOD_MS: u64 = 500;

fn main() -> Result<(), Box<dyn Error>> {
    // 引数パース（第1: GPIO番号, 第2: 周期ms）。省略時は既定値。
    let mut args = std::env::args().skip(1);
    let pin_num: u8 = match args.next() {
        Some(s) => s.parse().map_err(|_| format!("GPIO番号が不正です: {s}"))?,
        None => DEFAULT_GPIO,
    };
    let period_ms: u64 = match args.next() {
        Some(s) => s.parse().map_err(|_| format!("周期(ms)が不正です: {s}"))?,
        None => DEFAULT_PERIOD_MS,
    };
    let half = Duration::from_millis(period_ms / 2);

    // Ctrl-C を捕捉してループを抜ける（抜けると pin が drop され入力に戻る）。
    let running = Arc::new(AtomicBool::new(true));
    {
        let r = running.clone();
        ctrlc::set_handler(move || r.store(false, Ordering::SeqCst))?;
    }

    let mut pin = Gpio::new()?.get(pin_num)?.into_output();
    println!("LED blink (rust/rppal): BCM{pin_num}, 周期 {period_ms}ms。Ctrl-C で停止。");

    while running.load(Ordering::SeqCst) {
        pin.set_high();
        println!("BCM{pin_num}: ON");
        thread::sleep(half);
        if !running.load(Ordering::SeqCst) {
            break;
        }
        pin.set_low();
        println!("BCM{pin_num}: OFF");
        thread::sleep(half);
    }

    pin.set_low();
    println!("停止しました。ラインを解放します。");
    Ok(())
}
