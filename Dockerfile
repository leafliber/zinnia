# =====================================
# 阶段 1: 构建阶段
# =====================================
# 使用与开发环境一致的 Rust 版本，确保构建一致性
FROM rust:1.92-slim-bookworm AS builder

# 安装必要的构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 复制 Cargo 文件
COPY Cargo.toml Cargo.lock ./

# 创建虚拟 src 以缓存依赖
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "// lib stub" > src/lib.rs

# 构建依赖（这一层会被缓存）
RUN cargo build --release && \
    rm -rf src target/release/deps/zinnia*

# 复制实际源代码
COPY src ./src
COPY migrations ./migrations
COPY config ./config

# 构建应用
RUN cargo build --release

# =====================================
# 阶段 2: 运行阶段
# =====================================
FROM debian:bookworm-slim AS runtime

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 创建非 root 用户
RUN groupadd -r zinnia && useradd -r -g zinnia zinnia

WORKDIR /app

# 从构建阶段复制二进制文件
COPY --from=builder /app/target/release/zinnia /app/zinnia
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/config /app/config

# 设置文件权限
RUN chown -R zinnia:zinnia /app

# 切换到非 root 用户
USER zinnia

# 暴露端口
EXPOSE 8080

# 健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# 运行应用
ENTRYPOINT ["/app/zinnia"]
