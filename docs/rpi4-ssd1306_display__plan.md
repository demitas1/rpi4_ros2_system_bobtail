# SSD1306 OLEDディスプレイ出力システム 実装計画書

## 1. 概要

Docker上のROS2ノード(Ubuntu, C/C++ or Python)がロボットの状態値等をtmpfs上のファイルへ100ms間隔で上書き書き込みする。Raspberry Pi 4 のホスト OS（実機は Debian 13 trixie / aarch64）側のRustプロセスがこのファイルを監視・読み取り、SSD1306 OLEDディスプレイへ描画データとして変換・出力する。前回作成した「IMU読み取り(ホスト→コンテナ)」の**データフローが逆方向**になる構成。

Pi4/Pi5からSSD1306への接続はI2C・SPIいずれも可能であり、本計画では**両方式をサポートし、用途・要件に応じてコンフィグで切り替えられる設計**とする(5章でI2C/SPI双方の実装、5.10節で使い分けの指針)。ホスト側のデータ取得・描画ロジック(2〜3章、5.5〜5.7節)は接続方式に関わらず共通であり、差分は「ディスプレイとの物理接続層のみ」に閉じ込める設計とする。

### 1.1 要件サマリ

| 項目 | 内容 |
|---|---|
| 対象デバイス | SSD1306 OLED(128x64モノクロ想定) |
| 接続方式 | **I2CまたはSPI。用途に応じてコンフィグで切り替え可能な設計とする**(5.10節に使い分け指針) |
| 更新間隔 | 目標100ms。厳密な等間隔は不要(最新値の反映が優先) |
| データ保持方式 | 追記なし、上書きのみ(最新値のみ) |
| 永続化 | 不要。電源断で全ロスを許容 |
| コンテナ側実装 | C/C++ または Python(ROS2, Ubuntuベース)、**書き込み専用** |
| ホスト側実装 | Rust(Pi4/Pi5上でネイティブビルド・実行、クロスコンパイル不要)、**読み取り専用** |
| 共有方式 | tmpfs + bind mount |
| アトミック性 | 方式A(write-tmp → rename) |
| データ形式 | 固定長バイナリ |

### 1.2 全体構成図

```
                    ┌───────────────────────────────┐
                    │ Docker Container (Ubuntu+ROS2) │
                    │                                 │
                    │   ROS2 topic: /robot/status     │
                    │           │                     │
                    │           ▼                     │
                    │   display_bridge_node           │
                    │   (ROS2 subscriber)              │
                    │           │                     │
                    │           ▼ write-tmp → rename    │
                    │   /disp-shm/display_latest.bin  │
                    └───────────┬─────────────────────┘
                                │ bind mount (rw, コンテナ側書込み)
┌────────────────────────────────┼──────────────────────────┐
│ Raspberry Pi OS (Host)         ▼                          │
│                     ┌────────────────────┐                │
│                     │ tmpfs               │                │
│                     │ /run/disp-shm/       │                │
│                     │   display_latest.bin  │               │
│                     └─────────┬──────────┘                │
│                                │ read-only オープン           │
│                                ▼                            │
│                     ┌───────────────┐                       │
│                     │ disp-writer    │                       │
│                     │ (Rust, native  │                       │
│                     │  binary)       │                       │
│                     │  ┌──────────┐  │  I2C(400kHz)          │
│                     │  │Interface │──┼──────────┐            │
│                     │  │  切替    │  │  or       ▼            │
│                     │  │(config)  │──┼──SPI──►┌──────────┐    │
│                     │  └──────────┘  │ (数MHz~) │ SSD1306  │    │
│                     └───────────────┘         │(I2C/SPI  │    │
│                                                │ 両対応品) │    │
│                                                └──────────┘    │
└───────────────────────────────────────────────────────────┘
```

物理接続層(I2C/SPI)はコンフィグで選択し、それ以外(tmpfs監視・フレームパース・描画ロジック)は共通コードとする。詳細は5章参照。

**前回(IMU)構成との対称性**:

| 項目 | IMU(前回) | OLED(今回) |
|---|---|---|
| I2Cセンサ/デバイス操作 | ホスト側(読み取り) | ホスト側(書き込み) |
| tmpfsへの書き込み元 | ホスト | コンテナ |
| tmpfsからの読み取り元 | コンテナ | ホスト |
| コンテナの役割 | 読み取り専用 | 書き込み専用 |
| インターバル | 10ms | 100ms |

データフローの向きが反転しているだけで、**方式A(write-tmp→rename)によるアトミック共有という設計原理は共通**である。

---

## 1.3 本リポジトリ・実機環境との対応（現行システムに合わせた更新）

本計画書は当初リポジトリの知識なしに作成したため、実機・本リポの実態に合わせて以下を読み替える
（IMU 側 [`rpi4-mpu6050_imu_bridge_plan.md`](rpi4-mpu6050_imu_bridge_plan.md) §1.3 と共通）。

- **実行ユーザー / OS**: 実機は **Debian 13 (trixie) / aarch64**、ユーザーは **`demitas`**
  （`gpio`/`i2c`/`spi` グループ所属で sudo 不要）。本文の `User=pi` は **`demitas`** と読み替える。
- **ホスト側 Rust の置き場所**: 本リポの **`host/rust/bin/disp-writer/`**（`host/rust` 仮想ワークスペースの
  メンバー）。ビルド/デプロイは `host/scripts/deploy_and_build.sh`。詳細は [`../host/README.md`](../host/README.md)。
