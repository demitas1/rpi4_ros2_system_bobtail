// Copyright 2016 Open Source Robotics Foundation, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// display_bridge_cpp ノード（雛形）。
//
// ROS2 トピックを購読して最新値を保持し、100ms タイマーで tmpfs 上の共有ファイルへ
// DisplayFrame(88 bytes) を write-tmp→rename でアトミック書き込みする。ホスト側の
// disp-writer が読んで SSD1306 に描画する。設計: docs/rpi4-ssd1306_display__plan.md（§3）。
//
// 本ファイルは雛形。フレームのフィールド構築（on_timer 内の TODO）を実装して完成させる。
// 他ロボットでの再利用を意図し、パッケージ名に system 接頭辞は付けていない。

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <memory>
#include <string>

#include "rclcpp/rclcpp.hpp"
#include "std_msgs/msg/string.hpp"

using namespace std::chrono_literals;

// disp-writer(host/rust/bin/disp-writer/src/frame.rs)とバイト互換の固定長フレーム（88 bytes）。
#pragma pack(push, 1)
struct DisplayFrame
{
  uint32_t magic;            // "DIS0" = 0x44495330
  uint16_t version;
  uint16_t reserved;
  uint64_t timestamp_ns;     // CLOCK_REALTIME
  uint64_t seq;              // 更新検知用
  uint8_t robot_state;       // 0=IDLE,1=RUNNING,2=ERROR,3=CHARGING
  uint8_t pad0[3];
  float battery_voltage;
  float battery_percent;
  float linear_vel;
  float angular_vel;
  char line1[20];            // 固定長 ASCII（空白パディング）
  char line2[20];
  uint32_t status_flags;
};
#pragma pack(pop)
static_assert(sizeof(DisplayFrame) == 88, "DisplayFrame size mismatch");

constexpr uint32_t kMagic = 0x44495330;

class DisplayBridge : public rclcpp::Node
{
public:
  DisplayBridge()
  : Node("display_bridge_cpp"),
    shm_dir_("/disp-shm"),
    tmp_name_(".display_latest.tmp"),
    final_name_("display_latest.bin")
  {
    std::filesystem::create_directories(shm_dir_);

    std::memset(&current_, 0, sizeof(current_));
    current_.magic = kMagic;
    current_.version = 1;

    // 購読例（TODO: 実際のトピック/型に合わせて増減する）
    status_sub_ = create_subscription<std_msgs::msg::String>(
      "status_text", 10,
      [this](const std_msgs::msg::String & msg) { set_line(current_.line1, msg.data); });

    // 100ms タイマーで最新値を書き出す
    timer_ = create_wall_timer(100ms, [this]() { on_timer(); });
    RCLCPP_INFO(get_logger(), "display_bridge_cpp started -> %s/%s",
      shm_dir_.c_str(), final_name_.c_str());
  }

private:
  // 20 バイト固定長フィールドへ空白パディングでコピーする（null 終端に依存しない）。
  static void set_line(char (&dst)[20], const std::string & src)
  {
    std::memset(dst, ' ', sizeof(dst));
    std::memcpy(dst, src.data(), std::min(src.size(), sizeof(dst)));
  }

  void on_timer()
  {
    current_.timestamp_ns = now_ns();
    current_.seq += 1;

    // TODO: 購読で保持した最新値を current_ の各フィールドに反映してから書き出す。
    write_atomic();
  }

  // 同一ディレクトリ内 tmp→rename でアトミックに書き込む（方式A）。
  void write_atomic()
  {
    const auto tmp_path = shm_dir_ + "/" + tmp_name_;
    const auto final_path = shm_dir_ + "/" + final_name_;
    {
      std::ofstream f(tmp_path, std::ios::binary | std::ios::trunc);
      if (!f) {
        RCLCPP_WARN(get_logger(), "failed to open tmp file");
        return;
      }
      f.write(reinterpret_cast<const char *>(&current_), sizeof(current_));
    }
    if (std::rename(tmp_path.c_str(), final_path.c_str()) != 0) {
      RCLCPP_WARN(get_logger(), "rename failed");
    }
  }

  static uint64_t now_ns()
  {
    const auto now = std::chrono::system_clock::now();
    return std::chrono::duration_cast<std::chrono::nanoseconds>(
      now.time_since_epoch()).count();
  }

  std::string shm_dir_;
  std::string tmp_name_;
  std::string final_name_;
  DisplayFrame current_;
  rclcpp::Subscription<std_msgs::msg::String>::SharedPtr status_sub_;
  rclcpp::TimerBase::SharedPtr timer_;
};

int main(int argc, char * argv[])
{
  rclcpp::init(argc, argv);
  rclcpp::spin(std::make_shared<DisplayBridge>());
  rclcpp::shutdown();
  return 0;
}
