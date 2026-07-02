# Pi4 ホストへの C++ / Rust ビルド環境セットアップ

Raspberry Pi 4 の**ホスト OS 上に直接**（Docker を介さず）C++ と Rust の
ビルド環境を構築する手順。ROS2 とは非連携の**汎用開発環境**を対象とする。

> **位置づけ**: 本手順は ROS2 ノード開発の Docker/コンテナワークフローとは
> **意図的に分離**したホスト側の汎用ツールチェーン整備である。ROS2 ノードの
> ビルドは従来どおりベースイメージ `ghcr.io/demitas1/ros2_jazzy` のコンテナ内で行う
> （コンテナ側は C++ が既に完備、Rust は未導入）。本手順はリポジトリのコードには影響しない。
>
> 将来 Rust を ROS2 と連携させる場合（ros2-rust / rclrs）はコンテナ側の別作業となり、
> 本手順のスコープ外。

## 対象環境（検証時）

| 項目 | 値 |
|------|----|
| ハード | Raspberry Pi 4（aarch64 / RAM 3.7GB） |
| OS | Debian GNU/Linux 13 (trixie) |
| アクセス | `ssh rpi4-wifi`（ユーザー `demitas`・passwordless sudo） |
| 既存 C++ | `build-essential 12.12`（gcc/g++ **14.2**・make）導入済み。cmake は未導入 |
| 既存 Rust | なし（rustc/cargo/rustup 未導入） |

> 実システムでは対象ホスト・ユーザー名・OS バージョンを実環境に置き換える。
> 以降のコマンドは開発機から `ssh rpi4-wifi '...'` で実行する例。ホストに直接
> ログインして実行する場合は `ssh rpi4-wifi '...'` のラップを外す。

## 1. C++ ツールチェーン（apt）

`build-essential`（gcc/g++/make）は Debian 標準で導入済みのことが多い。不足する
`cmake` と一般的な開発ツールを apt で補完する。

```bash
ssh rpi4-wifi 'sudo apt-get update && \
  sudo apt-get install -y cmake gdb pkg-config ninja-build'
```

- `cmake` … 素の C++ ビルドの主軸（trixie の候補は `3.31.6`）
- `gdb` … デバッガ、`pkg-config` … ライブラリ検出、`ninja-build` … 高速ビルドジェネレータ
- `build-essential` が未導入の環境では `sudo apt-get install -y build-essential` も追加する
- （任意・エディタ補完向け）`clangd` / `clang-format` を足してもよい

## 2. Rust ツールチェーン（rustup / 素の cargo）

Debian の apt `rustc`/`cargo` ではなく、公式 **rustup** で導入する
（toolchain のバージョン管理・更新が容易）。非対話（`-y`）・既定 profile で
stable（rustc / cargo / rustfmt / clippy、host triple `aarch64-unknown-linux-gnu`）が入る。

```bash
ssh rpi4-wifi 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
```

- インストール先は `~/.cargo`（バイナリ）/ `~/.rustup`（toolchain 実体）
- rustup が `~/.bashrc` / `~/.profile` に PATH（`$HOME/.cargo/bin`）を追記するため、
  **次回ログインシェルから自動で有効**になる
- 現在の SSH セッションで即使う場合のみ `source ~/.cargo/env` が必要
- 既に `~/.rustup/settings.toml` がある環境では、そこの default toolchain 設定が優先される

更新・toolchain 管理:

```bash
rustup update              # stable の更新
rustup component add rust-analyzer   # 任意（LSP を rustup 管理下に置く場合）
```

## 3. 検証（end-to-end）

### C++（cmake + Ninja で最小プロジェクトをビルド・実行）

```bash
ssh rpi4-wifi 'set -e; d=$(mktemp -d); cd "$d";
cat > CMakeLists.txt <<EOF
cmake_minimum_required(VERSION 3.16)
project(hello CXX)
add_executable(hello main.cpp)
EOF
cat > main.cpp <<EOF
#include <iostream>
int main(){ std::cout << "C++ ok, std=" << __cplusplus << std::endl; return 0; }
EOF
cmake -G Ninja -B build >/dev/null 2>&1
cmake --build build >/dev/null 2>&1
./build/hello
rm -rf "$d"'
```

期待出力: `C++ ok, std=201703`（g++ 14 の既定 C++17）。

### Rust（cargo new → run）

```bash
ssh rpi4-wifi 'source ~/.cargo/env; set -e; d=$(mktemp -d); cd "$d";
cargo new hello -q && cd hello && cargo run -q;
rustc --version; cargo --version; rm -rf "$d"'
```

期待出力: `Hello, world!` と rustc / cargo のバージョン。

## 4. 完了確認

全ツールがパス解決できれば完了。

```bash
ssh rpi4-wifi 'for t in gcc g++ make cmake ninja gdb rustc cargo rustup; do \
  printf "%-8s " $t; (source ~/.cargo/env 2>/dev/null; command -v $t || echo MISSING); done'
```

検証時の実測:

```
gcc      /usr/bin/gcc
g++      /usr/bin/g++
make     /usr/bin/make
cmake    /usr/bin/cmake          (3.31.6)
ninja    /usr/bin/ninja
gdb      /usr/bin/gdb
rustc    /home/demitas/.cargo/bin/rustc   (1.96.1 stable)
cargo    /home/demitas/.cargo/bin/cargo   (1.96.1 stable)
rustup   /home/demitas/.cargo/bin/rustup
```

## 補足

- **sudo**: passwordless sudo 前提で apt を非対話実行している。パスワードが必要な環境では
  ホストに直接ログインするか、`ssh -t` で実行する。
- **アンインストール**: Rust は `rustup self uninstall`、apt 追加分は
  `sudo apt-get remove cmake gdb pkg-config ninja-build` で戻せる。
- **管理方針**: 本ドキュメントは bobtail システムリポジトリに属する。ROS2 の
  ビルド/デプロイ手順（`deployment.md`）とは独立した Pi ホスト環境の整備メモである。
