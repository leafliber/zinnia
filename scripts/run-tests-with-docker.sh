#!/usr/bin/env bash
set -euo pipefail

# 启动测试容器、运行迁移并执行测试，结束后销毁容器。
# 依赖：docker, docker-compose, sqlx (可选)

COMPOSE_FILE="docker-compose.test.yml"
DB_USER=zinnia
DB_PASS=secret
DB_NAME=zinnia_test
DB_HOST=127.0.0.1
DB_PORT=55432
REDIS_HOST=127.0.0.1
REDIS_PORT=63790

echo "Starting test containers..."
docker-compose -f "$COMPOSE_FILE" up -d

# 等待 PostgreSQL 可用
echo "Waiting for timescaledb to be ready..."
for i in {1..60}; do
  if docker exec $(docker-compose -f "$COMPOSE_FILE" ps -q timescaledb) pg_isready -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; then
    echo "Timescaledb ready"
    break
  fi
  sleep 1
  echo -n "."
  if [ "$i" -eq 60 ]; then
    echo "\nTimed out waiting for timescaledb"
    docker-compose -f "$COMPOSE_FILE" logs
    docker-compose -f "$COMPOSE_FILE" down -v
    exit 1
  fi
done

# 等待 Redis 可用
echo "Waiting for redis to be ready..."
for i in {1..30}; do
  if docker exec $(docker-compose -f "$COMPOSE_FILE" ps -q redis) redis-cli -p 6379 ping >/dev/null 2>&1; then
    echo "Redis ready"
    break
  fi
  sleep 1
  echo -n "."
  if [ "$i" -eq 30 ]; then
    echo "\nTimed out waiting for redis"
    docker-compose -f "$COMPOSE_FILE" logs
    docker-compose -f "$COMPOSE_FILE" down -v
    exit 1
  fi
done

# 导出环境变量供迁移与测试使用
export DATABASE_URL="postgres://${DB_USER}:${DB_PASS}@${DB_HOST}:${DB_PORT}/${DB_NAME}"
export REDIS_URL="redis://:${DB_PASS}@${REDIS_HOST}:${REDIS_PORT}/0"

echo "DATABASE_URL=${DATABASE_URL}"

# 尝试运行 sqlx migrate run（如果未安装，会跳过提示）
if command -v sqlx >/dev/null 2>&1; then
  echo "Running sqlx migrations..."
  sqlx database create || true
  sqlx migrate run
else
  echo "sqlx-cli not found; skipping sqlx migrate run. If migrations are required, install sqlx-cli or run migrations in your test setup."
fi

# 运行测试
echo "Running cargo test..."
# 如果你只想运行集成测试，可以使用 --test 或 --lib 等过滤参数
cargo test
TEST_EXIT_CODE=$?

# 清理
echo "Tearing down test containers..."
docker-compose -f "$COMPOSE_FILE" down -v

exit $TEST_EXIT_CODE
