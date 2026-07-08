# Copyright 2016 Open Source Robotics Foundation, Inc.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""
display_bridge ノード（雛形）.

ROS2 トピックを購読して最新値を保持し、100ms タイマーで tmpfs 上の共有ファイルへ
DisplayFrame(88 bytes) を write-tmp→rename でアトミック書き込みする。ホスト側の
disp-writer（host/rust/bin/disp-writer）がこのファイルを読んで SSD1306 に描画する。
設計: docs/rpi4-ssd1306_display__plan.md（§3 コンテナ側実装）.

本ファイルは雛形。フレームのフィールド構築（on_timer 内の TODO）を実装して完成させる。
他ロボットでの再利用を意図し、パッケージ名に system 接頭辞は付けていない。
"""

import os
import struct
import tempfile
import time

import rclpy
from rclpy.node import Node

from std_msgs.msg import String

# --- disp-writer(host/rust/bin/disp-writer/src/frame.rs)とバイト互換の契約 -----------------
# "<IHH QQ B3x ffff 20s20s I" = little-endian, パディング無し, 合計 88 bytes
FRAME_FMT = '<IHHQQB3xffff20s20sI'
FRAME_SIZE = struct.calcsize(FRAME_FMT)
assert FRAME_SIZE == 88, f'unexpected frame size: {FRAME_SIZE}'
MAGIC = 0x44495330  # "DIS0"
VERSION = 1


class DisplayBridge(Node):
    """ROS2 の状態を tmpfs 上の DisplayFrame に書き出すブリッジ（雛形）."""

    def __init__(self):
        """ノード・購読・タイマー・共有ファイルパスを初期化する."""
        super().__init__('display_bridge')

        self.shm_dir = '/disp-shm'
        self.filename = 'display_latest.bin'
        os.makedirs(self.shm_dir, exist_ok=True)
        self.final_path = os.path.join(self.shm_dir, self.filename)

        # 保持する最新状態（TODO: 実装で必要なフィールドを増やす）
        self.seq = 0
        self.robot_state = 0
        self.battery_voltage = 0.0
        self.battery_percent = 0.0
        self.linear_vel = 0.0
        self.angular_vel = 0.0
        self.line1 = ''
        self.line2 = ''
        self.status_flags = 0

        # 購読例（TODO: 実際のトピック/型に合わせて増減する）
        self.create_subscription(String, 'status_text', self.on_status_text, 10)

        # 100ms タイマーで最新値を書き出す（要件: 最新値のみ・厳密なタイミング不要）
        self.timer = self.create_timer(0.1, self.on_timer)
        self.get_logger().info(f'display_bridge started -> {self.final_path}')

    def on_status_text(self, msg):
        """status_text を保持する（購読コールバックの実装例）."""
        self.line1 = msg.data

    def on_timer(self):
        """100ms 周期で最新値を DisplayFrame にまとめて書き出す（要実装）."""
        self.seq += 1
        ts_ns = int(time.time() * 1e9)

        # TODO: 購読で保持した最新値からフレームを構築する。文字列は 20 バイト固定長に
        #       パディングする（末尾空白）。数値フィールドは実装で埋める。
        data = struct.pack(
            FRAME_FMT,
            MAGIC, VERSION, 0,
            ts_ns, self.seq,
            self.robot_state,
            self.battery_voltage, self.battery_percent,
            self.linear_vel, self.angular_vel,
            self.line1.encode('ascii', 'replace')[:20].ljust(20, b' '),
            self.line2.encode('ascii', 'replace')[:20].ljust(20, b' '),
            self.status_flags,
        )
        self._write_atomic(data)

    def _write_atomic(self, data):
        """同一ディレクトリ内 tmp→rename でアトミックに書き込む（方式A）."""
        fd, tmp_path = tempfile.mkstemp(dir=self.shm_dir, prefix='.display_latest_')
        try:
            with os.fdopen(fd, 'wb') as f:
                f.write(data)
            os.rename(tmp_path, self.final_path)
        except Exception:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)
            raise


def main(args=None):
    """ノードを起動して spin する."""
    rclpy.init(args=args)
    node = DisplayBridge()
    try:
        rclpy.spin(node)
    except KeyboardInterrupt:
        pass
    finally:
        node.destroy_node()
        rclpy.shutdown()


if __name__ == '__main__':
    main()
