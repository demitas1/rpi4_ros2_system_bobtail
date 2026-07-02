# MPU6050 IMUデータ共有システム 実装計画書

## 1. 概要

Raspberry Pi 4 のホスト OS（実機は Debian 13 trixie / aarch64）上で I2C 接続の MPU6050(6軸IMU)を10ms間隔で読み取り、tmpfs上のファイルに最新値のみを上書き保存する。Docker上のROS2ノード(Ubuntu, C/C++ or Python)がこのファイルをボリュームマウント経由で監視し、最新値を取り込んでROS2トピックとして配信する。

### 1.1 要件サマリ

| 項目 | 内容 |
|---|---|
| 対象センサ | MPU6050(I2C, 6軸: 加速度3軸+ジャイロ3軸、温度含む) |
| 読み取り間隔 | 目標10ms。厳密な等間隔は不要(最新値の取得が優先) |
| データ保持方式 | 追記なし、上書きのみ(最新値のみ) |
| 永続化 | 不要。電源断で全ロスを許容 |
| ホスト側実装 | Rust(Pi4/Pi5上でネイティブビルド・実行、クロスコンパイル不要) |
| コンテナ側実装 | C/C++ または Python(ROS2, Ubuntuベース)、読み取り専用 |
| 共有方式 | tmpfs + bind mount |
| アトミック性 | 方式A(write-tmp → rename) |
| データ形式 | 固定長バイナリ |

### 1.2 全体構成図

```
┌─────────────────────────────────────────────────────────────┐
│ Raspberry Pi OS (Host)                                       │
│                                                                │
│  ┌──────────────┐   I2C    ┌───────────────┐                 │
│  │  MPU6050     │◄────────►│ imu-reader     │                │
│  │ (0x68, /dev  │          │ (Rust, native  │                │
│  │  /i2c-1)     │          │  binary)       │                │
│  └──────────────┘          └───────┬────────┘                │
│                                     │ write-tmp → rename       │
│                                     ▼                         │
│                          ┌────────────────────┐               │
│                          │ tmpfs               │               │
│                          │ /run/imu-shm/        │               │
│                          │   imu_latest.bin      │               │
│                          └─────────┬──────────┘               │
│                                    │ bind mount (ro)            │
└────────────────────────────────────┼──────────────────────────┘
                                     ▼
                    ┌───────────────────────────────┐
                    │ Docker Container (Ubuntu+ROS2) │
                    │                                 │
                    │  /imu-shm/imu_latest.bin (ro)  │
                    │           │                     │
                    │           ▼                     │
                    │   imu_bridge_node               │
                    │   (inotify監視 or polling)      │
                    │           │                     │
                    │           ▼                     │
                    │   ROS2 topic: /imu/data_raw     │
                    │   (sensor_msgs/Imu)             │
                    └───────────────────────────────┘
```

---

## 1.3 本リポジトリ・実機環境との対応（現行システムに合わせた更新）

本計画書は当初リポジトリの知識なしに作成したため、実機・本リポの実態に合わせて以下を読み替える。

- **実行ユーザー / OS**: 「Raspberry Pi OS / `pi`」ではなく、実機は **Debian 13 (trixie) / aarch64**、
  ユーザーは **`demitas`**（passwordless sudo、`gpio`/`i2c`/`spi` グループ所属のため I2C 実行に sudo 不要）。
  以降の `User=pi` / `uid=pi` は **`demitas`** と読み替える。
- **ホスト側 Rust の置き場所**: 単独プロジェクトではなく本リポの **`host/rust/bin/imu-reader/`**
  （`host/rust` の仮想ワークスペースのメンバー）に置く。ビルド/デプロイは
  `host/scripts/deploy_and_build.sh`（dev → `rpi4-wifi:~/host/` に rsync し実機ビルド）を使う。
  詳細は [`../host/README.md`](../host/README.md)。
- **コンテナ側 ROS2 ノード**: ベースイメージは **ROS2 Jazzy（Ubuntu 24.04）** `ghcr.io/demitas1/ros2_jazzy`。
  ブリッジノード（`imu_bridge_node`）は本リポの **`src/` 配下の ament パッケージ**（例 `bobtail_imu_bridge`）
  として実装し、bind mount は既存の `docker-compose.yml` / `docker-compose.prod.yml` に統合する
  （§4 のスタンドアロン compose は概念例）。
