# host/python — Python ホスト側プログラム

GPIO は `gpiod` v2（Debian パッケージ `python3-libgpiod`）を使う。キャラクタデバイス
`/dev/gpiochip0` 経由で、`gpio` グループ所属ユーザーなら sudo 不要。

## 依存

- `python3-libgpiod`（`import gpiod` が v2 API）
  - 未導入の場合: `sudo apt-get install -y python3-libgpiod`
- I2C を使う場合: `python3-smbus2`、SPI を使う場合: `python3-spidev`（周辺追加時に導入）

追加の pip パッケージは不要。

## example

- `examples/led_blink.py` — GPIO LED 点滅。`python3 examples/led_blink.py [GPIO番号] [周期ms]`
