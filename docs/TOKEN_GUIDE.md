# Zinnia 令牌使用指南

## 概述

Zinnia 采用**双令牌架构**：API Key（长期设备凭据） + JWT（短期访问令牌），兼顾安全性和易用性。

## 令牌类型对比

| 特性 | API Key（设备访问令牌） | JWT（用户访问令牌） |
|------|----------------------|------------------|
| **用途** | 设备长期身份认证 | 用户短期会话授权 |
| **有效期** | 可配置（默认永久，支持自定义） | 短期（默认 15 分钟） |
| **格式** | `zdat_xxx...` 不透明令牌 | 标准 JWT 签名令牌 |
| **存储** | 数据库哈希存储 | 无状态签名验证 |
| **撤销** | 支持实时撤销 | 黑名单 + 短期过期 |
| **权限** | read/write/all 可配置 | 基于用户角色 |
| **IP 限制** | 支持白名单 | 不限制 |
| **适用场景** | IoT 设备、后台服务 | Web/移动应用用户 |

---

## 架构原则（方案 A）

### 核心设计
- **设备**：使用 API Key 作为长期凭据，每次请求可：
  1. **直接使用 API Key**（兼容模式）
  2. **用 API Key 换取短期 JWT**（推荐）
  
- **用户**：始终使用 JWT + 刷新令牌

### 安全策略
1. **优先使用 JWT**：短期有效、泄露风险低
2. **API Key 保护**：哈希存储、IP 白名单、速率限制、可撤销
3. **最小权限**：根据场景配置 read/write/all 权限
4. **定期轮换**：建议每 90 天轮换 API Key

---

## 1. 设备令牌（API Key）

### 1.1 创建 API Key

**端点**: `POST /api/v1/devices/{device_id}/tokens`  
**认证**: JWT（用户）  
**请求体**:
```json
{
  "name": "生产环境设备 A",
  "permission": "write",
  "expires_in_hours": 8760,
  "allowed_ips": ["192.168.1.100"],
  "rate_limit_per_minute": 120
}
```

**响应**:
```json
{
  "code": 201,
  "message": "success",
  "data": {
    "id": "uuid",
    "device_id": "uuid",
    "name": "生产环境设备 A",
    "token": "zdat_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
    "token_prefix": "zdat_xxxx",
    "permission": "write",
    "expires_at": "2027-01-13T00:00:00Z",
    "created_at": "2026-01-13T15:30:00Z"
  }
}
```

> ⚠️ **重要**：`token` 字段仅在创建时返回一次，请妥善保存！

### 1.2 权限说明

| 权限 | 说明 | 适用场景 |
|------|------|---------|
| `read` | 只读：仅可查询电量数据、设备状态 | 监控面板、数据分析 |
| `write` | 只写：仅可上报电量、更新设备状态 | IoT 设备上报 |
| `all` | 全部：读写均可 | 开发测试、管理工具 |

### 1.3 使用 API Key

#### 方式 1：直接请求（兼容模式）

**HTTP 请求头**:
```
Authorization: Bearer zdat_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

**示例（电量上报）**:
```bash
curl -X POST https://api.example.com/api/v1/battery/report \
  -H "Authorization: Bearer zdat_xxx..." \
  -H "Content-Type: application/json" \
  -d '{
    "battery_level": 85,
    "is_charging": false,
    "power_saving_mode": "off"
  }'
```

#### 方式 2：换取 JWT（推荐）

**端点**: `POST /api/v1/auth/exchange`  
**请求体**:
```json
{
  "api_key": "zdat_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
}
```

**响应**:
```json
{
  "code": 200,
  "message": "success",
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "refresh_token": "xxx",
    "expires_in": 900,
    "token_type": "Bearer",
    "device_id": "uuid"
  }
}
```

之后使用 JWT 进行请求：
```bash
curl -X POST https://api.example.com/api/v1/battery/report \
  -H "Authorization: Bearer eyJhbGci..." \
  -d '{ "battery_level": 85 }'
