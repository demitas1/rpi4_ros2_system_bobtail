#!/usr/bin/env python3
"""GPIO LED 点滅 example（Python / gpiod v2）

ホスト側（コンテナ外・Pi ネイティブ）で直接実行する。デーモン化はしない。
キャラクタデバイス /dev/gpiochip0 を使う。gpio グループ所属のユーザーなら
sudo なしで実行できる（旧 RPi.GPIO / sysfs は使わない）。

前提: python3-libgpiod（`import gpiod` が v2 API）。追加インストール不要。

使い方:
  python3 led_blink.py [GPIO番号] [周期ms]
  例) python3 led_blink.py           # BCM17, 500ms
      python3 led_blink.py 27 200    # BCM27, 200ms

配線例: GPIO17 -->|(LED)|-- [≈330Ω] -- GND
Ctrl-C で停止すると with ブロックを抜けてラインが解放される。
"""

import sys
import time

import gpiod
from gpiod.line import Direction, Value

CHIP_PATH = "/dev/gpiochip0"
CONSUMER = "led_blink"
DEFAULT_GPIO = 17
DEFAULT_PERIOD_MS = 500


def main() -> int:
    pin = int(sys.argv[1]) if len(sys.argv) >= 2 else DEFAULT_GPIO
    period_ms = int(sys.argv[2]) if len(sys.argv) >= 3 else DEFAULT_PERIOD_MS
    half = period_ms / 2000.0  # ms -> 秒、半周期

    with gpiod.request_lines(
        CHIP_PATH,
        consumer=CONSUMER,
        config={pin: gpiod.LineSettings(direction=Direction.OUTPUT)},
    ) as request:
        print(f"LED blink (python/gpiod): BCM{pin}, 周期 {period_ms}ms。Ctrl-C で停止。")
        try:
            while True:
                request.set_value(pin, Value.ACTIVE)
                print(f"BCM{pin}: ON")
                time.sleep(half)
                request.set_value(pin, Value.INACTIVE)
                print(f"BCM{pin}: OFF")
                time.sleep(half)
        except KeyboardInterrupt:
            request.set_value(pin, Value.INACTIVE)
            print("\n停止しました。ラインを解放します。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
