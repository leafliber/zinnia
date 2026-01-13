# Zinnia 生产环境部署指南

本文档描述如何在生产服务器上部署 Zinnia 电量监控系统。

## 📋 系统要求

### 硬件要求
- CPU: 2 核心以上
- 内存: 4GB 以上（推荐 8GB）
- 磁盘: 20GB 可用空间

### 软件要求
- 操作系统: Linux (Ubuntu 20.04+, CentOS 8+, Debian 11+)
- Docker: 20.10+
- Docker Compose: 2.0+
- 网络: 开放端口 80, 443

## 🚀 快速开始（推荐）

### 一键部署

```bash
# 克隆仓库
git clone <repository-url> /opt/zinnia
cd /opt/zinnia

# 运行交互式部署脚本
./scripts/deploy.sh
```

脚本会自动：
1. 检查系统环境
2. 交互式配置（数据库、Redis、密钥等）
3. 构建并启动所有服务
4. 运行数据库迁移
5. 执行健康检查
6. （可选）配置 SSL 证书

## 📦 部署架构

生产环境包含以下服务：

```
┌─────────────────────────────────────────┐
│           Internet (443/80)              │
└────────────────┬────────────────────────┘
                 │
        ┌────────▼────────┐
        │  Nginx (反向代理) │
        │  - SSL 终止      │
        │  - 限流         │
        │  - 安全头       │
        └────────┬────────┘
                 │
        ┌────────▼────────┐
        │  Zinnia 应用     │
        │  (Rust/Actix)   │
        └─────┬───┬────────┘
              │   │
      ┌───────┘   └───────┐
      │                   │
┌─────▼─────┐      ┌──────▼──────┐
│TimescaleDB│      │    Redis    │
│ (PostgreSQL)     │   (缓存)    │
└───────────┘      └─────────────┘
```

### 网络架构
- **frontend 网络**: Nginx ↔ Zinnia（172.20.0.0/24）
- **backend 网络**: Zinnia ↔ DB/Redis（172.21.0.0/24，内部隔离）

### 端口映射
- **80**: HTTP（Nginx）
- **443**: HTTPS（Nginx，配置 SSL 后）
- **内部**: 应用、数据库、Redis 不对外暴露

## 🔧 手动部署（高级）

### 1. 准备环境

```bash
# 安装 Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

# 安装 Docker Compose 插件
sudo apt install -y docker-compose-plugin

# 创建项目目录
sudo mkdir -p /opt/zinnia
cd /opt/zinnia
```

### 2. 配置密钥

```bash
# 创建密钥目录
mkdir -p secrets
chmod 700 secrets

# 生成数据库密码
openssl rand -base64 32 | tr -d '/+=' > secrets/db_password

# 生成 Redis 密码
openssl rand -base64 32 | tr -d '/+=' > secrets/redis_password

# 生成 JWT 密钥
openssl rand -base64 32 > secrets/jwt_secret

# 生成加密密钥
openssl rand -base64 32 > secrets/encryption_key

# 设置权限
chmod 600 secrets/*
```

### 3. 配置环境变量

```bash
# 复制配置模板
cp .env.production.example .env.production

# 编辑配置
nano .env.production
```

必需配置项：
```bash
POSTGRES_USER=zinnia
POSTGRES_DB=zinnia
DOMAIN=your-domain.com
SSL_EMAIL=admin@your-domain.com
```

### 4. 启动服务

```bash
# 构建并启动
docker compose -f docker-compose.prod.yml up -d

# 查看状态
docker compose -f docker-compose.prod.yml ps

# 查看日志
docker compose -f docker-compose.prod.yml logs -f
```

### 5. 验证部署

```bash
# 健康检查
curl http://localhost/health

# 预期输出
# {"status":"healthy"}
```

## 🔐 SSL 证书配置

### 使用 Let's Encrypt（自动）

部署脚本会自动配置，也可手动执行：

```bash
# 安装 certbot
sudo apt install -y certbot

# 获取证书（需要暂停 nginx）
docker compose -f docker-compose.prod.yml stop nginx

sudo certbot certonly --standalone \
  --email admin@your-domain.com \
  -d your-domain.com

docker compose -f docker-compose.prod.yml start nginx
```

### 手动配置证书

