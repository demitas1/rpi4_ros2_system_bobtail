//! tmpfs 上で共有される固定長バイナリ `DisplayFrame` の定義と読み取り。
//!
//! バイト配置は `docs/rpi4-ssd1306_display__plan.md` §2.1 のまま維持し、将来のコンテナ側
//! ROS2 ノード（§3.3 の Python `struct` フォーマット `"<IHH QQ B3x ffff 20s20s I"`）と
//! バイト互換にしてある。将来 IMU 実装時に `host/rust/crates/shm-frames` へ共通化する余地あり。

use std::fs;
use std::path::Path;

/// フォーマット識別マジック（"DIS0" = 0x44495330, little-endian）。
pub const MAGIC: u32 = 0x4449_5330;

/// コンテナ側と共有する固定長フレーム（合計 88 bytes）。
///
/// 明示パディング（`_reserved` / `_pad0`）を挟むことで暗黙パディングを持たないため、
/// `#[repr(C)]` のバイト配置は完全にパック相当（Python の `<...3x...>` と一致）になる。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DisplayFrame {
    pub magic: u32,            // 0
    pub version: u16,          // 4
    pub _reserved: u16,        // 6
    pub timestamp_ns: u64,     // 8  CLOCK_REALTIME
    pub seq: u64,              // 16 更新検知用シーケンス番号
    pub robot_state: u8,       // 24 0=IDLE,1=RUNNING,2=ERROR,3=CHARGING
    pub _pad0: [u8; 3],        // 25
    pub battery_voltage: f32,  // 28 V
    pub battery_percent: f32,  // 32 %
    pub linear_vel: f32,       // 36 m/s
    pub angular_vel: f32,      // 40 rad/s
    pub line1: [u8; 20],       // 44 固定長 ASCII（空白パディング）
    pub line2: [u8; 20],       // 64
    pub status_flags: u32,     // 84 ビットフラグ
}

// サイズが 88 バイトから外れたらコンパイルエラーにする（コンテナ側との契約）。
const _: () = assert!(std::mem::size_of::<DisplayFrame>() == 88);

impl DisplayFrame {
    /// `robot_state` を表示用文字列へ変換する。
    pub fn state_str(&self) -> &'static str {
        match self.robot_state {
            0 => "IDLE",
            1 => "RUNNING",
            2 => "ERROR",
            3 => "CHARGING",
            _ => "UNKNOWN",
        }
    }

    /// 固定長 ASCII フィールドを末尾空白/NUL を除いた文字列として取り出す。
    fn field_str(field: &[u8]) -> &str {
        std::str::from_utf8(field)
            .unwrap_or("")
            .trim_end_matches(['\0', ' '])
    }

    pub fn line1_str(&self) -> &str {
        Self::field_str(&self.line1)
    }
}

/// tmpfs 上の最新フレームを読み取る。
///
/// 方式A（rename 後の完全なファイル）を前提とするが、ファイル未存在・サイズ不一致・マジック不一致は
/// いずれも `Ok(None)`（描画スキップ）として扱い、プロセスは継続させる（防御的実装）。
pub fn read_latest_frame(path: &Path) -> anyhow::Result<Option<DisplayFrame>> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    if bytes.len() != std::mem::size_of::<DisplayFrame>() {
        // rename 直前の不完全ファイル等への防御。通常はここに来ない。
        return Ok(None);
    }

    let frame: DisplayFrame = *bytemuck::from_bytes(&bytes);
    if frame.magic != MAGIC {
        return Ok(None);
    }
    Ok(Some(frame))
}
