#!/usr/bin/env bash
# Zinnia 生产环境管理脚本

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

COMPOSE_FILE="docker-compose.prod.yml"
ENV_FILE=".env.production"

# 检测 Docker Compose 命令
if docker compose version >/dev/null 2>&1; then
    COMPOSE="docker compose"
elif command -v docker-compose >/dev/null 2>&1; then
    COMPOSE="docker-compose"
else
    echo "错误: 未找到 docker-compose"
    exit 1
fi

# 基础命令
compose_cmd() {
    $COMPOSE -f "$COMPOSE_FILE" --env-file "$ENV_FILE" "$@"
}

case "${1:-help}" in
    start)
        echo "启动所有服务..."
        compose_cmd up -d
        ;;
    stop)
        echo "停止所有服务..."
        compose_cmd down
        ;;
    restart)
        echo "重启服务..."
        compose_cmd restart "${2:-}"
        ;;
    logs)
        compose_cmd logs -f "${2:-}"
        ;;
    ps)
        compose_cmd ps
        ;;
    exec)
        if [ -z "${2:-}" ]; then
            echo "用法: $0 exec <service> [command]"
            exit 1
        fi
        compose_cmd exec "$2" "${@:3}"
        ;;
    backup)
        echo "备份数据库..."
        timestamp=$(date +%Y%m%d_%H%M%S)
        backup_file="backup_${timestamp}.sql.gz"
        docker exec zinnia-timescaledb pg_dump -U zinnia zinnia | gzip > "./backups/$backup_file"
        echo "备份完成: ./backups/$backup_file"
        ;;
    restore)
        if [ -z "${2:-}" ]; then
            echo "用法: $0 restore <backup_file>"
            exit 1
        fi
        echo "恢复数据库..."
        gunzip -c "$2" | docker exec -i zinnia-timescaledb psql -U zinnia zinnia
        echo "恢复完成"
        ;;
    update)
        echo "更新服务..."
        git pull
        compose_cmd build
        compose_cmd up -d
        echo "更新完成"
        ;;
    clean)
        echo "清理未使用的资源..."
        docker system prune -f
        ;;
    ssl-renew)
        echo "执行 SSL 证书续签..."
        "$ROOT_DIR/scripts/renew-ssl.sh"
        ;;
    ssl-status)
        echo "检查 SSL 证书状态..."
        "$ROOT_DIR/scripts/renew-ssl.sh" info
        ;;
    help|*)
        cat << EOF
Zinnia 生产环境管理脚本

用法: $0 <command> [options]

命令:
  start                启动所有服务
  stop                 停止所有服务
  restart [service]    重启服务（可指定服务名）
  logs [service]       查看日志（可指定服务名）
  ps                   查看服务状态
  exec <service> <cmd> 在容器中执行命令
  backup               备份数据库
  restore <file>       从备份恢复数据库
  update               更新并重新部署
  clean                清理未使用的 Docker 资源
  ssl-renew            手动续签 SSL 证书
  ssl-status           查看 SSL 证书状态
  help                 显示此帮助

示例:
  $0 start                    # 启动所有服务
  $0 logs zinnia              # 查看应用日志
  $0 restart nginx            # 重启 Nginx
  $0 exec zinnia bash         # 进入应用容器
  $0 backup                   # 备份数据库
  $0 ssl-status               # 查看 SSL 证书状态
  $0 ssl-renew                # 手动续签 SSL 证书
EOF
        ;;
esac
