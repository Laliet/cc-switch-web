#!/usr/bin/env bash
# CC-Switch Web Server 一键部署脚本
# Usage: curl -fsSL https://raw.githubusercontent.com/Laliet/cc-switch-web/main/scripts/deploy-web.sh | bash
#
# 快速部署（推荐）- 使用预编译二进制：
#   INSTALL_DIR=/opt/cc-switch curl -fsSL .../deploy-web.sh | bash -s -- --prebuilt
#
# 从源码编译：
#   curl -fsSL .../deploy-web.sh | bash
#
# 环境变量：
#   HOST         - 监听地址，默认 0.0.0.0
#   PORT         - 监听端口，默认 3000
#   INSTALL_DIR  - 安装目录，默认 ~/cc-switch-web
#   LIBC_VARIANT - 预编译二进制 libc 变体：auto/gnu/musl（默认 auto）

set -euo pipefail

# 颜色输出
log() { printf '\033[1;34m[CC-Switch]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[CC-Switch]\033[0m %s\n' "$*"; }
err() { printf '\033[1;31m[CC-Switch]\033[0m %s\n' "$*" >&2; }
success() { printf '\033[1;32m[CC-Switch]\033[0m %s\n' "$*"; }

# 配置
REPO_URL="${REPO_URL:-https://github.com/Laliet/cc-switch-web.git}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/cc-switch-web}"
HOST="${HOST:-0.0.0.0}"
PORT="${PORT:-3000}"
PREBUILT=0

# 检查命令是否存在
need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    return 1
  fi
  return 0
}

version_ge() {
  [ "$1" = "$2" ] && return 0
  printf '%s\n%s\n' "$2" "$1" | sort -V -C
}

get_glibc_version() {
  if ! need_cmd ldd; then
    return 1
  fi

  local line
  line=$(ldd --version 2>/dev/null | head -1 || true)
  if echo "$line" | grep -qi "musl"; then
    echo "musl"
    return 0
  fi

  echo "$line" | grep -Eo '[0-9]+\.[0-9]+' | head -1
}

