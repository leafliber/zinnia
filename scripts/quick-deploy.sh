#!/usr/bin/env bash
# ===========================================
# Zinnia 快速测试部署脚本（Podman 版本）
# ===========================================

set -euo pipefail

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[✓]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[⚠]${NC} $*"; }
log_error() { echo -e "${RED}[✗]${NC} $*"; }

# 检测容器工具
if command -v podman >/dev/null 2>&1; then
    DOCKER_CMD="podman"
    if command -v podman-compose >/dev/null 2>&1; then
        COMPOSE_CMD="podman-compose"
    elif podman compose version >/dev/null 2>&1; then
        COMPOSE_CMD="podman compose"
    else
        log_error "未找到 podman-compose，请安装：brew install podman-compose"
        exit 1
    fi
elif command -v docker >/dev/null 2>&1; then
    DOCKER_CMD="docker"
    if docker compose version >/dev/null 2>&1; then
        COMPOSE_CMD="docker compose"
    elif command -v docker-compose >/dev/null 2>&1; then
        COMPOSE_CMD="docker-compose"
    else
        log_error "未找到 docker-compose"
        exit 1
    fi
else
    log_error "未找到 Docker 或 Podman"
    exit 1
fi

log_success "检测到: $DOCKER_CMD, $COMPOSE_CMD"

# 1. 构建镜像
log_info "构建 Zinnia 镜像..."
$COMPOSE_CMD -f docker-compose.prod.yml build zinnia

# 2. 启动服务
log_info "启动所有服务..."
$COMPOSE_CMD -f docker-compose.prod.yml up -d

# 3. 等待服务就绪
log_info "等待服务启动..."
sleep 10

# 4. 检查服务状态
log_info "检查服务状态..."
$COMPOSE_CMD -f docker-compose.prod.yml ps

# 5. 测试健康检查
log_info "测试健康检查端点..."
for i in {1..30}; do
    if curl -sf http://localhost/health >/dev/null 2>&1; then
        log_success "应用服务已就绪！"
        echo ""
        echo "访问地址: http://localhost"
        echo "健康检查: http://localhost/health"
        echo ""
        echo "查看日志: $COMPOSE_CMD -f docker-compose.prod.yml logs -f zinnia"
        exit 0
    fi
    echo -n "."
    sleep 2
done

log_error "应用服务启动超时"
log_info "查看日志："
$COMPOSE_CMD -f docker-compose.prod.yml logs zinnia
exit 1