- **コンテナ側 ROS2 ノード**: ベースイメージ **ROS2 Jazzy（Ubuntu 24.04）** `ghcr.io/demitas1/ros2_jazzy`。
  `display_bridge_node` は **`src/` の ament パッケージ**（例 `bobtail_display_bridge`）として実装し、
  bind mount は既存の `docker-compose.yml` / `docker-compose.prod.yml` に統合する（§4 は概念例）。
- **I2C / SPI の有効化（必須・前提）**: 実機は既定で GPIO ヘッダの I2C・SPI が無効
  （**`/dev/i2c-1` も `/dev/spidev*` も存在しない**）。使用する方式に応じて
  **`/boot/firmware/config.txt`**（Pi OS の `/boot/config.txt` ではない）で有効化して再起動する:

  ```bash
  # I2C 接続で使う場合
  sudo sed -i 's/^#\?dtparam=i2c_arm=.*/dtparam=i2c_arm=on/' /boot/firmware/config.txt
  echo 'dtparam=i2c_arm_baudrate=400000' | sudo tee -a /boot/firmware/config.txt
  sudo apt-get install -y i2c-tools
  # SPI 接続で使う場合
  sudo sed -i 's/^#\?dtparam=spi=.*/dtparam=spi=on/' /boot/firmware/config.txt
  sudo reboot
  # 確認: I2C → ls /dev/i2c-1 && i2cdetect -y 1 (SSD1306 は 0x3C/0x3D)
  #       SPI → ls /dev/spidev0.*
  ```

- **rppal のバージョン**: 本文の `0.19` でも動作見込み。現行の最新は `0.22` 系のため、
  `ssd1306` / `embedded-graphics` / `display-interface-spi` との互換を実機ビルドで確認して固定する。

## 2. データフォーマット(固定長バイナリ)

### 2.1 表示内容の設計

SSD1306(128x64)で表示する典型的な内容として、ロボットの状態表示を想定する。テキスト整形(フォントレンダリング)は**ホスト側で行う**ため、コンテナ側は生の数値・短い文字列のみを渡す設計とする(コンテナ側にフォント処理・描画ロジックを持たせない=責務の分離)。

```rust
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DisplayFrame {
    pub magic: u32,          // フォーマット識別 (例: 0x44495330 "DIS0")
    pub version: u16,
    pub _reserved: u16,
    pub timestamp_ns: u64,   // CLOCK_REALTIME
    pub seq: u64,            // 更新検知用シーケンス番号

    pub robot_state: u8,     // enum的に扱う: 0=IDLE,1=RUNNING,2=ERROR,3=CHARGING 等
    pub _pad0: [u8; 3],

    pub battery_voltage: f32, // V
    pub battery_percent: f32, // %
    pub linear_vel: f32,      // m/s
    pub angular_vel: f32,     // rad/s

    pub line1: [u8; 20],      // 固定長ASCII文字列(null終端 or 空白パディング)
    pub line2: [u8; 20],

    pub status_flags: u32,    // ビットフラグ(警告灯、接続状態等)
}
// 合計サイズ: 4+2+2+8+8 +1+3 +4+4+4+4 +20+20 +4 = 88 bytes
```

- 文字列フィールド(`line1`, `line2`)を持たせることで、ROS2側の任意のメッセージ(diagnosticsのサマリ等)を柔軟に表示できるようにしつつ、数値フィールドは個別に持たせてホスト側でアイコン表示やレイアウト分岐に使えるようにする
- 将来的にフィールドを増やす場合は`version`をインクリメントし、ホスト側で後方互換的にパースする

### 2.2 タイムスタンプの扱い

前回同様、ホスト・コンテナ間でクロックが共有される前提で`CLOCK_REALTIME`(UNIXエポックのナノ秒)を採用する。ホスト側で「最後の更新から一定時間経過していたら表示を『通信断』状態に切り替える」といった鮮度チェックに利用できる(7.4節で触れる)。

---

## 3. コンテナ側実装(ROS2ノード、書き込み専用)

### 3.1 監視方式・書き込みトリガー

ROS2側は「トピック受信をトリガーに即座に書き込む」か「タイマーで100msごとに最新の保持値を書き込む」かの2方式がある。今回は要件が「最新値のみ・厳密なタイミング不要」なため、**タイマー駆動方式(直近のサブスクライブ値をメンバ変数に保持し、100msタイマーで書き込み)**を推奨する。トピック受信ごとに書き込むとバーストトラフィックがある場合に無駄なI/Oが発生するため。

### 3.2 C++実装例(rclcpp)

