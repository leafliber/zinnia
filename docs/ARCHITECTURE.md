# Zinnia 架构设计文档

## 认证与授权架构

### 双令牌体系（方案 A）

Zinnia 实现了完整且标准的双令牌架构，兼顾安全性、易用性和向后兼容性。

#### 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                         用户端                                │
├─────────────────────────────────────────────────────────────┤
│  登录 → JWT (access_token + refresh_token)                   │
│  • 访问令牌：15 分钟有效期                                      │
│  • 刷新令牌：7 天有效期                                          │
│  • 支持黑名单撤销                                              │
└──────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────┐
│                      设备认证（双模式）                          │
├────────────────────────────────────────────────────────────┤
│ 模式 1: API Key 直接使用（兼容）                            │
│   Authorization: Bearer zdat_xxx...                         │
│   ├─ 验证: 哈希比对 + 权限检查                             │
│   ├─ 性能: DB 查询 + 缓存                                 │
│   └─ 风险: 长期有效，泄露风险较高                          │
│                                                              │
│ 模式 2：API Key → JWT 交换（推荐）                         │
│   POST /api/v1/auth/exchange                               │
│   { "api_key": "zdat_xxx..." }                             │
│   → 返回短期 JWT (15分钟)                                   │
│                                                              │
│   优势：✅ 短期有效 ✅ 无状态验证 ✅ 可刷新                │
└──────────────────────────────────────────────────────────┘
```

### 技术实现

#### 核心组件

| 组件 | 路径 | 职责 |
|------|------|------|
| **JWT 管理器** | `src/security/jwt.rs` | JWT 生成、验证、过期管理 |
| **Token 生成器** | `src/security/token.rs` | 统一的令牌生成（API Key、刷新令牌） |
| **设备令牌仓库** | `src/repositories/device_token_repo.rs` | API Key 持久化、哈希存储 |
| **设备令牌服务** | `src/services/device_token_service.rs` | 令牌业务逻辑、验证、撤销 |
| **认证服务** | `src/services/auth_service.rs` | 认证编排、JWT/API Key 交换 |
| **认证中间件** | `src/middleware/auth.rs` | HTTP 层认证拦截、多策略支持 |

#### 数据库设计

**device_access_tokens 表**：
```sql
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

#### 认证流程

##### 1. 设备初始化（创建 API Key）

```
用户（JWT） → POST /api/v1/devices/{id}/tokens
  ├─ 验证用户权限（owns device）
  ├─ 检查令牌数量（最多 20 个/设备）
  ├─ 生成随机令牌: zdat_<32字节随机>
  ├─ Argon2 哈希存储
  └─ 返回明文令牌（仅此一次）
```

##### 2. 设备认证（API Key → JWT）

```
设备 → POST /api/v1/auth/exchange { "api_key": "zdat_xxx..." }
  ├─ 提取前缀 → 查询 DB（索引查找）
  ├─ Argon2 验证哈希
  ├─ 检查权限、IP、过期时间
  ├─ 记录 last_used_at（异步）
  ├─ 生成 JWT（device_id, role=device）
  └─ 返回 access_token + refresh_token
```

##### 3. 请求认证（JWT/API Key）

```
HTTP 请求 → Authorization: Bearer <token>
  ├─ JwtOrApiKeyAuth 中间件
  │   ├─ 检查令牌前缀
  │   │   ├─ eyJ* → JWT 验证（签名校验）
  │   │   └─ zdat_* → API Key 验证（DB 查询）
  │   └─ 注入 AuthInfo 到请求上下文
  └─ Handler 处理业务逻辑
```

### 安全特性

#### 1. API Key 保护

- ✅ **Argon2id 哈希存储**：成本参数 `m=19456, t=2, p=1`
- ✅ **前缀索引**：快速查找，无需全表扫描
- ✅ **IP 白名单**：`allowed_ips` 字段限制来源
- ✅ **权限分离**：read/write/all 最小权限原则
- ✅ **可撤销**：实时生效，记录 `revoked_at`
- ✅ **使用审计**：`last_used_at`, `use_count` 跟踪

#### 2. JWT 保护

- ✅ **短期有效**：15 分钟，减少泄露窗口
- ✅ **HMAC-SHA256 签名**：防篡改
- ✅ **黑名单机制**：Redis 存储已撤销 token
- ✅ **Claims 验证**：exp, iss, aud, jti
- ✅ **刷新令牌轮换**：每次刷新生成新的 refresh_token

#### 3. 多层防御

```
┌─────────────────────────────────────┐
│ Layer 1: Nginx 限流                 │
│  • 全局: 10 req/s                    │
│  • 登录: 5 req/min                  │
└─────────────────────────────────────┘
         ↓
┌─────────────────────────────────────┐
│ Layer 2: 认证中间件                 │
│  • JWT 签名校验                      │
│  • API Key 哈希验证                  │
│  • 黑名单检查（Redis）               │
└─────────────────────────────────────┘
         ↓
┌─────────────────────────────────────┐
│ Layer 3: 服务层                     │
│  • 权限检查（read/write/all）        │
│  • IP 白名单                         │
│  • 业务限流（rate_limit_per_minute）│
└─────────────────────────────────────┘
```

### 性能优化

#### 1. 缓存策略

- **API Key 验证**：查询结果缓存 5 分钟（Redis）
- **JWT 黑名单**：Redis SET 结构，O(1) 查询
- **设备信息**：缓存设备基本信息，减少 DB 查询

#### 2. 异步处理

- **使用记录更新**：`tokio::spawn` 异步执行 `record_usage()`
- **审计日志**：异步写入，不阻塞请求

