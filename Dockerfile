# =====================================
# 阶段 1: 构建阶段
# =====================================
# 使用 Rust 1.92 以支持 Edition 2024
# base64ct 1.8.3 需要 Edition 2024 支持
FROM rust:1.92-slim-bookworm AS builder

# 设置构建参数
ARG CARGO_NET_GIT_FETCH_WITH_CLI=true
ARG CARGO_INCREMENTAL=0

# 安装必要的构建依赖
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

WORKDIR /app

# 复制 Cargo 文件（利用 Docker 缓存层）
COPY Cargo.toml Cargo.lock ./

# 创建虚拟 src 以缓存依赖（骨架构建技巧）
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    echo "// lib stub" > src/lib.rs

# 预构建依赖（这一层会被缓存，加速后续构建）
RUN cargo build --release --locked && \
    rm -rf src target/release/deps/zinnia* target/release/.fingerprint/zinnia* target/release/zinnia*

# 复制实际源代码
COPY src ./src
COPY migrations ./migrations
# 注意：不再复制 config/ 文件夹，所有配置通过环境变量管理

# 构建应用（使用 --locked 确保 Cargo.lock 一致性）
RUN cargo build --release --locked

# =====================================
# 阶段 2: 运行阶段
# =====================================
FROM debian:bookworm-slim AS runtime

# 设置环境变量
ENV RUST_BACKTRACE=1 \
    APP_USER=zinnia

# 安装运行时依赖（包括 gosu 用于安全降权）
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    tini \
    gosu \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# 创建非 root 用户（应用将以此用户运行）
RUN groupadd -r ${APP_USER} && useradd -r -g ${APP_USER} ${APP_USER}

WORKDIR /app

# 复制 Entrypoint 脚本（先复制脚本，以 root 设置权限）
COPY --chmod=755 ./scripts/entrypoint.sh /app/entrypoint.sh

# 从构建阶段复制二进制文件和迁移脚本
COPY --from=builder /app/target/release/zinnia /app/zinnia
COPY --from=builder /app/migrations /app/migrations
# 注意：不再复制 config/ 文件夹，配置完全通过环境变量管理

# 设置文件权限（应用文件归 zinnia 用户所有）
RUN chown -R ${APP_USER}:${APP_USER} /app && \
    chmod +x /app/zinnia

# 保持以 root 运行容器（entrypoint 需要读取 secrets 后再降权）
# entrypoint.sh 会使用 gosu 切换到 zinnia 用户执行应用

# 暴露端口
EXPOSE 8080

# 健康检查
# - start-period: 给应用足够的启动时间（Rust 应用可能需要连接数据库）
# - interval: 检查间隔
# - retries: 失败重试次数
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=5 \
    CMD curl -sf http://localhost:8080/health || exit 1

# 使用 tini 作为 init 系统（正确处理信号）
ENTRYPOINT ["/usr/bin/tini", "--", "/app/entrypoint.sh"]
CMD ["/app/zinnia"]
