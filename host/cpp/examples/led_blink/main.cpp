// GPIO LED 点滅 example（C++ / libgpiod v2 C API）
//
// ホスト側（コンテナ外・Pi ネイティブ）で直接実行する。デーモン化はしない。
// キャラクタデバイス /dev/gpiochip0 を使う。gpio グループ所属のユーザーなら
// sudo なしで実行できる。旧 sysfs GPIO（/sys/class/gpio）は使わない。
//
// 使い方:
//   led_blink [GPIO番号] [周期ms]
//   例) led_blink            # BCM17, 500ms
//       led_blink 27 200     # BCM27, 200ms
//
// 配線例: GPIO17 -->|(LED)|-- [≈330Ω] -- GND
// Ctrl-C(SIGINT) を捕捉してラインを解放してから終了する。

#include <gpiod.h>

#include <atomic>
#include <cerrno>
#include <csignal>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <thread>

namespace {
constexpr const char *kChipPath = "/dev/gpiochip0";
constexpr const char *kConsumer = "led_blink";
constexpr unsigned int kDefaultGpio = 17;
constexpr unsigned int kDefaultPeriodMs = 500;

std::atomic<bool> g_running{true};

void handle_sigint(int) { g_running.store(false); }
}  // namespace

int main(int argc, char **argv) {
  unsigned int offset = kDefaultGpio;
  unsigned int period_ms = kDefaultPeriodMs;
  if (argc >= 2) offset = static_cast<unsigned int>(std::strtoul(argv[1], nullptr, 10));
  if (argc >= 3) period_ms = static_cast<unsigned int>(std::strtoul(argv[2], nullptr, 10));
  const auto half = std::chrono::milliseconds(period_ms / 2);

  std::signal(SIGINT, handle_sigint);
  std::signal(SIGTERM, handle_sigint);

  // 1) チップを開く
  gpiod_chip *chip = gpiod_chip_open(kChipPath);
  if (!chip) {
    std::fprintf(stderr, "gpiod_chip_open(%s) 失敗: %s\n", kChipPath, std::strerror(errno));
    return EXIT_FAILURE;
  }

  // 2) ライン設定（出力・初期値 INACTIVE）
  gpiod_line_settings *settings = gpiod_line_settings_new();
  gpiod_line_settings_set_direction(settings, GPIOD_LINE_DIRECTION_OUTPUT);
  gpiod_line_settings_set_output_value(settings, GPIOD_LINE_VALUE_INACTIVE);

  gpiod_line_config *line_cfg = gpiod_line_config_new();
  gpiod_line_config_add_line_settings(line_cfg, &offset, 1, settings);

  gpiod_request_config *req_cfg = gpiod_request_config_new();
  gpiod_request_config_set_consumer(req_cfg, kConsumer);

  // 3) ライン確保
  gpiod_line_request *request = gpiod_chip_request_lines(chip, req_cfg, line_cfg);

  // request 生成後は設定オブジェクトは不要
  gpiod_request_config_free(req_cfg);
  gpiod_line_config_free(line_cfg);
  gpiod_line_settings_free(settings);

  if (!request) {
    std::fprintf(stderr, "gpiod_chip_request_lines(BCM%u) 失敗: %s\n", offset,
                 std::strerror(errno));
    gpiod_chip_close(chip);
    return EXIT_FAILURE;
  }

  std::printf("LED blink (cpp/libgpiod): BCM%u, 周期 %ums。Ctrl-C で停止。\n", offset, period_ms);

  // 4) トグルループ
  while (g_running.load()) {
    gpiod_line_request_set_value(request, offset, GPIOD_LINE_VALUE_ACTIVE);
    std::printf("BCM%u: ON\n", offset);
    std::this_thread::sleep_for(half);
    if (!g_running.load()) break;
    gpiod_line_request_set_value(request, offset, GPIOD_LINE_VALUE_INACTIVE);
    std::printf("BCM%u: OFF\n", offset);
    std::this_thread::sleep_for(half);
  }

  // 5) 後片付け（消灯 → 解放）
  gpiod_line_request_set_value(request, offset, GPIOD_LINE_VALUE_INACTIVE);
  gpiod_line_request_release(request);
  gpiod_chip_close(chip);
  std::printf("\n停止しました。ラインを解放しました。\n");
  return EXIT_SUCCESS;
}