#### 3. 数据库索引

```sql
-- 核心查询索引
CREATE INDEX idx_device_access_tokens_prefix ON device_access_tokens(token_prefix);
CREATE INDEX idx_device_access_tokens_device ON device_access_tokens(device_id);
CREATE INDEX idx_device_access_tokens_expires ON device_access_tokens(expires_at) 
    WHERE expires_at IS NOT NULL AND is_revoked = FALSE;
```

### API 端点汇总

#### 认证端点

| 端点 | 方法 | 认证 | 说明 |
|------|------|------|------|
| `/api/v1/auth/exchange` | POST | 公开 | API Key → JWT 交换（推荐） |
| `/api/v1/auth/token` | POST | 公开 | 同上（兼容别名） |
| `/api/v1/auth/refresh` | POST | 公开 | 刷新 JWT |
| `/api/v1/auth/revoke` | POST | 公开 | 撤销 JWT |
| `/api/v1/auth/logout` | POST | JWT | 用户登出 |

#### 令牌管理端点

| 端点 | 方法 | 认证 | 说明 |
|------|------|------|------|
| `/api/v1/devices/{id}/tokens` | POST | JWT(用户) | 创建 API Key |
| `/api/v1/devices/{id}/tokens` | GET | JWT(用户) | 列出 API Keys |
| `/api/v1/devices/{id}/tokens/{token_id}` | DELETE | JWT(用户) | 撤销单个 API Key |
| `/api/v1/devices/{id}/tokens` | DELETE | JWT(用户) | 撤销所有 API Keys |

### 使用规范

#### 设备端最佳实践

```python
import requests
import time

class ZinniaClient:
    def __init__(self, api_key: str):
        self.api_key = api_key
        self.access_token = None
        self.refresh_token = None
        self.token_expires_at = 0
    
    def ensure_authenticated(self):
        """确保有有效的 JWT"""
        if time.time() < self.token_expires_at:
            return  # JWT 仍有效
        
        if self.refresh_token:
            # 尝试刷新
            try:
                self.refresh_jwt()
                return
            except:
                pass  # 刷新失败，降级到 API Key 交换
        
        # API Key → JWT 交换
        self.exchange_token()
    
    def exchange_token(self):
        """用 API Key 换取 JWT"""
        resp = requests.post(
            "https://api.example.com/api/v1/auth/exchange",
            json={"api_key": self.api_key}
        ).json()
        
        data = resp["data"]
        self.access_token = data["access_token"]
        self.refresh_token = data["refresh_token"]
        self.token_expires_at = time.time() + data["expires_in"] - 60  # 提前 1 分钟刷新
    
    def refresh_jwt(self):
        """刷新 JWT"""
        resp = requests.post(
            "https://api.example.com/api/v1/auth/refresh",
            json={"refresh_token": self.refresh_token}
        ).json()
        
        data = resp["data"]
        self.access_token = data["access_token"]
        self.refresh_token = data["refresh_token"]
        self.token_expires_at = time.time() + data["expires_in"] - 60
    
    def report_battery(self, level: int):
        """上报电量"""
        self.ensure_authenticated()
        
        headers = {"Authorization": f"Bearer {self.access_token}"}
        requests.post(
            "https://api.example.com/api/v1/battery/report",
            headers=headers,
            json={"battery_level": level}
        )

# 使用
client = ZinniaClient(api_key="zdat_xxx...")
client.report_battery(85)  # 自动处理认证
```

### 监控与运维

#### 关键指标

1. **API Key 使用**
   - 总数/设备
   - 最后使用时间 > 30 天的令牌数量（僵尸令牌）
   - 撤销率

2. **JWT 验证**
   - 签名验证失败率
   - 过期令牌比例
   - 刷新成功率

3. **性能指标**
   - API Key 验证耗时（P50/P95/P99）
   - JWT 验证耗时
   - 缓存命中率

#### 运维命令

```bash
# 查看令牌使用情况
GET /api/v1/devices/{device_id}/tokens

# 清理过期令牌（数据库维护）
DELETE FROM device_access_tokens 
WHERE (is_revoked = TRUE AND revoked_at < NOW() - INTERVAL '30 days')
   OR (expires_at IS NOT NULL AND expires_at < NOW() - INTERVAL '30 days');

# Redis 黑名单清理（自动 TTL 过期）
# 无需手动操作
```

### 迁移路径

#### Phase 1: 当前（全支持）
- ✅ API Key 直接使用
- ✅ API Key → JWT 交换
- ✅ 用户 JWT

#### Phase 2: 推荐 JWT（30 天后）
- 📝 文档标注 API Key 直接使用为"不推荐"
- 📊 监控 API Key 直接使用比例
- ⚡ JWT 请求更宽松的速率限制

#### Phase 3: 强制 JWT（可选，90 天后）
- 🚫 API Key 直接使用返回警告头
- 📧 发送迁移提醒邮件
- 🔒 最终仅支持 JWT（API Key 仅用于交换）

---

## 相关文档

- [TOKEN_GUIDE.md](./TOKEN_GUIDE.md) - 完整使用指南
- [API_REFERENCE.md](./API_REFERENCE.md) - API 接口文档
- [SECURITY_ANALYSIS.md](./SECURITY_ANALYSIS.md) - 安全分析
- [PRODUCTION_DEPLOYMENT.md](./PRODUCTION_DEPLOYMENT.md) - 生产部署

---

**文档版本**: 1.0  
**最后更新**: 2026-01-13  
**维护者**: Zinnia Team