- **I2C の有効化（必須・前提）**: 実機は既定で GPIO ヘッダの I2C が無効で **`/dev/i2c-1` が存在しない**
  （`i2cdetect` 用の `i2c-tools` も未導入）。次を実施してから本計画を進める:

  ```bash
  # /boot/firmware/config.txt（Pi OS の /boot/config.txt ではない）で I2C を有効化
  sudo sed -i 's/^#\?dtparam=i2c_arm=.*/dtparam=i2c_arm=on/' /boot/firmware/config.txt
  echo 'dtparam=i2c_arm_baudrate=400000' | sudo tee -a /boot/firmware/config.txt  # 任意（Fast Mode）
  sudo apt-get install -y i2c-tools
  sudo reboot                     # 再起動後に /dev/i2c-1 が出現
  # 確認: ls /dev/i2c-1 && i2cdetect -y 1   （MPU6050 は 0x68）
  ```

- **rppal のバージョン**: 本文の `0.19` でも動作（LED example で確認済み）。現行の最新は `0.22` 系のため、
  新規実装では最新安定版を確認して固定する。

## 2. ホスト側実装(Rust)

### 2.1 依存クレート

```toml
[package]
name = "imu-reader"
version = "0.1.0"
edition = "2021"

[dependencies]
rppal = "0.19"        # I2Cアクセス（0.19で動作確認済み。新規は最新0.22系の確認を推奨）
serde = { version = "1", features = ["derive"] }
toml = "0.8"
anyhow = "1"
bytemuck = { version = "1", features = ["derive"] }
ctrlc = "3"
```

ログ機能自体は今回「最新値の共有」が主目的でファイル追記ログは不要のため、`flexi_logger`等は必須としない。デバッグ用の標準エラー出力程度で十分。

### 2.2 コンフィグファイル(TOML)

```toml
# /etc/imu-reader/config.toml
[i2c]
bus = 1               # /dev/i2c-1
address = 0x68        # MPU6050デフォルトアドレス(AD0=Low)
baudrate_hz = 400000   # Fast Mode

[sensor]
accel_range = "±4g"    # ±2/4/8/16g のいずれか(内部でレジスタ値に変換)
gyro_range = "±500dps" # ±250/500/1000/2000dps
dlpf_bandwidth_hz = 94  # Digital Low Pass Filter設定

[timing]
interval_ms = 10

[output]
shm_dir = "/run/imu-shm"
filename = "imu_latest.bin"
tmp_filename = ".imu_latest.tmp"
```

### 2.3 データフォーマット(固定長バイナリ)

読み取り側(C/C++・Python双方)でのゼロコピー/低コスト解釈を優先し、パディングを意識した`#[repr(C)]`構造体とする。

```rust
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ImuFrame {
    pub magic: u32,        // フォーマット識別用マジックナンバー (例: 0x494D5530 "IMU0")
    pub version: u16,      // フォーマットバージョン
    pub _reserved: u16,    // アライメント用パディング
    pub timestamp_ns: u64, // CLOCK_MONOTONIC or CLOCK_REALTIME (要検討、後述)
    pub seq: u64,          // シーケンス番号(欠損検知用、単純インクリメント)
    pub accel: [f32; 3],   // m/s^2, [x, y, z]
    pub gyro: [f32; 3],    // rad/s, [x, y, z]
    pub temp_c: f32,       // 摂氏
    pub status: u32,       // ビットフラグ(0=正常、bit0=I2Cエラー時の最終正常値継続、等)
}
// 合計サイズ: 4+2+2+8+8+12+12+4+4 = 56 bytes
```

**タイムスタンプについて**: コンテナ側もホストとクロックが共有される(Dockerはデフォルトでホストと同一のシステムクロックを共有)ため、`CLOCK_REALTIME`(UNIXエポックのナノ秒)を採用する。`CLOCK_MONOTONIC`はプロセス間・コンテナ間での意味が保証されないため今回は避ける。

```rust
let timestamp_ns = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)?
    .as_nanos() as u64;
```

### 2.4 MPU6050 読み取りシーケンス

初期化(起動時1回のみ):