```

**优势**:
- ✅ 短期有效（15 分钟），泄露风险低
- ✅ 无状态验证，性能更好
- ✅ 可用 refresh_token 续期，无需频繁换取

### 1.4 管理 API Key

#### 列出所有令牌
```bash
GET /api/v1/devices/{device_id}/tokens?include_revoked=false&include_expired=false
```

#### 撤销单个令牌
```bash
DELETE /api/v1/devices/{device_id}/tokens/{token_id}
```

#### 撤销所有令牌
```bash
DELETE /api/v1/devices/{device_id}/tokens
```

---

## 2. 用户令牌（JWT）

### 2.1 用户登录

**端点**: `POST /api/v1/users/login`  
**请求体**:
```json
{
  "login": "user@example.com",
  "password": "securePassword123",
  "recaptcha_token": "optional"
}
```

**响应**:
```json
{
  "code": 200,
  "message": "success",
  "data": {
    "user": {
      "id": "uuid",
      "email": "user@example.com",
      "username": "user123",
      "role": "user"
    },
    "access_token": "eyJhbGci...",
    "refresh_token": "refresh_xxx",
    "expires_in": 900
  }
}
```

### 2.2 刷新令牌

**端点**: `POST /api/v1/users/refresh`  
**请求体**:
```json
{
  "refresh_token": "refresh_xxx"
}
```

### 2.3 登出

**端点**: `POST /api/v1/users/logout`  
**认证**: JWT  
**说明**: 将当前 access_token 加入黑名单

---

## 3. WebSocket 认证

WebSocket 连接支持两种认证方式：

### 方式 1：连接时携带令牌
```javascript
// API Key
const ws = new WebSocket('wss://api.example.com/ws?token=zdat_xxx...');

// JWT
const ws = new WebSocket('wss://api.example.com/ws?token=eyJhbGci...&auth_type=jwt');
```

### 方式 2：连接后发送认证消息
```javascript
const ws = new WebSocket('wss://api.example.com/ws');

ws.onopen = () => {
  // API Key
  ws.send(JSON.stringify({
    type: 'auth',
    token: 'zdat_xxx...',
    auth_type: 'device_token'
  }));
  
  // 或 JWT
  ws.send(JSON.stringify({
    type: 'auth',
    token: 'eyJhbGci...',
    auth_type: 'jwt'
  }));
};
```

**认证超时**: 连接建立后 30 秒内必须完成认证

---

## 4. 安全最佳实践

### 4.1 设备端（API Key）

✅ **推荐做法**:
- 使用 API Key → JWT 交换机制
- 配置 IP 白名单限制
- 设置合理的过期时间（如 1 年）
- 定期轮换 API Key
- 为不同环境使用不同 API Key

❌ **避免做法**:
- 在客户端代码中硬编码 API Key
- 使用永久有效的 API Key
- 同一个 API Key 在多个设备共享
- 在日志中明文记录完整 API Key

### 4.2 用户端（JWT）

✅ **推荐做法**:
- 将 access_token 存储在内存中
- refresh_token 存储在 httpOnly Cookie 或安全存储
- 定期刷新 access_token
- 登出时主动撤销令牌

❌ **避免做法**:
- 将 JWT 存储在 localStorage（XSS 风险）
- 使用超长有效期的 JWT
- 在 URL 中传递 JWT

### 4.3 API Key 管理策略

#### 开发环境
- 有效期: 30 天
- 权限: all
- IP 限制: 开发者 IP

#### 测试环境
- 有效期: 90 天
- 权限: all
- IP 限制: 测试环境 IP 段

#### 生产环境
- 有效期: 365 天
- 权限: write（设备仅上报）
- IP 限制: 生产环境固定 IP
- 速率限制: 120 次/分钟

---

## 5. 错误处理

### 常见错误码

| 错误码 | 说明 | 解决方案 |
|--------|------|----------|
| 401 Unauthorized | 令牌无效或已过期 | 重新认证或刷新令牌 |
| 403 Forbidden | IP 不在白名单 | 检查 IP 配置或联系管理员 |
| 403 Forbidden | 权限不足 | 使用具有足够权限的令牌 |
| 429 Too Many Requests | 超过速率限制 | 降低请求频率或增加限额 |

### 错误响应示例
```json
{
  "code": 401,
  "message": "令牌无效或已过期",
  "error": {
    "type": "UNAUTHORIZED",
    "details": "Token has expired"
  }
}
```

---

## 6. 迁移指南

### 从纯 API Key 迁移到 JWT

**阶段 1: 双模式支持（当前）**
- API Key 和 JWT 均可使用
- 鼓励新设备使用 JWT 模式

**阶段 2: JWT 优先（30 天后）**
- API 文档标注 API Key 直接使用为"不推荐"
- 速率限制：JWT 更宽松

**阶段 3: 强制 JWT（90 天后，可选）**
- API Key 仅用于换取 JWT
- 直接使用 API Key 返回警告

### 更新代码示例

**旧方式（仍支持）**:
```python
import requests