```bash
# 复制证书到 nginx 目录
sudo cp /etc/letsencrypt/live/your-domain.com/fullchain.pem nginx/certs/
sudo cp /etc/letsencrypt/live/your-domain.com/privkey.pem nginx/certs/
sudo cp /etc/letsencrypt/live/your-domain.com/chain.pem nginx/certs/

# 生成 DH 参数
openssl dhparam -out nginx/dhparam/dhparam.pem 2048

# 更新 Nginx 配置
# 取消注释 nginx/conf.d/zinnia.conf 中的 HTTPS 部分

# 重启 Nginx
docker compose -f docker-compose.prod.yml restart nginx
```

## 📊 日常运维

### 使用管理脚本

```bash
# 查看服务状态
./scripts/manage.sh ps

# 查看日志
./scripts/manage.sh logs zinnia

# 重启服务
./scripts/manage.sh restart nginx

# 备份数据库
./scripts/manage.sh backup

# 更新服务
./scripts/manage.sh update
```

### 数据库备份

```bash
# 自动备份（推荐设置 cron）
./scripts/manage.sh backup

# 手动备份
docker exec zinnia-timescaledb pg_dump -U zinnia zinnia | gzip > backup.sql.gz

# 恢复备份
gunzip -c backup.sql.gz | docker exec -i zinnia-timescaledb psql -U zinnia zinnia
```

### 查看日志

```bash
# 查看所有服务日志
docker compose -f docker-compose.prod.yml logs -f

# 查看特定服务
docker compose -f docker-compose.prod.yml logs -f zinnia
docker compose -f docker-compose.prod.yml logs -f nginx
docker compose -f docker-compose.prod.yml logs -f timescaledb
```

### 更新部署

```bash
# 方式 1：使用管理脚本
./scripts/manage.sh update

# 方式 2：手动更新
git pull
docker compose -f docker-compose.prod.yml build
docker compose -f docker-compose.prod.yml up -d
```

## 🛡️ 安全最佳实践

### 已实现的安全措施

1. **网络隔离**: 数据库和 Redis 在内部网络，不对外暴露
2. **密钥管理**: 使用 Docker secrets 存储敏感信息
3. **最小权限**: 容器使用非 root 用户运行
4. **只读文件系统**: 关键目录只读挂载
5. **安全头**: Nginx 配置完整的安全响应头
6. **限流**: API 请求限流和连接限制
7. **日志轮转**: 自动日志轮转，防止磁盘占满

### 额外建议

1. **防火墙配置**
```bash
# 仅开放必要端口
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw allow 22/tcp  # SSH
sudo ufw enable
```

2. **定期更新**
```bash
# 更新系统
sudo apt update && sudo apt upgrade -y

# 更新 Docker 镜像
docker compose -f docker-compose.prod.yml pull
```

3. **监控告警**
   - 配置健康检查监控
   - 设置磁盘空间告警
   - 监控日志错误

## 🔍 故障排查

### 服务无法启动

```bash
# 查看服务状态
docker compose -f docker-compose.prod.yml ps

# 查看详细日志
docker compose -f docker-compose.prod.yml logs

# 检查容器健康
docker inspect zinnia-app --format='{{.State.Health.Status}}'
```

### 数据库连接失败

```bash
# 检查数据库容器
docker exec zinnia-timescaledb pg_isready -U zinnia

# 检查密钥文件
ls -la secrets/

# 验证连接
docker exec zinnia-timescaledb psql -U zinnia -d zinnia -c "\l"
```

### Nginx 502 错误

```bash
# 检查应用是否运行
curl http://localhost:8080/health

# 检查 Nginx 配置
docker exec zinnia-nginx nginx -t

# 重启 Nginx
docker compose -f docker-compose.prod.yml restart nginx
```

## 📈 性能优化

### 数据库优化

```sql
-- 创建必要的索引
-- 查看慢查询
-- 调整连接池大小
```

### Redis 优化

```bash
# 调整内存策略（已配置）
# - maxmemory: 512MB
# - maxmemory-policy: allkeys-lru
```

### 应用优化

```bash
# 增加 worker 数量（编辑 config/production.toml）
# 调整连接池大小
# 启用响应缓存
```

## 🔗 相关文档

- [API 文档](./API_REFERENCE.md)
- [安全分析](./SECURITY_ANALYSIS.md)
- [开发指南](../README.md)

## 📞 支持

如遇问题，请：
1. 查看日志：`./scripts/manage.sh logs`
2. 检查健康状态：`./scripts/manage.sh ps`
3. 提交 Issue 或联系管理员
