# ホスト側 C++ プログラムのトップレベル CMake。
# example → 本実装を add_subdirectory で束ねる。
cmake_minimum_required(VERSION 3.16)
project(bobtail_host_cpp CXX)

if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_subdirectory(examples/led_blink)
