#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# vsnote —— 在 Linux x86_64 (amd64) 上构建 RPM 安装包
#
# 用法（在 WSL / 任意 Linux 机器上）：
#   bash scripts/build-linux-rpm.sh
#
# 可选环境变量：
#   VSNOTE_CN_MIRROR=1   使用国内镜像（rustup 清华 / npm npmmirror / crates.io 镜像）
#                        本机网络访问 GitHub Release 常被代理拦截时建议开启
#   KEEP_WORKDIR=1       保留构建目录 ~/vsnote-build，便于排查
#
# 产物：
#   src-tauri/target/release/bundle/rpm/vsnote-<ver>-1.x86_64.rpm
#   并复制到 <项目根>/dist/
# ---------------------------------------------------------------------------
set -euo pipefail

APP_NAME="vsnote"
WORKDIR="${HOME}/vsnote-build"

log()  { printf '\n\033[1;36m[%s]\033[0m %s\n' "$(date +%H:%M:%S)" "$*"; }
warn() { printf '\033[1;33m[WARN]\033[0m %s\n' "$*" >&2; }
die()  { printf '\n\033[1;31m[FATAL]\033[0m %s\n' "$*" >&2; exit 1; }

# ---------------------------------------------------------------- 0. 前置检查
ARCH="$(uname -m)"
[ "$ARCH" = "x86_64" ] || die "RPM 目标为 x86_64(amd64)，当前架构是 ${ARCH}"

SRC_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
log "源码根目录: ${SRC_ROOT}"

