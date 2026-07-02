# libgpiod v2 の C ライブラリを pkg-config で検出してリンクする。
# 前提: sudo apt-get install -y libgpiod-dev  (libgpiod.pc を提供)
find_package(PkgConfig REQUIRED)
pkg_check_modules(GPIOD REQUIRED IMPORTED_TARGET libgpiod>=2.0)

add_executable(led_blink main.cpp)
target_link_libraries(led_blink PRIVATE PkgConfig::GPIOD)