```cpp
#include <rclcpp/rclcpp.hpp>
#include <std_msgs/msg/string.hpp>
#include <sensor_msgs/msg/battery_state.hpp>
#include <geometry_msgs/msg/twist.hpp>
#include <fstream>
#include <cstring>
#include <cstdint>
#include <filesystem>

#pragma pack(push, 1)
struct DisplayFrame {
    uint32_t magic;
    uint16_t version;
    uint16_t reserved;
    uint64_t timestamp_ns;
    uint64_t seq;
    uint8_t  robot_state;
    uint8_t  pad0[3];
    float    battery_voltage;
    float    battery_percent;
    float    linear_vel;
    float    angular_vel;
    char     line1[20];
    char     line2[20];
    uint32_t status_flags;
};
#pragma pack(pop)
static_assert(sizeof(DisplayFrame) == 88, "DisplayFrame size mismatch");

class DisplayBridgeNode : public rclcpp::Node {
public:
    DisplayBridgeNode() : Node("display_bridge_node"),
        shm_dir_("/disp-shm"), tmp_name_(".display_latest.tmp"), final_name_("display_latest.bin")
    {
        std::filesystem::create_directories(shm_dir_);
        std::memset(&current_, 0, sizeof(current_));
        current_.magic = 0x44495330;
        current_.version = 1;

        battery_sub_ = create_subscription<sensor_msgs::msg::BatteryState>(
            "/battery_state", 10,
            [this](sensor_msgs::msg::BatteryState::SharedPtr msg) {
                current_.battery_voltage = msg->voltage;
                current_.battery_percent = msg->percentage * 100.0f;
            });

        twist_sub_ = create_subscription<geometry_msgs::msg::Twist>(
            "/cmd_vel", 10,
            [this](geometry_msgs::msg::Twist::SharedPtr msg) {
                current_.linear_vel = msg->linear.x;
                current_.angular_vel = msg->angular.z;
            });

        status_sub_ = create_subscription<std_msgs::msg::String>(
            "/robot/status_text", 10,
            [this](std_msgs::msg::String::SharedPtr msg) {
                set_line(current_.line1, msg->data);
            });

        timer_ = create_wall_timer(
            std::chrono::milliseconds(100),
            std::bind(&DisplayBridgeNode::on_timer, this));
    }

private:
    void set_line(char (&dst)[20], const std::string& src) {
        std::memset(dst, ' ', sizeof(dst));
        std::memcpy(dst, src.data(), std::min(src.size(), sizeof(dst)));
    }

    void on_timer() {
        current_.timestamp_ns = now_ns();
        current_.seq += 1;

        auto tmp_path = shm_dir_ + "/" + tmp_name_;
        auto final_path = shm_dir_ + "/" + final_name_;

        {
            std::ofstream f(tmp_path, std::ios::binary | std::ios::trunc);
            if (!f) { RCLCPP_WARN(get_logger(), "failed to open tmp file"); return; }
            f.write(reinterpret_cast<const char*>(&current_), sizeof(current_));
        }
        // 同一tmpfs内であればアトミックにリネームされる
        if (std::rename(tmp_path.c_str(), final_path.c_str()) != 0) {
            RCLCPP_WARN(get_logger(), "rename failed");
        }
    }

    static uint64_t now_ns() {
        auto now = std::chrono::system_clock::now();
        return std::chrono::duration_cast<std::chrono::nanoseconds>(
            now.time_since_epoch()).count();
    }

    std::string shm_dir_, tmp_name_, final_name_;
    DisplayFrame current_;
    rclcpp::Subscription<sensor_msgs::msg::BatteryState>::SharedPtr battery_sub_;
    rclcpp::Subscription<geometry_msgs::msg::Twist>::SharedPtr twist_sub_;
    rclcpp::Subscription<std_msgs::msg::String>::SharedPtr status_sub_;
    rclcpp::TimerBase::SharedPtr timer_;
};

int main(int argc, char** argv) {
    rclcpp::init(argc, argv);
    rclcpp::spin(std::make_shared<DisplayBridgeNode>());
    rclcpp::shutdown();
    return 0;
}
```

### 3.3 Python実装例(rclpy、代替案)

```python
import os
import struct
import tempfile
import rclpy
from rclpy.node import Node
from std_msgs.msg import String
from sensor_msgs.msg import BatteryState
from geometry_msgs.msg import Twist
import time

FRAME_FMT = "<IHH QQ B3x ffff 20s20s I"
FRAME_SIZE = struct.calcsize(FRAME_FMT)
MAGIC = 0x44495330

class DisplayBridgeNode(Node):
    def __init__(self):
        super().__init__('display_bridge_node')
        self.shm_dir = '/disp-shm'
        os.makedirs(self.shm_dir, exist_ok=True)
        self.final_path = os.path.join(self.shm_dir, 'display_latest.bin')

        self.robot_state = 0
        self.battery_voltage = 0.0
        self.battery_percent = 0.0
        self.linear_vel = 0.0
        self.angular_vel = 0.0
        self.line1 = b''
        self.line2 = b''
        self.status_flags = 0
        self.seq = 0

        self.create_subscription(BatteryState, '/battery_state', self.on_battery, 10)
        self.create_subscription(Twist, '/cmd_vel', self.on_twist, 10)
        self.create_subscription(String, '/robot/status_text', self.on_status_text, 10)
        self.timer = self.create_timer(0.1, self.on_timer)

    def on_battery(self, msg):
        self.battery_voltage = msg.voltage
        self.battery_percent = msg.percentage * 100.0

    def on_twist(self, msg):
        self.linear_vel = msg.linear.x
        self.angular_vel = msg.angular.z

    def on_status_text(self, msg):
        self.line1 = msg.data.encode('ascii', errors='replace')[:20].ljust(20, b' ')

    def on_timer(self):
        self.seq += 1
        ts_ns = int(time.time() * 1e9)

        data = struct.pack(
            FRAME_FMT,
            MAGIC, 1, 0,
            ts_ns, self.seq,
            self.robot_state,
            self.battery_voltage, self.battery_percent,
            self.linear_vel, self.angular_vel,
            self.line1.ljust(20, b' ')[:20],
            self.line2.ljust(20, b' ')[:20],
            self.status_flags,
        )

        # 同一ディレクトリ内で作成してからrenameすることでアトミック性を確保
        fd, tmp_path = tempfile.mkstemp(dir=self.shm_dir, prefix='.display_latest_')
        try:
            with os.fdopen(fd, 'wb') as f:
                f.write(data)
            os.rename(tmp_path, self.final_path)
        except Exception:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)
            raise

def main():
    rclpy.init()
    node = DisplayBridgeNode()
    rclpy.spin(node)
    rclpy.shutdown()

if __name__ == '__main__':
    main()
```

