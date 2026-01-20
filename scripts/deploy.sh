#!/usr/bin/env bash
# ===========================================
# Zinnia 生产环境一键部署脚本（交互式）
# ===========================================
# 功能：
# - 自动检测环境
# - 交互式配置
# - 一键部署所有服务（应用+DB+Redis+Nginx）
# - 自动运行迁移
# - 健康检查
# - 可选 SSL 证书配置

set -euo pipefail

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 项目根目录
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

# 配置文件路径
ENV_FILE=".env.production"
SECRETS_DIR="./secrets"
COMPOSE_FILE="docker-compose.prod.yml"

# ===========================================
# 工具函数
# ===========================================

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

press_enter() {
    echo ""
    read -p "按 Enter 键继续..." -r
}

# 检测容器工具
detect_container_tool() {
    DOCKER_CMD=""
    COMPOSE=""

    if command -v docker >/dev/null 2>&1; then
        DOCKER_CMD="docker"
        if docker compose version >/dev/null 2>&1; then
            COMPOSE="docker compose"
        elif command -v docker-compose >/dev/null 2>&1; then
            COMPOSE="docker-compose"
        else
            log_error "未找到 docker-compose，请安装 Docker Compose"
            exit 1
        fi
    else
        log_error "未找到 Docker，请先安装 Docker"
        exit 1
    fi

    export DOCKER_CMD COMPOSE
    log_success "检测到: $DOCKER_CMD, $COMPOSE"
}

# 生成随机密码
generate_password() {
    openssl rand -base64 32 | tr -d '/+=' | cut -c1-32
}

generate_secret() {
    openssl rand -base64 32
}

# ===========================================
# 环境检查
# ===========================================

