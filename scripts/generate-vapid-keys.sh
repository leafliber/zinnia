#!/usr/bin/env bash
# ===========================================
# VAPID 密钥生成脚本
# ===========================================
# 用于为 Web Push 通知生成 VAPID 密钥对
# 
# 使用方法:
#   ./scripts/generate-vapid-keys.sh
#
# 输出格式:
#   - 公钥和私钥将输出到控制台
#   - 可选：自动添加到 .env 文件

set -euo pipefail

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

print_header() {
    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}  $*${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""
}

# 项目根目录
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

print_header "VAPID 密钥生成器"

log_info "Web Push 使用 VAPID (Voluntary Application Server Identification) 协议"
log_info "需要生成一对公钥和私钥用于身份验证"
echo ""

# 优先使用 Docker/Podman 临时容器来执行 web-push（避免依赖本地 npx）
if command -v docker >/dev/null 2>&1; then
    log_info "使用 Docker 容器生成 VAPID 密钥..."
    vapid_keys=$(docker run --rm -v "$ROOT_DIR":/work -w /work node:18-bullseye-slim npx -y web-push generate-vapid-keys --json 2>/dev/null || echo "")
elif command -v podman >/dev/null 2>&1; then
    log_info "使用 Podman 容器生成 VAPID 密钥..."
    vapid_keys=$(podman run --rm -v "$ROOT_DIR":/work -w /work docker.io/node:18-bullseye-slim npx -y web-push generate-vapid-keys --json 2>/dev/null || echo "")
elif command -v npx >/dev/null 2>&1; then
    log_info "使用本地 npx 生成 VAPID 密钥..."
    vapid_keys=$(npx -y web-push generate-vapid-keys --json 2>/dev/null || echo "")
else
    log_error "未检测到 Docker/Podman 或本地 npx，无法自动生成 VAPID 密钥"
    log_info "请先安装 Node.js 或 Docker，或手动使用 web-push 生成密钥"
    log_info "在线工具: https://web-push-codelab.glitch.me/"
    exit 1
fi

log_success "已生成 VAPID 密钥（或已尝试生成）"

# 解析 JSON 输出
vapid_public=$(echo "$vapid_keys" | grep -o '"publicKey":"[^"]*"' | cut -d'"' -f4)
vapid_private=$(echo "$vapid_keys" | grep -o '"privateKey":"[^"]*"' | cut -d'"' -f4)

if [ -z "$vapid_public" ] || [ -z "$vapid_private" ]; then
    log_error "解析 VAPID 密钥失败"
    log_info "原始输出: $vapid_keys"
    exit 1
fi

print_header "生成的 VAPID 密钥"

echo "公钥 (Public Key):"
echo -e "${GREEN}$vapid_public${NC}"
echo ""
echo "私钥 (Private Key):"
echo -e "${YELLOW}$vapid_private${NC}"
echo ""

log_warn "请妥善保管私钥，不要泄露！"
echo ""

# 询问是否写入 .env 文件
read -p "是否自动添加到 .env 文件？[y/N] " -r write_env

if [[ $write_env =~ ^[Yy]$ ]]; then
    ENV_FILE=".env"
    
    if [ ! -f "$ENV_FILE" ]; then
        log_warn ".env 文件不存在，是否从 .env.example 创建？[y/N]"
        read -p "" -r create_env
        if [[ $create_env =~ ^[Yy]$ ]]; then
            cp .env.example "$ENV_FILE"
            log_success "已从 .env.example 创建 .env 文件"
        else
            log_info "跳过写入 .env 文件"
            exit 0
        fi
    fi
    
    # 检查是否已存在 VAPID 配置
    if grep -q "^VAPID_PUBLIC_KEY=" "$ENV_FILE"; then
        log_warn ".env 文件中已存在 VAPID 配置"
        read -p "是否覆盖现有配置？[y/N] " -r overwrite
        if [[ ! $overwrite =~ ^[Yy]$ ]]; then
            log_info "保留现有配置"
            exit 0
        fi
        
        # 替换现有配置
        if [[ "$OSTYPE" == "darwin"* ]]; then
            # macOS
            sed -i '' "s|^VAPID_PUBLIC_KEY=.*|VAPID_PUBLIC_KEY=$vapid_public|" "$ENV_FILE"
            sed -i '' "s|^VAPID_PRIVATE_KEY=.*|VAPID_PRIVATE_KEY=$vapid_private|" "$ENV_FILE"
        else
            # Linux
            sed -i "s|^VAPID_PUBLIC_KEY=.*|VAPID_PUBLIC_KEY=$vapid_public|" "$ENV_FILE"
            sed -i "s|^VAPID_PRIVATE_KEY=.*|VAPID_PRIVATE_KEY=$vapid_private|" "$ENV_FILE"
        fi
        log_success "已更新 .env 文件中的 VAPID 配置"
    else
        # 添加新配置
        cat >> "$ENV_FILE" <<EOF

# ============================================
# Web Push (PWA) 通知配置
# 自动生成于 $(date)
# ============================================
VAPID_PUBLIC_KEY=$vapid_public
VAPID_PRIVATE_KEY=$vapid_private
EOF
        log_success "已添加 VAPID 配置到 .env 文件"
    fi
else
    log_info "请手动将以上密钥添加到环境变量中"
    echo ""
    echo "添加到 .env 文件："
    echo "  VAPID_PUBLIC_KEY=$vapid_public"
    echo "  VAPID_PRIVATE_KEY=$vapid_private"
    echo ""
    echo "或者设置为环境变量："
    echo "  export VAPID_PUBLIC_KEY='$vapid_public'"
    echo "  export VAPID_PRIVATE_KEY='$vapid_private'"
fi

echo ""
print_header "后续步骤"

echo "1. 确保 VAPID 密钥已配置到环境变量"
echo "2. 重启应用以加载新配置"
echo "3. 前端可通过 GET /api/v1/web-push/vapid-key 获取公钥"
echo "4. 使用公钥订阅 Web Push 通知"
echo ""
log_info "参考文档: docs/WEB_PUSH_TESTING_GUIDE.md"

