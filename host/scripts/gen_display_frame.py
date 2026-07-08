#!/usr/bin/env python3
"""
検証用: DisplayFrame(88 bytes) を tmpfs にアトミック書き込みする単体スクリプト。

disp-writer の end-to-end 確認をコンテナ無しで行うためのもの。実機(Pi)で実行する。
バイト配置は docs/rpi4-ssd1306_display__plan.md §2.1/§3.3 準拠（Rust 側 frame.rs とバイト互換）。

  # 一発書き込み
  python3 gen_display_frame.py --state 1 --batt-v 12.3 --batt-pct 87 \
      --lin 0.25 --ang -0.1 --line1 "hello bobtail"

  # 100ms 間隔で値を変えながら書き続ける（OLED の追従を目視確認）
  python3 gen_display_frame.py --loop
"""
import argparse
import os
import struct
import sys
import tempfile
import time

# "<IHH QQ B3x ffff 20s20s I" = little-endian, パディング無し, 合計 88 bytes
FRAME_FMT = "<IHHQQB3xffff20s20sI"
FRAME_SIZE = struct.calcsize(FRAME_FMT)
assert FRAME_SIZE == 88, f"unexpected frame size: {FRAME_SIZE}"
MAGIC = 0x44495330  # "DIS0"


def pack_frame(seq, args):
    ts_ns = int(time.time() * 1e9)
    line1 = args.line1.encode("ascii", errors="replace")[:20].ljust(20, b" ")
    line2 = args.line2.encode("ascii", errors="replace")[:20].ljust(20, b" ")
    return struct.pack(
        FRAME_FMT,
        MAGIC, 1, 0,
        ts_ns, seq,
        args.state,
        args.batt_v, args.batt_pct,
        args.lin, args.ang,
        line1, line2,
        args.status_flags,
    )


def write_atomic(shm_dir, filename, data):
    """同一ディレクトリ内に tmp を作って rename（方式A: アトミック共有）。"""
    os.makedirs(shm_dir, exist_ok=True)
    final_path = os.path.join(shm_dir, filename)
    fd, tmp_path = tempfile.mkstemp(dir=shm_dir, prefix=".display_latest_")
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(data)
        os.rename(tmp_path, final_path)
    except Exception:
        if os.path.exists(tmp_path):
            os.remove(tmp_path)
        raise


def main():
    p = argparse.ArgumentParser(description="DisplayFrame を tmpfs に書き込む（disp-writer 検証用）")
    p.add_argument("--shm-dir", default="/run/disp-shm")
    p.add_argument("--filename", default="display_latest.bin")
    p.add_argument("--state", type=int, default=0, help="0=IDLE 1=RUNNING 2=ERROR 3=CHARGING")
    p.add_argument("--batt-v", type=float, default=0.0)
    p.add_argument("--batt-pct", type=float, default=0.0)
    p.add_argument("--lin", type=float, default=0.0)
    p.add_argument("--ang", type=float, default=0.0)
    p.add_argument("--line1", default="")
    p.add_argument("--line2", default="")
    p.add_argument("--status-flags", type=int, default=0)
    p.add_argument("--loop", action="store_true", help="100ms 間隔で値を変えながら書き続ける")
    p.add_argument("--interval-ms", type=int, default=100)
    args = p.parse_args()

    if not args.loop:
        write_atomic(args.shm_dir, args.filename, pack_frame(1, args))
        print(f"wrote 1 frame -> {os.path.join(args.shm_dir, args.filename)}")
        return

    print("loop mode: Ctrl-C で停止")
    seq = 0
    base_v = args.batt_v if args.batt_v else 12.0
    try:
        while True:
            seq += 1
            # 目視で追従が分かるよう、いくつかの値を seq に応じて変化させる
            args.batt_pct = float((seq * 2) % 100)
            args.batt_v = base_v + 0.5 * ((seq % 10) / 10.0)
            args.lin = round(0.1 * (seq % 10), 2)
            args.ang = round(-0.1 * (seq % 5), 2)
            if not args.line1:
                args.line1 = f"seq {seq}"
            else:
                args.line1 = f"{args.line1.split(' #')[0]} #{seq}"
            write_atomic(args.shm_dir, args.filename, pack_frame(seq, args))
            time.sleep(args.interval_ms / 1000.0)
    except KeyboardInterrupt:
        print(f"\nstopped (last seq={seq})")
        sys.exit(0)


if __name__ == "__main__":
    main()
