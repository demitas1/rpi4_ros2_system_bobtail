#!/usr/bin/env bash
#
# 実機（Pi）にデプロイ済み・ビルド済みの disp-writer を、開発機（dev）から実行する。
#
#   使い方:  host/scripts/run_disp_writer.sh [-- disp-writer への追加引数...]
#   例:      host/scripts/run_disp_writer.sh                       # 既定値（i2c / 0x3C / /run/disp-shm）
#            host/scripts/run_disp_writer.sh -- --config ~/host/rust/bin/disp-writer/config.example.toml
#   対象ホスト: 既定 rpi4-wifi。環境変数 RPI_HOST で上書き可。
#
# - 起動前に /run/disp-shm を demitas 所有で用意する（/run は root 所有 tmpfs のため sudo。passwordless 前提）。
# - ssh -t で TTY を確保するため Ctrl-C が実機プロセスへ伝わり、クリーンに終了する。
# - フレーム投入は実機側で host/scripts/gen_display_frame.py を使う（別シェル）。
# - 事前に host/scripts/deploy_and_build.sh でデプロイ＆ビルドしておくこと。
set -euo pipefail

RPI_HOST="${RPI_HOST:-rpi4-wifi}"
BIN="host/rust/target/release/disp-writer"

# "--" 以降を disp-writer への引数として素通しする
EXTRA=()
if [ "${1:-}" = "--" ]; then
  shift
  EXTRA=("$@")
fi

# 実機にビルド済みバイナリが在るか確認
if ! ssh "${RPI_HOST}" "test -x ~/${BIN}"; then
  echo "エラー: ${RPI_HOST}:~/${BIN} が見つかりません。" >&2
  echo "先に host/scripts/deploy_and_build.sh を実行してください。" >&2
  exit 1
fi

# /run/disp-shm を用意（demitas 所有。tmpfs 上、再起動で消える）
ssh "${RPI_HOST}" 'sudo install -d -o "$(id -un)" -g "$(id -gn)" /run/disp-shm'

echo "==> ${RPI_HOST}: disp-writer 起動。Ctrl-C で停止。"
echo "    別シェルで: ssh ${RPI_HOST} 'python3 ~/host/scripts/gen_display_frame.py --loop'"
exec ssh -t "${RPI_HOST}" "~/${BIN} ${EXTRA[*]}"
