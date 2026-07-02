# システム個別ノードの開発手順（Python / ament_python）

システムリポジトリ（本リポジトリおよびこれを雛形に作る各 `rpi4_ros2_system_<name>`）で、
Python ノード（`ament_python` パッケージ）を追加・開発する手順をまとめる。

リポジトリ構成の全体方針は [`node_development_architecture.md`](node_development_architecture.md) を参照。
実例として本リポジトリ同梱の `src/bobtail_pubsub`（talker/listener）を用いる。

## 前提

- ノードはシステムリポジトリの `src/` 配下に ament パッケージとして置く。
- 基盤環境（ROS2 Jazzy / colcon / vcstool）は `rpi4_ros2` のベースイメージが提供する。
- 開発用コンテナの起動・ビルド・実機反映は [`deployment.md`](deployment.md)、
  amd64 PC での事前検証は [`local_verification.md`](local_verification.md) を参照。

## パッケージ構成（ament_python）

```
src/<package_name>/
├── package.xml                 # パッケージ定義（name / 依存 / build_type）
├── setup.py                    # package_name / entry_points（実行ファイル）
├── setup.cfg                   # script_dir / install_scripts
├── resource/<package_name>     # ament index マーカー（空ファイル）
├── <package_name>/             # Python モジュール
│   ├── __init__.py
│   └── *.py                    # ノード実装（main を持つ）
└── test/                       # ament_copyright / flake8 / pep257
```

## 新規パッケージの作成

### 方法A: ros2 pkg create（推奨・新規ゼロから）

コンテナ内で実行する（`docker exec -it ros2_jazzy_container bash`）。

```bash
cd /home/ros2_user/ros2_ws/src/<system_repo>/src   # システムリポの src/
ros2 pkg create --build-type ament_python \
  --node-name <node_name> <package_name> \
  --dependencies rclpy std_msgs
```

### 方法B: 既存パッケージを雛形にコピー

`bobtail_pubsub` を雛形にする場合、コピー後に以下をすべて新名称へ置き換える
（命名規約はシステム由来が分かる接頭辞を付ける。例 `bobtail_*`）。

| 変更箇所 | 内容 |
|----------|------|
| ディレクトリ `<package_name>/` | Python モジュール名 |
| `resource/<package_name>` | マーカーファイル名 |
| `package.xml` の `<name>` | パッケージ名 |
| `setup.py` の `package_name` | パッケージ名 |
| `setup.py` の `entry_points`（console_scripts） | 実行ファイル名 = モジュール:main |
| `setup.cfg` の `script_dir` / `install_scripts` | `$base/lib/<package_name>` |
| 各ノードの `super().__init__('<node_name>')` | 実行時ノード名 |
| クラス名 | 任意（識別しやすい名前） |

> 改名漏れ確認:
> `grep -rn '<旧名>' src/<package>` で旧パッケージ名・旧ノード名が残っていないこと。

## ノード実装の要点

- ノードは `rclpy.node.Node` を継承し、`main()` で `rclpy.init()` →
  `rclpy.spin(node)` → `node.destroy_node()` / `rclpy.shutdown()` を行う。
- 実行ファイルは `setup.py` の `console_scripts` に
  `<exec_name> = <package_name>.<module>:main` として登録する。
- パブリッシャ/サブスクライバのトピック名・型は pub/sub 間で一致させる。

## 依存関係

- ROS パッケージ依存は `package.xml` に `<exec_depend>`（実行時）/ `<depend>` で宣言。
- ビルド前に依存解決:
  ```bash
  rosdep install -i --from-path src --rosdistro jazzy -y
  ```
- 複数システムで再利用する共通ノードは、別リポジトリ化して `<system>.repos`
  （vcstool マニフェスト）で取り込む。詳細は方針書を参照。

## アーキテクチャ依存ノード

arm64 専用ノード等は独立パッケージに分離し、arm64 追加分の `.repos`
（例 `*.arm64.repos`）または `COLCON_IGNORE` で取り込み・ビルドを制御する。
詳細は [`node_development_architecture.md`](node_development_architecture.md) を参照。

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
