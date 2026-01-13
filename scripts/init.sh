#!/bin/bash
# ===========================================
# Zinnia 初始化脚本
# 用于设置开发环境
# ===========================================

set -e

# Detect container runtime and compose command (docker or podman)
detect_container_tool() {
    DOCKER_CMD=""
    COMPOSE=""

    if command -v podman >/dev/null 2>&1 && command -v podman-compose >/dev/null 2>&1; then
        DOCKER_CMD="podman"
        COMPOSE="podman-compose"
    elif command -v docker >/dev/null 2>&1; then
        DOCKER_CMD="docker"
        if docker compose version >/dev/null 2>&1; then
            COMPOSE="docker compose"
        elif command -v docker-compose >/dev/null 2>&1; then
            COMPOSE="docker-compose"
        else
            COMPOSE="docker compose"
        fi
    fi

    if [ -z "${DOCKER_CMD}" ] || [ -z "${COMPOSE}" ]; then
        echo "Neither docker nor podman+podman-compose found. Please install one of them." >&2
        exit 1
    fi

    export DOCKER_CMD COMPOSE
}

detect_container_tool

echo "🌱 Zinnia 开发环境初始化"
echo "=========================="

# 检查必要的工具
echo "📋 检查必要工具..."

if ! command -v cargo &> /dev/null; then
    echo "❌ cargo 未安装，请先安装 Rust"
    exit 1
fi
echo "  ✅ cargo $(cargo --version)"

echo "  ✅ Container runtime: ${DOCKER_CMD}"
if ${DOCKER_CMD} --version >/dev/null 2>&1; then
    echo "    $(${DOCKER_CMD} --version)"
fi

echo "  ✅ Compose command: ${COMPOSE}"
if [ "${COMPOSE}" = "docker compose" ]; then
    docker compose version >/dev/null 2>&1 && docker compose version || true
elif [ "${COMPOSE}" = "docker-compose" ]; then
    docker-compose --version >/dev/null 2>&1 && docker-compose --version || true
else
    ${COMPOSE} --version >/dev/null 2>&1 && ${COMPOSE} --version || true
fi

# 安装 sqlx-cli
echo ""
echo "📦 检查 sqlx-cli..."
if ! command -v sqlx &> /dev/null; then
    echo "  ⏳ 安装 sqlx-cli..."
    cargo install sqlx-cli --features postgres
fi
echo "  ✅ sqlx-cli 已安装"

# 创建 .env 文件
echo ""
echo "📝 创建 .env 文件..."
if [ ! -f .env ]; then
    cp .env.example .env
    
    # 生成随机密钥
    JWT_SECRET=$(openssl rand -base64 32)
    ENCRYPTION_KEY=$(openssl rand -base64 32)
    DB_PASSWORD=$(openssl rand -base64 16 | tr -d '/+=')
    REDIS_PASSWORD=$(openssl rand -base64 16 | tr -d '/+=')

    # 替换默认值（与 .env.example 中的占位符一致）
    sed -i.bak "s|your_jwt_secret_key_at_least_256_bits_long_here|$JWT_SECRET|g" .env
    sed -i.bak "s|your_base64_encoded_32_byte_key_here|$ENCRYPTION_KEY|g" .env
    sed -i.bak "s|your_password_here|$DB_PASSWORD|g" .env
    sed -i.bak "s|your_redis_password|$REDIS_PASSWORD|g" .env
    rm -f .env.bak
    
    echo "  ✅ .env 文件已创建并生成随机密钥"
else
    echo "  ⏭️  .env 文件已存在，跳过"
fi

# 启动依赖服务
echo ""
echo "🐳 启动 container 服务..."
# 在开发环境启动前，清理所有现有容器以避免凭据/端口冲突
echo ""
echo "🧹 清理现有 Docker 容器（开发环境）..."
if [ "${DOCKER_CMD}" = "docker" ] || [ "${DOCKER_CMD}" = "podman" ]; then
    CONTAINERS=$(${DOCKER_CMD} ps -aq)
    if [ -n "${CONTAINERS}" ]; then
        echo "  ⚠️ 停止并删除所有容器..."
        ${DOCKER_CMD} rm -f ${CONTAINERS} || true
    else
        echo "  ℹ️ 没有运行的容器需要删除"
    fi