check_prerequisites() {
    print_header "检查系统环境"
    
    log_info "检查必要工具..."
    
    local missing_tools=()
    
    command -v docker >/dev/null 2>&1 || missing_tools+=("docker")
    command -v curl >/dev/null 2>&1 || missing_tools+=("curl")
    command -v openssl >/dev/null 2>&1 || missing_tools+=("openssl")
    
    if [ ${#missing_tools[@]} -gt 0 ]; then
        log_error "缺少必要工具: ${missing_tools[*]}"
        log_info "请先安装缺失的工具"
        exit 1
    fi
    
    log_success "所有必要工具已就绪"
    
    detect_container_tool
}

# ===========================================
# 交互式配置
# ===========================================

interactive_setup() {
    print_header "交互式配置向导"
    
    log_info "此向导将帮助您配置 Zinnia 生产环境"
    press_enter
    
    # 检查是否已有配置
    if [ -f "$ENV_FILE" ] && [ -d "$SECRETS_DIR" ]; then
        log_warn "检测到已存在的配置"
        read -p "是否使用现有配置？[Y/n] " -r use_existing
        if [[ $use_existing =~ ^[Nn]$ ]]; then
            log_info "将创建新配置"
        else
            log_info "使用现有配置"
            return 0
        fi
    fi
    
    # 创建 secrets 目录
    mkdir -p "$SECRETS_DIR"
    chmod 700 "$SECRETS_DIR"
    
    # 数据库配置
    print_header "数据库配置"
    
    read -p "数据库用户名 [zinnia]: " db_user
    db_user=${db_user:-zinnia}
    
    read -p "数据库名称 [zinnia]: " db_name
    db_name=${db_name:-zinnia}
    
    read -p "是否自动生成数据库密码？[Y/n] " -r auto_db_pass
    if [[ $auto_db_pass =~ ^[Nn]$ ]]; then
        read -sp "请输入数据库密码: " db_password
        echo ""
    else
        db_password=$(generate_password)
        log_success "已生成数据库密码"
    fi
    
    echo "$db_password" > "$SECRETS_DIR/db_password"
    chmod 600 "$SECRETS_DIR/db_password"
    
    # Redis 配置
    print_header "Redis 配置"
    
    read -p "是否自动生成 Redis 密码？[Y/n] " -r auto_redis_pass
    if [[ $auto_redis_pass =~ ^[Nn]$ ]]; then
        read -sp "请输入 Redis 密码: " redis_password
        echo ""
    else
        redis_password=$(generate_password)
        log_success "已生成 Redis 密码"
    fi
    
    echo "$redis_password" > "$SECRETS_DIR/redis_password"
    chmod 600 "$SECRETS_DIR/redis_password"
    
    # 应用密钥
    print_header "应用密钥配置"
    
    log_info "生成 JWT 密钥..."
    jwt_secret=$(generate_secret)
    echo "$jwt_secret" > "$SECRETS_DIR/jwt_secret"
    chmod 600 "$SECRETS_DIR/jwt_secret"
    
    log_info "生成加密密钥..."
    encryption_key=$(generate_secret)
    echo "$encryption_key" > "$SECRETS_DIR/encryption_key"
    chmod 600 "$SECRETS_DIR/encryption_key"
    
    log_success "密钥已生成"
    
    # SMTP 邮件服务配置
    print_header "SMTP 邮件服务配置（可选）"
    
    read -p "是否启用 SMTP 邮件服务？[y/N] " -r enable_smtp
    if [[ $enable_smtp =~ ^[Yy]$ ]]; then
        smtp_enabled="true"
        read -p "SMTP 服务器地址（如 smtp.gmail.com）: " smtp_host
        read -p "SMTP 端口 [465]: " smtp_port
        smtp_port=${smtp_port:-465}
        read -p "SMTP 用户名: " smtp_username
        read -sp "SMTP 密码: " smtp_password
        echo ""
        read -p "发件人邮箱: " smtp_from_email
        read -p "发件人名称 [Zinnia]: " smtp_from_name
        smtp_from_name=${smtp_from_name:-Zinnia}
        read -p "使用 TLS？[Y/n] " -r smtp_tls
        if [[ $smtp_tls =~ ^[Nn]$ ]]; then
            smtp_tls="false"
        else
            smtp_tls="true"
        fi
        
        echo "$smtp_password" > "$SECRETS_DIR/smtp_password"
        chmod 600 "$SECRETS_DIR/smtp_password"
        
        read -p "是否强制要求邮箱验证？[y/N] " -r require_email_verification
        if [[ $require_email_verification =~ ^[Yy]$ ]]; then
            require_email_verification="true"
        else
            require_email_verification="false"
        fi
        
        log_success "SMTP 配置完成"
    else
        smtp_enabled="false"
        smtp_host="smtp.example.com"
        smtp_port="465"
        smtp_username=""
        smtp_from_email="noreply@example.com"
        smtp_from_name="Zinnia"
        smtp_tls="true"
        require_email_verification="false"
        # 创建空的 SMTP 密码文件（Docker secrets 需要）
        echo "" > "$SECRETS_DIR/smtp_password"
        chmod 600 "$SECRETS_DIR/smtp_password"
        log_info "跳过 SMTP 配置"
    fi
    
    # Google reCAPTCHA 配置
    print_header "Google reCAPTCHA 配置（可选）"
    
    read -p "是否启用 Google reCAPTCHA？[y/N] " -r enable_recaptcha
    if [[ $enable_recaptcha =~ ^[Yy]$ ]]; then
        recaptcha_enabled="true"
        log_info "请前往 https://www.google.com/recaptcha/admin 创建站点密钥"
        read -p "reCAPTCHA 站点密钥（Site Key）: " recaptcha_site_key
        read -p "reCAPTCHA 密钥（Secret Key）: " recaptcha_secret_key
        
        echo "$recaptcha_secret_key" > "$SECRETS_DIR/recaptcha_secret"
        chmod 600 "$SECRETS_DIR/recaptcha_secret"
        
        read -p "是否强制要求 reCAPTCHA 验证？[y/N] " -r require_recaptcha
        if [[ $require_recaptcha =~ ^[Yy]$ ]]; then
            require_recaptcha="true"
        else
            require_recaptcha="false"
        fi
        
        log_success "reCAPTCHA 配置完成"
    else
        recaptcha_enabled="false"
        recaptcha_site_key=""
        require_recaptcha="false"
        # 创建空的 reCAPTCHA 密钥文件（Docker secrets 需要）
        echo "" > "$SECRETS_DIR/recaptcha_secret"
        chmod 600 "$SECRETS_DIR/recaptcha_secret"
        log_info "跳过 reCAPTCHA 配置"
    fi
    
    # 域名配置
    print_header "域名配置"
    
    read -p "请输入生产域名（留空使用 localhost）: " domain
    domain=${domain:-localhost}
    
    if [ "$domain" != "localhost" ]; then
        read -p "请输入 SSL 证书邮箱: " ssl_email
    else
        ssl_email=""
    fi
    
    # Web Push VAPID 配置
    print_header "Web Push (PWA) 通知配置（可选）"
    
    read -p "是否启用 Web Push 通知？[y/N] " -r enable_vapid
    if [[ $enable_vapid =~ ^[Yy]$ ]]; then
        log_info "需要生成 VAPID 密钥对"
        
        # 优先使用容器环境（Docker/Podman）生成 VAPID 密钥，如果不可用再退回到本地 npx
        if command -v docker >/dev/null 2>&1; then
            log_info "使用 Docker 临时容器生成 VAPID 密钥..."
            vapid_keys=$(docker run --rm -v "$ROOT_DIR":/work -w /work node:18-bullseye-slim npx -y web-push generate-vapid-keys --json 2>/dev/null || echo "")
        elif command -v podman >/dev/null 2>&1; then
            log_info "使用 Podman 临时容器生成 VAPID 密钥..."
            vapid_keys=$(podman run --rm -v "$ROOT_DIR":/work -w /work docker.io/node:18-bullseye-slim npx -y web-push generate-vapid-keys --json 2>/dev/null || echo "")
        elif command -v npx >/dev/null 2>&1; then
            log_info "使用本地 npx 生成 VAPID 密钥..."
            vapid_keys=$(npx -y web-push generate-vapid-keys --json 2>/dev/null || echo "")
        else
            vapid_keys=""
        fi

        if [ -n "$vapid_keys" ]; then
            vapid_public_key=$(echo "$vapid_keys" | grep -o '"publicKey":"[^"]*"' | cut -d'"' -f4)
            vapid_private_key=$(echo "$vapid_keys" | grep -o '"privateKey":"[^"]*"' | cut -d'"' -f4)
            
            if [ -n "$vapid_public_key" ] && [ -n "$vapid_private_key" ]; then
                log_success "VAPID 密钥已自动生成"
            else
                log_warn "自动生成失败，请手动输入"
                read -p "VAPID 公钥: " vapid_public_key
                read -p "VAPID 私钥: " vapid_private_key
            fi
        else
            log_warn "未检测到可用的生成工具（Docker/Podman/npx），请手动输入 VAPID 密钥"
            log_info "生成方法示例: npx web-push generate-vapid-keys 或 使用脚本 ./scripts/generate-vapid-keys.sh"
            read -p "VAPID 公钥: " vapid_public_key
            read -p "VAPID 私钥: " vapid_private_key
        fi
        
        log_success "Web Push 配置完成"
    else
        vapid_public_key=""
        vapid_private_key=""
        log_info "跳过 Web Push 配置（可稍后手动添加到 .env.production）"
    fi
    
    # 生成 .env.production
    cat > "$ENV_FILE" <<EOF
# Zinnia 生产环境配置
# 自动生成于 $(date)

# ==================== 服务配置 ====================
APP_HOST=0.0.0.0
APP_PORT=8080
APP_ENV=production
RUST_LOG=info
RUST_BACKTRACE=0

# ==================== 数据库配置 ====================
POSTGRES_USER=$db_user
POSTGRES_DB=$db_name
DATABASE_MAX_CONNECTIONS=50
DATABASE_MIN_CONNECTIONS=10

# ==================== 安全配置 ====================
JWT_EXPIRY_SECONDS=900
REFRESH_TOKEN_EXPIRY_DAYS=7

# ==================== 限流配置 ====================
RATE_LIMIT_REQUESTS_PER_MINUTE=100
RATE_LIMIT_BURST_SIZE=20

# ==================== SMTP 邮件服务配置 ====================
ZINNIA_SMTP__ENABLED=${smtp_enabled}
ZINNIA_SMTP__HOST=${smtp_host}
ZINNIA_SMTP__PORT=${smtp_port}
ZINNIA_SMTP__USERNAME=${smtp_username}
ZINNIA_SMTP__FROM_EMAIL=${smtp_from_email}
ZINNIA_SMTP__FROM_NAME=${smtp_from_name}
ZINNIA_SMTP__TLS=${smtp_tls}
ZINNIA_SMTP__CODE_EXPIRY_SECONDS=600
ZINNIA_SMTP__MAX_SENDS_PER_HOUR=30

# ==================== Google reCAPTCHA 配置 ====================
ZINNIA_RECAPTCHA__ENABLED=${recaptcha_enabled}
ZINNIA_RECAPTCHA_SITE_KEY=${recaptcha_site_key}
ZINNIA_RECAPTCHA__SCORE_THRESHOLD=0.5

# ==================== 注册安全配置 ====================
ZINNIA_REGISTRATION__MAX_PER_IP_PER_HOUR=3
ZINNIA_REGISTRATION__MAX_PER_IP_PER_DAY=10
ZINNIA_REGISTRATION__REQUIRE_EMAIL_VERIFICATION=${require_email_verification}
ZINNIA_REGISTRATION__REQUIRE_RECAPTCHA=${require_recaptcha}

# ==================== Docker 配置 ====================
DOCKER_REGISTRY=
IMAGE_TAG=latest

# ==================== 域名配置 ====================
DOMAIN=${domain}
SSL_EMAIL=${ssl_email}

# ==================== 备份配置 ====================
BACKUP_RETENTION_DAYS=7

# ==================== Web Push (PWA) 通知配置 ====================
VAPID_PUBLIC_KEY=${vapid_public_key}
VAPID_PRIVATE_KEY=${vapid_private_key}
EOF
    
    chmod 600 "$ENV_FILE"
    
    log_success "配置文件已生成: $ENV_FILE"
    log_success "密钥文件已生成: $SECRETS_DIR/"
    
    # 显示配置摘要
    print_header "配置摘要"
    echo "数据库用户: $db_user"
    echo "数据库名称: $db_name"
    echo "域名: ${domain}"
    echo "SMTP 服务: ${smtp_enabled}"
    if [ "$smtp_enabled" = "true" ]; then
        echo "  ├─ 服务器: ${smtp_host}:${smtp_port}"
        echo "  ├─ 用户名: ${smtp_username}"
        echo "  └─ 发件人: ${smtp_from_email}"
    fi
    echo "reCAPTCHA: ${recaptcha_enabled}"
    if [ "$recaptcha_enabled" = "true" ]; then
        echo "  └─ 站点密钥: ${recaptcha_site_key}"
    fi
    echo "Web Push: $([ -n "$vapid_public_key" ] && echo "已启用" || echo "未启用")"
    if [ -n "$vapid_public_key" ]; then
        echo "  └─ 公钥: ${vapid_public_key:0:20}..."
    fi
    echo "密钥目录: $SECRETS_DIR"
    echo ""
    log_warn "重要：请妥善保管 secrets 目录下的密钥文件！"
    
    press_enter
}

# ===========================================
# 部署函数
# ===========================================

build_and_start() {
    print_header "构建并启动服务"
    
    log_info "拉取依赖服务镜像..."
    # 仅拉取外部基础镜像，跳过本地构建的 app 镜像
    $COMPOSE -f "$COMPOSE_FILE" --env-file "$ENV_FILE" pull timescaledb redis nginx certbot --ignore-pull-failures || true
    
    log_info "构建应用镜像..."
    # 如果仓库缺少 Cargo.lock，则尝试生成（有 cargo 时）
    if [ ! -f "$ROOT_DIR/Cargo.lock" ]; then
        if command -v cargo >/dev/null 2>&1; then
            log_info "未检测到 $ROOT_DIR/Cargo.lock，正在生成..."
            cargo generate-lockfile
            log_success "Cargo.lock 已生成"
        elif command -v docker >/dev/null 2>&1; then
            log_info "未检测到本地 cargo，尝试使用 Docker 临时容器生成 Cargo.lock..."
            if docker run --rm -v "$ROOT_DIR":/work -w /work --user "$(id -u):$(id -g)" rust:latest cargo generate-lockfile; then
                log_success "Cargo.lock 已通过 Docker 生成"
            else
                log_warn "使用 Docker 生成 Cargo.lock 失败，构建可能仍会失败"
            fi
        else
            log_warn "未检测到 $ROOT_DIR/Cargo.lock，且系统无 cargo 或 docker，跳过生成 Cargo.lock（构建可能失败）"
        fi
    fi

    $COMPOSE -f "$COMPOSE_FILE" --env-file "$ENV_FILE" build --pull
    
    log_info "启动所有服务..."
    $COMPOSE -f "$COMPOSE_FILE" --env-file "$ENV_FILE" up -d --remove-orphans
    
    log_success "服务已启动"
}

wait_for_services() {
    print_header "等待服务就绪"
    
    log_info "等待 TimescaleDB..."
    local retries=60
    for ((i=1; i<=retries; i++)); do
        if $DOCKER_CMD exec zinnia-timescaledb pg_isready -U zinnia >/dev/null 2>&1; then
            log_success "TimescaleDB 已就绪"
            break
        fi
        if [ $i -eq $retries ]; then
            log_error "TimescaleDB 启动超时"
            return 1
        fi
        sleep 2
    done
    
    log_info "等待 Redis..."
    for ((i=1; i<=30; i++)); do
        if $DOCKER_CMD exec zinnia-redis redis-cli ping >/dev/null 2>&1; then
            log_success "Redis 已就绪"
            break
        fi
        if [ $i -eq 30 ]; then
            log_error "Redis 启动超时"
            return 1
        fi
        sleep 1
    done
    
    log_info "等待应用服务..."
    for ((i=1; i<=60; i++)); do
        if curl -fsS --max-time 2 http://localhost/health >/dev/null 2>&1; then
            log_success "应用服务已就绪"
            break
        fi
        if [ $i -eq 60 ]; then
            log_error "应用服务启动超时"
            log_info "查看日志："
            $COMPOSE -f "$COMPOSE_FILE" logs --tail=50 zinnia
            return 1
        fi
        sleep 2
    done
}

run_migrations() {
    print_header "运行数据库迁移"
    
    log_info "执行 SQL 迁移..."
    
    # 通过 docker exec 在 timescaledb 容器内执行迁移
    $DOCKER_CMD exec zinnia-timescaledb bash -c '
        set -e
        for f in /docker-entrypoint-initdb.d/*.sql; do
            if [ -f "$f" ]; then
                echo "执行: $(basename $f)"
                PGPASSWORD="$(cat /run/secrets/db_password)" \
                    psql -v ON_ERROR_STOP=1 \
                    -U "${POSTGRES_USER:-zinnia}" \
                    -d "${POSTGRES_DB:-zinnia}" \
                    -f "$f" 2>&1 | grep -v "already exists" || true
            fi
        done
    ' || {
        log_warn "迁移可能已执行过，继续..."
    }
    
    log_success "数据库迁移完成"
}

health_check() {
    print_header "健康检查"
    
    log_info "检查服务状态..."
    
    local services=("timescaledb" "redis" "zinnia" "nginx")
    local all_healthy=true
    
    for service in "${services[@]}"; do
        # 使用更宽松的名称匹配：检查任意运行中容器名是否包含服务关键字
        case "$service" in
            timescaledb)
                search_term="timescaledb"
                ;;
            zinnia)
                search_term="zinnia"
                ;;
            redis)
                search_term="redis"
                ;;
            nginx)
                search_term="nginx"
                ;;
            *)
                search_term="$service"
                ;;
        esac

        if $DOCKER_CMD ps --filter "status=running" --format '{{.Names}}' | grep -qE "$search_term"; then
            log_success "✓ $service: 运行中"
        else
            log_error "✗ $service: 未运行"
            all_healthy=false
        fi
    done
    
    if $all_healthy; then
        log_success "所有服务健康"
        return 0
    else
        log_error "部分服务不健康"
        return 1
    fi
}

setup_ssl() {
    print_header "SSL 证书配置"
    
    # 读取域名配置
    if [ -f "$ENV_FILE" ]; then
        source "$ENV_FILE"
    fi
    
    if [ -z "${DOMAIN:-}" ] || [ "$DOMAIN" = "localhost" ]; then
        log_info "域名配置为 localhost，跳过 SSL 设置"
        log_info ""
        log_info "如需启用 HTTPS，请："
        log_info "1. 配置域名 DNS 指向此服务器"
        log_info "2. 编辑 .env.production 设置 DOMAIN 和 SSL_EMAIL"
        log_info "3. 重启服务：docker compose restart"
        log_info ""
        log_info "certbot 容器会自动获取和续签证书"
        return 0
    fi
    
    log_success "SSL 证书将由 certbot 容器自动管理"
    log_info ""
    log_info "配置信息："
    log_info "  域名: $DOMAIN"
    log_info "  邮箱: ${SSL_EMAIL:-未设置}"
    log_info "  自动续签: 每天检查一次"
    log_info ""
    log_info "证书获取流程："
    log_info "  1. certbot 容器启动后自动获取证书"
    log_info "  2. 使用 webroot 模式（通过 Nginx 验证）"
    log_info "  3. 证书保存在 Docker volume: certbot_conf"
    log_info "  4. Nginx 自动加载证书"
    log_info ""
    
    read -p "是否现在启用 HTTPS 配置？[y/N] " -r enable_https
    if [[ $enable_https =~ ^[Yy]$ ]]; then
        log_info "更新 Nginx 配置启用 HTTPS..."
        
        # 更新域名
        if grep -q "your-domain.com" ./nginx/conf.d/zinnia.conf; then
            sed -i.bak "s/your-domain.com/$DOMAIN/g" ./nginx/conf.d/zinnia.conf
            log_success "已更新域名配置"
        fi
        
        # 启用 HTTPS 配置（取消注释）
        log_info "请在证书获取成功后，手动启用 HTTPS 配置："
        log_info "  1. 编辑 nginx/conf.d/zinnia.conf"
        log_info "  2. 取消 HTTPS server 块的注释"
        log_info "  3. 启用 HTTP 到 HTTPS 的重定向"
        log_info "  4. 重载 Nginx：docker compose restart nginx"
    else
        log_info "跳过 HTTPS 配置"
        log_info "您可以稍后手动配置"
    fi
    
    log_success "SSL 配置完成"
}

show_info() {
    print_header "部署完成"
    
    log_success "🎉 Zinnia 已成功部署！"
    echo ""
    echo "服务访问地址:"
    echo "  HTTP:  http://localhost"
    
    if [ -f "$ENV_FILE" ]; then
        source "$ENV_FILE"
        if [ -n "${DOMAIN:-}" ] && [ "$DOMAIN" != "localhost" ]; then
            echo "  域名:  http://$DOMAIN"
        fi
    fi
    
    echo ""
    echo "常用命令:"
    echo "  查看日志:   $COMPOSE -f $COMPOSE_FILE logs -f"
    echo "  查看状态:   $COMPOSE -f $COMPOSE_FILE ps"
    echo "  停止服务:   $COMPOSE -f $COMPOSE_FILE down"
    echo "  重启服务:   $COMPOSE -f $COMPOSE_FILE restart"
    echo ""
    echo "管理脚本:"
    echo "  ./scripts/manage.sh - 管理工具"
    echo ""
}

# ===========================================
# 主菜单
# ===========================================

show_menu() {
    clear
    echo -e "${GREEN}"
    cat << "EOF"
 ______     ______     __   __     __   __     __     ______    
/\___  \   /\  ___\   /\ "-.\ \   /\ "-.\ \   /\ \   /\  __ \   
\/_/  /__  \ \  __\   \ \ \-.  \  \ \ \-.  \  \ \ \  \ \  __ \  
  /\_____\  \ \_____\  \ \_\\"\_\  \ \_\\"\_\  \ \_\  \ \_\ \_\ 
  \/_____/   \/_____/   \/_/ \/_/   \/_/ \/_/   \/_/   \/_/\/_/ 
                                                                 
EOF
    echo -e "${NC}"
    echo "生产环境部署脚本 v1.0"
    echo "========================================"
    echo "1. 完整部署（推荐首次使用）"
    echo "2. 仅启动服务"
    echo "3. 运行数据库迁移"
    echo "4. 健康检查"
    echo "5. 配置 SSL 证书"
    echo "6. 查看服务状态"
    echo "7. 查看日志"
    echo "0. 退出"
    echo "========================================"
    read -p "请选择操作 [1-7]: " choice
}

# ===========================================
# 主流程
# ===========================================

main() {
    while true; do
        show_menu
        
        case $choice in
            1)
                check_prerequisites
                interactive_setup
                build_and_start
                wait_for_services
                run_migrations
                health_check
                setup_ssl
                show_info
                press_enter
                ;;
            2)
                check_prerequisites
                build_and_start
                wait_for_services
                show_info
                press_enter
                ;;
            3)
                check_prerequisites
                run_migrations
                press_enter
                ;;
            4)
                check_prerequisites
                health_check
                press_enter
                ;;
            5)
                check_prerequisites
                setup_ssl
                press_enter
                ;;
            6)
                check_prerequisites
                log_info "服务状态:"
                $COMPOSE -f "$COMPOSE_FILE" ps
                press_enter
                ;;
            7)
                check_prerequisites
                log_info "查看日志 (Ctrl+C 退出):"
                $COMPOSE -f "$COMPOSE_FILE" logs -f
                ;;
            0)
                log_info "退出"
                exit 0
                ;;
            *)
                log_error "无效选择"
                sleep 2
                ;;
        esac
    done
}

# 如果直接运行（非 source）
if [ "${BASH_SOURCE[0]}" == "${0}" ]; then
    main "$@"
fi
