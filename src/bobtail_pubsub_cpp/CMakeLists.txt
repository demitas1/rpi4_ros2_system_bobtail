cmake_minimum_required(VERSION 3.8)
project(bobtail_pubsub_cpp)

if(CMAKE_COMPILER_IS_GNUCXX OR CMAKE_CXX_COMPILER_ID MATCHES "Clang")
  add_compile_options(-Wall -Wextra -Wpedantic)
endif()

find_package(ament_cmake REQUIRED)
find_package(rclcpp REQUIRED)
find_package(std_msgs REQUIRED)

add_executable(bobtail_talker_cpp src/publisher_member_function.cpp)
ament_target_dependencies(bobtail_talker_cpp rclcpp std_msgs)

add_executable(bobtail_listener_cpp src/subscriber_member_function.cpp)
ament_target_dependencies(bobtail_listener_cpp rclcpp std_msgs)

# ros2 run が探すのは lib/<package>/ 配下（ament_python の setup.cfg の $base/lib/<pkg> に対応）
install(TARGETS
  bobtail_talker_cpp
  bobtail_listener_cpp
  DESTINATION lib/${PROJECT_NAME})

ament_package()
