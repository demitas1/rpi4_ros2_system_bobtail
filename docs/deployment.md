# デプロイ手順（開発機 / Raspberry Pi 4）

本リポジトリのノードを動かすデプロイ手順。配布は2方式を併用する。

| 方式 | 用途 | イメージ | ビルド | Pi がやること |
|------|------|----------|--------|---------------|
| **A** | 開発中デプロイ | ベースイメージを pull | コンテナ内で `colcon build` | clone → `up` → build |
| **B** | 本番配布 | システムイメージを pull | **CI で焼く**（issue #1） | `pull` → `up` のみ |

> **基盤リポジトリ `rpi4_ros` への依存は「ベースイメージの pull」だけ**。両方式とも
> `rpi4_ros` を clone する必要はない。ノード開発者は本リポジトリだけを扱えばよい。

## 前提

- Docker / Docker Compose 導入済み（未導入なら [Docker 公式手順](https://docs.docker.com/engine/install/)）。
- 公開イメージ `ghcr.io/demitas1/ros2_jazzy` を pull できるネットワーク。
- 実機反映時は Pi にリモート接続できること（例 `ssh rpi4-wifi`）。

---

## 方式A: 開発中デプロイ

ノードを開発しながらビルド・実行する。ベースイメージのみで自己完結する
（`docker-compose.yml` を使用）。

### 1. 取得して起動

```bash
git clone https://github.com/demitas1/rpi4_ros2_system_bobtail.git
cd rpi4_ros2_system_bobtail
docker compose up -d        # ベースイメージを pull し、本リポを src にマウントして起動
```

### 2. コンテナに入る

```bash
docker compose exec dev bash
```

### 3. ビルド（コンテナ内）

```bash
source /opt/ros/jazzy/setup.bash         # .bashrc で自動 source されるが明示
# 共通依存（.repos）があれば取り込む:
# vcs import src < src/rpi4_ros2_system_bobtail/system_bobtail.repos
rosdep install -i --from-path src --rosdistro jazzy -y
colcon build                             # 単一パッケージ: --packages-select <pkg>
source install/setup.bash
```

> `host/` は `COLCON_IGNORE` マーカーで colcon の走査対象外なので、無指定の `colcon build` でも
> `src/` 配下の ROS ノードのみがビルドされる（`host/` はコンテナ外＝Pi ホスト用でビルド系統が別）。

### 4. 実行・確認

```bash
ros2 pkg executables bobtail_pubsub
ros2 run bobtail_pubsub bobtail_talker           # 別シェルで bobtail_listener を実行して確認
ros2 node list
```

### 5. 編集 → 再ビルド

ホスト側でソースを編集（マウント経由で即反映）したら、コンテナ内で再ビルド:

```bash
colcon build --packages-select <pkg> && source install/setup.bash
```

`build/ install/ log/` は名前付きボリューム `ros2_ws` に残るため、コンテナを
作り直しても増分ビルドが効いて速い。ホストの作業ツリーは汚れない。

### 6. 後片付け

```bash
docker compose down         # コンテナ停止（ボリュームは保持＝次回も高速）
docker compose down -v      # ボリューム（build/install/log）も破棄して完全リセット
```

---

## 方式B: 本番配布

CI でシステムイメージを焼いて ghcr に公開し、Pi は pull するだけで動かす
（`docker-compose.prod.yml` を使用）。

### ビルド & 公開（CI 側）

`docker-compose.prod.yml` の `build` が `Dockerfile`（`FROM` ベースイメージ →
`COPY src` → `rosdep` + `colcon build`）からイメージを焼く。**個別の colcon
ビルド手順は不要**（compose が実行する）。

```bash
docker compose -f docker-compose.prod.yml build
docker compose -f docker-compose.prod.yml push    # ghcr へ（要 docker login）
```

> 本番イメージのビルドは **CI（GitHub Actions）で行う**（構築は issue #1）。
> arm64 実機ビルドは重いため、Pi 上ではビルドしない。

### Pi で起動（pull のみ）

```bash
ssh rpi4-wifi 'cd <配置先> && \
  docker compose -f docker-compose.prod.yml pull && \
  docker compose -f docker-compose.prod.yml up -d'
```

- `restart: unless-stopped` により Pi 再起動後も自動起動する。
- 起動内容は `docker-compose.prod.yml` の `command`（実システムでは
  `ros2 launch <bringup>` に差し替える。雛形サンプルは `bobtail_talker` を起動）。

### 停止

```bash
docker compose -f docker-compose.prod.yml down
```

---

## 動作確認

```bash
# 方式A（コンテナ内）:
docker compose exec dev bash -lc 'source install/setup.bash && ros2 node list'

# 方式B（本番コンテナのログ）:
docker logs rpi4_ros2_system_bobtail | tail
```

期待: `ros2 node list` に想定のノード（サンプルなら `/bobtail_publisher`
`/bobtail_subscriber`）。listener に `I heard: "Hello World: N"`。

> `timeout` でノードを止めた際に出る `ExternalShutdownException` は想定内。

## トラブルシュート

- **ノードが互いに見えない**: 双方の `ROS_DOMAIN_ID`（既定 42）と
  `network_mode: host` を確認する。
- **`docker compose pull` でベースイメージが取得できない**: ネットワークと
  `ghcr.io/demitas1/ros2_jazzy` の公開状態を確認
  （`docker manifest inspect ghcr.io/demitas1/ros2_jazzy:latest` で arm64 の有無）。
- **ローカル検証手順**（amd64 PC での事前確認）は
  [`local_verification.md`](local_verification.md) を参照。
