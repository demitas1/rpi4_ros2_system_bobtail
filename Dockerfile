# system_bobtail 本番イメージ（方式B: 本番配布）
# 対応ベースイメージ: ghcr.io/demitas1/ros2_jazzy
# （運用時は :latest ではなく tag 固定を推奨）
#
# このイメージは docker-compose.prod.yml の build から焼かれ、CI（issue #1）で
# ghcr に push される。Pi は pull するだけで起動する。
# 開発中にソースからビルドする手順（方式A）は docs/deployment.md / docker-compose.yml。
FROM ghcr.io/demitas1/ros2_jazzy:latest

WORKDIR /home/ros2_user/ros2_ws

# システム固有ノードをコピー
COPY src ./src

# 共通ノードを vcstool で取得する場合は以下を有効化
# COPY system_bobtail.repos .
# RUN vcs import src < system_bobtail.repos

# 依存解決とビルド
RUN bash -lc "source /opt/ros/jazzy/setup.bash \
 && rosdep install --from-paths src -y --ignore-src \
 && colcon build"

CMD ["bash"]