**ポイント(コンテナ側共通)**:
- `tempfile.mkstemp(dir=shm_dir, ...)` / C++の`std::ofstream`+`rename`のいずれも、**必ず`final_path`と同一ディレクトリ内に一時ファイルを作る**ことがアトミック性の前提条件(3.2/3.3節共通の注意点)
- 文字列フィールドは固定長にパディングし、null終端に依存しない(ホスト側で長さ固定として扱えるようにする)

---

## 4. Docker側 bind mount 設定

```yaml
# docker-compose.yml
services:
  ros2-display-bridge:
    build: .
    volumes:
      - /run/disp-shm:/disp-shm:rw   # コンテナ側が書き込み元のため rw
```

- 前回のIMU構成では`:ro`だったが、今回はコンテナ側が書き込み元なので`:rw`とする
- ホスト側は開く際に`OpenOptions::new().read(true)`(書き込みフラグなし)としてオープンし、読み取り専用アクセスに徹する。ファイルシステムレベルのアクセス制御(パーミッション)まで厳密にしたい場合は、ホスト側プロセスのユーザーに書き込み権限を与えない設計にすることも検討可(運用上の複雑さと要件のバランスで判断)

> **本リポジトリでの統合状況**: 上記 bind mount は既に `docker-compose.yml`（方式A）/
> `docker-compose.prod.yml`（方式B）へ `-/run/disp-shm:/disp-shm:rw` として追加済み。
> コンテナ側 ROS2 ノードの雛形は `src/display_bridge`(Python) / `src/display_bridge_cpp`(C++)。

### 4.1 `/run/disp-shm` の所有者（uid 1000）設定 —— 必須

`/run` は **root 所有の tmpfs**（再起動で消える）。何もしないと `/run/disp-shm` は存在しないか
root 所有になる。コンテナ内 `ros2_user` は **uid 1000**、実機ホストの `demitas` も **uid 1000** なので、
`/run/disp-shm` を **uid 1000 所有**にしておけば、コンテナ側 `display_bridge`（書込み）とホスト側
`disp-writer`（読取り）の双方がアクセスできる。

> **Docker の落とし穴**: bind mount 先が起動時に存在しないと Docker が **root:root で自動作成**し、
> コンテナ内 uid 1000 が書けなくなる。**コンテナ起動前に uid 1000 所有で用意**しておくのが要点。

**手順1（一回だけ・検証用）**:

```bash
# 実機(Pi)で。demitas は passwordless sudo 可。-o 1000 はコンテナ ros2_user の uid に合わせる
sudo install -d -o 1000 -g 1000 -m 0775 /run/disp-shm
ls -lnd /run/disp-shm     # → drwxrwxr-x ... 1000 1000
```

`host/scripts/run_disp_writer.sh` は起動時にこれと同等の処理をするため、disp-writer を先に起動する
運用ならこの手順は自動で済む。

**手順2（再起動後も自動・推奨。systemd-tmpfiles）**: `/run` 配下は tmpfiles.d で管理するのが定石。

```bash
echo 'd /run/disp-shm 0775 1000 1000 -' | sudo tee /etc/tmpfiles.d/disp-shm.conf
sudo systemd-tmpfiles --create /etc/tmpfiles.d/disp-shm.conf   # 即時反映（再起動を待たず作成）
ls -lnd /run/disp-shm
```

これで reboot 後も 1000:1000 で自動生成され、コンテナ／ホストどちらが先に起動しても書き込める。

**確認**:

```bash
ls -lnd /run/disp-shm                      # 1000 1000 / drwxrwxr-x
docker exec rpi4_ros2_system_bobtail_dev bash -lc 'touch /disp-shm/.w && echo OK && rm /disp-shm/.w'
```

---

## 5. ホスト側実装(Rust)

### 5.1 依存クレート

```toml
[package]
name = "disp-writer"
version = "0.1.0"
edition = "2021"

[dependencies]
rppal = { version = "0.19", features = ["embedded-hal"] } # embedded-hal traitを実装(I2C/SPI共通)。新規は最新0.22系の互換確認を推奨
ssd1306 = "0.9"
embedded-graphics = "0.8"
display-interface-spi = "0.5"  # SPI接続時のみ使用(display_interfaceのSPI実装)
serde = { version = "1", features = ["derive"] }
toml = "0.8"
anyhow = "1"
bytemuck = { version = "1", features = ["derive"] }
```

