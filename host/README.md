# host — ホスト側（コンテナ外）プログラム

Raspberry Pi 4 の**ホスト OS 上で直接**（Docker/ROS2 コンテナを介さず）動くプログラム群。
GPIO・I2C・SPI・PWM を直接操作する用途を対象とする。ROS2 の ament ワークスペース（`../src/`）
とは**意図的に分離**している。`host/` 直下に `COLCON_IGNORE` マーカーを置いてあるため
colcon は `host/` を一切走査しない（`host/cpp/CMakeLists.txt` は `project()` を持ち、これが
無いと colcon-cmake が拾ってコンテナ内ビルドが libgpiod 不在で失敗する）。

> ツールチェーン（gcc/g++/cmake/rust）の導入は [`../docs/pi4_host_toolchain_setup.md`](../docs/pi4_host_toolchain_setup.md) を参照。

## 構成

```
host/
├── rust/      Rust（rppal: GPIO/I2C/SPI/PWM を単一クレートで網羅）。仮想ワークスペース。
│   ├── examples/led_blink/   GPIO LED 点滅 example
│   └── bin/disp-writer/       SSD1306 OLED(I2C) 表示バイナリ（docs/rpi4-ssd1306_display__plan.md）
├── cpp/       C++（libgpiod v2 の C API）。トップレベル CMake で束ねる。
├── python/    Python（gpiod v2 = python3-libgpiod）。
└── scripts/   実機への定常デプロイ／実行スクリプト（下記）
```

当面の主対象は Rust による実装（[`../docs/rpi4-mpu6050_imu_bridge_plan.md`](../docs/rpi4-mpu6050_imu_bridge_plan.md) /
[`../docs/rpi4-ssd1306_display__plan.md`](../docs/rpi4-ssd1306_display__plan.md)）。まずは疎通確認用の
GPIO LED 点滅 example を 3 言語で用意している。

## 前提（実機）

- GPIO は `/dev/gpiochip0`（`pinctrl-bcm2711`）をキャラクタデバイスで操作する（旧 sysfs は使わない）。
- 実行ユーザーが `gpio` グループ（I2C/SPI 使用時は `i2c`/`spi`）に所属していれば **sudo 不要**。
- C++ のビルドには `libgpiod-dev` が必要: `sudo apt-get install -y libgpiod-dev`。
- Python は `python3-libgpiod`（`import gpiod` が v2）を使用。標準で導入済みのことが多い。

## 実機への定常デプロイ（scripts/）

開発機（dev）から Pi へ `host/` を rsync してビルドし、ビルド済み example を実行する定型作業を
スクリプト化してある。対象ホストは既定 `rpi4-wifi`、環境変数 `RPI_HOST` で上書き可。

```bash
# 1) host/ を rpi4-wifi:~/host/ へ転送し、実機で Rust/C++ をビルド（libgpiod-dev 未導入なら自動導入）
host/scripts/deploy_and_build.sh
#    別ホストへ:  RPI_HOST=rpi4-eth host/scripts/deploy_and_build.sh

# 2) ビルド済み led-blink を実機で実行（ssh -t。Ctrl-C で停止・ライン解放）
host/scripts/run_led_blink.sh rust   17 500   # [cpp|rust|python] [GPIO] [周期ms]、既定 rust 17 500
host/scripts/run_led_blink.sh cpp    27 200
host/scripts/run_led_blink.sh python
```

## SSD1306 OLED 表示（disp-writer, I2C）

tmpfs 上の固定長フレーム `DisplayFrame`(88B) を読んで SSD1306(128x64) に描画するホスト側 Rust
バイナリ。設計は [`../docs/rpi4-ssd1306_display__plan.md`](../docs/rpi4-ssd1306_display__plan.md)。
既定は I2C・アドレス `0x3C`・`/run/disp-shm/display_latest.bin`。

前提（実機）: `dtparam=i2c_arm=on`（＋`i2c_arm_baudrate=400000`）で I2C を有効化し、
`i2cdetect -y 1` に `0x3C` が出ること。配線は SDA=BCM2(物理3) / SCL=BCM3(物理5) / VCC=3.3V / GND 共通。

```bash
# デプロイ＆ビルド（disp-writer もワークスペースの一部として一緒にビルドされる）
host/scripts/deploy_and_build.sh

# disp-writer を実機で起動（ssh -t。/run/disp-shm は自動作成。Ctrl-C で停止）
host/scripts/run_disp_writer.sh
#   config を渡す:  host/scripts/run_disp_writer.sh -- --config ~/host/rust/bin/disp-writer/config.example.toml

# 別シェルで検証用フレームを投入（コンテナ無しで表示確認できる）
ssh rpi4-wifi 'python3 ~/host/scripts/gen_display_frame.py --state 1 --batt-v 12.3 --batt-pct 87 \
    --lin 0.25 --ang -0.1 --line1 "hello bobtail"'
ssh rpi4-wifi 'python3 ~/host/scripts/gen_display_frame.py --loop'   # 値を変えながら追従確認
```

将来コンテナ側 ROS2 ノード（`bobtail_display_bridge`）が `DisplayFrame` を書き込む構成に統合する。
その場合 `gen_display_frame.py` は不要（検証用）。SPI 対応・systemd 常駐化は設計書 §5.9/§6 参照。

## LED 点滅 example のビルド・実行（手動 / 実機ローカル）

いずれも既定は BCM17（物理ピン11）、周期 500ms。第1引数で GPIO 番号、第2引数で周期(ms)を変更できる。
Ctrl-C でラインを解放して停止する。配線例: `GPIO17 -->|(LED)|-- [≈330Ω] -- GND`。

### Rust

```bash
cd host/rust
cargo run --release -p led_blink            # 既定 BCM17
cargo run --release -p led_blink -- 27 200  # BCM27, 200ms
```

### C++

```bash
cd host/cpp
cmake -G Ninja -B build && cmake --build build
./build/examples/led_blink/led_blink        # 既定 BCM17
./build/examples/led_blink/led_blink 27 200  # BCM27, 200ms
```

### Python

```bash
cd host/python
python3 examples/led_blink.py               # 既定 BCM17
python3 examples/led_blink.py 27 200         # BCM27, 200ms
```

## 動作確認（LED 現物が無くても可）

実行中に別シェルで `gpioinfo gpiochip0` を見ると、対象ラインが `used` / `output` /
consumer=`led_blink` になっている。物理 LED を配線していれば点滅を目視できる。
Ctrl-C 後は `unused` に戻る。
