cmake_minimum_required(VERSION 3.8)
project(display_bridge_cpp)

if(CMAKE_COMPILER_IS_GNUCXX OR CMAKE_CXX_COMPILER_ID MATCHES "Clang")
  add_compile_options(-Wall -Wextra -Wpedantic)
endif()

find_package(ament_cmake REQUIRED)
find_package(rclcpp REQUIRED)
find_package(std_msgs REQUIRED)

add_executable(display_bridge_cpp src/display_bridge_node.cpp)
ament_target_dependencies(display_bridge_cpp rclcpp std_msgs)

# ros2 run が探すのは lib/<package>/ 配下
install(TARGETS
  display_bridge_cpp
  DESTINATION lib/${PROJECT_NAME})

ament_package()