`rppal`は`embedded-hal`featureを有効にすることで、`rppal::i2c::I2c`・`rppal::spi::Spi`・`rppal::gpio::OutputPin`のいずれも`embedded-hal`traitを実装した状態で使えるようになる。これにより`ssd1306`クレート(embedded-hal 1.0系ベース)にI2C/SPIどちらの場合もそのまま渡せるため、`linux-embedded-hal`のような追加の変換レイヤーは不要。

### 5.2 接続方式の選択とアブストラクション設計

I2C/SPIをコンフィグで切り替え可能にするため、`ssd1306`クレートが要求する`WriteOnlyDataCommand`traitを実装する型を実行時に切り替えられるようにする。Rustでは静的ディスパッチ(ジェネリクス)だと型がコンパイル時に確定してしまい実行時切り替えができないため、**`Box<dyn WriteOnlyDataCommand>`による動的ディスパッチ**、または**enumで両方を保持し呼び出し側でmatchする**方式のいずれかを採る。今回は実装のシンプルさを優先し、後者(enum方式)を採用する。

```rust
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use ssd1306::mode::BufferedGraphicsMode;
use rppal::i2c::I2c;
use rppal::spi::Spi;
use rppal::gpio::OutputPin;
use display_interface_spi::SPIInterface;

pub enum DisplayHandle {
    I2c(Ssd1306<display_interface_i2c::I2CInterface<I2c>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>),
    Spi(Ssd1306<SPIInterface<Spi, OutputPin>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>),
}

impl DisplayHandle {
    pub fn clear(&mut self) {
        match self {
            DisplayHandle::I2c(d) => { d.clear(BinaryColor::Off).ok(); }
            DisplayHandle::Spi(d) => { d.clear(BinaryColor::Off).ok(); }
        }
    }
    pub fn draw_text(&mut self, text: &str, pos: Point, style: MonoTextStyle<BinaryColor>) {
        match self {
            DisplayHandle::I2c(d) => { Text::new(text, pos, style).draw(d).ok(); }
            DisplayHandle::Spi(d) => { Text::new(text, pos, style).draw(d).ok(); }
        }
    }
    pub fn flush(&mut self) -> anyhow::Result<()> {
        match self {
            DisplayHandle::I2c(d) => d.flush().map_err(|e| anyhow::anyhow!("{e:?}")),
            DisplayHandle::Spi(d) => d.flush().map_err(|e| anyhow::anyhow!("{e:?}")),
        }
    }
}
```

`render_frame`関数(5.7節)はこの`DisplayHandle`を受け取るように書けば、**描画ロジック自体はI2C/SPIどちらでも無変更で共通利用できる**。このenumラッパーが「差分を物理接続層のみに閉じ込める」設計の要となる。

### 5.3 コンフィグファイル(TOML、interfaceフィールド追加)

```toml
# /etc/disp-writer/config.toml
[display]
interface = "i2c"        # "i2c" または "spi"
width = 128
height = 64
rotation = "Rotate0"      # Rotate0/90/180/270

[i2c]                      # interface = "i2c" の場合に使用
bus = 1
address = 0x3C            # SSD1306デフォルト(0x3Dの個体もあるため要確認)
baudrate_hz = 400000       # Fast Mode(フルフレーム書き込みの時間短縮のため推奨)

[spi]                      # interface = "spi" の場合に使用
bus = 0
slave_select = 0           # CE0=0, CE1=1
clock_hz = 8000000         # 8MHz(モジュール仕様により20MHz程度まで対応可能な場合あり)
dc_gpio = 25                # Data/Commandピン
reset_gpio = 24              # Resetピン

[timing]
poll_interval_ms = 20       # tmpfsのポーリング間隔

[input]
shm_dir = "/run/disp-shm"
filename = "display_latest.bin"
```

`[i2c]`/`[spi]`両方の設定項目をコンフィグに残しておき、`interface`フィールドで使用する方を選択する形とすることで、**同じバイナリ・同じコンフィグファイル形式のまま配線だけ変更して切り替えられる**運用上のメリットがある。

### 5.4 初期化処理(interfaceによる分岐)

```rust
use rppal::i2c::I2c;
use rppal::spi::{Bus, Mode, SlaveSelect, Spi};
use rppal::gpio::Gpio;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use display_interface_spi::SPIInterface;

fn init_display(config: &Config) -> anyhow::Result<DisplayHandle> {
    match config.display.interface.as_str() {
        "i2c" => {
            let mut i2c = I2c::with_bus(config.i2c.bus)?;
            i2c.set_slave_address(config.i2c.address)?;
            let interface = I2CDisplayInterface::new(i2c);
            let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
                .into_buffered_graphics_mode();
            display.init().map_err(|e| anyhow::anyhow!("display init failed: {e:?}"))?;
            Ok(DisplayHandle::I2c(display))
        }
        "spi" => {
            let spi = Spi::new(
                Bus::try_from(config.spi.bus)?,
                SlaveSelect::try_from(config.spi.slave_select)?,
                config.spi.clock_hz,
                Mode::Mode0,
            )?;
            let gpio = Gpio::new()?;
            let dc = gpio.get(config.spi.dc_gpio)?.into_output();
            let mut rst = gpio.get(config.spi.reset_gpio)?.into_output();

            rst.set_low();
            std::thread::sleep(std::time::Duration::from_millis(10));
            rst.set_high();

            let interface = SPIInterface::new(spi, dc);
            let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
                .into_buffered_graphics_mode();
            display.init().map_err(|e| anyhow::anyhow!("display init failed: {e:?}"))?;
            Ok(DisplayHandle::Spi(display))
        }
        other => anyhow::bail!("unknown interface: {other}"),
    }
}
```

