#!/usr/bin/env bash
set -euo pipefail

# Detect container runtime and compose command (docker or podman)
# Sets: DOCKER_CMD and COMPOSE variables (use with eval for compose)
detect_container_tool() {
  DOCKER_CMD=""
  COMPOSE=""

  if command -v podman >/dev/null 2>&1 && command -v podman-compose >/dev/null 2>&1; then
    DOCKER_CMD="podman"
    COMPOSE="podman-compose"
  elif command -v docker >/dev/null 2>&1; then
    DOCKER_CMD="docker"
    # prefer `docker compose` if supported
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

# 启动测试容器、运行迁移并执行测试，结束后销毁容器。
# 依赖：docker/podman, docker-compose/podman-compose, sqlx (可选)

COMPOSE_FILE="docker-compose.test.yml"
DB_USER=zinnia
DB_PASS=secret
DB_NAME=zinnia_test
DB_HOST=127.0.0.1
DB_PORT=55432
REDIS_HOST=127.0.0.1
REDIS_PORT=63790

echo "Starting test containers..."
eval "$COMPOSE -f \"$COMPOSE_FILE\" up -d"

# 等待 PostgreSQL 可用
echo "Waiting for timescaledb to be ready..."
for i in {1..60}; do
  CONTAINER_ID=$(eval "$COMPOSE -f \"$COMPOSE_FILE\" ps -q timescaledb" || true)
  if [ -n "$CONTAINER_ID" ]; then
    if $DOCKER_CMD exec "$CONTAINER_ID" pg_isready -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; then
      echo "Timescaledb ready"
      break
    fi
  fi
  sleep 1
  echo -n "."
  if [ "$i" -eq 60 ]; then
    echo "\nTimed out waiting for timescaledb"
    eval "$COMPOSE -f \"$COMPOSE_FILE\" logs"
    eval "$COMPOSE -f \"$COMPOSE_FILE\" down -v"
    exit 1
  fi
done

# 等待 Redis 可用
echo "Waiting for redis to be ready..."
for i in {1..30}; do
  REDIS_CONTAINER_ID=$(eval "$COMPOSE -f \"$COMPOSE_FILE\" ps -q redis" || true)
  if [ -n "$REDIS_CONTAINER_ID" ]; then
    if $DOCKER_CMD exec "$REDIS_CONTAINER_ID" redis-cli -p 6379 ping >/dev/null 2>&1; then
      echo "Redis ready"
      break
    fi
  fi
  sleep 1
  echo -n "."
  if [ "$i" -eq 30 ]; then
    echo "\nTimed out waiting for redis"
    eval "$COMPOSE -f \"$COMPOSE_FILE\" logs"
    eval "$COMPOSE -f \"$COMPOSE_FILE\" down -v"
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
  eval "$COMPOSE -f \"$COMPOSE_FILE\" down -v"

exit $TEST_EXIT_CODE
