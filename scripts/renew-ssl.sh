#!/usr/bin/env bash
# ===========================================
# SSL 证书续签脚本（Docker 版本）
# 通过 certbot 容器管理证书
# ===========================================

set -euo pipefail

# 项目根目录
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

# 日志文件
LOG_DIR="$ROOT_DIR/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/ssl-renew.log"

# 配置文件
ENV_FILE=".env.production"
COMPOSE_FILE="docker-compose.prod.yml"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG_FILE"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*" | tee -a "$LOG_FILE"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*" | tee -a "$LOG_FILE"
}

log_info() {
    echo -e "${YELLOW}[INFO]${NC} $*" | tee -a "$LOG_FILE"
}

# 检测 Docker Compose 命令
detect_compose() {
    if docker compose version >/dev/null 2>&1; then
        COMPOSE="docker compose"
    elif command -v docker-compose >/dev/null 2>&1; then
        COMPOSE="docker-compose"
    else
        log_error "未找到 docker-compose"
        exit 1
    fi
}

# 读取域名配置
load_config() {
    if [ ! -f "$ENV_FILE" ]; then
        log_error "配置文件不存在: $ENV_FILE"
        exit 1
    fi
    
    source "$ENV_FILE"
    
    if [ -z "${DOMAIN:-}" ] || [ "$DOMAIN" = "localhost" ]; then
        log_error "未配置有效域名，无需续签"
        exit 0
    fi
    
    log_info "域名: $DOMAIN"
}

# 检查 certbot 容器状态
check_certbot_container() {
    log_info "检查 certbot 容器状态..."
    
    if ! docker ps --format '{{.Names}}' | grep -q "zinnia-certbot"; then
        log_error "certbot 容器未运行"
        log_info "启动 certbot 容器..."
        $COMPOSE -f "$COMPOSE_FILE" --env-file "$ENV_FILE" up -d certbot
        sleep 5
    fi
    
    log_success "certbot 容器运行中"
}

# 检查证书到期时间
check_certificate_expiry() {
    log_info "检查证书到期时间..."
    
    # 在 certbot 容器内检查证书
    if ! docker exec zinnia-certbot test -f "/etc/letsencrypt/live/$DOMAIN/fullchain.pem" 2>/dev/null; then
        log_info "证书不存在，需要首次获取"
        return 0
    fi
    
    # 获取证书到期时间
    local expiry_date
    expiry_date=$(docker exec zinnia-certbot openssl x509 -enddate -noout -in "/etc/letsencrypt/live/$DOMAIN/fullchain.pem" | cut -d= -f2)
    
    # 计算剩余天数（兼容 macOS 和 Linux）
    local expiry_epoch
    if date -d "$expiry_date" +%s >/dev/null 2>&1; then
        # Linux
        expiry_epoch=$(date -d "$expiry_date" +%s)
    else
        # macOS
        expiry_epoch=$(date -j -f "%b %d %H:%M:%S %Y %Z" "$expiry_date" +%s 2>/dev/null || echo "0")
    fi
    
    local current_epoch
    current_epoch=$(date +%s)
    local days_left=$(( (expiry_epoch - current_epoch) / 86400 ))
    
    log_info "证书剩余有效期: $days_left 天"
    
    # 如果剩余天数大于 30，跳过续签
    if [ "$days_left" -gt 30 ]; then
        log_success "证书仍然有效（剩余 $days_left 天），无需续签"
        return 1
    fi
    
    log_info "证书即将过期，准备续签"
    return 0
}