### 5.5 メインループ(共通)

```rust
use embedded_graphics::{
    mono_font::{ascii::FONT_7X13, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};

fn main() -> anyhow::Result<()> {
    let config = load_config("/etc/disp-writer/config.toml")?;
    let mut display = init_display(&config)?; // interfaceに応じてI2C/SPIどちらかを初期化(5.4節)

    let file_path = std::path::Path::new(&config.input.shm_dir).join(&config.input.filename);
    let mut last_seq: Option<u64> = None;
    let interval = std::time::Duration::from_millis(config.timing.poll_interval_ms);

    loop {
        let cycle_start = std::time::Instant::now();

        match read_latest_frame(&file_path) {
            Ok(Some(frame)) if Some(frame.seq) != last_seq => {
                last_seq = Some(frame.seq);
                render_frame(&mut display, &frame)?;
            }
            Ok(_) => {
                // 新規データなし、または未取得(ファイル未存在等)→今回は描画スキップ
            }
            Err(e) => {
                eprintln!("frame read error: {e}");
            }
        }

        let elapsed = cycle_start.elapsed();
        if elapsed < interval {
            std::thread::sleep(interval - elapsed);
        }
    }
}
```

**この`main`関数はI2C/SPIどちらの接続方式でも変更不要**であり、`init_display`が返す`DisplayHandle`(5.2節のenum)によって差分が吸収されている点が本設計のポイント。

### 5.6 tmpfsからの読み取り(方式Aの読み取り側)

```rust
use std::fs;
use std::path::Path;

fn read_latest_frame(path: &Path) -> anyhow::Result<Option<DisplayFrame>> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    if bytes.len() != std::mem::size_of::<DisplayFrame>() {
        // renameの直後で読み取りタイミングが極端にズレた場合のガード
        // (通常方式Aではここに来ないはずだが、防御的に実装)
        return Ok(None);
    }

    let frame: DisplayFrame = *bytemuck::from_bytes(&bytes);
    if frame.magic != 0x44495330 {
        return Ok(None);
    }
    Ok(Some(frame))
}
```

`fs::read`はファイル全体を一度に読み込むため、方式A(rename後の完全なファイル)であればサイズ不一致は基本的に発生しない。念のためサイズチェック・マジックナンバーチェックを両方入れておくことで、想定外のタイミング(コンテナ再起動直後の不完全ファイル等)への耐性を持たせる。

### 5.7 描画ロジック(共通)

```rust
fn render_frame(display: &mut DisplayHandle, frame: &DisplayFrame) -> anyhow::Result<()> {
    display.clear();

    let style = MonoTextStyle::new(&FONT_7X13, BinaryColor::On);

    let state_str = match frame.robot_state {
        0 => "IDLE",
        1 => "RUNNING",
        2 => "ERROR",
        3 => "CHARGING",
        _ => "UNKNOWN",
    };
    display.draw_text(state_str, Point::new(0, 12), style);

    let batt_str = format!("Batt: {:.1}V {:.0}%", frame.battery_voltage, frame.battery_percent);
    display.draw_text(&batt_str, Point::new(0, 28), style);

    let vel_str = format!("v={:.2} w={:.2}", frame.linear_vel, frame.angular_vel);
    display.draw_text(&vel_str, Point::new(0, 44), style);

    let line1 = std::str::from_utf8(&frame.line1).unwrap_or("").trim_end();
    display.draw_text(line1, Point::new(0, 60), style);

    display.flush()?;
    Ok(())
}
```

- `display.flush()`が実際のI2C/SPI転送(フルフレームバッファ送信)を行う箇所。ここが本構成における主要な転送コストとなる(次節参照)。**5.2節のenumラッパー(`DisplayHandle`)によって、この関数はI2C/SPIどちらの接続方式でも無変更**である

### 5.8 転送コストの試算とI2C/SPI比較

128x64モノクロの全画面フレームバッファは `128 × 64 / 8 = 1024 bytes`。SSD1306へのフルフレーム転送はこのバイト数(+コマンドバイト)を送出する。

| 接続方式・クロック | 概算転送時間(1024byte) |
|---|---|
| I2C 100kHz (Standard Mode) | 約 90〜100 ms |
| I2C 400kHz (Fast Mode) | 約 23〜25 ms |
| SPI 8MHz | 約 1 ms |
| SPI 20MHz(モジュールが対応する場合の上限帯) | 約 0.4 ms |

**I2Cを使う場合は必ず400kHz(Fast Mode)へ設定すること**を強く推奨する(100kHzのままだと100ms間隔の要件に対してほぼ時間いっぱいまで転送に食われてしまう)。SSD1306は400kHzまで対応しているモジュールが一般的だが、個体差があるため実機での確認が必要。

`/boot/firmware/config.txt`(I2C使用時。Pi OS の `/boot/config.txt` ではなく実機は firmware 配下):
```
dtparam=i2c_arm_baudrate=400000
```

