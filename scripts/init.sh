#!/bin/bash
# ===========================================
# Zinnia 初始化脚本
# 用于设置开发环境
# ===========================================

set -e

echo "🌱 Zinnia 开发环境初始化"
echo "=========================="

# 检查必要的工具
echo "📋 检查必要工具..."

if ! command -v cargo &> /dev/null; then
    echo "❌ cargo 未安装，请先安装 Rust"
    exit 1
fi
echo "  ✅ cargo $(cargo --version)"

if ! command -v docker &> /dev/null; then
    echo "❌ docker 未安装，请先安装 Docker"
    exit 1
fi
echo "  ✅ docker $(docker --version)"

if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
    echo "❌ docker-compose 未安装"
    exit 1
fi
echo "  ✅ docker-compose 已安装"

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
    
    # 替换默认值
    sed -i.bak "s|your-super-secure-jwt-secret-key-at-least-32-characters-long|$JWT_SECRET|g" .env
    sed -i.bak "s|your-base64-encoded-32-byte-encryption-key==|$ENCRYPTION_KEY|g" .env
    sed -i.bak "s|your_secure_password|$DB_PASSWORD|g" .env
    sed -i.bak "s|your_redis_password|$REDIS_PASSWORD|g" .env
    rm -f .env.bak
    
    echo "  ✅ .env 文件已创建并生成随机密钥"
else
    echo "  ⏭️  .env 文件已存在，跳过"
fi

# 启动依赖服务
echo ""
echo "🐳 启动 Docker 服务..."
docker compose -f docker-compose.dev.yml up -d

# 等待数据库就绪
echo ""
echo "⏳ 等待 TimescaleDB 就绪..."
for i in {1..30}; do
    if docker exec zinnia-timescaledb-dev pg_isready -U zinnia -d zinnia &> /dev/null; then
        echo "  ✅ TimescaleDB 已就绪"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "  ❌ TimescaleDB 启动超时"
        exit 1
    fi
    sleep 1
done

# 等待 Redis 就绪
echo ""
echo "⏳ 等待 Redis 就绪..."
for i in {1..30}; do
    if docker exec zinnia-redis-dev redis-cli -a dev_password ping &> /dev/null; then
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
export DATABASE_URL="postgres://zinnia:dev_password@localhost:5432/zinnia"
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