fi
eval "$COMPOSE -f docker-compose.dev.yml up -d"

# 等待数据库就绪
echo ""
echo "⏳ 等待 TimescaleDB 就绪..."
# 加载 .env 导出变量用于后续检查
if [ -f .env ]; then
    set -a
    # shellcheck disable=SC1091
    source .env
    set +a
fi

echo "⏳ 等待 TimescaleDB 就绪（包括认证检查）..."
MAX_RETRIES=60
RETRY=0
while [ $RETRY -lt $MAX_RETRIES ]; do
    # 先检查服务是否响应
    if $DOCKER_CMD exec zinnia-timescaledb-dev pg_isready -U "${POSTGRES_USER:-zinnia}" -d "${POSTGRES_DB:-zinnia}" &> /dev/null; then
        # 再用 psql 验证认证是否可用（通过设置 PGPASSWORD 环境变量）
        if $DOCKER_CMD exec -e PGPASSWORD="${POSTGRES_PASSWORD:-}" zinnia-timescaledb-dev psql -U "${POSTGRES_USER:-zinnia}" -d "${POSTGRES_DB:-zinnia}" -c "\q" &> /dev/null; then
            echo "  ✅ TimescaleDB 已就绪并可通过给定凭据认证"
            break
        else
            echo "  ⚠️ TimescaleDB 可达，但凭据认证失败（尝试 ${RETRY}/${MAX_RETRIES}）"
            # 如果存在卷且密码不匹配，后续尝试不会成功 — 继续重试直到超时以便容器完成启动流程
        fi
    fi

    RETRY=$((RETRY+1))
    if [ $RETRY -ge $MAX_RETRIES ]; then
        echo "  ❌ TimescaleDB 启动或认证超时"
        echo "  可能原因：容器使用的数据库密码与 .env 中的 POSTGRES_PASSWORD 不一致（存在已有卷）。"
        echo "  选项："
        echo "    - 如果想保留现有数据，请将 .env 中的 POSTGRES_PASSWORD 设置为现有 DB 密码，然后重试。"
        echo "    - 如果想重新初始化（删除数据），运行: ./scripts/dev.sh delete-volumes 然后重新运行 ./scripts/init.sh"
        exit 1
    fi
    sleep 1
done

# 等待 Redis 就绪
echo ""
echo "⏳ 等待 Redis 就绪..."
for i in {1..30}; do
    if $DOCKER_CMD exec zinnia-redis-dev redis-cli -a "${REDIS_PASSWORD:-dev_password}" ping &> /dev/null; then
        echo "  ✅ Redis 已就绪"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "  ❌ Redis 启动超时"
        exit 1
    fi
    sleep 1
done

# 运行数据库迁移
echo ""
echo "🗃️  运行数据库迁移..."
export DATABASE_URL="${DATABASE_URL:-postgres://${POSTGRES_USER:-zinnia}:${POSTGRES_PASSWORD:-dev_password}@localhost:5432/${POSTGRES_DB:-zinnia}}"
sqlx migrate run
echo "  ✅ 数据库迁移完成"

# 构建项目
echo ""
echo "🔨 构建项目..."
cargo build
echo "  ✅ 项目构建完成"

echo ""
echo "=========================="
echo "🎉 开发环境初始化完成！"
echo ""
echo "📍 服务地址:"
echo "   - API:       http://localhost:8080"
echo "   - Adminer:   http://localhost:8081"
echo "   - Redis UI:  http://localhost:8082"
echo ""
echo "🚀 启动开发服务器:"
echo "   cargo run"
echo ""
echo "📚 查看更多命令:"
echo "   ./scripts/dev.sh help"
