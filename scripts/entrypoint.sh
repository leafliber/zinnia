#!/usr/bin/env bash
# ===========================================
# Zinnia Docker Entrypoint 脚本
# ===========================================
# 功能:
#   - 将 Docker Secrets 注入到环境变量中
#   - 提供统一的启动入口
#   - 优雅处理信号（配合 tini）
#
# 使用方式:
#   作为 Docker ENTRYPOINT 使用
# ===========================================

set -euo pipefail

# 日志函数
log() {
    echo "[Entrypoint $(date '+%Y-%m-%d %H:%M:%S')] $1"
}

log_error() {
    echo "[Entrypoint $(date '+%Y-%m-%d %H:%M:%S')] ERROR: $1" >&2
}

# 加载 secret 文件到环境变量
load_secret() {
    local secret_path="$1"
    local env_var="$2"
    local required="${3:-false}"
    
    if [ -f "$secret_path" ]; then
        local value
        value=$(cat "$secret_path")
        if [ -n "$value" ]; then
            export "$env_var"="$value"
            log "✓ Loaded $env_var from secrets"
            return 0
        fi
    fi
    
    if [ "$required" = "true" ]; then
        log_error "Required secret not found: $secret_path"
        return 1
    else
        log "⚠ Optional secret not found: $secret_path (skipped)"
        return 0
    fi
}

# 替换 URL 中的占位符
replace_placeholder() {
    local env_var="$1"
    local placeholder="$2"
    local value="$3"
    
    local current_value="${!env_var:-}"
    if [ -n "$current_value" ] && [ -n "$value" ]; then
        export "$env_var"="${current_value//$placeholder/$value}"
    fi
}

# ===========================================
# 主流程
# ===========================================

log "Zinnia Entrypoint starting..."
log "RUN_MODE: ${RUN_MODE:-development}"

# 1. 处理数据库密码（必需）
if [ -f "/run/secrets/db_password" ]; then
    log "Loading db_password from secrets..."
    DB_PASS=$(cat /run/secrets/db_password)
    # 替换 DATABASE_URL 中的占位符
    if [ -n "${DATABASE_URL:-}" ]; then
        export DATABASE_URL=${DATABASE_URL//__DB_PASS__/$DB_PASS}
        log "✓ DATABASE_URL configured"
    fi
else
    log_error "db_password secret not found - database connection will fail"
fi

# 2. 处理 Redis 密码（必需）
if [ -f "/run/secrets/redis_password" ]; then
    log "Loading redis_password from secrets..."
    REDIS_PASS=$(cat /run/secrets/redis_password)
    # 替换 REDIS_URL 中的占位符
    if [ -n "${REDIS_URL:-}" ]; then
        export REDIS_URL=${REDIS_URL//__REDIS_PASS__/$REDIS_PASS}
        log "✓ REDIS_URL configured"
    fi
else
    log_error "redis_password secret not found - Redis connection will fail"
fi

# 3. 处理 JWT 密钥（必需）
if [ -f "/run/secrets/jwt_secret" ]; then
    log "Loading jwt_secret from secrets..."
    export JWT_SECRET=$(cat /run/secrets/jwt_secret)
    log "✓ JWT_SECRET configured"
else
    log_error "jwt_secret not found - authentication will fail"
fi

# 4. 处理加密密钥（必需）
if [ -f "/run/secrets/encryption_key" ]; then
    log "Loading encryption_key from secrets..."
    export ENCRYPTION_KEY=$(cat /run/secrets/encryption_key)
    log "✓ ENCRYPTION_KEY configured"
else
    log_error "encryption_key not found - token encryption will fail"
fi

# 5. 处理 SMTP 密码（可选）
if [ -f "/run/secrets/smtp_password" ]; then
    SMTP_PASS=$(cat /run/secrets/smtp_password)
    if [ -n "$SMTP_PASS" ]; then
        log "Loading smtp_password from secrets..."
        export SMTP_PASSWORD="$SMTP_PASS"
        log "✓ SMTP_PASSWORD configured"
    else
        log "⚠ smtp_password is empty (email disabled)"
    fi
fi

# 6. 处理 reCAPTCHA 密钥（可选）
if [ -f "/run/secrets/recaptcha_secret" ]; then
    RECAPTCHA_PASS=$(cat /run/secrets/recaptcha_secret)
    if [ -n "$RECAPTCHA_PASS" ]; then
        log "Loading recaptcha_secret from secrets..."
        export RECAPTCHA_SECRET_KEY="$RECAPTCHA_PASS"
        log "✓ RECAPTCHA_SECRET_KEY configured"
    else
        log "⚠ recaptcha_secret is empty (reCAPTCHA disabled)"
    fi
fi

# 显示启动信息
log "Environment ready. Starting application..."
log "Server will listen on ${SERVER__HOST:-0.0.0.0}:${SERVER__PORT:-8080}"

# 使用 gosu 降权执行应用（从 root 切换到 zinnia 用户）
# 这样 secrets 由 root 读取，应用以非特权用户运行，符合最小权限原则
if command -v gosu >/dev/null 2>&1; then
    log "Switching to user 'zinnia' and executing: $*"
    exec gosu zinnia "$@"
else
    log_error "gosu not found, falling back to direct execution (not recommended)"
    exec "$@"
fi
