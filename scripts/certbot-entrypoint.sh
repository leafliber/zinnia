#!/bin/sh
# ===========================================
# Certbot 容器入口脚本
# 功能：自动获取和续签 SSL 证书
# ===========================================

set -e

echo "=========================================="
echo "Certbot 容器启动"
echo "=========================================="

# 读取环境变量
DOMAIN="${DOMAIN:-localhost}"
EMAIL="${EMAIL:-}"
RENEW_INTERVAL="${RENEW_INTERVAL:-86400}"  # 默认每天检查一次（秒）

echo "域名: $DOMAIN"
echo "邮箱: ${EMAIL:-未设置}"
echo "续签检查间隔: ${RENEW_INTERVAL}秒"

# 如果域名是 localhost，跳过证书获取
if [ "$DOMAIN" = "localhost" ]; then
    echo "域名为 localhost，跳过 SSL 证书配置"
    echo "容器将保持运行状态，但不执行任何操作"
    # 保持容器运行
    trap 'exit 0' TERM INT
    while :; do
        sleep 3600
    done
fi

# 检查邮箱是否配置
if [ -z "$EMAIL" ]; then
    echo "警告: 未配置 SSL_EMAIL，将使用 --register-unsafely-without-email"
    EMAIL_ARG="--register-unsafely-without-email"
else
    EMAIL_ARG="--email $EMAIL"
fi

# 等待 Nginx 启动
echo "等待 Nginx 服务就绪..."
sleep 10

# 检查证书是否已存在
CERT_PATH="/etc/letsencrypt/live/$DOMAIN/fullchain.pem"

if [ ! -f "$CERT_PATH" ]; then
    echo "=========================================="
    echo "首次获取证书: $DOMAIN"
    echo "=========================================="
    
    # 首次获取证书（使用 webroot 模式）
    certbot certonly \
        --webroot \
        --webroot-path=/var/www/certbot \
        $EMAIL_ARG \
        --agree-tos \
        --no-eff-email \
        --force-renewal \
        -d "$DOMAIN" \
        --non-interactive \
        || {
            echo "错误: 证书获取失败"
            echo "可能原因："
            echo "  1. 域名 DNS 未正确解析"
            echo "  2. Nginx 配置未正确设置 /.well-known/acme-challenge/"
            echo "  3. 防火墙阻止了 80 端口"
            exit 1
        }
    
    echo "✅ 证书获取成功"
    
    # 重载 Nginx 配置
    echo "重载 Nginx 配置..."
    if command -v docker >/dev/null 2>&1; then
        docker exec zinnia-nginx nginx -s reload 2>/dev/null || true
    fi
else
    echo "证书已存在: $CERT_PATH"
    
    # 检查证书到期时间
    EXPIRY=$(openssl x509 -enddate -noout -in "$CERT_PATH" | cut -d= -f2)
    echo "证书到期时间: $EXPIRY"
fi

# 进入证书续签循环
echo "=========================================="
echo "启动证书自动续签服务"
echo "检查间隔: ${RENEW_INTERVAL}秒"
echo "=========================================="

# 捕获退出信号
trap 'echo "收到退出信号，停止服务"; exit 0' TERM INT

while :; do
    # 等待指定时间
    sleep "$RENEW_INTERVAL" &
    wait $!
    
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] 检查证书续签..."
    
    # 尝试续签证书（certbot 会自动判断是否需要续签）
    certbot renew \
        --webroot \
        --webroot-path=/var/www/certbot \
        --quiet \
        --deploy-hook "echo '证书已更新，重载 Nginx'; docker exec zinnia-nginx nginx -s reload 2>/dev/null || true" \
        || echo "证书续签检查失败或无需续签"
    
    # 检查证书状态
    if [ -f "$CERT_PATH" ]; then
        DAYS_LEFT=$(( ($(date -d "$(openssl x509 -enddate -noout -in "$CERT_PATH" | cut -d= -f2)" +%s) - $(date +%s)) / 86400 ))
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] 证书剩余有效期: $DAYS_LEFT 天"
        
        if [ "$DAYS_LEFT" -le 30 ]; then
            echo "⚠️  警告: 证书即将过期（剩余 $DAYS_LEFT 天）"
        fi
    fi
done