# 检查并安装依赖
check_dependencies() {
  log "检查系统依赖..."

  local require_build_tools="${1:-1}"
  local missing=()

  # 检查 Git
  if ! need_cmd git; then
    missing+=("git")
  fi

  if [[ "$require_build_tools" == "1" ]]; then
    # 检查 Node.js
    if ! need_cmd node; then
      missing+=("nodejs")
    else
      local node_version
      node_version=$(node -v | sed 's/v//' | cut -d. -f1)
      if [[ "$node_version" -lt 18 ]]; then
        warn "Node.js 版本过低 (需要 >= 18)，当前: $(node -v)"
        missing+=("nodejs>=18")
      fi
    fi

    # 检查 pnpm
    if ! need_cmd pnpm; then
      log "安装 pnpm..."
      npm install -g pnpm || {
        err "pnpm 安装失败，请手动安装: npm install -g pnpm"
        exit 1
      }
    fi

    # 检查 Rust/Cargo
    if ! need_cmd cargo; then
      missing+=("rust/cargo")
    fi
  fi

  # 检查 curl
  if ! need_cmd curl; then
    missing+=("curl")
  fi

  # 检查 Python3
  if ! need_cmd python3; then
    missing+=("python3")
  fi

  if [[ ${#missing[@]} -gt 0 ]]; then
    err "缺少以下依赖: ${missing[*]}"
    echo ""
    echo "请先安装缺失的依赖："
    echo "  - Git: sudo apt install git"
    if [[ "$require_build_tools" == "1" ]]; then
      echo "  - Node.js 18+: https://nodejs.org/ 或 nvm install 20"
      echo "  - Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    fi
    echo "  - curl: sudo apt install curl"
    echo "  - Python3: sudo apt install python3"
    exit 1
  fi

  success "依赖检查通过"
}

# 安装 Linux 系统依赖（用于编译 Tauri）
install_linux_deps() {
  if [[ "$(uname)" != "Linux" ]]; then
    return 0
  fi

  log "检查 Linux 编译依赖..."

  # 检测包管理器
  if need_cmd apt-get; then
    # Debian/Ubuntu
    local packages=(
      build-essential
      pkg-config
      libssl-dev
    )

    local to_install=()
    for pkg in "${packages[@]}"; do
      if ! dpkg -s "$pkg" >/dev/null 2>&1; then
        to_install+=("$pkg")
      fi
    done

    if [[ ${#to_install[@]} -gt 0 ]]; then
      log "安装编译依赖: ${to_install[*]}"
      sudo apt-get update
      sudo apt-get install -y "${to_install[@]}"
    fi
  elif need_cmd dnf; then
    # Fedora/RHEL
    sudo dnf install -y \
      gcc gcc-c++ \
      openssl-devel \
      pkg-config
  elif need_cmd pacman; then
    # Arch Linux
    sudo pacman -S --needed --noconfirm \
      base-devel \
      openssl \
      pkgconf
  fi

  success "Linux 编译依赖就绪"
}

# 克隆或更新仓库
clone_or_update() {
  if [[ -d "$INSTALL_DIR/.git" ]]; then
    log "更新已有仓库..."
    cd "$INSTALL_DIR"
    git fetch origin
    git reset --hard origin/main
  else
    log "克隆仓库到 $INSTALL_DIR..."
    local retry=0
    local max_retries=3
    while [[ $retry -lt $max_retries ]]; do
      if git clone --depth 1 "$REPO_URL" "$INSTALL_DIR"; then
        break
      fi
      retry=$((retry + 1))
      if [[ $retry -lt $max_retries ]]; then
        warn "克隆失败，第 $retry 次重试..."
        rm -rf "$INSTALL_DIR"
        sleep 2
      else
        err "克隆仓库失败，请检查网络连接"
        exit 1
      fi
    done
    cd "$INSTALL_DIR"
  fi
  success "代码就绪"
}

# 构建前端
build_frontend() {
  log "安装前端依赖..."
  pnpm install --frozen-lockfile || pnpm install

  log "构建 Web 资源..."
  pnpm build:web

  success "前端构建完成"
}

# 构建后端
build_backend() {
  log "构建 Rust Web 服务器..."
  cd src-tauri
  cargo build --release --no-default-features --features web-server --example server
  cd ..

  success "后端构建完成"
}

download_prebuilt() {
  log "下载预编译的 CC-Switch Web 服务器..."

  local arch
  case "$(uname -m)" in
    x86_64|amd64) arch="x86_64" ;;
    aarch64|arm64) arch="aarch64" ;;
    *)
      err "暂不支持的架构: $(uname -m)"
      exit 1
      ;;
  esac

  local target_path="$INSTALL_DIR/src-tauri/target/release/examples/server"
  local glibc_version
  local requested_variant="${LIBC_VARIANT:-auto}"
  local chosen_variant=""
  local -a candidates=()
  local -a attempted_urls=()
  local download_url=""

  glibc_version=$(get_glibc_version || true)

  case "$requested_variant" in
    auto|"")
      if [[ "$glibc_version" == "musl" ]]; then
        candidates=(musl)
      elif [[ -z "$glibc_version" ]]; then
        warn "未检测到 glibc 版本，优先尝试 musl 预编译以提高兼容性。"
        candidates=(musl gnu)
      elif version_ge "$glibc_version" "2.31"; then
        candidates=(gnu musl)
      else
        warn "检测到 glibc $glibc_version (< 2.31)，优先尝试 musl 预编译。"
        candidates=(musl gnu)
      fi
      ;;
    gnu|glibc)
      candidates=(gnu)
      ;;
    musl)
      candidates=(musl)
      ;;
    *)
      err "LIBC_VARIANT 仅支持 auto/gnu/musl，当前：${LIBC_VARIANT}"
      exit 1
      ;;
  esac

  mkdir -p "$(dirname "$target_path")"

  for candidate in "${candidates[@]}"; do
    if [[ "$candidate" == "gnu" ]]; then
      if [[ "$glibc_version" == "musl" ]]; then
        if [[ "$requested_variant" == "gnu" || "$requested_variant" == "glibc" ]]; then
          err "检测到 musl libc（如 Alpine），无法使用 gnu 预编译。请改用 LIBC_VARIANT=musl。"
          exit 1
        fi
        warn "当前系统为 musl，跳过 gnu 预编译。"
        continue
      fi
      if [[ -n "$glibc_version" ]] && ! version_ge "$glibc_version" "2.31"; then
        if [[ "$requested_variant" == "gnu" || "$requested_variant" == "glibc" ]]; then
          err "当前 glibc 版本为 $glibc_version，gnu 预编译需要 glibc >= 2.31。请改用 LIBC_VARIANT=musl。"
          exit 1
        fi
        warn "当前 glibc 版本为 $glibc_version，跳过 gnu 预编译。"
        continue
      fi
    fi

    local suffix=""
    if [[ "$candidate" == "musl" ]]; then
      suffix="-musl"
    fi
    download_url="https://github.com/Laliet/cc-switch-web/releases/latest/download/cc-switch-server-linux-${arch}${suffix}"
    attempted_urls+=("$download_url")
    log "尝试下载 ${candidate} 预编译: $download_url"

    if curl -fL "$download_url" -o "$target_path"; then
      chosen_variant="$candidate"
      chmod +x "$target_path"
      break
    fi

    warn "下载失败：$download_url"
    rm -f "$target_path"
  done

  if [[ -z "$chosen_variant" ]]; then
    err "预编译二进制下载失败（已尝试：${attempted_urls[*]:-无}）。"
    err "建议改用 Docker（ghcr.io/laliet/cc-switch-web:latest）或源码构建（不加 --prebuilt）。"
    exit 1
  fi

  success "预编译服务器已下载(${chosen_variant}): $target_path"
}

