//! SSD1306 への物理接続層と描画。`docs/rpi4-ssd1306_display__plan.md` §5.2/§5.4/§5.7 準拠。
//!
//! [`DisplayHandle`] は接続方式を吸収する enum。現状 I2C のみだが、SPI は将来 `Spi` variant を
//! 追加するだけで `render_frame`（描画ロジック）は無変更で共通利用できる設計にしてある。

use crate::config::Config;
use crate::frame::DisplayFrame;

use anyhow::anyhow;
use display_interface_i2c::I2CInterface;
use embedded_graphics::{
    mono_font::{ascii::FONT_7X13, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};
use rppal::i2c::I2c;
use ssd1306::{
    mode::BufferedGraphicsMode, prelude::*, size::DisplaySize128x64, I2CDisplayInterface, Ssd1306,
};
use std::time::Instant;

/// I2C 接続時の SSD1306 具象型（128x64・バッファ描画モード）。
type I2cDisplay =
    Ssd1306<I2CInterface<I2c>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>;

/// 物理接続層を吸収するラッパ。SPI 対応時は variant を足すだけで描画側は不変。
pub enum DisplayHandle {
    I2c(I2cDisplay),
}

/// config の rotation 文字列を `DisplayRotation` に変換する。
fn parse_rotation(s: &str) -> anyhow::Result<DisplayRotation> {
    Ok(match s {
        "Rotate0" => DisplayRotation::Rotate0,
        "Rotate90" => DisplayRotation::Rotate90,
        "Rotate180" => DisplayRotation::Rotate180,
        "Rotate270" => DisplayRotation::Rotate270,
        other => return Err(anyhow!("unknown rotation: {other}")),
    })
}

/// config の interface に応じてディスプレイを初期化する。現状 "i2c" のみ対応。
pub fn init_display(config: &Config) -> anyhow::Result<DisplayHandle> {
    match config.display.interface.as_str() {
        "i2c" => {
            // I2C クロックは /boot/firmware/config.txt の i2c_arm_baudrate で決まる（rppal 側指定不要）。
            let i2c = I2c::with_bus(config.i2c.bus)?;
            let interface = I2CDisplayInterface::new_custom_address(i2c, config.i2c.address as u8);
            let rotation = parse_rotation(&config.display.rotation)?;
            let mut display = Ssd1306::new(interface, DisplaySize128x64, rotation)
                .into_buffered_graphics_mode();
            display
                .init()
                .map_err(|e| anyhow!("display init failed: {e:?}"))?;
            Ok(DisplayHandle::I2c(display))
        }
        "spi" => Err(anyhow!(
            "interface = \"spi\" は未対応（次フェーズ）。現状は \"i2c\" のみ"
        )),
        other => Err(anyhow!("unknown interface: {other}")),
    }
}

/// フレーム内容を SSD1306 に描画する。I2C/SPI どちらでも無変更で共通（enum で吸収）。
///
/// 戻り値はフルフレーム転送（`flush`）に要した時間。転送コスト実測（§5.8）に使う。
pub fn render_frame(
    display: &mut DisplayHandle,
    frame: &DisplayFrame,
) -> anyhow::Result<std::time::Duration> {
    let style = MonoTextStyle::new(&FONT_7X13, BinaryColor::On);

    let batt = format!(
        "Batt: {:.1}V {:.0}%",
        frame.battery_voltage, frame.battery_percent
    );
    let vel = format!("v={:.2} w={:.2}", frame.linear_vel, frame.angular_vel);
    let line1 = frame.line1_str();

    match display {
        DisplayHandle::I2c(d) => {
            d.clear(BinaryColor::Off)
                .map_err(|e| anyhow!("clear failed: {e:?}"))?;
            Text::new(frame.state_str(), Point::new(0, 12), style)
                .draw(d)
                .map_err(|e| anyhow!("draw failed: {e:?}"))?;
            Text::new(&batt, Point::new(0, 28), style)
                .draw(d)
                .map_err(|e| anyhow!("draw failed: {e:?}"))?;
            Text::new(&vel, Point::new(0, 44), style)
                .draw(d)
                .map_err(|e| anyhow!("draw failed: {e:?}"))?;
            Text::new(line1, Point::new(0, 60), style)
                .draw(d)
                .map_err(|e| anyhow!("draw failed: {e:?}"))?;

            let t0 = Instant::now();
            d.flush().map_err(|e| anyhow!("flush failed: {e:?}"))?;
            Ok(t0.elapsed())
        }
    }
}
