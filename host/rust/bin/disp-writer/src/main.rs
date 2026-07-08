//! disp-writer: tmpfs 上の DisplayFrame を読んで SSD1306 OLED(I2C) へ描画するホスト側バイナリ。
//! 設計: `docs/rpi4-ssd1306_display__plan.md`（§5 ホスト側実装）。
//!
//! 使い方:
//!   disp-writer                    # 既定値（i2c / 0x3C / /run/disp-shm）で起動
//!   disp-writer --config PATH      # TOML config で上書き
//!
//! tmpfs 上のフレームは検証用に host/scripts/gen_display_frame.py で投入できる。

mod config;
mod display;
mod frame;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use config::Config;

/// コマンドライン引数から config ファイルパスを取り出す（未指定なら None）。
fn parse_args() -> anyhow::Result<Option<PathBuf>> {
    let mut args = std::env::args().skip(1);
    let mut config_path = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                let p = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--config にパスが必要です"))?;
                config_path = Some(PathBuf::from(p));
            }
            "-h" | "--help" => {
                println!("使い方: disp-writer [--config PATH]");
                std::process::exit(0);
            }
            other => anyhow::bail!("不明な引数: {other}（--config PATH のみ対応）"),
        }
    }
    Ok(config_path)
}

fn main() -> anyhow::Result<()> {
    let config = match parse_args()? {
        Some(path) => {
            eprintln!("config を読み込み: {}", path.display());
            config::load_config(&path)?
        }
        None => {
            eprintln!("config 未指定 → 既定値で起動");
            Config::default()
        }
    };

    let file_path = Path::new(&config.input.shm_dir).join(&config.input.filename);
    let interval = Duration::from_millis(config.timing.poll_interval_ms);
    eprintln!(
        "interface={} i2c_addr=0x{:02X} shm={} poll={}ms",
        config.display.interface,
        config.i2c.address,
        file_path.display(),
        config.timing.poll_interval_ms
    );

    let mut display = display::init_display(&config)?;
    eprintln!("ディスプレイ初期化 OK。フレーム待機中… (Ctrl-C で終了)");

    let mut last_seq: Option<u64> = None;
    let mut render_count: u64 = 0;

    loop {
        let cycle_start = Instant::now();

        match frame::read_latest_frame(&file_path) {
            // seq が変化したときだけ再描画（重複描画=無駄な I2C 転送を避ける。§5.5）
            Ok(Some(f)) if Some(f.seq) != last_seq => {
                last_seq = Some(f.seq);
                match display::render_frame(&mut display, &f) {
                    Ok(flush_dur) => {
                        // 転送時間の実測（§5.8: 400kHz で ~23–25ms、90ms 台でないこと）。
                        // 初回と以降 50 回ごとにログ（毎回出すとうるさいため）。
                        if render_count == 0 || render_count % 50 == 0 {
                            eprintln!(
                                "render seq={} flush={:.1}ms state={}",
                                f.seq,
                                flush_dur.as_secs_f64() * 1e3,
                                f.state_str()
                            );
                        }
                        render_count += 1;
                    }
                    Err(e) => eprintln!("描画エラー（継続）: {e}"),
                }
            }
            Ok(_) => { /* 新規データなし or ファイル未存在 → スキップ */ }
            Err(e) => eprintln!("フレーム読み取りエラー（継続）: {e}"),
        }

        let elapsed = cycle_start.elapsed();
        if elapsed < interval {
            std::thread::sleep(interval - elapsed);
        }
    }
}