# ------------------------------------------------- 1. 搬到 Linux 原生文件系统
# 在 /mnt/* (Windows drvfs) 上跑 cargo/npm 会极慢且权限异常，
# 因此统一复制到 ext4 本地目录再构建。
if [[ "$SRC_ROOT" == /mnt/* ]]; then
  log "检测到源码位于 Windows 挂载目录，复制到 ${WORKDIR}"
  mkdir -p "$WORKDIR"
  if command -v rsync >/dev/null 2>&1; then
    rsync -a --delete \
      --exclude 'node_modules' --exclude 'src-tauri/target' \
      --exclude '.git' --exclude 'dist' --exclude 'release' \
      "$SRC_ROOT/" "$WORKDIR/"
  else
    rm -rf "$WORKDIR"
    cp -a "$SRC_ROOT" "$WORKDIR"
    rm -rf "$WORKDIR/node_modules" "$WORKDIR/src-tauri/target" "$WORKDIR/.git"
  fi
  cd "$WORKDIR"
else
  WORKDIR="$SRC_ROOT"
  cd "$WORKDIR"
fi
log "构建目录: ${WORKDIR}"

VERSION="$(node -p "require('./package.json').version" 2>/dev/null || true)"
[ -n "$VERSION" ] || VERSION="$(grep -m1 '"version"' package.json | sed -E 's/.*"version"[^0-9]*([0-9.]+).*/\1/')"
log "版本号: ${VERSION}"

# ------------------------------------------------------------ 2. 国内镜像开关
if [ "${VSNOTE_CN_MIRROR:-0}" = "1" ]; then
  log "启用国内镜像"
  export RUSTUP_DIST_SERVER="https://mirrors.tuna.tsinghua.edu.cn/rustup"
  export RUSTUP_UPDATE_ROOT="https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup"
  export CARGO_HTTP_MULTIPLEXING=false
  mkdir -p "${HOME}/.cargo"
  cat > "${HOME}/.cargo/config.toml" <<'EOF'
[source.crates-io]
replace-with = 'mirror'
[source.mirror]
registry = "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"
EOF
fi

# ---------------------------------------------------------- 3. 系统依赖
# wsl --import 导入的发行版默认以 root 登录，此时不需要（也没有）sudo
if [ "$(id -u)" = "0" ]; then SUDO=""; else SUDO="sudo"; fi

# WSL 偶尔 generateResolvConf 失败，导致 apt 连不上任何源
if ! getent hosts archive.ubuntu.com >/dev/null 2>&1 \
   && ! getent hosts mirrors.tuna.tsinghua.edu.cn >/dev/null 2>&1; then
  warn "DNS 解析异常，写入公共 DNS"
  printf 'nameserver 223.5.5.5\nnameserver 8.8.8.8\n' > /etc/resolv.conf
fi

# Ubuntu 官方源在部分网络下很慢或不通，CN 镜像模式换成清华源
if [ "${VSNOTE_CN_MIRROR:-0}" = "1" ] && command -v apt-get >/dev/null 2>&1; then
  CODENAME="$(. /etc/os-release && echo "${UBUNTU_CODENAME:-${VERSION_CODENAME:-}}")"
  if [ -n "$CODENAME" ]; then
    log "apt 源切换为清华镜像 (${CODENAME})"
    if [ -f /etc/apt/sources.list ]; then
      cp /etc/apt/sources.list /etc/apt/sources.list.bak
    fi
    if [ -f /etc/apt/sources.list.d/ubuntu.sources ]; then
      cp /etc/apt/sources.list.d/ubuntu.sources /etc/apt/sources.list.d/ubuntu.sources.bak
    fi
    cat > /etc/apt/sources.list <<EOF
deb https://mirrors.tuna.tsinghua.edu.cn/ubuntu/ ${CODENAME} main restricted universe multiverse
deb https://mirrors.tuna.tsinghua.edu.cn/ubuntu/ ${CODENAME}-updates main restricted universe multiverse
deb https://mirrors.tuna.tsinghua.edu.cn/ubuntu/ ${CODENAME}-security main restricted universe multiverse
EOF
    rm -f /etc/apt/sources.list.d/ubuntu.sources
  fi
fi

log "安装 Tauri v2 Linux 系统依赖"
if command -v apt-get >/dev/null 2>&1; then
  export DEBIAN_FRONTEND=noninteractive
  $SUDO apt-get update -qq
  $SUDO apt-get install -y \
    libwebkit2gtk-4.1-dev build-essential curl wget file git rsync \
    libssl-dev libayatana-appindicator3-dev librsvg2-dev
elif command -v dnf >/dev/null 2>&1; then
  $SUDO dnf install -y \
    webkit2gtk4.1-devel openssl-devel curl wget file git \
    libappindicator-gtk3-devel librsvg2-devel gcc gcc-c++ make
elif command -v pacman >/dev/null 2>&1; then
  $SUDO pacman -Syu --noconfirm --needed \
    webkit2gtk-4.1 base-devel curl wget file git openssl librsvg libappindicator-gtk3
else
  warn "未识别的包管理器，请手动安装 webkit2gtk-4.1 / gtk3 / openssl 开发包"
fi

# ------------------------------------------------------------- 4. Rust 工具链
# 坑：cargo 默认用内置 libgit2 拉 git 依赖，在受限网络下会 SSL 超时并反复重试；
#     系统 git CLI 通常能通，强制切过去（brotli 的 git patch 必须靠它）
mkdir -p "${HOME}/.cargo"
if [ -f "${HOME}/.cargo/config.toml" ]; then
  if ! grep -q 'git-fetch-with-cli' "${HOME}/.cargo/config.toml"; then
    printf '\n[net]\ngit-fetch-with-cli = true\n' >> "${HOME}/.cargo/config.toml"
  fi
else
  printf '[net]\ngit-fetch-with-cli = true\n' > "${HOME}/.cargo/config.toml"
fi
log "cargo 网络配置: $(cat "${HOME}/.cargo/config.toml" | tr '\n' ' ')"

# 注意：必须先补 PATH 再检测，否则每次都会重装 rustup
export PATH="${HOME}/.cargo/bin:${PATH}"
if ! command -v cargo >/dev/null 2>&1; then
  log "安装 Rust stable"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  # shellcheck disable=SC1090
  . "${HOME}/.cargo/env"
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi
log "Rust: $(rustc --version)"

# --------------------------------------------------------------- 5. Node 环境
# 坑：WSL 会把 Windows 的 PATH 追加进来，可能只解析到 Windows 版 npm，
#     那样 esbuild 等原生模块会平台不匹配，所以必须装 Linux 原生 Node
export PATH="/usr/local/bin:${PATH}"
NEED_NODE=0
if ! command -v node >/dev/null 2>&1; then
  NEED_NODE=1
else
  NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)"
  if [ "${NODE_MAJOR:-0}" -lt 18 ] 2>/dev/null; then NEED_NODE=1; fi
  if [[ "$(command -v node)" == /mnt/* ]]; then NEED_NODE=1; fi
fi
if [ "$NEED_NODE" = "1" ]; then
  NODE_VER="v22.22.2"
  NODE_TAR="node-${NODE_VER}-linux-x64.tar.xz"
  log "安装 Node.js ${NODE_VER} 到 /usr/local"
  if [ "${VSNOTE_CN_MIRROR:-0}" = "1" ]; then
    NODE_URL="https://mirrors.tuna.tsinghua.edu.cn/nodejs-release/${NODE_VER}/${NODE_TAR}"
  else
    NODE_URL="https://nodejs.org/dist/${NODE_VER}/${NODE_TAR}"
  fi
  curl -fsSL -o /tmp/"${NODE_TAR}" "${NODE_URL}" \
    || curl -fsSL -o /tmp/"${NODE_TAR}" \
       "https://mirrors.tuna.tsinghua.edu.cn/nodejs-release/${NODE_VER}/${NODE_TAR}"
  tar -xJf /tmp/"${NODE_TAR}" -C /usr/local --strip-components=1
  hash -r
fi
log "Node: $(node --version) / npm: $(npm --version) [path=$(command -v node)]"
if [ "${VSNOTE_CN_MIRROR:-0}" = "1" ]; then
  npm config set registry https://registry.npmmirror.com
fi

# ------------------------------- 6. 移除 Windows 专用 patch（Linux 下必须去掉）
CARGO_TOML="src-tauri/Cargo.toml"
if grep -q 'vsbuild/patches/schemars' "$CARGO_TOML"; then
  log "移除 Windows 专用 schemars patch（Linux 上该路径不存在，会导致构建失败）"
  cp "$CARGO_TOML" "${CARGO_TOML}.orig"
  sed -i '/vsbuild\/patches\/schemars/d' "$CARGO_TOML"
fi

# ⚠️ brotli 的 git patch 必须保留，不能退回 crates.io 版本！
#    crates.io 的 brotli 8.0.x 写死 alloc-no-stdlib "^2.0.4" + alloc-stdlib "~0.5"，
#    而 brotli-decompressor 5.x 要 alloc-no-stdlib 3.0 → 依赖图里出现 2.0.4 与 3.0.0 两个版本，
#    编译报 error[E0277]（combined_alloc.rs 共 36 处）：
#      the trait bound `StandardAlloc: alloc::Allocator<ZopfliNode>` is not satisfied
#    而 git rev 9cbed81 把约束放宽成 ">=2.0.4, <4"，能让整个图统一到单一版本。
# 但本机 WSL 里 github 极不稳定：`git ls-remote` 偶尔成功，真正 fetch 会卡 134s 后超时。
# 所以只要本地有副本就优先用本地（与 git rev 完全等价，且离线可靠）。
if grep -q 'rust-brotli' "$CARGO_TOML"; then
  BROTLI_TARBALL="/mnt/c/Users/45574/wsl-dl/rust-brotli.tar.gz"
  BROTLI_REV="9cbed81a1a098fcd4ffcf956429dce6154415970"
  if [ ! -d /root/rust-brotli ] && [ -f "$BROTLI_TARBALL" ]; then
    log "解压 brotli 本地副本（来自 Windows 侧已下载的 tarball）"
    rm -rf /root/brotli-extract && mkdir -p /root/brotli-extract
    tar -xzf "$BROTLI_TARBALL" -C /root/brotli-extract
    mv "/root/brotli-extract/rust-brotli-${BROTLI_REV}" /root/rust-brotli
    rm -rf /root/brotli-extract
  fi
  if [ -d /root/rust-brotli ]; then
    warn "brotli 使用本地 path 副本: /root/rust-brotli（github fetch 本机不可靠）"
    [ -f "${CARGO_TOML}.orig" ] || cp "$CARGO_TOML" "${CARGO_TOML}.orig"
    sed -i 's|^brotli = .*|brotli = { path = "/root/rust-brotli" }|' "$CARGO_TOML"
  else
    warn "无本地副本，尝试直接从 github 拉取（可能超时）"
  fi
fi

# ------------------------------------------------------------ 7. 前端依赖
log "安装前端依赖"
npm ci --no-audit --no-fund 2>/dev/null || npm install --no-audit --no-fund

log "构建渲染 bundle（tauri.conf.json 无 beforeBuildCommand，必须手动执行）"
npm run build:renderer

# ------------------------------------------ 8. 强制重建 vsnote 内嵌前端产物
# ⚠️ 关键坑（2026-10-13 实测）：tauri 通过 cargo 的 build script 把 frontendDist(../src)
#    内嵌进二进制。若本次只改了前端（src/*.js|html|css）而 src-tauri/ 的 Rust 源码没变，
#    cargo 会认为 vsnote 的 build script 无需重跑，于是**内嵌的仍是上一次的旧前端** ——
#    表现为"rpm 装出来还是旧版界面"，而且前端 diff、源码 rsync 全都查不出问题。
#    这里主动删掉 vsnote 的 build script 产物与指纹，强制 tauri 重新读取并内嵌最新前端。
log "强制重建 vsnote build script 产物（确保内嵌的是最新前端）"
rm -rf src-tauri/target/release/build/vsnote-* src-tauri/target/release/.fingerprint/vsnote-*

# --------------------------------------------------------------- 9. 构建 RPM
log "开始构建 RPM（首次全量编译约 15~30 分钟）"
npm run tauri -- build --bundles rpm

RPM_DIR="src-tauri/target/release/bundle/rpm"
RPM_FILE="$(ls -1 "${RPM_DIR}"/*.rpm 2>/dev/null | head -1 || true)"
[ -n "$RPM_FILE" ] || die "未生成 RPM，检查 ${RPM_DIR}"

log "构建完成: ${RPM_FILE}"
ls -lh "$RPM_FILE"
file "$RPM_FILE"

# ----------------------------------------------------------- 10. 归档到 dist
mkdir -p "${SRC_ROOT}/dist"
cp -f "$RPM_FILE" "${SRC_ROOT}/dist/"
log "已复制到: ${SRC_ROOT}/dist/$(basename "$RPM_FILE")"

# -------------------------------------------------------- 11. 还原 Cargo.toml
if [ -f "${CARGO_TOML}.orig" ]; then
  mv -f "${CARGO_TOML}.orig" "$CARGO_TOML"
  log "已还原 src-tauri/Cargo.toml"
fi

if [ "${KEEP_WORKDIR:-0}" != "1" ] && [ "$WORKDIR" != "$SRC_ROOT" ]; then
  log "提示：构建目录 ${WORKDIR} 已保留（设 KEEP_WORKDIR=0 之外默认保留，方便排查）"
fi

printf '\n\033[1;32mDONE\033[0m  %s\n' "$(basename "$RPM_FILE")"