# 手动触发证书续签
renew_certificate() {
    log_info "开始续签证书..."
    
    # 在 certbot 容器内执行续签
    log_info "执行 certbot renew..."
    if docker exec zinnia-certbot certbot renew --webroot --webroot-path=/var/www/certbot; then
        log_success "证书续签成功"
        
        # 重载 Nginx 配置
        log_info "重载 Nginx 配置..."
        if docker exec zinnia-nginx nginx -s reload; then
            log_success "Nginx 配置已重载"
        else
            log_error "Nginx 重载失败"
            return 1
        fi
    else
        log_error "证书续签失败"
        return 1
    fi
    
    # 验证证书
    log_info "验证证书..."
    if docker exec zinnia-certbot test -f "/etc/letsencrypt/live/$DOMAIN/fullchain.pem"; then
        local expiry_date
        expiry_date=$(docker exec zinnia-certbot openssl x509 -enddate -noout -in "/etc/letsencrypt/live/$DOMAIN/fullchain.pem" | cut -d= -f2)
        log_success "新证书到期时间: $expiry_date"
    fi
}

# 验证 HTTPS 连接
verify_https() {
    log_info "验证 HTTPS 连接..."
    
    if curl -sSf --max-time 10 "https://$DOMAIN/health" >/dev/null 2>&1; then
        log_success "HTTPS 连接验证成功"
        return 0
    else
        log_error "HTTPS 连接验证失败"
        return 1
    fi
}

# 查看证书信息
show_certificate_info() {
    log_info "=========================================="
    log_info "证书信息"
    log_info "=========================================="
    
    if docker exec zinnia-certbot test -f "/etc/letsencrypt/live/$DOMAIN/fullchain.pem" 2>/dev/null; then
        docker exec zinnia-certbot certbot certificates -d "$DOMAIN"
    else
        log_info "证书尚未获取"
    fi
}

# 强制重新获取证书
force_obtain() {
    log_info "=========================================="
    log_info "强制重新获取证书"
    log_info "=========================================="
    
    read -p "⚠️  此操作会删除现有证书并重新获取，是否继续？[y/N] " -r confirm
    if [[ ! $confirm =~ ^[Yy]$ ]]; then
        log_info "操作已取消"
        return 0
    fi
    
    log_info "删除现有证书..."
    docker exec zinnia-certbot rm -rf "/etc/letsencrypt/live/$DOMAIN" "/etc/letsencrypt/archive/$DOMAIN" "/etc/letsencrypt/renewal/$DOMAIN.conf" 2>/dev/null || true
    
    log_info "重启 certbot 容器以获取新证书..."
    $COMPOSE -f "$COMPOSE_FILE" --env-file "$ENV_FILE" restart certbot
    
    log_info "等待证书获取..."
    sleep 15
    
    # 检查证书
    if docker exec zinnia-certbot test -f "/etc/letsencrypt/live/$DOMAIN/fullchain.pem" 2>/dev/null; then
        log_success "证书获取成功"
        show_certificate_info
    else
        log_error "证书获取失败"
        log_info "查看 certbot 日志："
        docker logs --tail=50 zinnia-certbot
        return 1
    fi
}

# 主流程
main() {
    local action="${1:-renew}"
    
    log_info "=========================================="
    log_info "SSL 证书管理（Docker 版本）"
    log_info "=========================================="
    
    detect_compose
    load_config
    check_certbot_container
    
    case "$action" in
        renew)
            # 检查并续签
            if check_certificate_expiry; then
                renew_certificate
            fi
            ;;
        force-renew)
            # 强制续签
            renew_certificate
            ;;
        info)
            # 显示证书信息
            show_certificate_info
            ;;
        force-obtain)
            # 强制重新获取
            force_obtain
            ;;
        verify)
            # 验证 HTTPS
            verify_https
            ;;
        *)
            echo "用法: $0 {renew|force-renew|info|force-obtain|verify}"
            echo ""
            echo "命令:"
            echo "  renew         检查并续签证书（默认）"
            echo "  force-renew   强制续签证书"
            echo "  info          显示证书信息"
            echo "  force-obtain  强制重新获取证书"
            echo "  verify        验证 HTTPS 连接"
            exit 1
            ;;
    esac
    
    log_info "=========================================="
    log_info "操作完成"
    log_info "=========================================="
}

# 执行主流程
if [ "${BASH_SOURCE[0]}" == "${0}" ]; then
    main "$@"
fi
