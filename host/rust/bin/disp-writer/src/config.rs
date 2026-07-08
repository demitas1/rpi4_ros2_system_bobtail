//! disp-writer の設定（TOML）。`docs/rpi4-ssd1306_display__plan.md` §5.3 準拠。
//!
//! すべてのフィールドに既定値を持たせてあり、`--config` 未指定なら [`Config::default`] で動作する。
//! `--config PATH` 指定時は TOML を読み込み、記載のあった項目だけ上書きする。

use anyhow::Context;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub display: DisplayConfig,
    pub i2c: I2cConfig,
    pub timing: TimingConfig,
    pub input: InputConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    /// "i2c" のみ対応（"spi" は次フェーズ。指定時はエラー）。
    pub interface: String,
    pub width: u32,
    pub height: u32,
    /// "Rotate0" / "Rotate90" / "Rotate180" / "Rotate270"。
    pub rotation: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct I2cConfig {
    pub bus: u8,
    pub address: u16,
    /// 参考値。実際の I2C クロックは /boot/firmware/config.txt の i2c_arm_baudrate で決まる。
    pub baudrate_hz: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TimingConfig {
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct InputConfig {
    pub shm_dir: String,
    pub filename: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            display: DisplayConfig::default(),
            i2c: I2cConfig::default(),
            timing: TimingConfig::default(),
            input: InputConfig::default(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        DisplayConfig {
            interface: "i2c".to_string(),
            width: 128,
            height: 64,
            rotation: "Rotate0".to_string(),
        }
    }
}

impl Default for I2cConfig {
    fn default() -> Self {
        I2cConfig {
            bus: 1,
            address: 0x3C, // SSD1306 デフォルト（0x3D の個体もある）
            baudrate_hz: 400_000,
        }
    }
}

impl Default for TimingConfig {
    fn default() -> Self {
        TimingConfig {
            poll_interval_ms: 20,
        }
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        InputConfig {
            shm_dir: "/run/disp-shm".to_string(),
            filename: "display_latest.bin".to_string(),
        }
    }
}

/// TOML ファイルから設定を読み込む。未記載の項目は既定値で補完される。
pub fn load_config(path: &Path) -> anyhow::Result<Config> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("config を開けません: {}", path.display()))?;
    let config: Config =
        toml::from_str(&text).with_context(|| format!("config のパースに失敗: {}", path.display()))?;
    Ok(config)
}