**さらなる最適化(将来検討)**: `ssd1306`クレートの`BufferedGraphicsMode`は変更のあった箇所のみを再送する「dirty region」機能を内部的に持つ場合があり(バージョンによる)、テキストの一部だけが変わるようなケースではフルフレーム転送よりも高速化できる可能性がある。今回の要件(100ms)には両方式とも十分収まるため、本計画では標準のフル転送で問題ないと判断するが、将来的により短い間隔が必要になった場合の検討候補として記載する。

### 5.9 SPI接続時の配線

| 信号 | 役割 | Pi4/Pi5側 |
|---|---|---|
| SCK (SCLK) | クロック | SPI0 SCLK (GPIO11) |
| MOSI | データ | SPI0 MOSI (GPIO10) |
| CS | チップセレクト | SPI0 CE0 (GPIO8)、コンフィグの`slave_select`に対応 |
| DC (D/C) | データ/コマンド切り替え | 任意のGPIO(コンフィグの`spi.dc_gpio`) |
| RES (RST) | リセット | 任意のGPIO(コンフィグの`spi.reset_gpio`) |
| VCC/GND | 電源 | 3.3V, GND |

I2Cと異なり、**DCピンとRESピンの2本を追加のGPIOで制御する必要がある**(MISOはSSD1306側が応答を返さないため未使用)。SPI対応のためにはSSD1306モジュール自体がSPIモードに対応している必要があり(基板上のジャンパ/抵抗実装で切り替え可能な製品が多い)、購入前にデータシートで確認する。

### 5.10 用途に応じた使い分けの指針

| 観点 | I2C | SPI |
|---|---|---|
| 配線本数 | 少ない(SDA/SCL 2本 + 電源) | 多い(SCK/MOSI/CS/DC/RES 5本 + 電源) |
| 使用ピン | I2Cバス1本で複数デバイスと共有可能(アドレスが異なれば同一バスに複数接続可) | SPIバスは共有可能だがCSが個体ごとに必要、DC/RESは個体ごとに専用GPIOが必要 |
| フルフレーム転送速度 | 400kHzで約23〜25ms | 8MHzで約1ms、20MHzで約0.4ms |
| GPIO消費 | 追加GPIO不要 | DC/RESで2本消費(複数台接続時はさらに増加) |
| 適したケース | 更新間隔が数十ms以上で十分、他のI2Cデバイス(IMU等)と配線をまとめたい、GPIOを他用途で使い切っている | 高フレームレートが必要、アニメーション的な表示更新をしたい、GPIOに余裕がある |

**本計画(100ms間隔、最新値のみ)の場合**: I2C(400kHz)でも転送時間(約23ms)は100msの予算内に十分収まるため、**配線本数が少なく済むI2Cを標準構成として推奨**する。一方、以下のような場合はSPIへの切り替えを検討する。

- 将来的に更新間隔を数十ms以下まで短縮したい場合(フレームレート向上の要件がある場合)
- 同一I2Cバスに他のデバイス(MPU6050等)を多数接続しており、バス帯域やアドレス競合が懸念される場合
- 描画内容がグラフ・アニメーション等、頻繁な全画面更新を伴う場合

コンフィグの`interface`フィールドを`"i2c"`から`"spi"`に変更し、配線を切り替えるだけで移行できるため、**まずI2Cで構築し、必要になった時点でSPIへ切り替える**という段階的なアプローチが取れる設計になっている。

---

## 6. systemdサービス化(ホスト側)

```ini
# /etc/systemd/system/disp-writer.service
[Unit]
Description=SSD1306 Display Writer
After=local-fs.target

[Service]
Type=simple
ExecStart=/usr/local/bin/disp-writer
Restart=on-failure
RestartSec=1
User=demitas
SupplementaryGroups=i2c spi gpio
# 注: /run は root 所有のため、User=demitas のままだと下記 mkdir は失敗する。
#     '+' 付きは User 指定に関わらず root で実行される。§4.1 の tmpfiles.d を使うなら本行は不要。
ExecStartPre=+/usr/bin/install -d -o 1000 -g 1000 -m 0775 /run/disp-shm

[Install]
WantedBy=multi-user.target
```

`/run/disp-shm`のディレクトリ自体は、コンテナ側の書き込みで自動生成される想定だが(3.2/3.3節で`create_directories`/`os.makedirs`実施済み)、ホスト側プロセスの起動タイミングがコンテナより先行する場合を考慮し、ホスト側でも作成しておく(どちらが先に起動しても問題ないようにする防御的設計)。ただし **`/run` は root 所有の tmpfs** なので、`User=demitas` で走る `ExecStartPre` から素の `mkdir` はできない —— `+` 付き（root 実行）で `install -d -o 1000` として所有者ごと作るか、より疎結合な **§4.1 の systemd-tmpfiles(`/etc/tmpfiles.d/disp-shm.conf`)** で用意して `ExecStartPre` を省く。uid 1000 はコンテナ `ros2_user` と実機 `demitas` に一致させるための値。`SupplementaryGroups`は`i2c`/`spi`/`gpio`をまとめて付与しておくことで、コンフィグの`interface`切り替えだけで再起動すれば動作する状態にしておく(実行ユーザーの権限起因のトラブルを避ける)。

---

## 7. 動作確認・検証計画

### 7.1 単体確認(ホスト側のみ)

