#!/bin/bash
# ===========================================
# Zinnia 开发辅助脚本
# ===========================================

set -e

case "$1" in
    # 启动开发环境
    start)
        echo "🚀 启动开发环境..."
        docker compose -f docker-compose.dev.yml up -d
        echo "✅ Docker 服务已启动"
        echo ""
        echo "📍 服务地址:"
        echo "   - Adminer:   http://localhost:8081"
        echo "   - Redis UI:  http://localhost:8082"
        ;;

    # 停止开发环境
    stop)
        echo "🛑 停止开发环境..."
        docker compose -f docker-compose.dev.yml down
        echo "✅ Docker 服务已停止"
        ;;

    # 重启开发环境
    restart)
        $0 stop
        $0 start
        ;;

    # 查看日志
    logs)
        docker compose -f docker-compose.dev.yml logs -f ${2:-}
        ;;

    # 运行数据库迁移
    migrate)
        echo "🗃️  运行数据库迁移..."
        export DATABASE_URL="postgres://zinnia:dev_password@localhost:5432/zinnia"
        sqlx migrate run
        echo "✅ 迁移完成"
        ;;

    # 回滚迁移
    migrate-revert)
        echo "🔙 回滚最近一次迁移..."
        export DATABASE_URL="postgres://zinnia:dev_password@localhost:5432/zinnia"
        sqlx migrate revert
        echo "✅ 回滚完成"
        ;;

    # 重置数据库
    db-reset)
        echo "⚠️  即将重置数据库，所有数据将被删除！"
        read -p "确认继续? [y/N] " confirm
        if [[ $confirm == [yY] ]]; then
            export DATABASE_URL="postgres://zinnia:dev_password@localhost:5432/zinnia"
            docker exec zinnia-timescaledb-dev psql -U zinnia -d postgres -c "DROP DATABASE IF EXISTS zinnia;"
            docker exec zinnia-timescaledb-dev psql -U zinnia -d postgres -c "CREATE DATABASE zinnia;"
            docker exec zinnia-timescaledb-dev psql -U zinnia -d zinnia -c "CREATE EXTENSION IF NOT EXISTS timescaledb;"
            sqlx migrate run
            echo "✅ 数据库已重置"
        else
            echo "❌ 已取消"
        fi
        ;;

    # 进入数据库 CLI
    db-cli)
        echo "🗄️  连接到 TimescaleDB..."
        docker exec -it zinnia-timescaledb-dev psql -U zinnia -d zinnia
        ;;

    # 进入 Redis CLI
    redis-cli)
        echo "📦 连接到 Redis..."
        docker exec -it zinnia-redis-dev redis-cli -a dev_password
        ;;

    # 检查代码
    check)
        echo "🔍 检查代码..."
        cargo check
        cargo clippy -- -D warnings
        echo "✅ 代码检查通过"
        ;;

    # 格式化代码
    fmt)
        echo "✨ 格式化代码..."
        cargo fmt
        echo "✅ 格式化完成"
        ;;

    # 运行测试
    test)
        echo "🧪 运行测试..."
        cargo test ${2:-}
        ;;

    # 构建 release
    build)
        echo "📦 构建 release 版本..."
        cargo build --release
        echo "✅ 构建完成: target/release/zinnia"
        ;;

    # 构建 Docker 镜像
    docker-build)
        echo "🐳 构建 Docker 镜像..."
        docker build -t zinnia:latest .
        echo "✅ 镜像构建完成: zinnia:latest"
        ;;

    # 清理
    clean)
        echo "🧹 清理..."
        cargo clean
        docker compose -f docker-compose.dev.yml down -v
        echo "✅ 清理完成"
        ;;

    # 帮助
    help|*)
        echo "Zinnia 开发辅助脚本"
        echo ""
        echo "用法: ./scripts/dev.sh <命令>"
        echo ""
        echo "命令:"
        echo "  start          启动 Docker 开发环境"
        echo "  stop           停止 Docker 开发环境"
        echo "  restart        重启 Docker 开发环境"
        echo "  logs [服务]    查看日志"
        echo ""
        echo "  migrate        运行数据库迁移"
        echo "  migrate-revert 回滚最近一次迁移"
        echo "  db-reset       重置数据库"
        echo "  db-cli         进入数据库 CLI"
        echo "  redis-cli      进入 Redis CLI"
        echo ""
        echo "  check          检查代码 (cargo check + clippy)"
        echo "  fmt            格式化代码"
        echo "  test [测试名]  运行测试"
        echo "  build          构建 release 版本"
        echo "  docker-build   构建 Docker 镜像"
        echo ""
        echo "  clean          清理构建产物和 Docker 数据"
        echo "  help           显示帮助"
        ;;
esac