| ステップ | レジスタ | 値 | 内容 |
|---|---|---|---|
| 1 | PWR_MGMT_1 (0x6B) | 0x00 | スリープ解除、内部8MHzオシレータ |
| 2 | SMPLRT_DIV (0x19) | 用途に応じて | サンプルレート分周(DLPF有効時のベースは1kHz) |
| 3 | CONFIG (0x1A) | DLPF設定値 | ローパスフィルタ帯域 |
| 4 | GYRO_CONFIG (0x1B) | レンジ設定 | ジャイロフルスケール |
| 5 | ACCEL_CONFIG (0x1C) | レンジ設定 | 加速度フルスケール |

メインループ(10msごと):

1. `ACCEL_XOUT_H`(0x3B)から14バイトをバースト読み取り(加速度6byte + 温度2byte + ジャイロ6byte、連続レジスタ)
2. 生値(int16, big-endian)をレンジ設定に応じたスケールファクタで物理量へ変換
3. `ImuFrame`構造体を構築、`seq`をインクリメント
4. tmpfsへ write-tmp → rename で書き込み(2.5節)
5. 次サイクルまでスリープ

```rust
use rppal::i2c::I2c;

fn read_raw(i2c: &mut I2c) -> anyhow::Result<[u8; 14]> {
    let mut buf = [0u8; 14];
    i2c.block_read(0x3B, &mut buf)?;
    Ok(buf)
}

fn to_frame(raw: &[u8; 14], accel_scale: f32, gyro_scale: f32, seq: u64) -> ImuFrame {
    let be16 = |hi: u8, lo: u8| i16::from_be_bytes([hi, lo]);

    let ax = be16(raw[0], raw[1]) as f32 * accel_scale;
    let ay = be16(raw[2], raw[3]) as f32 * accel_scale;
    let az = be16(raw[4], raw[5]) as f32 * accel_scale;
    let temp_raw = be16(raw[6], raw[7]) as f32;
    let gx = be16(raw[8], raw[9]) as f32 * gyro_scale;
    let gy = be16(raw[10], raw[11]) as f32 * gyro_scale;
    let gz = be16(raw[12], raw[13]) as f32 * gyro_scale;

    ImuFrame {
        magic: 0x494D5530,
        version: 1,
        _reserved: 0,
        timestamp_ns: now_ns(),
        seq,
        accel: [ax, ay, az],
        gyro: [gx, gy, gz],
        temp_c: temp_raw / 340.0 + 36.53,
        status: 0,
    }
}
```

### 2.5 tmpfsへの書き込み(方式A: write-tmp → rename)

```rust
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

fn write_latest(shm_dir: &Path, tmp_name: &str, final_name: &str, frame: &ImuFrame) -> anyhow::Result<()> {
    let tmp_path = shm_dir.join(tmp_name);
    let final_path = shm_dir.join(final_name);

    let bytes: &[u8] = bytemuck::bytes_of(frame);

    {
        let mut f = File::create(&tmp_path)?;
        f.write_all(bytes)?;
        // tmpfs上のため fsync は不要(ページキャッシュ=実データそのもの)
    }
    fs::rename(&tmp_path, &final_path)?; // 同一ファイルシステム内なのでアトミック
    Ok(())
}
```

**注意点**:
- `tmp_path` と `final_path` は必ず同一ディレクトリ(同一マウントポイント)に置くこと。異なるファイルシステム間では`rename`がアトミックにならず、内部的にcopy+deleteとなる
- 起動時に`shm_dir`が存在しない場合は`fs::create_dir_all`で作成する処理を入れる(tmpfsは再起動で消えるため)

### 2.6 メインループとエラーハンドリング

