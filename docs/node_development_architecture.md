# ROS2 ノード開発のリポジトリ構成方針

基盤リポジトリ `rpi4_ros2`（GitHub: `demitas1/rpi4_ros`）は ROS2 の
**基盤（環境）部分のみ** を汎用的に管理し、個別のノード開発は
**システムごとの別リポジトリ**（本リポジトリ `rpi4_ros2_system_bobtail` 等）で行う。
本書はその構成方針をまとめたものである。

## 背景と目的

- ノードには次の3種類が存在する。
  - amd64 / arm64 で共通に動作するノード
  - arm64 でしか動作しないノード（GPIO・カメラ等のハードウェア依存）
  - 個別システムでしか動作しないノード
- ノード単位でリポジトリを分けると数が増えて非効率なため、**システム単位**で
  リポジトリを分割する。
- 基盤リポジトリ `rpi4_ros2` はアーキテクチャや個別システムに依存しない汎用環境
  として維持する。

## 全体構成：3層リポジトリ

| 層 | リポジトリ | 役割 | アーキ |
|----|-----------|------|--------|
| 基盤 | `rpi4_ros2`（GitHub: `rpi4_ros`） | Docker環境・ベースイメージ・ワークスペース骨格・ツール。**ノードは持たない**（動作確認用サンプルのみ） | 非依存 |
| 共通ノード | `ros2_common_nodes`（**将来**） | 複数システムで再利用する汎用ノード（msg定義・共通ドライバ等） | amd64 / arm64 共通 |
| arm64共通 | `ros2_arm64_nodes`（**将来**） | arm64専用だが複数システムで再利用するもの | arm64 のみ |
| システム | `system_<name>` | システム固有ノード + 統合 launch/config + 依存マニフェスト | 混在可 |

> **現時点の方針**: 共通リポジトリ（`ros2_common_nodes` / `ros2_arm64_nodes`）は
> まだ作らない。まず1つのシステムリポジトリで開発を始め、複数システムで再利用
> される対象が明確になった段階で切り出す。vcstool を使うため後からの抽出は容易。

ノード単位ではなくシステム単位でリポジトリを切る。1リポジトリに複数の ament
パッケージを同居させるのは ROS2 で標準的であり、システムリポジトリ内に複数の
ノードを置いてよい。

## ワークスペースの合成：vcstool（`.repos`）

複数リポジトリを1つの ROS2 ワークスペースに集約する方法として
**vcstool**（`.repos` マニフェスト）を採用する。git submodule は採用しない。

- 理由: colcon との相性が良く、バージョン固定（tag/commit）が明示的で、
  ネストした submodule 管理が不要。ROS2 のマルチリポ管理における事実上の標準。
- ベースイメージには vcstool が既に含まれている（`ros-dev-tools` 経由、`vcs` コマンド）。
  追加インストールは不要。

各システムリポジトリに `<system>.repos` を置き、依存する共通リポジトリと
バージョンを宣言する。

## システムリポジトリの雛形

例として `system_bobtail` の構成を示す。

```
system_bobtail/                      ← 別リポジトリ
├── src/
│   ├── bobtail_bringup/             統合 launch / config（システム固有）
│   ├── bobtail_driver/              システム固有ノード
│   └── bobtail_gpio_arm64/          arm64専用パッケージ
├── system_bobtail.repos             共通依存（当面は空 or 最小）
├── system_bobtail.arm64.repos       arm64追加依存（あれば）
├── Dockerfile                   本番イメージ用（FROM ghcrベース）
└── README.md                    対応ベースイメージtagを明記
```

3分類の落とし先:

- **共通（amd64/arm64）**: 将来の `ros2_common_nodes`（当面はシステムリポジトリ内に置く）
- **arm64共通**: arm64専用パッケージ（将来の `ros2_arm64_nodes`）
- **システム固有**: `system_<name>/src/` に隔離 → 他システムへ混入しない

## ビルド・配布の方針（2系統を併用）

### 開発時：ワークスペースをマウントして都度ビルド

```bash
cd rpi4_ros2/docker_rpi4/ros2_ws/src
git clone https://github.com/<owner>/system_bobtail.git
cd ..
vcs import src < src/system_bobtail/system_bobtail.repos          # 共通依存があれば
# arm64 機なら追加で:
# vcs import src < src/system_bobtail/system_bobtail.arm64.repos
bash ../start.sh                                          # コンテナ起動
# コンテナ内:
#   rosdep install --from-paths src -y
#   colcon build
#   source install/setup.bash
```

### 本番時：システムごとにイメージをビルド

