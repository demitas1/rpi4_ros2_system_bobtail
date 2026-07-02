# ローカル検証手順（amd64 PC）

Raspberry Pi 4（arm64）へ反映する前に、開発機（amd64 PC）でノードのビルド・動作を
検証する手順。amd64/arm64 共通ノードであれば、ローカルで改名漏れ・ビルドエラー・
pub/sub の動作を先に潰せる。

対象例: 本リポジトリの `src/bobtail_pubsub`（talker/listener）。

## 前提

- 開発機に Docker が導入済み。
- ベースイメージ `ghcr.io/demitas1/ros2_jazzy:latest`（amd64/arm64 マルチアーキ）を取得可能。
  - 未取得なら `docker pull ghcr.io/demitas1/ros2_jazzy:latest`

検証方法は2つある。いずれも `rpi4_ros` の clone は不要で、ベースイメージを pull
するだけ。一度きりの確認は**使い捨てコンテナ方式を推奨**（ホストを汚さず後片付け不要）。
繰り返しビルドするなら本リポの `docker-compose.yml` を使う方法もある。

---

## 方法1: 使い捨てコンテナ（推奨）

システムリポジトリを read-only でマウントし、コンテナ内へコピーしてビルド・実行する。
`--rm` でコンテナは破棄され、ホスト側に成果物は残らない。

```bash
docker run --rm \
  -v /path/to/rpi4_ros2_system_bobtail:/staging:ro \
  -e ROS_DOMAIN_ID=42 \
  ghcr.io/demitas1/ros2_jazzy:latest \
  bash -lc '
set -e
mkdir -p ~/ros2_ws/src
cp -r /staging ~/ros2_ws/src/rpi4_ros2_system_bobtail
cd ~/ros2_ws
source /opt/ros/jazzy/setup.bash
colcon build --packages-select bobtail_pubsub
source install/setup.bash
ros2 pkg executables bobtail_pubsub
# pub/sub 動作確認
(ros2 run bobtail_pubsub bobtail_talker > /tmp/talker.log 2>&1 &)
sleep 3
timeout 5 ros2 run bobtail_pubsub bobtail_listener || true
echo "--- talker ログ ---"; head -5 /tmp/talker.log
'
```

期待される出力:

- `colcon build` が成功する。
- `ros2 pkg executables bobtail_pubsub` に `bobtail_talker` / `bobtail_listener`。
- listener に `I heard: "Hello World: N"`、talker ログに `Publishing: "Hello World: N"`。

> **補足**: 末尾に出る `rclpy.executors.ExternalShutdownException` は `timeout` が
> listener を終了させた際の想定内のメッセージで、コードの不具合ではない。

---

## 方法2: docker compose（本リポ・繰り返し検証向き）

本リポの `docker-compose.yml`（方式A 開発用）をそのまま使う。`rpi4_ros` の clone は
不要で、ベースイメージを pull するだけ。コンテナを残せば名前付きボリュームで増分
ビルドが効くため、繰り返しの検証に向く。実機デプロイ（方式A）と同じ流れを再現できる。

```bash
cd /path/to/rpi4_ros2_system_bobtail
docker compose up -d
docker compose exec dev bash
```

コンテナ内:

```bash
source /opt/ros/jazzy/setup.bash
colcon build --packages-select bobtail_pubsub
source install/setup.bash
ros2 run bobtail_pubsub bobtail_talker          # 別シェルで bobtail_listener を実行して確認
```

後片付け:

```bash
docker compose down       # ボリュームは保持（次回の再ビルドが速い）
docker compose down -v    # build/install/log も破棄して完全リセット
```

> `build/ install/ log/` は名前付きボリューム `ros2_ws` に入り、ホストの作業ツリーには
> 残らない。デプロイ手順の詳細は [`deployment.md`](deployment.md)（方式A）。

---

## 検証チェックリスト

- [ ] `colcon build` がエラーなく完了する
- [ ] `ros2 pkg executables <package>` に想定の実行ファイルが出る
- [ ] pub/sub のメッセージ授受が確認できる
- [ ] `ros2 node list` に想定のノード名が出る
- [ ] 改名した場合、旧名（旧パッケージ名・旧ノード名）が残っていない

ローカル検証が通ったら、実機反映は [`deployment.md`](deployment.md) へ。