```rust
fn main() -> anyhow::Result<()> {
    let config = load_config("/etc/imu-reader/config.toml")?;
    fs::create_dir_all(&config.output.shm_dir)?;

    let mut i2c = I2c::with_bus(config.i2c.bus)?;
    i2c.set_slave_address(config.i2c.address)?;
    i2c.set_timeout(5000)?; // μs単位、クロックストレッチ等の異常時にハングしない保険

    init_mpu6050(&mut i2c, &config.sensor)?;

    let interval = std::time::Duration::from_millis(config.timing.interval_ms);
    let mut seq: u64 = 0;
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    {
        let r = running.clone();
        ctrlc::set_handler(move || r.store(false, std::sync::atomic::Ordering::SeqCst))?;
    }

    while running.load(std::sync::atomic::Ordering::SeqCst) {
        let cycle_start = std::time::Instant::now();

        match read_raw(&mut i2c) {
            Ok(raw) => {
                let frame = to_frame(&raw, accel_scale, gyro_scale, seq);
                if let Err(e) = write_latest(&shm_dir, tmp_name, final_name, &frame) {
                    eprintln!("write error: {e}"); // 書き込み失敗は次サイクルへスキップ、致命停止しない
                }
                seq += 1;
            }
            Err(e) => {
                eprintln!("i2c read error: {e}"); // I2Cエラーは継続(次サイクルでリトライ)
                // 必要であれば連続エラー数をカウントし、閾値超えでMPU6050再初期化を試みる
            }
        }

        let elapsed = cycle_start.elapsed();
        if elapsed < interval {
            std::thread::sleep(interval - elapsed);
        }
        // elapsed > interval の場合はそのまま次サイクルへ(厳密な等間隔は要件外)
    }
    Ok(())
}
```

**設計方針**:
- I2C読み取り失敗・書き込み失敗のいずれも**プロセスを停止させない**(要件上、最新値の継続提供が優先のため、一時的エラーでプロセスが落ちる方が悪い)
- 連続エラーが一定回数を超えた場合のみ再初期化やプロセス再起動をsystemd側に委ねる設計とする(3章)

---

## 3. systemdサービス化(ホスト側)

```ini
# /etc/systemd/system/imu-reader.service
[Unit]
Description=MPU6050 IMU Reader
After=local-fs.target

[Service]
Type=simple
ExecStart=/usr/local/bin/imu-reader
Restart=on-failure
RestartSec=1
User=demitas
SupplementaryGroups=i2c

# tmpfsディレクトリを起動前に確実に用意
ExecStartPre=/bin/mkdir -p /run/imu-shm

[Install]
WantedBy=multi-user.target
```

`/run` は Debian(systemd) 環境で既にtmpfsとしてマウントされている（実機で確認済み）ため、独立した`/etc/fstab`エントリを追加せず`/run/imu-shm`を使うのが最も簡単(サイズ制約が気になる場合は専用tmpfsを別途マウントしても良い)。

```bash
# 専用tmpfsを切りたい場合の /etc/fstab エントリ(任意)
tmpfs /run/imu-shm tmpfs defaults,size=1M,mode=0755,uid=demitas,gid=i2c 0 0
```

---

## 4. Docker側 bind mount 設定

```yaml
# docker-compose.yml
services:
  ros2-imu-bridge:
    build: .
    volumes:
      - /run/imu-shm:/imu-shm:ro
    devices: []   # I2Cデバイス直接アクセスは不要(ホスト側で読み取り済みのため)
    network_mode: host  # ROS2のDDS通信を考慮する場合は要検討(別途設計判断)
```

- `:ro` でコンテナ側は読み取り専用マウントとし、誤ってコンテナ側から書き込む事故を防止
- ホストの`/run/imu-shm`が実体はtmpfsであるため、bind mount先でも同じメモリ領域を参照する(コピーは発生しない)

---

## 5. コンテナ側実装(ROS2ノード)

### 5.1 監視方式の選択

| 方式 | 特徴 |
|---|---|
| ポーリング(定周期read) | 実装が単純。10ms間隔なら数msごとにポーリングしてもCPU負荷は軽微 |
| inotify (`IN_MOVED_TO`) | ファイル更新イベント駆動。CPU効率は良いが、rename検知の実装がやや複雑 |

**推奨**: 今回は「最新値のみ取得できれば良く、厳密なタイミング同期は不要」という要件のため、**シンプルなポーリング方式**を基本推奨とする。ROS2ノード側のタイマーコールバック(例: 5ms周期)で単純に読み取れば十分であり、inotifyによる実装・デバッグコストをかける必要性は薄い。

高頻度化・省電力化が将来的に必要になった場合にinotify方式へ切り替える、という段階的な設計とする。

### 5.2 C++実装例(rclcpp)