`system_bobtail` 側の CI（GitHub Actions）で、ベースイメージを `FROM` して
ノードを焼き込んだイメージをビルドし、`ghcr.io/<owner>/system_bobtail` として公開する。
Pi はそれを pull するだけで動作する。

```dockerfile
# system_bobtail/Dockerfile（概念例）
FROM ghcr.io/demitas1/ros2_jazzy:<tag>
COPY src /home/ros2_user/ros2_ws/src
WORKDIR /home/ros2_user/ros2_ws
RUN . /opt/ros/jazzy/setup.bash \
 && rosdep install --from-paths src -y \
 && colcon build
```

基盤リポ `rpi4_ros2`（ベースイメージ）の CI とシステムリポジトリの CI は完全に分離する。

## アーキテクチャ依存ノードの扱い

ノード単位ではなく **パッケージ単位 + マニフェスト** で吸収する。

1. **パッケージ分離**: arch依存コードは独立した ament パッケージにする。
2. **arch別の取得・ビルド制御**:
   - `system_bobtail.repos`（共通）と `system_bobtail.arm64.repos`（arm64追加分）に
     分け、arm64機では後者も `vcs import` する。
   - または arm64専用パッケージに、amd64ビルド時のみ `COLCON_IGNORE` を置いて
     除外する（`package.xml` の `condition` 属性も併用可）。
3. **システム固有ノードはシステムリポジトリに隔離** → 他システムへ混入しない。

## バージョン管理・再現性

- ベースイメージは ghcr の **tag で固定** し、各システムリポジトリの README /
  `Dockerfile` / `.repos` に「対応ベースイメージ tag」を明記する。
- `.repos` で各リポジトリの commit / tag を固定し、再現可能にする。

## 公開範囲と private 化への備え

- 当面、システムリポジトリ・共通リポジトリは **public** とし、`.repos` には
  **HTTPS URL** を記述する（認証不要で `vcs import` できる）。
- 将来 private 化する場合は、`.repos` を書き換えずに git の URL 書き換え機能で
  SSH 鍵に切り替えられる。Pi 側に deploy key / SSH 鍵を配置した上で:

  ```bash
  git config --global url."git@github.com:".insteadOf "https://github.com/"
  ```

  により、HTTPS 記述のままシームレスに private 取得へ移行できる。

### 認証方式：deploy key（SSH鍵）とトークンの違い

上記の「deploy key」は **GitHub トークンではなく SSH 鍵** である。混同しやすいため
整理する。

- **deploy key** は、GitHub の**特定の1リポジトリだけ**に登録する SSH 公開鍵
  （対象リポジトリの Settings → Deploy keys）。対になる秘密鍵を Pi 側に置き、
  `git@github.com:...`（SSH URL）でアクセスする。
  - スコープが**そのリポジトリに限定**される点が最大の利点。アカウント全体や
    他リポジトリへの権限を渡さずに済み、デバイス単位での鍵失効も容易。
  - デフォルトは read-only。Pi で取得（pull）するだけなら read-only で十分。

- 対比として認証方式を整理する:

  | 方式 | 認証 | URL 形式 | スコープ |
  |------|------|----------|----------|
  | **deploy key** | SSH 鍵 | `git@github.com:` | **リポジトリ単位（1リポのみ）** |
  | アカウント SSH 鍵 | SSH 鍵 | `git@github.com:` | そのアカウントの全リポジトリ |
  | PAT（fine-grained token） | HTTPS トークン | `https://...` | リポジトリ / 権限を指定可 |
  | GitHub App | インストールトークン | `https://...` | App の許可範囲 |

- Pi のような単一デバイスで**特定のシステムリポジトリだけ**を取得する用途では、
  リポジトリ限定の **deploy key（read-only）を推奨**する。トークン（PAT）でも
  実現できるが、権限がリポジトリ横断に広がりやすく、ローテーション管理の手間が
  増えるため、限定スコープの SSH 鍵の方が無難。

## 基盤リポジトリ（`rpi4_ros2`）側で予定する対応

1. ~~vcstool の導入~~ → **対応済み**（`ros-dev-tools` に含まれ導入不要）
2. README の「開発ワークフロー」を vcstool / 多リポ前提に更新する。
3. `ros2_ws/src/py_pubsub` は動作確認用サンプルとして残置し、「実ノードは
   システムリポジトリから取得する」旨を明記する（または `ros2_examples` 別リポ化）。

## 参考

- 環境の手動セットアップ手順: 基盤リポ `rpi4_ros2` の `docs/rpi4_setup.md`
- vcstool: https://github.com/dirk-thomas/vcstool
