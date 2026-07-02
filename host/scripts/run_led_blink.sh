#!/usr/bin/env bash
#
# 実機（Pi）にデプロイ済み・ビルド済みの led-blink example を、開発機（dev）から実行する。
#
#   使い方:  host/scripts/run_led_blink.sh [cpp|rust|python] [GPIO番号] [周期ms]
#   既定:    rust 17 500
#   例:      host/scripts/run_led_blink.sh cpp 27 200
#   対象ホスト: 既定 rpi4-wifi。環境変数 RPI_HOST で上書き可。
#
# ssh -t で TTY を確保するため、Ctrl-C が実機プロセスへ伝わり、消灯・ライン解放して終了する。
# 事前に host/scripts/deploy_and_build.sh でデプロイ＆ビルドしておくこと。
set -euo pipefail

RPI_HOST="${RPI_HOST:-rpi4-wifi}"
LANG_SEL="${1:-rust}"
GPIO="${2:-17}"
PERIOD="${3:-500}"

case "${LANG_SEL}" in
  cpp)    BIN="host/cpp/build/examples/led_blink/led_blink"; TEST="-x" ;;
  rust)   BIN="host/rust/target/release/led_blink";          TEST="-x" ;;
  python) BIN="host/python/examples/led_blink.py";           TEST="-f" ;;
  *)
    echo "エラー: 言語は cpp / rust / python のいずれか (指定: '${LANG_SEL}')" >&2
    echo "使い方: $(basename "$0") [cpp|rust|python] [GPIO番号] [周期ms]" >&2
    exit 2 ;;
esac

# 実機側にビルド済みバイナリ/スクリプトが在るか確認
if ! ssh "${RPI_HOST}" "test ${TEST} ~/${BIN}"; then
  echo "エラー: ${RPI_HOST}:~/${BIN} が見つかりません。" >&2
  echo "先に host/scripts/deploy_and_build.sh を実行してください。" >&2
  exit 1
fi

# 実行コマンドを組み立て（python はインタプリタ経由）
if [ "${LANG_SEL}" = "python" ]; then
  CMD="python3 ~/${BIN} ${GPIO} ${PERIOD}"
else
  CMD="~/${BIN} ${GPIO} ${PERIOD}"
fi

echo "==> ${RPI_HOST}: ${LANG_SEL} led-blink (BCM${GPIO}, ${PERIOD}ms)。Ctrl-C で停止。"
exec ssh -t "${RPI_HOST}" "${CMD}"