```cpp
#include <rclcpp/rclcpp.hpp>
#include <sensor_msgs/msg/imu.hpp>
#include <fstream>
#include <cstring>
#include <cstdint>

#pragma pack(push, 1)
struct ImuFrame {
    uint32_t magic;
    uint16_t version;
    uint16_t reserved;
    uint64_t timestamp_ns;
    uint64_t seq;
    float accel[3];
    float gyro[3];
    float temp_c;
    uint32_t status;
};
#pragma pack(pop)
static_assert(sizeof(ImuFrame) == 56, "ImuFrame size mismatch");

class ImuBridgeNode : public rclcpp::Node {
public:
    ImuBridgeNode() : Node("imu_bridge_node") {
        pub_ = create_publisher<sensor_msgs::msg::Imu>("/imu/data_raw", 10);
        timer_ = create_wall_timer(
            std::chrono::milliseconds(5),
            std::bind(&ImuBridgeNode::on_timer, this));
        last_seq_ = 0;
    }

private:
    void on_timer() {
        std::ifstream f("/imu-shm/imu_latest.bin", std::ios::binary);
        if (!f) return; // まだファイルが存在しない(起動直後など)

        ImuFrame frame;
        f.read(reinterpret_cast<char*>(&frame), sizeof(frame));
        if (!f || f.gcount() != sizeof(frame)) return; // 読み取り不完全→今回はスキップ

        if (frame.magic != 0x494D5530) return; // フォーマット不一致

        if (frame.seq == last_seq_) return; // 新しいデータがまだ来ていない
        last_seq_ = frame.seq;

        sensor_msgs::msg::Imu msg;
        msg.header.stamp.sec = static_cast<int32_t>(frame.timestamp_ns / 1'000'000'000ULL);
        msg.header.stamp.nanosec = static_cast<uint32_t>(frame.timestamp_ns % 1'000'000'000ULL);
        msg.header.frame_id = "imu_link";
        msg.linear_acceleration.x = frame.accel[0];
        msg.linear_acceleration.y = frame.accel[1];
        msg.linear_acceleration.z = frame.accel[2];
        msg.angular_velocity.x = frame.gyro[0];
        msg.angular_velocity.y = frame.gyro[1];
        msg.angular_velocity.z = frame.gyro[2];
        // orientationは未使用(MPU6050単体ではフュージョン未実施のため単位クォータニオン)
        msg.orientation_covariance[0] = -1; // 「orientation未提供」を示す標準的な慣習

        pub_->publish(msg);
    }

    rclcpp::Publisher<sensor_msgs::msg::Imu>::SharedPtr pub_;
    rclcpp::TimerBase::SharedPtr timer_;
    uint64_t last_seq_;
};

int main(int argc, char** argv) {
    rclcpp::init(argc, argv);
    rclcpp::spin(std::make_shared<ImuBridgeNode>());
    rclcpp::shutdown();
    return 0;
}
```

**ポイント**:
- `rename`によるアトミック更新のおかげで、`ifstream`でオープンした時点のファイルは常に「完全な1フレーム」であることが保証される(方式Aの効果)
- `seq`フィールドで新規データかどうかを判定し、同じ値の再発行(=まだホスト側が更新していない)を検出して重複パブリッシュを避ける
- ファイル未存在・読み取り不完全時は**例外を出さず単純にスキップ**する設計(要件上「最新値が取れなければ待てば良い」ため)

### 5.3 Python実装例(rclpy、代替案)

```python
import struct
import rclpy
from rclpy.node import Node
from sensor_msgs.msg import Imu

FRAME_FMT = "<IHH QQ 3f 3f f I"  # magic,version,reserved,timestamp_ns,seq,accel[3],gyro[3],temp,status
FRAME_SIZE = struct.calcsize(FRAME_FMT)
MAGIC = 0x494D5530

class ImuBridgeNode(Node):
    def __init__(self):
        super().__init__('imu_bridge_node')
        self.pub = self.create_publisher(Imu, '/imu/data_raw', 10)
        self.last_seq = None
        self.timer = self.create_timer(0.005, self.on_timer)

    def on_timer(self):
        try:
            with open('/imu-shm/imu_latest.bin', 'rb') as f:
                data = f.read(FRAME_SIZE)
        except FileNotFoundError:
            return
        if len(data) != FRAME_SIZE:
            return

        magic, version, _reserved, ts_ns, seq, ax, ay, az, gx, gy, gz, temp, status = \
            struct.unpack(FRAME_FMT, data)

        if magic != MAGIC or seq == self.last_seq:
            return
        self.last_seq = seq

        msg = Imu()
        msg.header.stamp.sec = ts_ns // 1_000_000_000
        msg.header.stamp.nanosec = ts_ns % 1_000_000_000
        msg.header.frame_id = 'imu_link'
        msg.linear_acceleration.x = ax
        msg.linear_acceleration.y = ay
        msg.linear_acceleration.z = az
        msg.angular_velocity.x = gx
        msg.angular_velocity.y = gy
        msg.angular_velocity.z = gz
        msg.orientation_covariance[0] = -1.0
        self.pub.publish(msg)

def main():
    rclpy.init()
    node = ImuBridgeNode()
    rclpy.spin(node)
    rclpy.shutdown()

if __name__ == '__main__':
    main()
```

