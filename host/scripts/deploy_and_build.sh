#!/usr/bin/env bash
#
# host/ を実機（Pi）へ rsync でデプロイし、実機上でビルドする。開発機（dev）で実行する。
#
#   使い方:  host/scripts/deploy_and_build.sh
#   対象ホスト: 既定 rpi4-wifi。環境変数 RPI_HOST で上書き可（例: RPI_HOST=rpi4-eth ...）。
#
# - デプロイ先は $RPI_HOST:~/host/（独立ツリー。git push/pull 不要）。
# - ビルド成果物（rust/target, cpp build, __pycache__）は転送・削除しない（Pi 側の増分ビルド保持）。
# - C++ に必要な libgpiod-dev が無ければ apt で導入する（passwordless sudo 前提）。
set -euo pipefail

RPI_HOST="${RPI_HOST:-rpi4-wifi}"
# スクリプト位置から host/ ルートを解決（CWD 非依存）
HOST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> デプロイ先: ${RPI_HOST}:~/host/  (source: ${HOST_DIR})"

# 1) rsync（ソースのみ。ビルド成果物は除外）
rsync -az --delete \
  --exclude 'rust/target/' \
  --exclude '**/build/' \
  --exclude '__pycache__/' \
  "${HOST_DIR}/" "${RPI_HOST}:host/"
echo "==> rsync 完了"

# 2) 実機で前提確認＋ビルド（単一 ssh セッション）
ssh "${RPI_HOST}" 'bash -s' <<'REMOTE'
set -euo pipefail
source ~/.cargo/env 2>/dev/null || true

echo "--- 前提: libgpiod-dev ---"
if pkg-config --exists libgpiod 2>/dev/null; then
  echo "libgpiod $(pkg-config --modversion libgpiod) OK"
else
  echo "libgpiod-dev 未導入 → apt で導入"
  sudo apt-get update -qq
  sudo apt-get install -y libgpiod-dev
fi

echo "--- Rust (cargo build --release) ---"
( cd ~/host/rust && cargo build --release )

echo "--- C++ (cmake + ninja) ---"
( cd ~/host/cpp && cmake -G Ninja -B build >/dev/null && cmake --build build )

echo "--- Python (構文チェック) ---"
python3 -m py_compile ~/host/python/examples/led_blink.py && echo "py_compile OK"

echo "--- 生成物 ---"
ls -l ~/host/rust/target/release/led_blink \
      ~/host/rust/target/release/disp-writer \
      ~/host/cpp/build/examples/led_blink/led_blink
REMOTE

cat <<EOF

==> 完了。実行するには:
    host/scripts/run_led_blink.sh rust   17 500
    host/scripts/run_led_blink.sh cpp    17 500
    host/scripts/run_led_blink.sh python 17 500
    host/scripts/run_disp_writer.sh                 # SSD1306 OLED(I2C) 表示
    (フレーム投入は実機で: python3 ~/host/scripts/gen_display_frame.py --loop)
    (対象ホスト変更: RPI_HOST=rpi4-eth host/scripts/run_led_blink.sh rust)
EOF
