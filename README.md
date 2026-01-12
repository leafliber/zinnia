# 🌱 Zinnia

高性能时间序列后端服务 - 设备电量监控与预警系统

## 🎯 核心特性

- **Rust** - 零 GC、内存安全、高并发
- **Actix Web** - 异步非阻塞，高性能 Web 框架
- **TimescaleDB** - 基于 PostgreSQL 的时间序列数据库
- **Redis** - 高速缓存、限流、会话管理
- **安全优先** - JWT 认证、API Key、审计日志

## 📁 项目结构

```
zinnia/
├── src/
│   ├── main.rs              # 程序入口
│   ├── lib.rs               # 库入口
│   ├── config/              # 配置管理
│   ├── db/                  # 数据库连接
│   ├── errors/              # 错误处理
│   ├── handlers/            # HTTP 处理器
│   ├── middleware/          # 中间件
│   ├── models/              # 数据模型
│   ├── repositories/        # 数据访问层
│   ├── routes/              # 路由配置
│   ├── security/            # 安全模块
│   ├── services/            # 业务逻辑
│   └── utils/               # 工具函数
├── migrations/              # 数据库迁移
├── config/                  # 配置文件
└── tests/                   # 测试文件
```

## 🚀 快速开始

### 1. 环境要求

- Rust 1.75+
- PostgreSQL 15+ with TimescaleDB
- Redis 7+

### 2. 配置环境变量

```bash
cp .env.example .env
# 编辑 .env 文件，填入实际配置
```

### 3. 启动依赖服务

```bash
# 使用 Docker Compose (推荐)
docker-compose up -d timescaledb redis

# 或手动启动
# TimescaleDB
docker run -d --name timescaledb \
  -p 5432:5432 \
  -e POSTGRES_PASSWORD=your_password \
  -e POSTGRES_DB=zinnia \
  timescale/timescaledb:latest-pg15

# Redis
docker run -d --name redis \
  -p 6379:6379 \
  redis:7-alpine --requirepass your_redis_password
```

### 4. 运行数据库迁移

```bash
# 安装 sqlx-cli
cargo install sqlx-cli

# 运行迁移
sqlx migrate run
```

### 5. 构建运行

```bash
# 开发模式
cargo run

# 生产构建
cargo build --release
./target/release/zinnia
```

## 📡 API 端点

### 认证

| 方法 | 路径 | 描述 |
|------|------|------|
| POST | `/api/v1/auth/token` | 使用 API Key 获取 JWT |
| POST | `/api/v1/auth/refresh` | 刷新 Token |
| POST | `/api/v1/auth/revoke` | 吊销 Token |

### 设备管理

| 方法 | 路径 | 描述 |
|------|------|------|
| POST | `/api/v1/devices` | 注册设备 |
| GET | `/api/v1/devices` | 设备列表 |
| GET | `/api/v1/devices/:id` | 设备详情 |
| PUT | `/api/v1/devices/:id` | 更新设备 |
| DELETE | `/api/v1/devices/:id` | 删除设备 |
| GET | `/api/v1/devices/:id/config` | 获取配置 |
| PUT | `/api/v1/devices/:id/config` | 更新配置 |
| POST | `/api/v1/devices/:id/rotate-key` | 轮换 API Key |

### 电量数据

| 方法 | 路径 | 描述 |
|------|------|------|
| POST | `/api/v1/battery/report` | 上报电量 |
| POST | `/api/v1/battery/batch-report` | 批量上报 |
| GET | `/api/v1/battery/latest/:device_id` | 最新电量 |
| GET | `/api/v1/battery/history/:device_id` | 历史数据 |
| GET | `/api/v1/battery/stats/:device_id` | 统计信息 |

### 预警

| 方法 | 路径 | 描述 |
|------|------|------|
| POST | `/api/v1/alerts/rules` | 创建规则 |
| GET | `/api/v1/alerts/rules` | 规则列表 |
| GET | `/api/v1/alerts/events` | 事件列表 |
| POST | `/api/v1/alerts/events/:id/acknowledge` | 确认预警 |
| POST | `/api/v1/alerts/events/:id/resolve` | 解决预警 |

### 健康检查

| 方法 | 路径 | 描述 |
|------|------|------|
| GET | `/health` | 简单健康检查 |
| GET | `/health/detailed` | 详细健康检查 |
| GET | `/health/ready` | 就绪检查 |
| GET | `/health/live` | 存活检查 |

## 🔐 安全特性

- **JWT 认证** - 短期 Access Token (15 分钟) + 长期 Refresh Token (7 天)
- **API Key 认证** - 设备端使用，只存储哈希值
- **密码哈希** - Argon2id 算法
- **数据加密** - AES-256-GCM
- **限流保护** - 滑动窗口算法
- **安全头** - X-Content-Type-Options, X-Frame-Options 等
- **审计日志** - 记录关键操作

## 📊 性能指标

- **吞吐量**: 10,000+ req/s（单机）
- **延迟**: P99 < 50ms (缓存命中)
- **并发连接**: 10,000+
- **数据写入**: 100,000 点/秒（批量）

## 📄 许可证

MIT License