Python版は`struct`モジュールのオーバーヘッドがC++版より大きいが、10ms〜数msのポーリング間隔であれば十分に間に合う。開発速度優先ならPython版から始め、性能問題が出た場合にC++へ移行するのが現実的な進め方。

---

## 6. 動作確認・検証計画

### 6.1 単体確認(ホスト側のみ)

1. `imu-reader`をコンテナ抜きで起動し、`/run/imu-shm/imu_latest.bin`のバイナリを`xxd`等で確認
2. `seq`が10msごとにインクリメントされていることを、簡易スクリプト(バイナリを繰り返し読んでseqの差分をログ)で確認
3. I2Cバスアナライザやオシロがなくても、`i2cdetect -y 1`でMPU6050(0x68)が認識されているか事前確認（§1.3 の手順で I2C 有効化・`/dev/i2c-1` 出現・`i2c-tools` 導入が前提）

### 6.2 結合確認

1. `docker-compose up`でROS2ノードを起動、`ros2 topic hz /imu/data_raw`で実際のパブリッシュ周波数を確認
2. `ros2 topic echo /imu/data_raw`で値の妥当性確認(静止状態でaccel.z ≈ 9.8 m/s²になっているか等)
3. ホスト側プロセスを`systemctl stop imu-reader`で意図的に停止し、ROS2側が「ファイルが古いまま(seq停止)」の状態を正しく検知できるか確認(タイムスタンプの鮮度チェックロジックを追加する場合はここで検証)

### 6.3 異常系確認

1. I2Cケーブル切断状態での`imu-reader`の挙動(エラーログを出しつつクラッシュしないこと)
2. ホスト再起動直後、tmpfsファイルが存在しない状態でのROS2ノードの起動順序耐性(リトライ・待機ロジックの確認)
3. 長時間稼働時のメモリリーク・ファイルディスクリプタリークの確認(`valgrind`や`ps`での監視)

---

## 7. 将来的な拡張候補(本計画のスコープ外)

- **鮮度チェック**: `timestamp_ns`が現在時刻から一定以上古い場合、ROS2側でセンサ異常とみなしてダイアグノスティクスを発行する仕組み(現状要件では未実施)
- **inotifyベースへの切り替え**: CPU効率をさらに追求する場合
- **複数センサ対応**: 同一tmpfs内に複数の固定長ファイル(IMU、気圧センサ等)を配置し、`shm_dir`配下を規約化する
- **DMP/FIFO活用**: MPU6050内蔵のDMP機能を使ったより高頻度な内部サンプリングとの分離(要件次第で将来検討)

---

## 8. 実装タスク一覧(チェックリスト)

- [ ] ホスト側 `imu-reader` Rustプロジェクト作成、`rppal`でI2C初期化確認
- [ ] MPU6050初期化シーケンス実装(レンジ設定、DLPF設定)
- [ ] `ImuFrame`構造体定義、`bytemuck`でのバイト列変換確認
- [ ] write-tmp → rename 書き込みロジック実装
- [ ] TOMLコンフィグ読み込み実装
- [ ] systemdサービスファイル作成、自動起動確認
- [ ] Dockerfile作成(ROS2 + Ubuntuベース)、bind mount設定確認
- [ ] ROS2ノード実装(C++ or Python選定、上記いずれか)
- [ ] 結合テスト(6章の検証計画に基づく)
- [ ] ドキュメント整備(README、config.toml記述例、トラブルシューティング)
