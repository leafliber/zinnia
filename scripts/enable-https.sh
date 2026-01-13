#!/usr/bin/env bash
# ===========================================
# Zinnia HTTPS 启用脚本
# ===========================================
# 用途：获取 SSL 证书后，自动配置 HTTPS
# 使用：./scripts/enable-https.sh your-domain.com
# ===========================================

set -euo pipefail

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# 项目根目录
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
NGINX_CONF="$ROOT_DIR/nginx/conf.d/zinnia.conf"
NGINX_CONF_HTTPS="$ROOT_DIR/nginx/conf.d/zinnia-https.conf"

# 检查参数
if [ $# -lt 1 ]; then
    log_error "用法: $0 <domain>"
    log_info "示例: $0 api.example.com"
    exit 1
fi

DOMAIN="$1"
log_info "域名: $DOMAIN"

# 检查证书是否存在
CERT_PATH="/etc/letsencrypt/live/$DOMAIN/fullchain.pem"
log_info "检查证书..."

# 在 Docker 环境中检查
if docker exec zinnia-nginx test -f "$CERT_PATH" 2>/dev/null; then
    log_success "找到 SSL 证书"
else
    log_error "未找到 SSL 证书: $CERT_PATH"
    log_info "请先运行 certbot 获取证书:"
    log_info "  docker exec zinnia-certbot certbot certonly --webroot -w /var/www/certbot -d $DOMAIN"
    exit 1
fi

# 备份原配置
BACKUP_FILE="$NGINX_CONF.$(date +%Y%m%d%H%M%S).bak"
cp "$NGINX_CONF" "$BACKUP_FILE"
log_info "已备份配置到: $BACKUP_FILE"

# 生成启用 HTTPS 的配置
log_info "生成 HTTPS 配置..."

cat > "$NGINX_CONF" << EOF
# ===========================================
# Zinnia Nginx 配置 (HTTPS 已启用)
# ===========================================
# 域名: $DOMAIN
# 生成时间: $(date '+%Y-%m-%d %H:%M:%S')
# ===========================================

# 上游服务器定义
upstream zinnia_backend {
    server zinnia:8080;
    keepalive 32;
}

# HTTP -> HTTPS 重定向
server {
    listen 80;
    listen [::]:80;
    server_name $DOMAIN;

    # 健康检查端点（内部使用）
    location /health {
        access_log off;
        proxy_pass http://zinnia_backend/health;
        proxy_set_header Host \$host;
    }

    # ACME 证书验证（续期用）
    location ^~ /.well-known/acme-challenge/ {
        root /var/www/certbot;
        try_files \$uri =404;
    }

    # 其他所有请求重定向到 HTTPS
    location / {
        return 301 https://\$host\$request_uri;
    }
}

# HTTPS 服务器配置
server {
    listen 443 ssl;
    listen [::]:443 ssl;
    http2 on;
    server_name $DOMAIN;

    # SSL 证书
    ssl_certificate /etc/letsencrypt/live/$DOMAIN/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/$DOMAIN/privkey.pem;
    ssl_trusted_certificate /etc/letsencrypt/live/$DOMAIN/chain.pem;

    # SSL 安全配置（Mozilla Modern 配置）
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:DHE-RSA-AES128-GCM-SHA256:DHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    
    # SSL 会话缓存
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 1d;
    ssl_session_tickets off;
    
    # OCSP Stapling
    ssl_stapling on;
    ssl_stapling_verify on;
    resolver 8.8.8.8 8.8.4.4 valid=300s;
    resolver_timeout 5s;

    # 安全响应头
    add_header Strict-Transport-Security "max-age=63072000; includeSubDomains; preload" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
    add_header Permissions-Policy "geolocation=(), microphone=(), camera=()" always;

    # 健康检查
    location /health {
        access_log off;
        proxy_pass http://zinnia_backend/health;
        proxy_set_header Host \$host;
    }

    # 主要 API 路由
    location / {
        limit_req zone=api_limit burst=20 nodelay;
        limit_conn conn_limit 10;

        proxy_pass http://zinnia_backend;
        proxy_http_version 1.1;
        
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_set_header X-Forwarded-Host \$host;
        
        # WebSocket 支持
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
        
        proxy_buffering off;
        proxy_request_buffering off;
    }

    # 认证接口特殊限流
    location ~ ^/api/v1/(auth|users/login|users/register) {
        limit_req zone=login_limit burst=3 nodelay;
        
        proxy_pass http://zinnia_backend;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }
}
EOF

log_success "HTTPS 配置已生成"

# 测试配置
log_info "测试 Nginx 配置..."
if docker exec zinnia-nginx nginx -t 2>&1; then
    log_success "配置测试通过"
else
    log_error "配置测试失败，正在恢复备份..."
    cp "$BACKUP_FILE" "$NGINX_CONF"
    exit 1
fi

# 重载 Nginx
log_info "重载 Nginx..."
docker exec zinnia-nginx nginx -s reload

log_success "HTTPS 已成功启用！"
log_info "请访问: https://$DOMAIN"
log_info ""
log_warn "建议："
log_info "1. 使用 https://www.ssllabs.com/ssltest/ 测试 SSL 配置"
log_info "2. 确保防火墙开放 443 端口"
log_info "3. 设置证书自动续期: docker exec zinnia-certbot certbot renew --dry-run"
