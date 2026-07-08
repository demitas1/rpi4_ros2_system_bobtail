# システム個別ノードの開発手順（C++ / ament_cmake）

システムリポジトリ（本リポジトリおよびこれを雛形に作る各 `rpi4_ros2_system_<name>`）で、
C++ ノード（`ament_cmake` パッケージ）を追加・開発する手順をまとめる。

リポジトリ構成の全体方針は [`node_development_architecture.md`](node_development_architecture.md) を参照。
Python 版は [`node_development_python.md`](node_development_python.md) を参照。実例として本リポジトリ同梱の
`src/bobtail_pubsub_cpp`（talker/listener、公式 cpp_pubsub 由来）を用いる。

## 前提

- ノードはシステムリポジトリの `src/` 配下に ament パッケージとして置く（Python と同居可）。
- 基盤環境（ROS2 Jazzy / colcon / vcstool）は `rpi4_ros2` のベースイメージが提供する。
- 開発用コンテナの起動・ビルド・実機反映は [`deployment.md`](deployment.md)、
  amd64 PC での事前検証は [`local_verification.md`](local_verification.md) を参照。

## パッケージ構成（ament_cmake）

```
src/<package_name>/
├── package.xml                 # パッケージ定義（name / 依存 / build_type=ament_cmake）
├── CMakeLists.txt              # add_executable / ament_target_dependencies / install
├── LICENSE                     # ライセンス本文（任意だが推奨）
└── src/                        # C++ ソース（各ノードが main を持つ）
    └── *.cpp
```

Python（`ament_python`）との差分:

- `setup.py` / `setup.cfg` / `resource/<package_name>` マーカー / モジュールディレクトリは**不要**。
- 実行ファイルは `CMakeLists.txt` の `add_executable` で作り、`install(TARGETS ... DESTINATION lib/${PROJECT_NAME})`
  で配置する（`ros2 run` が探すのは `lib/<package>/` 配下。ament_python の `setup.cfg` の `$base/lib/<pkg>` に対応）。
- 依存は `package.xml` の `<depend>`（build/exec 両方に効く）に加え、`CMakeLists.txt` の `find_package` /
  `ament_target_dependencies` でビルド時に解決する（両方に書く必要がある）。

## 新規パッケージの作成

### 方法A: ros2 pkg create（推奨・新規ゼロから）

コンテナ内で実行する（`docker compose exec dev bash`）。

```bash
cd /home/ros2_user/ros2_ws/src/<system_repo>/src   # システムリポの src/
ros2 pkg create --build-type ament_cmake \
  --node-name <node_name> <package_name> \
  --dependencies rclcpp std_msgs
```

### 方法B: 既存パッケージを雛形にコピー

`bobtail_pubsub_cpp` を雛形にする場合、コピー後に以下をすべて新名称へ置き換える
（命名規約はシステム由来が分かる接頭辞を付ける。例 `bobtail_*`）。

| 変更箇所 | 内容 |
|----------|------|
| `package.xml` の `<name>` | パッケージ名 |
| `CMakeLists.txt` の `project()` | パッケージ名（`${PROJECT_NAME}` に波及） |
| `CMakeLists.txt` の `add_executable` 名 | 実行ファイル名 |
| `CMakeLists.txt` の `install(TARGETS ...)` | 上記実行ファイル名（`DESTINATION lib/${PROJECT_NAME}`） |
| 各ソースの `Node("<node_name>")` | 実行時ノード名 |
| クラス名 | 任意（識別しやすい名前） |

> 改名漏れ確認:
> `grep -rn '<旧名>' src/<package>` で旧パッケージ名・旧実行ファイル名・旧ノード名が残っていないこと。

## ノード実装の要点

- ノードは `rclcpp::Node` を継承し、`main()` で `rclcpp::init()` →
  `rclcpp::spin(std::make_shared<Node派生>())` → `rclcpp::shutdown()` を行う。
- 実行ファイルは `CMakeLists.txt` の `add_executable(<exec_name> src/<file>.cpp)` +
  `ament_target_dependencies(<exec_name> rclcpp std_msgs ...)` で登録する。
- パブリッシャ/サブスクライバのトピック名・型は pub/sub 間で一致させる。

## 依存関係

- ROS パッケージ依存は `package.xml` に `<depend>` で宣言し、`CMakeLists.txt` の `find_package` にも書く。
- ビルド前に依存解決:
  ```bash
  rosdep install -i --from-path src --rosdistro jazzy -y
  ```
- 複数システムで再利用する共通ノードは、別リポジトリ化して `<system>.repos`
  （vcstool マニフェスト）で取り込む。詳細は方針書を参照。

## ビルドと実行（コンテナ内）

```bash
cd /home/ros2_user/ros2_ws
source /opt/ros/jazzy/setup.bash          # .bashrc で自動実行される
colcon build --packages-select <package_name>
source install/setup.bash
ros2 pkg executables <package_name>        # 登録された実行ファイルを確認
ros2 run <package_name> <exec_name>
```

> `build/` `install/` `log/` は `.gitignore` 済み。コミットしないこと。

## 言語間の相互運用

`bobtail_pubsub_cpp` は Python 版 `bobtail_pubsub` と**同じトピック `topic`・同じ型 `std_msgs/msg/String`**
を使うため、言語をまたいで pub/sub できる。例えば C++ talker と Python listener の組み合わせが可能:

```bash
ros2 run bobtail_pubsub_cpp bobtail_talker_cpp    # C++ publisher
ros2 run bobtail_pubsub     bobtail_listener       # Python subscriber（別端末）
```

ノード名は Python 版（`bobtail_publisher` / `bobtail_subscriber`）と C++ 版
（`bobtail_publisher_cpp` / `bobtail_subscriber_cpp`）で分けてあるため、両版を同時起動しても衝突しない。