# 创建 systemd 服务（可选）
create_systemd_service() {
  if [[ "${CREATE_SERVICE:-0}" != "1" ]]; then
    return 0
  fi

  log "创建 systemd 服务..."

  local service_file="/etc/systemd/system/cc-switch-web.service"
  local server_path="$INSTALL_DIR/src-tauri/target/release/examples/server"

  sudo tee "$service_file" > /dev/null <<EOF
[Unit]
Description=CC-Switch Web Server
After=network.target

[Service]
Type=simple
User=$USER
WorkingDirectory=$INSTALL_DIR
Environment=HOST=$HOST
Environment=PORT=$PORT
ExecStart=$server_path
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

  sudo systemctl daemon-reload
  sudo systemctl enable cc-switch-web

  success "systemd 服务已创建: cc-switch-web"
}

# 显示完成信息
show_completion() {
  local server_path="$INSTALL_DIR/src-tauri/target/release/examples/server"
  local password_file="$HOME/.cc-switch/web_password"

  echo ""
  success "=========================================="
  success "  CC-Switch Web 服务器部署完成！"
  success "=========================================="
  echo ""
  echo "启动服务器："
  echo "  cd $INSTALL_DIR"
  echo "  HOST=$HOST PORT=$PORT $server_path"
  echo ""
  echo "或使用快捷命令："
  echo "  $INSTALL_DIR/scripts/start-web.sh"
  echo ""
  echo "访问地址："
  echo "  http://$HOST:$PORT"
  echo ""
  echo "登录凭据："
  echo "  用户名: admin"
  echo "  密码: 首次启动后查看 $password_file"
  echo ""
  if [[ "${CREATE_SERVICE:-0}" == "1" ]]; then
    echo "systemd 服务管理："
    echo "  sudo systemctl start cc-switch-web   # 启动"
    echo "  sudo systemctl stop cc-switch-web    # 停止"
    echo "  sudo systemctl status cc-switch-web  # 状态"
    echo ""
  fi
  echo "如需创建 systemd 服务，运行："
  echo "  CREATE_SERVICE=1 $0"
  echo ""
}

# 创建启动脚本
create_start_script() {
  local start_script="$INSTALL_DIR/scripts/start-web.sh"

  cat > "$start_script" <<EOF
#!/usr/bin/env bash
# CC-Switch Web 服务器启动脚本

cd "\$(dirname "\$0")/.."
HOST="\${HOST:-$HOST}" PORT="\${PORT:-$PORT}" ./src-tauri/target/release/examples/server
EOF

  chmod +x "$start_script"
  log "创建启动脚本: $start_script"
}

# 主函数
main() {
  echo ""
  log "CC-Switch Web 服务器一键部署"
  log "=============================="
  echo ""

  for arg in "$@"; do
    case "$arg" in
      --prebuilt) PREBUILT=1 ;;
    esac
  done

  if [[ "$PREBUILT" == "1" ]]; then
    log "使用预编译二进制部署"
  fi

  check_dependencies "$((1 - PREBUILT))"
  install_linux_deps
  clone_or_update
  if [[ "$PREBUILT" == "1" ]]; then
    download_prebuilt
  else
    build_frontend
    build_backend
  fi
  create_start_script
  create_systemd_service
  show_completion
}

main "$@"
