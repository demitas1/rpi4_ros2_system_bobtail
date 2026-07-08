# rpi4_ros2_system_bobtail

ROS2 システム **bobtail** のノード管理リポジトリ。

基盤環境 `rpi4_ros2` のもとで、bobtail システム固有の ament パッケージ（ノード）と
デプロイ構成を管理する。3層リポジトリ構成の最下層（システム層）にあたり、
本リポジトリ自体には ROS2 ランタイムや colcon は含まれない —— 基盤リポジトリが提供する
ベースイメージのコンテナ内でビルド・実行する。

基盤環境（Docker / ROS2 ベースイメージ）は別リポジトリ
[`rpi4_ros2`](https://github.com/demitas1/rpi4_ros) が提供する。本リポジトリはその
ワークスペース（`ros2_ws/src`）に取り込んで使用する。

リポジトリ構成の全体方針は [`docs/node_development_architecture.md`](docs/node_development_architecture.md) を参照。

## 構成

```
rpi4_ros2_system_bobtail/
├── src/                        システム固有の ament パッケージ群（コンテナ内で colcon ビルド）
├── host/                       ホスト側（コンテナ外）で直接動くプログラム（GPIO/I2C/SPI/PWM。colcon 非対象）
├── system_bobtail.repos        共通依存（vcstool マニフェスト。当面は空 or 最小）
├── system_bobtail.arm64.repos  arm64 追加依存（あれば）
├── docker-compose.yml          方式A: 開発中デプロイ（ベースイメージにソースをマウント）
├── docker-compose.prod.yml     方式B: 本番配布（システムイメージを pull して起動）
├── Dockerfile                  本番イメージ用（方式B。FROM ghcr ベースイメージ）
└── docs/                       ドキュメント
```

> `host/` は ROS2 コンテナとは分離した Pi ホストネイティブのプログラム
> （[`host/README.md`](host/README.md)）。ビルド環境は
> [`docs/pi4_host_toolchain_setup.md`](docs/pi4_host_toolchain_setup.md) を参照。

現在 `src/bobtail_pubsub`（Python / ament_python）と `src/bobtail_pubsub_cpp`
（C++ / ament_cmake）を talker/listener の参照実装として同梱している。両者は同一トピック
`topic`（`std_msgs/String`）を使い言語間で相互運用できる。実ノードの追加に伴い順次差し替える。
C++ ノードの追加手順は [`docs/node_development_cpp.md`](docs/node_development_cpp.md)、
Python 版は [`docs/node_development_python.md`](docs/node_development_python.md)。

## 対応ベースイメージ

- `ghcr.io/demitas1/ros2_jazzy`（運用時は `:latest` ではなく tag 固定を推奨）

## デプロイ（2方式）

`rpi4_ros` への依存は **ベースイメージの pull のみ**。基盤リポジトリの clone は不要。
詳細手順は [`docs/deployment.md`](docs/deployment.md)。

### 方式A: 開発中デプロイ（ベースイメージにソースをマウント）

本リポを clone し、`docker-compose.yml` でコンテナ内ビルドする。

```bash
git clone https://github.com/demitas1/rpi4_ros2_system_bobtail.git
cd rpi4_ros2_system_bobtail
docker compose up -d            # ベースイメージを pull、本リポを src にマウント
docker compose exec dev bash
# コンテナ内:
#   source /opt/ros/jazzy/setup.bash
#   rosdep install -i --from-path src --rosdistro jazzy -y
#   colcon build && source install/setup.bash
#   ros2 run bobtail_pubsub bobtail_talker
```

> `host/` はコンテナ外（Pi ホスト）用のプログラムで colcon の対象外。`host/COLCON_IGNORE`
> マーカーにより colcon は `host/` を走査しないので、上記の `colcon build`（無指定）で
> `src/` 配下の ROS ノードのみがビルドされる。特定パッケージだけビルドするなら
> `colcon build --packages-select bobtail_pubsub bobtail_pubsub_cpp`。

### 方式B: 本番配布（システムイメージを焼いて Pi が pull）

CI（GitHub Actions）でシステムイメージを焼いて ghcr に公開し、Pi は pull するだけ。
CI（システムイメージの build & push）は**未整備**（今後 issue 化）。

```bash
# CI 側: docker-compose.prod.yml の build がイメージを焼く（個別ビルド手順は不要）
docker compose -f docker-compose.prod.yml build
docker compose -f docker-compose.prod.yml push

# Pi 側: pull して起動
docker compose -f docker-compose.prod.yml pull
docker compose -f docker-compose.prod.yml up -d
```