API_KEY = "zdat_xxx..."
headers = {"Authorization": f"Bearer {API_KEY}"}

requests.post(
    "https://api.example.com/api/v1/battery/report",
    headers=headers,
    json={"battery_level": 85}
)
```

**新方式（推荐）**:
```python
import requests

API_KEY = "zdat_xxx..."

# 1. 换取 JWT
auth_resp = requests.post(
    "https://api.example.com/api/v1/auth/exchange",
    json={"api_key": API_KEY}
).json()

access_token = auth_resp["data"]["access_token"]
refresh_token = auth_resp["data"]["refresh_token"]

# 2. 使用 JWT
headers = {"Authorization": f"Bearer {access_token}"}
requests.post(
    "https://api.example.com/api/v1/battery/report",
    headers=headers,
    json={"battery_level": 85}
)

# 3. 定期刷新（可选，建议在过期前）
refresh_resp = requests.post(
    "https://api.example.com/api/v1/auth/refresh",
    json={"refresh_token": refresh_token}
).json()

access_token = refresh_resp["data"]["access_token"]
```

---

## 7. SDK 示例

### Python SDK
```python
from zinnia import Client

# 使用 API Key（自动换取 JWT）
client = Client(api_key="zdat_xxx...")

# 上报电量
client.battery.report(
    battery_level=85,
    is_charging=False,
    power_saving_mode="off"
)

# SDK 自动处理 JWT 刷新
```

### Node.js SDK
```javascript
const { ZinniaClient } = require('@zinnia/sdk');

const client = new ZinniaClient({
  apiKey: 'zdat_xxx...',
  autoRefresh: true  // 自动刷新 JWT
});

await client.battery.report({
  batteryLevel: 85,
  isCharging: false
});
```

---

## 8. FAQ

### Q: API Key 和 JWT 哪个更安全？
A: JWT 更安全。API Key 是长期凭据，一旦泄露风险较大；JWT 短期有效（15 分钟），泄露窗口小。推荐使用 API Key → JWT 交换模式。

### Q: 为什么不完全弃用 API Key？
A: API Key 适合资源受限的 IoT 设备，无需频繁换取和缓存 JWT。保留直接使用模式提供了向后兼容性。

### Q: JWT 过期后怎么办？
A: 使用 refresh_token 刷新（`POST /api/v1/auth/refresh`）或重新用 API Key 换取。

### Q: 可以用一个 API Key 给多个设备吗？
A: 不建议。每个设备应有独立的 API Key，便于撤销和审计。

### Q: API Key 被泄露了怎么办？
A: 立即在管理后台撤销该 API Key，并创建新的。已撤销的令牌会立即失效。

### Q: WebSocket 断线重连需要重新认证吗？
A: 是的。每次新建 WebSocket 连接都需要认证。建议在客户端缓存 JWT 用于重连。

---

## 9. 监控与审计

### 令牌使用监控
```bash
# 查看令牌使用记录
GET /api/v1/devices/{device_id}/tokens
```

**响应示例**:
```json
{
  "code": 200,
  "data": [
    {
      "id": "uuid",
      "name": "生产设备 A",
      "token_prefix": "zdat_xxxx",
      "permission": "write",
      "last_used_at": "2026-01-13T15:45:00Z",
      "use_count": 15234,
      "expires_at": "2027-01-13T00:00:00Z",
      "is_revoked": false
    }
  ]
}
```

### 审计日志
所有令牌操作（创建、撤销、验证失败）会记录在审计日志中，可通过管理后台查看。

---

## 10. 相关文档

- [API 参考文档](./API_REFERENCE.md)
- [安全分析](./SECURITY_ANALYSIS.md)
- [生产部署指南](./PRODUCTION_DEPLOYMENT.md)
- [WebSocket 协议](./API_REFERENCE.md#websocket-api)

---

## 变更历史

- **2026-01-13**: 初始版本，实现方案 A（API Key + JWT 共存）
