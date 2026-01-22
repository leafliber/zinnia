# Zinnia 实施指南

> **设备电量监控与预警系统 - 完整实施指南**
>
> 版本：v1.0 | 最后更新：2026-01-22

## 目录

1. [系统架构](#系统架构)
2. [认证与授权](#认证与授权)
3. [通知系统](#通知系统)
4. [Web Push 实现](#web-push-实现)
5. [部署指南](#部署指南)
6. [令牌管理](#令牌管理)

---

## 系统架构

### 整体架构
Zinnia 采用分层架构设计，确保高内聚低耦合：

```
┌─────────────────────────────────────┐
│  表现层 (Handlers/Websocket)         │
│  • HTTP REST API                     │
│  • WebSocket 实时通信                │
└─────────────────────────────────────┘
         ↓
┌─────────────────────────────────────┐
│  业务逻辑层 (Services)               │
│  • AuthService - 认证服务            │
│  • BatteryService - 电量服务         │
│  • AlertService - 预警服务           │
│  • NotificationService - 通知服务    │
└─────────────────────────────────────┘
         ↓
┌─────────────────────────────────────┐
│  数据访问层 (Repositories)           │
│  • DeviceRepository - 设备仓库       │
│  • BatteryRepository - 电量仓库      │
│  • AlertRepository - 预警仓库        │
│  • UserRepository - 用户仓库         │
└─────────────────────────────────────┘
         ↓
┌─────────────────────────────────────┐
│  基础设施层 (DB/Cache)               │
│  • TimescaleDB - 时间序列数据库      │
│  • Redis - 缓存/会话管理             │
└─────────────────────────────────────┘
```

### 技术栈
- **编程语言**: Rust 2021 edition
- **Web 框架**: Actix Web 4.x
- **数据库**: TimescaleDB (PostgreSQL 扩展)
- **缓存**: Redis 7+
- **认证**: JWT + API Key 双令牌体系
- **实时通信**: WebSocket
- **通知**: Web Push + Email + Webhook
- **部署**: Docker + Docker Compose + Nginx

---

## 认证与授权

### 双令牌体系
Zinnia 实现了完整的双令牌架构，兼顾安全性和易用性：

#### 用户认证流程
1. **登录**: 用户通过邮箱/用户名 + 密码登录
2. **获取令牌**: 返回 access_token (15分钟) + refresh_token (7天)
3. **访问资源**: 使用 access_token 访问受保护资源
4. **刷新令牌**: access_token 过期后使用 refresh_token 获取新令牌

#### 设备认证流程（推荐模式）
1. **创建设备**: 用户创建设备，获取 API Key
2. **令牌交换**: 设备使用 API Key 交换 JWT
3. **数据上报**: 使用 JWT 上报电量数据
4. **自动续期**: JWT 过期后可刷新或重新交换

#### 数据库设计
```sql
-- 设备访问令牌表
CREATE TABLE device_access_tokens (
    id UUID PRIMARY KEY,
    device_id UUID NOT NULL,
    created_by UUID NOT NULL,
    token_hash VARCHAR(255) NOT NULL,      -- Argon2 哈希
    token_prefix VARCHAR(30) NOT NULL,      -- 用于快速查找
    name VARCHAR(100) NOT NULL,
    permission token_permission NOT NULL,   -- read/write/all
    expires_at TIMESTAMPTZ,                 -- NULL = 永不过期
    last_used_at TIMESTAMPTZ,              -- 使用跟踪
    use_count INTEGER NOT NULL DEFAULT 0,
    is_revoked BOOLEAN NOT NULL DEFAULT FALSE,
    revoked_at TIMESTAMPTZ,
    allowed_ips TEXT[],                     -- IP 白名单
    rate_limit_per_minute INTEGER,         -- 速率限制
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_device_access_tokens_prefix ON device_access_tokens(token_prefix);
```

### 安全特性
- **API Key 保护**: Argon2id 哈希存储，前缀索引快速查找
- **JWT 保护**: 短期有效 (15分钟)，HMAC-SHA256 签名
- **多层防御**: Nginx 限流 + 应用层认证 + 服务层权限检查
- **审计日志**: 记录关键操作和使用统计

---

## 通知系统

### 通知渠道
Zinnia 支持多种通知渠道：

1. **邮件通知 (Email)**
   - SMTP 协议发送
   - 支持 TLS 加密
   - 模板化邮件内容

2. **Web Push**
   - VAPID 协议
   - 浏览器推送通知
   - 支持离线消息

3. **Webhook**
   - HTTP POST 回调
   - 自定义 payload
   - 重试机制

### 通知流程
```
预警触发 → 规则评估 → 渠道选择 → 消息构建 → 发送执行 → 状态跟踪
```

### 用户偏好设置
用户可配置：
- 启用/禁用特定渠道
- 设置静默时间段
- 配置通知级别 (info/warning/critical)
- Webhook URL 和认证信息

---

## Web Push 实现

### VAPID 密钥生成
```bash
# 生成 VAPID 密钥对
./scripts/generate-vapid-keys.sh

# 输出示例
VAPID_PRIVATE_KEY=BJ3sN... (使用 .env.production)
VAPID_PUBLIC_KEY=BM1sT... (前端使用)
```

### Web Push 流程
1. **订阅**: 前端浏览器订阅推送服务
2. **保存**: 后端保存 subscription 信息
3. **触发**: 预警系统触发 Web Push 通知
4. **发送**: 使用 VAPID 签名发送推送
5. **展示**: 浏览器接收并展示通知

### 安全考虑
- VAPID 签名验证
- 订阅信息加密存储
- 推送频率限制
- 错误处理和重试机制

---

## 部署指南

### 环境要求
- **操作系统**: Linux/Unix (推荐 Ubuntu 20.04+)
- **内存**: 最小 2GB，推荐 4GB+
- **存储**: 最小 20GB，推荐 50GB+ (取决于数据量)
- **网络**: 静态公网 IP 或域名

### 快速部署
```bash
# 1. 克隆项目
git clone https://github.com/yourusername/zinnia.git
cd zinnia

# 2. 运行部署脚本 (交互式)
chmod +x scripts/deploy.sh
./scripts/deploy.sh

# 3. 访问服务
# HTTP: http://your-domain.com
# HTTPS: https://your-domain.com (自动配置 SSL)
```

### 手动部署
```bash
# 1. 安装依赖
sudo apt update
sudo apt install -y docker.io docker-compose nginx certbot

# 2. 配置环境变量
cp .env.production.example .env.production
# 编辑 .env.production 填入实际配置

# 3. 生成 VAPID 密钥
./scripts/generate-vapid-keys.sh

# 4. 启动服务
docker-compose -f docker-compose.prod.yml up -d

# 5. 配置 SSL (可选)
./scripts/enable-https.sh your-domain.com

# 6. 健康检查
curl http://localhost:8080/health/detailed
```

### 配置说明
主要配置文件：
- `.env.production` - 环境变量配置
- `docker-compose.prod.yml` - Docker 服务配置
- `nginx/conf.d/zinnia.conf` - Nginx 配置

### 运维命令
```bash
# 查看日志
docker-compose logs -f zinnia

# 重启服务
./scripts/manage.sh restart

# 备份数据库
docker-compose exec timescaledb pg_dump -U postgres zinnia > backup.sql

# 更新服务
./scripts/deploy.sh --update

# 安全扫描
./scripts/security-check.sh
```

---

## 令牌管理

### API Key 最佳实践
1. **安全存储**: API Key 仅在创建时显示一次，请立即保存
2. **定期轮换**: 建议每 30 天轮换一次 API Key
3. **权限最小化**: 根据设备需求分配 read/write/all 权限
4. **IP 白名单**: 配置 allowed_ips 限制来源 IP
5. **速率限制**: 设置 rate_limit_per_minute 防止滥用

### JWT 使用建议
1. **短期有效**: access_token 有效期 15 分钟，减少泄露风险
2. **自动刷新**: 使用 refresh_token 自动续期
3. **黑名单机制**: 支持令牌吊销，实时生效
4. **无状态验证**: 服务端无需存储，性能更优

### 监控指标
```bash
# 查看令牌使用情况
GET /api/v1/devices/{device_id}/tokens

# 监控指标
- API Key 总数/设备
- 僵尸令牌数量 (>30天未使用)
- JWT 验证失败率
- 令牌吊销率
```

---

## 开发指南

### 项目结构
```
src/
├── main.rs                 # 程序入口
├── lib.rs                  # 库入口
├── config/                 # 配置管理
├── db/                     # 数据库连接
├── errors/                 # 错误处理
├── handlers/               # HTTP 处理器
├── middleware/             # Actix 中间件
├── models/                 # 数据模型
├── repositories/           # 数据访问层
├── routes/                 # 路由配置
├── security/               # 安全模块
├── services/               # 业务逻辑
├── utils/                  # 工具函数
└── websocket/              # WebSocket 实现
```

### 添加新 API
1. 在 `src/models/` 定义请求/响应结构
2. 在 `src/handlers/` 实现处理器
3. 在 `src/services/` 添加业务逻辑
4. 在 `src/routes/` 注册路由
5. 在 `tests/integration/` 添加测试

### 数据库迁移
```bash
# 创建新迁移
sqlx migrate add <migration_name>

# 运行迁移
sqlx migrate run

# 回滚迁移
sqlx migrate revert

# 检查状态
sqlx migrate info
```

---

## 故障排除

### 常见问题

**数据库连接失败**
```bash
# 检查数据库状态
docker-compose ps
docker-compose logs timescaledb

# 验证连接
psql -h localhost -U postgres -d zinnia
```

**Redis 连接失败**
```bash
# 检查 Redis 状态
docker-compose logs redis
redis-cli ping

# 验证认证
redis-cli --raw incr ping
```

**JWT 验证失败**
```bash
# 检查密钥配置
echo $JWT_SECRET

# 验证令牌格式
jwt decode <token>
```

**Web Push 发送失败**
```bash
# 检查 VAPID 密钥
echo $VAPID_PRIVATE_KEY
echo $VAPID_PUBLIC_KEY

# 测试推送
./scripts/test-webpush.sh
```

### 性能优化

**数据库优化**
```sql
-- 添加索引
CREATE INDEX idx_battery_data_device_time ON battery_data(device_id, recorded_at DESC);

-- 清理旧数据
DELETE FROM battery_data WHERE recorded_at < NOW() - INTERVAL '90 days';

-- 分析查询性能
EXPLAIN ANALYZE SELECT * FROM battery_data WHERE device_id = '...';
```

**缓存优化**
```rust
// 使用 Redis 缓存设备信息
let cache_key = format!("device:{}", device_id);
let cached: Option<Device> = redis.get(&cache_key).await?;
```

---

## 安全最佳实践

### 网络安全
- 使用 HTTPS 加密所有通信
- 配置 CORS 白名单
- 启用 HSTS 头
- 定期更新 SSL 证书

### 数据安全
- 敏感数据 AES-256-GCM 加密
- 密码 Argon2id 哈希
- API Key 前缀索引 + 哈希存储
- 定期备份数据

### 访问控制
- 最小权限原则
- IP 白名单限制
- 速率限制保护
- 操作审计日志

---

## 监控与告警

### 健康检查端点
```bash
# 基础健康检查
curl http://localhost:8080/health

# 详细健康检查
curl http://localhost:8080/health/detailed

# Kubernetes 就绪检查
curl http://localhost:8080/health/ready

# Kubernetes 存活检查
curl http://localhost:8080/health/live
```

### 关键监控指标
- 请求延迟 (P50/P95/P99)
- 错误率 (4xx/5xx)
- 数据库连接池使用率
- Redis 缓存命中率
- JWT 验证失败率
- 通知发送成功率

### 日志分析
```bash
# 查看错误日志
docker-compose logs -f zinnia | grep ERROR

# 统计请求
docker-compose logs zinnia | awk '{print $7}' | sort | uniq -c | sort -nr

# 监控 WebSocket 连接
docker-compose logs zinnia | grep "WebSocket"
```

---

## 参考信息

### 相关文档
- [API 参考文档](API_REFERENCE.md) - 完整 API 接口说明
- [令牌使用指南](TOKEN_GUIDE.md) - 详细的令牌使用说明

### 联系支持
- **GitHub Issues**: https://github.com/yourusername/zinnia/issues
- **文档问题**: 提交 PR 到 docs/ 目录
- **安全漏洞**: 请通过 GitHub Security Advisory 报告

### 版本历史
- **v0.1.0** (2026-01-22): 初始版本，包含核心功能
  - 用户管理
  - 设备管理
  - 电量数据上报
  - 预警规则
  - 通知系统
  - WebSocket 实时通信
  - Web Push 推送

---

*文档版本: 1.0.0*
*最后更新: 2026-01-22*
*维护者: Zinnia Team*