1. tmpfs上に手動で`DisplayFrame`構造体相当のバイナリファイルを`dd`やRustの簡易スクリプトで書き込み、`disp-writer`が正しく描画するか確認(I2C/SPI両方の`interface`設定で実施)
2. **I2C使用時**: `i2cdetect -y 1`でSSD1306(0x3Cまたは0x3D)が認識されているか事前確認（§1.3 の I2C 有効化・`/dev/i2c-1` 出現・`i2c-tools` 導入が前提）。`i2c_arm_baudrate=400000`設定後、実際の転送時間を`Instant`計測で確認(90〜100msかかっていないことの確認)
3. **SPI使用時**: `ls /dev/spidev0.*`でSPIデバイスファイルが存在するか確認（§1.3 の `dtparam=spi=on` 有効化・再起動が前提。既定では存在しない）。DC/RESピンの配線・GPIO番号がコンフィグと一致しているか確認。転送時間が数ms以内であることを`Instant`計測で確認

### 7.2 結合確認

1. `docker-compose up`でROS2ノードを起動、ダミーの`/battery_state`や`/cmd_vel`をパブリッシュして、OLED表示が追従するか確認
2. `seq`の重複検知が正しく機能し、同一フレームに対して再描画(=不要な転送)が発生していないことを確認
3. コンテナを停止した状態でホスト側が「最後の表示のまま固まる」ことを確認(要件上は問題ないが、意図した挙動であることの確認として)
4. `interface`設定を`"i2c"`↔`"spi"`で切り替えて再起動し、配線を対応するものに変更するだけで同じ動作になることを確認(アブストラクション設計の検証)

### 7.3 異常系確認

1. コンテナ起動前にホスト側`disp-writer`を先に起動した場合、ファイル未存在状態でクラッシュしないこと
2. 通信線切断・SSD1306の電源断状態での`disp-writer`の挙動(エラーログを出しつつプロセス継続すること、I2C/SPI両方で確認)
3. 不完全なフレーム(サイズ不一致・マジックナンバー不一致)を受け取った場合に描画をスキップし、直前の表示を保持し続けること

### 7.4 鮮度チェック(オプション拡張、要件外だが検討候補)

`timestamp_ns`を用いて「最後の更新から一定時間(例: 1秒)以上経過していればコンテナ側が停止している可能性がある」と判断し、画面に警告表示(例: "NO DATA"オーバーレイ)を出す設計も可能。今回の要件では「最新値のみ取り込めれば良い」ため実装は必須としないが、実運用の安全性を高める拡張として検討候補に残す。

---

## 8. 前回(IMU)構成との実装再利用ポイント

- write-tmp → rename のアトミック書き込みロジックは、書き込み元がRust(前回)かC++/Python(今回)かに関わらず**同一の原理**。コンテナ側言語が変わっても設計原則は共通化できる
- `bytemuck`を用いた固定長バイナリ変換パターンはホスト側Rustコードとして共通化しやすく、`ImuFrame`/`DisplayFrame`のような複数のフレーム型を扱う共通クレート(例: `shm-frames`)として切り出すことも将来的に検討可能
- systemdサービス化・tmpfsディレクトリ管理の運用パターンも共通

---

## 9. 実装タスク一覧(チェックリスト)

> 実装状況（本リポジトリ `host/rust/bin/disp-writer/`。I2C 版・ホスト側単体を先行実装）:
> ホスト側 disp-writer（I2C）と検証用フレーム生成器 `host/scripts/gen_display_frame.py` を実装済み。
> コンテナ側 ROS2 ブリッジ・SPI 版・systemd 常駐化は次フェーズ。

- [x] `DisplayFrame`構造体の最終フィールド確定(表示したい情報の洗い出し) — §2.1 の 88B で確定、`host/rust/bin/disp-writer/src/frame.rs`
- [ ] コンテナ側 ROS2ノード実装(C++ or Python選定) — 次フェーズ（`bobtail_display_bridge`）
- [ ] write-tmp → rename 書き込みロジック実装(コンテナ側) — 次フェーズ（検証用に `gen_display_frame.py` で先行代替）
- [ ] Dockerfile作成、`:rw`でのbind mount設定確認 — 次フェーズ
- [x] `DisplayHandle` enumによるI2C/SPIアブストラクション設計の実装(5.2節) — I2C 単一 variant で実装（`display.rs`。SPI は seam のみ）
- [x] ホスト側 `disp-writer` Rustプロジェクト作成、`rppal`(embedded-hal feature)でI2C初期化確認
- [x] `ssd1306` + `embedded-graphics`での描画確認(まずは固定文字列でOLED表示テスト、I2C版)
- [x] `i2c_arm_baudrate=400000`設定、フルフレーム転送時間の実測(I2C) — 設定済み、disp-writer が flush 時間をログ出力
- [ ] SPI版の配線・GPIO(DC/RES)接続、`display-interface-spi`での初期化確認 — 次フェーズ（実機未配線）
- [ ] SPI版のフルフレーム転送時間の実測、I2C版との比較 — 次フェーズ
- [ ] `interface`コンフィグ切り替えによるI2C↔SPI動作確認(7.2節) — 次フェーズ
- [x] tmpfs読み取り + `seq`による重複描画スキップロジック実装 — `main.rs`（last_seq 差分）
- [~] systemdサービスファイル作成、自動起動確認 — unit 同梱(`disp-writer.service`)。enable は未実施
- [ ] 結合テスト(7章の検証計画に基づく) — コンテナ側実装後
- [ ] (オプション)鮮度チェック・"NO DATA"表示ロジックの追加検討
