# Zinnia API 参考文档

> **设备电量监控与预警系统 - RESTful API 接口规范**
> 
> 版本：v1.0 | 基础路径：`/api/v1`

---

## 目录

1. [通用规范](#通用规范)
2. [认证机制](#认证机制)
3. [用户接口](#用户接口)
4. [设备接口](#设备接口)
5. [电量数据接口](#电量数据接口)
6. [预警接口](#预警接口)
7. [健康检查接口](#健康检查接口)
8. [WebSocket 接口](#websocket-接口)
9. [错误码参考](#错误码参考)

---

## 通用规范

### 请求格式

- **Content-Type**: `application/json`
- **字符编码**: UTF-8
- **时间格式**: ISO 8601（如 `2026-01-12T10:30:00Z`）

### 响应结构

所有 API 响应遵循统一格式：

```json
{
  "code": 200,
  "message": "success",
  "data": { ... },
  "timestamp": "2026-01-12T10:30:00.000Z",
  "request_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `code` | number | HTTP 状态码 |
| `message` | string | 响应消息 |
| `data` | object/array/null | 响应数据（成功时） |
| `timestamp` | string | 服务器时间戳 |
| `request_id` | string | 请求追踪 ID（可选） |

### 分页响应

列表类接口返回分页结构：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "items": [ ... ],
    "pagination": {
      "page": 1,
      "page_size": 20,
      "total_items": 150,
      "total_pages": 8
    }
  }
}
```

### 通用查询参数

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `page` | number | 1 | 页码（从1开始） |
| `page_size` | number | 20 | 每页数量（1-100） |

---

## 认证机制

Zinnia 采用**双令牌架构**：API Key（设备长期凭据）+ JWT（短期访问令牌）。

### 用户认证（JWT）

用户通过登录获取 JWT 令牌，在请求头中携带：

```
Authorization: Bearer <access_token>
```

**令牌类型**：
- `access_token`: 访问令牌，有效期 15 分钟
- `refresh_token`: 刷新令牌，有效期 7 天

### 设备认证（API Key 与 JWT）

设备有两种认证方式：

#### 方式 1：直接使用 API Key（兼容模式）

在请求头中携带 API Key：

```
Authorization: Bearer zdat_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

> ⚠️ API Key 仅在创建时返回一次，请妥善保管。

#### 方式 2：API Key 换取 JWT（推荐）

更安全的方式是用 API Key 换取短期 JWT：

**端点**: `POST /api/v1/auth/exchange`

**请求体**:
```json
{
  "api_key": "zdat_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
}
```

**成功响应** (200 OK):
```json
{
  "code": 200,
  "message": "success",
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIs...",
    "refresh_token": "dGhpcyBpcyBhIHJlZnJlc2g...",
    "expires_in": 900,
    "token_type": "Bearer",
    "device_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

之后使用 JWT 进行后续请求：

```
Authorization: Bearer eyJhbGciOiJIUzI1NiIs...
```

**优势**：
- ✅ 短期有效（15 分钟），泄露风险低
- ✅ 可用 refresh_token 续期
- ✅ 无状态验证，性能更好

> 📘 **详细指南**: 参见 [TOKEN_GUIDE.md](./TOKEN_GUIDE.md)

### 角色权限

| 角色 | 权限 |
|------|------|
| `admin` | 完全管理权限 |
| `user` | 管理自己的设备和数据 |
| `readonly` | 只读访问 |
| `device` | 设备级别（仅能操作自身数据） |

---

## 用户接口

### 用户注册

创建新用户账户。

```
POST /api/v1/users/register
```

**请求体**：

```json
{
  "email": "user@example.com",
  "username": "johndoe",
  "password": "SecurePass123!",
  "confirm_password": "SecurePass123!"
}
```

| 字段 | 类型 | 必填 | 验证规则 |
|------|------|------|----------|
| `email` | string | ✅ | 有效邮箱格式 |
| `username` | string | ✅ | 3-50字符，仅字母/数字/下划线 |
| `password` | string | ✅ | 8-128字符 |
| `confirm_password` | string | ✅ | 必须与 password 一致 |

**成功响应** (201 Created)：

```json
{
  "code": 201,
  "message": "created",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "user@example.com",
    "username": "johndoe",
    "role": "user",
    "email_verified": false,
    "created_at": "2026-01-12T10:30:00Z",
    "last_login_at": null
  }
}
```

---

### 用户登录

使用邮箱/用户名和密码登录。

```
POST /api/v1/users/login
```

**请求体**：

```json
{
  "login": "user@example.com",
  "password": "SecurePass123!",
  "device_info": "Chrome on macOS"
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `login` | string | ✅ | 邮箱或用户名 |
| `password` | string | ✅ | 密码 |
| `device_info` | string | ❌ | 客户端设备信息 |

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "user": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "email": "user@example.com",
      "username": "johndoe",
      "role": "user",
      "email_verified": false,
      "created_at": "2026-01-12T10:30:00Z",
      "last_login_at": "2026-01-12T10:30:00Z"
    },
    "access_token": "eyJhbGciOiJIUzI1NiIs...",
    "refresh_token": "dGhpcyBpcyBhIHJlZnJlc2g...",
    "token_type": "Bearer",
    "expires_in": 900
  }
}
```

---

### 刷新令牌

使用 refresh_token 获取新的 access_token。

```
POST /api/v1/users/refresh
```

**请求体**：

```json
{
  "refresh_token": "dGhpcyBpcyBhIHJlZnJlc2g..."
}
```

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "user": { ... },
    "access_token": "eyJhbGciOiJIUzI1NiIs...",
    "refresh_token": "bmV3IHJlZnJlc2ggdG9rZW4...",
    "token_type": "Bearer",
    "expires_in": 900
  }
}
```

---

### 用户登出

使当前 refresh_token 失效。

```
POST /api/v1/users/logout
```

**认证**：需要有效的 `access_token`

**请求体**：

```json
{
  "refresh_token": "dGhpcyBpcyBhIHJlZnJlc2g..."
}
```

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "已登出",
  "data": null
}
```

---

### 获取当前用户

获取当前登录用户的信息。

```
GET /api/v1/users/me
```

**认证**：需要有效的 `access_token`

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "user@example.com",
    "username": "johndoe",
    "role": "user",
    "email_verified": false,
    "created_at": "2026-01-12T10:30:00Z",
    "last_login_at": "2026-01-12T10:30:00Z"
  }
}
```

---

### 更新当前用户

更新当前用户的个人信息。

```
PUT /api/v1/users/me
```

**认证**：需要有效的 `access_token`

**请求体**：

```json
{
  "username": "newusername",
  "metadata": { "theme": "dark" }
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `username` | string | ❌ | 新用户名（3-50字符） |
| `metadata` | object | ❌ | 自定义元数据 |

---

### 修改密码

修改当前用户的密码。

```
PUT /api/v1/users/me/password
```

**认证**：需要有效的 `access_token`

**请求体**：

```json
{
  "current_password": "OldPass123!",
  "new_password": "NewSecure456!",
  "confirm_password": "NewSecure456!"
}
```

---

### 登出所有设备

使所有 refresh_token 失效（除当前会话）。

```
POST /api/v1/users/logout-all
```

**认证**：需要有效的 `access_token`

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "message": "已登出所有设备",
    "sessions_revoked": 3
  }
}
```

---

### 共享设备给用户

将设备共享给其他用户。

```
POST /api/v1/users/devices/{device_id}/share
```

**认证**：需要有效的 `access_token`（必须是设备所有者）

**路径参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `device_id` | UUID | 设备 ID |

**请求体**：

```json
{
  "user_identifier": "friend@example.com",
  "permission": "read"
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `user_identifier` | string | ✅ | 目标用户邮箱或用户名 |
| `permission` | string | ❌ | 权限级别（默认 `read`） |

**权限级别**：
- `read`: 只读（查看电量数据）
- `write`: 读写（可修改配置）
- `admin`: 管理（可删除、转让）

---

### 获取设备共享列表

获取设备的所有共享记录。

```
GET /api/v1/users/devices/{device_id}/shares
```

---

### 取消设备共享

取消对某用户的设备共享。

```
DELETE /api/v1/users/devices/{device_id}/share/{user_id}
```

---

### 管理员：用户列表

获取所有用户列表（仅管理员）。

```
GET /api/v1/users
```

**认证**：需要 `admin` 角色

**查询参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `page` | number | 页码 |
| `page_size` | number | 每页数量 |
| `role` | string | 按角色筛选 (`admin`/`user`/`readonly`) |
| `is_active` | boolean | 按激活状态筛选 |
| `search` | string | 搜索用户名或邮箱 |

---

### 管理员：获取用户

获取指定用户详情（仅管理员）。

```
GET /api/v1/users/{user_id}
```

---

### 管理员：更新用户

更新指定用户信息（仅管理员）。

```
PUT /api/v1/users/{user_id}
```

---

### 管理员：删除用户

删除指定用户（仅管理员）。

```
DELETE /api/v1/users/{user_id}
```

---

## 设备接口

### 创建设备

注册新设备，返回设备信息和 API Key。

```
POST /api/v1/devices
```

**认证**：可选（如携带 `access_token`，设备将绑定到该用户）

**请求体**：

```json
{
  "name": "客厅传感器",
  "device_type": "battery_sensor",
  "metadata": {
    "location": "living_room",
    "firmware": "1.2.0"
  }
}
```

| 字段 | 类型 | 必填 | 验证规则 |
|------|------|------|----------|
| `name` | string | ✅ | 1-100字符 |
| `device_type` | string | ✅ | 1-50字符 |
| `metadata` | object | ❌ | 自定义元数据 |

**成功响应** (201 Created)：

```json
{
  "code": 201,
  "message": "created",
  "data": {
    "device": {
      "id": "660e8400-e29b-41d4-a716-446655440000",
      "owner_id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "客厅传感器",
      "device_type": "battery_sensor",
      "status": "offline",
      "api_key_prefix": "zin_live_abc123",
      "created_at": "2026-01-12T10:30:00Z",
      "updated_at": "2026-01-12T10:30:00Z",
      "last_seen_at": null,
      "metadata": { "location": "living_room" }
    },
    "api_key": "zin_live_abc123def456ghi789jkl012mno345pqr678",
    "config": {
      "device_id": "660e8400-e29b-41d4-a716-446655440000",
      "low_battery_threshold": 20,
      "critical_battery_threshold": 10,
      "report_interval_seconds": 60,
      "high_temperature_threshold": 45.0,
      "updated_at": "2026-01-12T10:30:00Z"
    }
  }
}
```

> ⚠️ **重要**：`api_key` 仅在创建时返回一次，请立即保存！

---

### 获取设备列表

获取当前用户有权访问的设备列表。

```
GET /api/v1/devices
```

**认证**：需要有效的 `access_token`

**查询参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `page` | number | 页码 |
| `page_size` | number | 每页数量（1-100） |
| `status` | string | 按状态筛选 |
| `device_type` | string | 按类型筛选 |

**设备状态**：
- `online`: 在线
- `offline`: 离线
- `maintenance`: 维护中
- `disabled`: 已禁用

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "items": [
      {
        "id": "660e8400-e29b-41d4-a716-446655440000",
        "owner_id": "550e8400-e29b-41d4-a716-446655440000",
        "name": "客厅传感器",
        "device_type": "battery_sensor",
        "status": "online",
        "api_key_prefix": "zin_live_abc123",
        "created_at": "2026-01-12T10:30:00Z",
        "updated_at": "2026-01-12T10:30:00Z",
        "last_seen_at": "2026-01-12T11:00:00Z"
      }
    ],
    "pagination": {
      "page": 1,
      "page_size": 20,
      "total_items": 5,
      "total_pages": 1
    }
  }
}
```

---

### 获取设备详情

获取指定设备的详细信息。

```
GET /api/v1/devices/{id}
```

**认证**：需要有效的 `access_token`（必须有权访问该设备）

---

### 更新设备

更新设备基本信息。

```
PUT /api/v1/devices/{id}
```

**认证**：需要有效的 `access_token`（必须有写权限）

**请求体**：

```json
{
  "name": "更新后的名称",
  "status": "maintenance",
  "metadata": { "note": "维护中" }
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | ❌ | 设备名称 |
| `status` | string | ❌ | 设备状态 |
| `metadata` | object | ❌ | 自定义元数据 |

---

### 删除设备

删除指定设备及其所有数据。

```
DELETE /api/v1/devices/{id}
```

**认证**：需要有效的 `access_token`（必须是设备所有者）

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "设备已删除",
  "data": null
}
```

---

### 获取设备配置

获取设备的阈值和上报配置。

```
GET /api/v1/devices/{id}/config
```

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "device_id": "660e8400-e29b-41d4-a716-446655440000",
    "low_battery_threshold": 20,
    "critical_battery_threshold": 10,
    "report_interval_seconds": 60,
    "high_temperature_threshold": 45.0,
    "updated_at": "2026-01-12T10:30:00Z"
  }
}
```

---

### 更新设备配置

更新设备的阈值和上报配置。

```
PUT /api/v1/devices/{id}/config
```

**请求体**：

```json
{
  "low_battery_threshold": 25,
  "critical_battery_threshold": 10,
  "report_interval_seconds": 120,
  "high_temperature_threshold": 45.0
}
```

| 字段 | 类型 | 必填 | 验证规则 |
|------|------|------|----------|
| `low_battery_threshold` | number | ❌ | 1-100 |
| `critical_battery_threshold` | number | ❌ | 1-100 |
| `report_interval_seconds` | number | ❌ | 10-3600 秒 |


新增字段：
| `high_temperature_threshold` | number | ❌ | -40.0 - 200.0（摄氏度） |

说明：
- 设备配置中的 `high_temperature_threshold` 用于判断设备的高温预警触发；系统在判断和触发所有预警时优先使用设备配置的阈值（设备优先）。
- `alert_rules` 表中只定义预警规则的级别（level）和冷却时间（cooldown_minutes），触发阈值由设备配置决定。

### 数据隔离与权限

**用户隔离**：
- ✅ **设备**：每个设备有 `owner_id`，用户只能管理自己的设备或被共享的设备
- ✅ **预警规则**：每个用户拥有独立的预警规则集（`alert_rules.user_id`），互不干扰
- ✅ **预警事件**：通过 `device_id` 关联到设备，用户只能查看自己设备的预警事件

**权限控制**：
- 所有预警规则 API 必须使用用户 JWT 认证（不支持设备 API Key）
- 用户只能创建、修改、删除自己的预警规则
- 用户只能查看、确认、解决自己设备的预警事件
- 设备数据上报时会自动触发其所有者的预警规则

---

### 轮换设备 API Key

生成新的 API Key，旧 Key 立即失效。

```
POST /api/v1/devices/{id}/rotate-key
```

**认证**：需要有效的 `access_token`（必须是设备所有者）

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "api_key": "zin_live_new123key456here789...",
    "api_key_prefix": "zin_live_new123"
  }
}
```

> ⚠️ **重要**：新 `api_key` 仅返回一次，请立即更新设备配置！

### 创建设备访问令牌

为设备创建一个具有自定义有效期和权限的访问令牌，用于兼容模式 API。

```
POST /api/v1/devices/{id}/tokens
```

**认证**：需要有效的 `access_token`（必须是设备所有者）

**请求体**：

```json
{
  "name": "IoT 传感器令牌",
  "permission": "write",
  "expires_in_hours": 720,
  "allowed_ips": ["192.168.1.0/24", "10.0.0.100"],
  "rate_limit_per_minute": 60
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | ✅ | 令牌名称（1-100字符） |
| `permission` | string | ✅ | 权限级别：`read`/`write`/`all` |
| `expires_in_hours` | number | ❌ | 有效期（1-8760小时），为空表示永久有效 |
| `allowed_ips` | string[] | ❌ | IP 白名单（支持 CIDR） |
| `rate_limit_per_minute` | number | ❌ | 每分钟请求限制（1-1000） |

**权限说明**：
- `read`: 仅允许读取数据（获取电量）
- `write`: 仅允许写入数据（上报电量）
- `all`: 允许读取和写入

**成功响应** (201 Created)：

```json
{
  "code": 201,
  "message": "created",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440001",
    "device_id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "IoT 传感器令牌",
    "token": "zn_dat_AbCdEfGh123456789...",
    "token_prefix": "zn_dat_AbCdEfGh1234...",
    "permission": "write",
    "expires_at": "2026-02-11T10:30:00Z",
    "created_at": "2026-01-12T10:30:00Z"
  }
}
```

> ⚠️ **重要**：完整的 `token` 仅在创建时返回一次，请妥善保存！

### 列出设备访问令牌

获取设备的所有访问令牌列表。

```
GET /api/v1/devices/{id}/tokens
```

**认证**：需要有效的 `access_token`（必须是设备所有者）

**查询参数**：

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `include_revoked` | boolean | false | 是否包含已吊销的令牌 |
| `include_expired` | boolean | false | 是否包含已过期的令牌 |

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440001",
      "device_id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "IoT 传感器令牌",
      "token_prefix": "zn_dat_AbCdEfGh1234...",
      "permission": "write",
      "is_revoked": false,
      "expires_at": "2026-02-11T10:30:00Z",
      "last_used_at": "2026-01-12T11:00:00Z",
      "use_count": 42,
      "created_at": "2026-01-12T10:30:00Z"
    }
  ]
}
```

### 吊销单个令牌

吊销指定的设备访问令牌。

```
DELETE /api/v1/devices/{device_id}/tokens/{token_id}
```

**认证**：需要有效的 `access_token`（必须是设备所有者）

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "令牌已吊销",
  "data": null
}
```

### 吊销设备所有令牌

吊销设备的所有访问令牌。

```
DELETE /api/v1/devices/{id}/tokens
```

**认证**：需要有效的 `access_token`（必须是设备所有者）

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "revoked_count": 5,
    "message": "已吊销 5 个令牌"
  }
}
```

---

## 兼容模式接口

> 兼容模式 API 专为不支持设置 HTTP 请求头的设备设计（如某些 IoT 传感器、低功耗设备）。
> 所有认证信息和数据都通过 URL 查询参数传递。

### 兼容模式 - 上报电量

设备通过 URL 参数上报电量数据。

```
GET /api/v1/compat/battery/report
POST /api/v1/compat/battery/report
```

**认证**：URL 参数 `token`（需要 `write` 或 `all` 权限）

**查询参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `token` | string | ✅ | 设备访问令牌 |
| `level` | number | ✅ | 电量百分比（0-100） |
| `charging` | number | ❌ | 充电状态：0=否, 1=是 |
| `temp` | number | ❌ | 温度（摄氏度） |
| `voltage` | number | ❌ | 电压（伏特） |
| `ts` | number | ❌ | Unix 时间戳（秒） |

**示例请求**：

```
GET /api/v1/compat/battery/report?token=zn_dat_xxx&level=75&charging=1&temp=28.5
```

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "id": "...",
    "device_id": "...",
    "battery_level": 75,
    "is_charging": true,
    "recorded_at": "2026-01-12T10:30:00Z"
  }
}
```

### 兼容模式 - 极简上报

极简参数上报，适用于带宽/资源极其受限的设备。

```
GET /api/v1/compat/battery/simple
```

**认证**：URL 参数 `token`（需要 `write` 或 `all` 权限）

**查询参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `token` | string | ✅ | 设备访问令牌 |
| `l` | number | ✅ | 电量百分比（0-100） |
| `c` | number | ❌ | 充电状态：0=否, 1=是 |

**示例请求**：

```
GET /api/v1/compat/battery/simple?token=zn_dat_xxx&l=75&c=1
```

**成功响应** (200 OK)：

```json
{
  "ok": true,
  "ts": 1736677800
}
```

### 兼容模式 - 获取最新电量

获取设备最新电量数据。

```
GET /api/v1/compat/battery/latest
```

**认证**：URL 参数 `token`（需要 `read` 或 `all` 权限）

**查询参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `token` | string | ✅ | 设备访问令牌 |

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "level": 75,
    "charging": true,
    "timestamp": 1736677800
  }
}
```

### 兼容模式 - 健康检查

验证令牌是否有效并获取基本信息。

```
GET /api/v1/compat/ping
```

**认证**：URL 参数 `token`

**查询参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `token` | string | ✅ | 设备访问令牌 |

**成功响应** (200 OK)：

```json
{
  "ok": true,
  "device_id": "550e8400-e29b-41d4-a716-446655440000",
  "permission": "write",
  "expires_at": "2026-02-11T10:30:00Z"
}
```

---

## 电量数据接口

### 上报电量（设备端）

设备上报当前电量数据。

```
POST /api/v1/battery/report
```

**认证**：需要设备 `X-API-Key`

**请求体**：

```json
{
  "battery_level": 75,
  "is_charging": false,
  "power_saving_mode": "off",
  "temperature": 28.5,
  "voltage": 3.85,
  "recorded_at": "2026-01-12T10:30:00Z"
}
```

| 字段 | 类型 | 必填 | 验证规则 |
|------|------|------|----------|
| `battery_level` | number | ✅ | 0-100 整数 |
| `is_charging` | boolean | ❌ | 默认 `false` |
| `power_saving_mode` | string | ❌ | 见下方枚举 |
| `temperature` | number | ❌ | -40 到 85 摄氏度 |
| `voltage` | number | ❌ | 0-10V |
| `recorded_at` | string | ❌ | ISO 8601 时间戳（默认使用服务器时间） |

**省电模式枚举**：
- `off`: 关闭
- `low`: 低
- `medium`: 中
- `high`: 高
- `extreme`: 极限

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "id": "770e8400-e29b-41d4-a716-446655440000",
    "device_id": "660e8400-e29b-41d4-a716-446655440000",
    "battery_level": 75,
    "is_charging": false,
    "power_saving_mode": "off",
    "temperature": 28.5,
    "voltage": 3.85,
    "recorded_at": "2026-01-12T10:30:00Z",
    "created_at": "2026-01-12T10:30:01Z"
  }
}
```

---

### 批量上报电量（设备端）

设备批量上报历史电量数据。

```
POST /api/v1/battery/batch-report
```

**认证**：需要设备 `X-API-Key`

**请求体**：

```json
{
  "data": [
    {
      "battery_level": 80,
      "is_charging": true,
      "recorded_at": "2026-01-12T10:00:00Z"
    },
    {
      "battery_level": 75,
      "is_charging": false,
      "recorded_at": "2026-01-12T10:30:00Z"
    }
  ]
}
```

| 字段 | 类型 | 必填 | 验证规则 |
|------|------|------|----------|
| `data` | array | ✅ | 1-1000 条记录 |

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "inserted_count": 2
  }
}
```

---

### 获取最新电量

获取设备的最新电量数据。

```
GET /api/v1/battery/latest/{device_id}
```

**认证**：需要有效的 `access_token` 或设备 `X-API-Key`

**路径参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `device_id` | UUID | 设备 ID |

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "device_id": "660e8400-e29b-41d4-a716-446655440000",
    "battery_level": 75,
    "is_charging": false,
    "power_saving_mode": "off",
    "recorded_at": "2026-01-12T10:30:00Z",
    "is_low_battery": false,
    "is_critical": false
  }
}
```

---

### 查询历史数据

查询设备的历史电量数据。

```
GET /api/v1/battery/history/{device_id}
```

**认证**：需要有效的 `access_token`

**查询参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `start_time` | string | ✅ | 开始时间（ISO 8601） |
| `end_time` | string | ✅ | 结束时间（ISO 8601） |
| `limit` | number | ❌ | 返回条数（默认100，最大1000） |
| `offset` | number | ❌ | 偏移量（默认0） |

> ⚠️ 查询时间范围不能超过 30 天

**示例请求**：

```
GET /api/v1/battery/history/660e8400-e29b-41d4-a716-446655440000?start_time=2026-01-11T00:00:00Z&end_time=2026-01-12T00:00:00Z&limit=50
```

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": [
    {
      "id": "770e8400-e29b-41d4-a716-446655440001",
      "device_id": "660e8400-e29b-41d4-a716-446655440000",
      "battery_level": 80,
      "is_charging": true,
      "power_saving_mode": "off",
      "temperature": 27.5,
      "voltage": 4.2,
      "recorded_at": "2026-01-11T10:00:00Z",
      "created_at": "2026-01-11T10:00:01Z"
    },
    ...
  ]
}
```

---

### 获取聚合统计

获取按时间聚合的电量统计数据（用于图表展示）。

```
GET /api/v1/battery/aggregated/{device_id}
```

**认证**：需要有效的 `access_token`

**查询参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `start_time` | string | ✅ | 开始时间 |
| `end_time` | string | ✅ | 结束时间 |
| `interval` | string | ❌ | 聚合间隔（默认 `hour`） |

**聚合间隔**：
- `minute`: 按分钟聚合
- `hour`: 按小时聚合
- `day`: 按天聚合

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": [
    {
      "bucket": "2026-01-12T10:00:00Z",
      "avg_level": 72.5,
      "min_level": 68,
      "max_level": 78,
      "count": 6
    },
    {
      "bucket": "2026-01-12T11:00:00Z",
      "avg_level": 65.2,
      "min_level": 60,
      "max_level": 70,
      "count": 6
    }
  ]
}
```

---

### 获取统计摘要

获取时间段内的电量统计摘要。

```
GET /api/v1/battery/stats/{device_id}
```

**认证**：需要有效的 `access_token`

**查询参数**：

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `start_time` | string | ✅ | 开始时间 |
| `end_time` | string | ✅ | 结束时间 |

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "device_id": "660e8400-e29b-41d4-a716-446655440000",
    "period_start": "2026-01-11T00:00:00Z",
    "period_end": "2026-01-12T00:00:00Z",
    "avg_battery_level": 68.5,
    "min_battery_level": 45,
    "max_battery_level": 100,
    "total_records": 1440,
    "charging_duration_minutes": 180,
    "low_battery_count": 5
  }
}
```

---

## 预警接口

### 创建预警规则

创建用户专属的预警规则。每个用户可以独立配置自己的预警规则。

```
POST /api/v1/alerts/rules
```

**认证**：需要有效的 `access_token`（用户认证）

**请求体**：

```json
{
  "name": "低电量预警",
  "alert_type": "low_battery",
  "level": "warning",
  "cooldown_minutes": 30,
  "enabled": true
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | ✅ | 规则名称（1-100字符） |
| `alert_type` | string | ✅ | 预警类型 |
| `level` | string | ✅ | 预警级别 |
| `cooldown_minutes` | number | ❌ | 冷却时间（默认30，范围1-1440分钟） |
| `enabled` | boolean | ❌ | 是否启用（默认 `true`） |

**预警类型**：
- `low_battery`: 低电量
- `critical_battery`: 临界电量
- `high_temperature`: 高温
- `device_offline`: 设备离线
- `rapid_drain`: 电量快速下降

**预警级别**：
- `info`: 信息
- `warning`: 警告
- `critical`: 严重

**成功响应** (201 Created)：

```json
{
  "code": 201,
  "message": "created",
  "data": {
    "id": "880e8400-e29b-41d4-a716-446655440000",
    "user_id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "低电量预警",
    "alert_type": "low_battery",
    "level": "warning",
    "cooldown_minutes": 30,
    "enabled": true,
    "created_at": "2026-01-12T10:30:00Z",
    "updated_at": "2026-01-12T10:30:00Z"
  }
}
```

**说明**：
- 每个用户拥有独立的预警规则集，互不干扰
- 每个用户的每种预警类型只能有一个启用的规则
- 触发阈值由设备配置决定（`device_configs` 表），规则只定义预警级别和冷却时间

---

### 获取预警规则列表

获取当前用户的所有预警规则。

```
GET /api/v1/alerts/rules
```

**认证**：需要有效的 `access_token`（用户认证）

---

### 更新预警规则

更新当前用户的预警规则配置。

```
PUT /api/v1/alerts/rules/{id}
```

**认证**：需要有效的 `access_token`（用户认证，只能更新自己的规则）

**路径参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `id` | UUID | 预警规则 ID |

**请求体**（所有字段可选）：

```json
{
  "name": "更新后的规则名称",
  "alert_type": "critical_battery",
  "level": "critical",
  "cooldown_minutes": 60,
  "enabled": false
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | ❌ | 规则名称（1-100字符） |
| `alert_type` | string | ❌ | 预警类型 |
| `level` | string | ❌ | 预警级别 |
| `cooldown_minutes` | number | ❌ | 冷却时间（1-1440分钟） |
| `enabled` | boolean | ❌ | 是否启用 |

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "id": "880e8400-e29b-41d4-a716-446655440000",
    "user_id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "更新后的规则名称",
    "alert_type": "critical_battery",
    "level": "critical",
    "cooldown_minutes": 60,
    "enabled": false,
    "created_at": "2026-01-12T10:30:00Z",
    "updated_at": "2026-01-13T14:20:00Z"
  }
}
```

**错误响应**：

| 状态码 | 错误码 | 说明 |
|--------|--------|------|
| 404 | NOT_FOUND | 规则不存在 |
| 422 | VALIDATION_ERROR | 验证失败 |

---

### 删除预警规则

删除当前用户的预警规则。

```
DELETE /api/v1/alerts/rules/{id}
```

**认证**：需要有效的 `access_token`（用户认证，只能删除自己的规则）

**路径参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `id` | UUID | 预警规则 ID |

**成功响应** (204 No Content)：

无响应体

**错误响应**：

| 状态码 | 错误码 | 说明 |
|--------|--------|------|
| 404 | NOT_FOUND | 规则不存在 |

---

### 获取预警事件列表

获取当前用户设备的预警事件记录。只能查看自己拥有或被共享的设备的预警。

```
GET /api/v1/alerts/events
```

**认证**：需要有效的 `access_token`（用户认证）

**查询参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `device_id` | UUID | 按设备筛选 |
| `level` | string | 按级别筛选 (`info`/`warning`/`critical`) |
| `status` | string | 按状态筛选 |
| `alert_type` | string | 按类型筛选 |
| `page` | number | 页码 |
| `page_size` | number | 每页数量 |

**预警状态**：
- `active`: 活跃（未处理）
- `acknowledged`: 已确认
- `resolved`: 已解决

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "items": [
      {
        "id": "990e8400-e29b-41d4-a716-446655440000",
        "device_id": "660e8400-e29b-41d4-a716-446655440000",
        "rule_id": "880e8400-e29b-41d4-a716-446655440000",
        "alert_type": "low_battery",
        "level": "warning",
        "status": "active",
        "message": "设备电量低于阈值",
        "value": 18.0,
        "threshold": 20.0,
        "triggered_at": "2026-01-12T10:30:00Z",
        "acknowledged_at": null,
        "resolved_at": null
      }
    ],
    "pagination": { ... }
  }
}
```

---

### 确认预警

标记预警为已确认（仅限当前用户设备的预警）。

```
POST /api/v1/alerts/events/{id}/acknowledge
```

**认证**：需要有效的 `access_token`（用户认证）

---

### 解决预警

标记预警为已解决（仅限当前用户设备的预警）。

```
POST /api/v1/alerts/events/{id}/resolve
```

**认证**：需要有效的 `access_token`（用户认证）

---

### 更新预警状态

手动更新预警状态（仅限当前用户设备的预警）。

```
PUT /api/v1/alerts/events/{id}/status
```

**认证**：需要有效的 `access_token`（用户认证）

**请求体**：

```json
{
  "status": "resolved"
}
```

---

### 获取设备活跃预警数

获取指定设备的活跃预警数量（仅限当前用户的设备）。

```
GET /api/v1/alerts/devices/{device_id}/count
```

**认证**：需要有效的 `access_token`（用户认证，需要设备访问权限）

**成功响应** (200 OK)：

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "active_count": 2
  }
}
```

---

## 健康检查接口

### 基础健康检查

```
GET /health
```

**响应** (200 OK)：

```json
{
  "status": "ok"
}
```

---

### 详细健康检查

```
GET /health/detailed
```

**响应** (200 OK)：

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "database": {
    "status": "healthy",
    "latency_ms": 5
  },
  "redis": {
    "status": "healthy",
    "latency_ms": 2
  },
  "uptime_seconds": 86400
}
```

---

### 就绪检查

```
GET /health/ready
```

用于 Kubernetes 就绪探针。

---

### 存活检查

```
GET /health/live
```

用于 Kubernetes 存活探针。

---

## 错误码参考

### HTTP 状态码

| 状态码 | 说明 | 场景 |
|--------|------|------|
| 200 | OK | 请求成功 |
| 201 | Created | 资源创建成功 |
| 204 | No Content | 删除成功 |
| 400 | Bad Request | 请求参数无效 |
| 401 | Unauthorized | 未认证或令牌无效 |
| 403 | Forbidden | 无权限访问 |
| 404 | Not Found | 资源不存在 |
| 409 | Conflict | 资源冲突（如邮箱已存在） |
| 429 | Too Many Requests | 请求频率过高 |
| 500 | Internal Server Error | 服务器内部错误 |

### 错误响应格式

```json
{
  "code": 401,
  "message": "认证失败：令牌已过期",
  "request_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

### 常见错误消息

| 错误消息 | 说明 |
|----------|------|
| `认证失败` | 未携带令牌或令牌无效 |
| `令牌已过期` | access_token 已过期，需刷新 |
| `令牌已被吊销` | 令牌已被手动撤销 |
| `权限不足` | 当前角色无权执行此操作 |
| `无权访问此设备的数据` | 用户未绑定或未被共享该设备 |
| `资源不存在` | 请求的设备/用户/规则不存在 |
| `邮箱已被注册` | 注册时邮箱冲突 |
| `用户名已被占用` | 注册时用户名冲突 |
| `密码错误` | 登录或修改密码时验证失败 |
| `账户已锁定` | 登录失败次数过多 |
| `请求参数无效` | 验证失败（具体字段在 message 中） |

---

## 使用示例

### 完整登录流程

```javascript
// 1. 用户登录
const loginResponse = await fetch('/api/v1/users/login', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    login: 'user@example.com',
    password: 'SecurePass123!'
  })
});
const { data } = await loginResponse.json();
const { access_token, refresh_token } = data;

// 2. 使用 access_token 获取设备列表
const devicesResponse = await fetch('/api/v1/devices', {
  headers: { 'Authorization': `Bearer ${access_token}` }
});

// 3. 获取设备电量
const batteryResponse = await fetch(`/api/v1/battery/latest/${deviceId}`, {
  headers: { 'Authorization': `Bearer ${access_token}` }
});

// 4. 令牌过期时刷新
const refreshResponse = await fetch('/api/v1/users/refresh', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ refresh_token })
});
```

### 设备上报电量

```javascript
// 设备端使用 API Key 上报
const reportResponse = await fetch('/api/v1/battery/report', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'X-API-Key': 'zin_live_abc123def456...'
  },
  body: JSON.stringify({
    battery_level: 75,
    is_charging: false,
    temperature: 28.5
  })
});
```

---

## TypeScript 类型定义

前端开发建议使用以下类型定义：

```typescript
// 用户相关
interface User {
  id: string;
  email: string;
  username: string;
  role: 'admin' | 'user' | 'readonly';
  email_verified: boolean;
  created_at: string;
  last_login_at: string | null;
}

interface LoginRequest {
  login: string;
  password: string;
  device_info?: string;
}

interface LoginResponse {
  user: User;
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
}

// 设备相关
interface Device {
  id: string;
  owner_id: string | null;
  name: string;
  device_type: string;
  status: 'online' | 'offline' | 'maintenance' | 'disabled';
  api_key_prefix: string;
  created_at: string;
  updated_at: string;
  last_seen_at: string | null;
  metadata?: Record<string, unknown>;
}

interface DeviceConfig {
  device_id: string;
  low_battery_threshold: number;
  critical_battery_threshold: number;
  report_interval_seconds: number;
  high_temperature_threshold: number;
  updated_at: string;
}

// 电量相关
interface BatteryData {
  id: string;
  device_id: string;
  battery_level: number;
  is_charging: boolean;
  power_saving_mode: 'off' | 'low' | 'medium' | 'high' | 'extreme';
  temperature?: number;
  voltage?: number;
  recorded_at: string;
  created_at: string;
}

interface LatestBattery {
  device_id: string;
  battery_level: number;
  is_charging: boolean;
  power_saving_mode: string;
  recorded_at: string;
  is_low_battery: boolean;
  is_critical: boolean;
}

interface BatteryStats {
  device_id: string;
  period_start: string;
  period_end: string;
  avg_battery_level: number;
  min_battery_level: number;
  max_battery_level: number;
  total_records: number;
  charging_duration_minutes: number;
  low_battery_count: number;
}

// 预警相关
interface AlertRule {
  id: string;
  user_id: string;
  name: string;
  alert_type: 'low_battery' | 'critical_battery' | 'high_temperature' | 'device_offline' | 'rapid_drain';
  level: 'info' | 'warning' | 'critical';
  cooldown_minutes: number;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

interface AlertEvent {
  id: string;
  device_id: string;
  rule_id: string;
  alert_type: string;
  level: 'info' | 'warning' | 'critical';
  status: 'active' | 'acknowledged' | 'resolved';
  message: string;
  value: number;
  threshold: number;
  triggered_at: string;
  acknowledged_at: string | null;
  resolved_at: string | null;
}

// 通用
interface ApiResponse<T> {
  code: number;
  message: string;
  data: T | null;
  timestamp: string;
  request_id?: string;
}

interface Pagination {
  page: number;
  page_size: number;
  total_items: number;
  total_pages: number;
}

interface PaginatedResponse<T> {
  items: T[];
  pagination: Pagination;
}
```

---

> 📝 **文档版本**：1.1.0  
> 📅 **最后更新**：2026-01-13  
> 🔗 **后端仓库**：zinnia

---

## WebSocket 接口

### 概述

WebSocket 接口提供实时双向通信，适用于：

- **设备端**：持久连接实时上报电量数据
- **用户端**：订阅设备数据，接收实时推送

**连接端点**：
```
ws://<host>/ws
wss://<host>/ws  (TLS)
```

### 连接流程

```mermaid
sequenceDiagram
    participant C as 客户端
    participant S as 服务器
    
    C->>S: WebSocket 握手
    S->>C: Connected (auth_timeout: 30s)
    C->>S: Auth (token)
    S->>C: AuthResult (success/failed)
    
    loop 已认证
        C->>S: BatteryReport / Ping
        S->>C: BatteryReportResult / Pong
    end
```

### 认证

连接建立后，客户端需在 30 秒内发送认证消息，否则连接将被关闭。

**设备认证**：
```json
{
  "type": "auth",
  "token": "<device_access_token>",
  "auth_type": "device_token"
}
```

**用户认证（JWT）**：
```json
{
  "type": "auth",
  "token": "<jwt_access_token>",
  "auth_type": "jwt"
}
```

**认证成功响应**：
```json
{
  "type": "auth_result",
  "success": true,
  "message": "认证成功",
  "device_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**认证失败响应**：
```json
{
  "type": "auth_result",
  "success": false,
  "message": "令牌无效或已过期"
}
```

---

### 消息类型

#### 客户端消息

| 类型 | 描述 | 权限 |
|------|------|------|
| `auth` | 认证消息 | 所有 |
| `battery_report` | 上报电量数据 | 设备 |
| `batch_battery_report` | 批量上报电量 | 设备 |
| `ping` | 心跳 | 所有 |
| `subscribe` | 订阅设备数据 | 用户 |
| `unsubscribe` | 取消订阅 | 用户 |

#### 服务器消息

| 类型 | 描述 |
|------|------|
| `connected` | 连接成功 |
| `auth_result` | 认证结果 |
| `battery_report_result` | 上报结果 |
| `batch_battery_report_result` | 批量上报结果 |
| `pong` | 心跳响应 |
| `subscribe_result` | 订阅结果 |
| `battery_push` | 电量数据推送 |
| `alert_push` | 预警推送 |
| `error` | 错误消息 |

---

### 电量上报（设备）

**请求**：
```json
{
  "type": "battery_report",
  "battery_level": 75,
  "is_charging": false,
  "power_saving_mode": "off",
  "temperature": 25.5,
  "voltage": 3.8,
  "recorded_at": "2026-01-13T10:30:00Z",
  "msg_id": "req-001"
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `battery_level` | number | ✅ | 电量值 (0-100) |
| `is_charging` | boolean | ❌ | 是否充电（默认 false） |
| `power_saving_mode` | string | ❌ | 省电模式 |
| `temperature` | number | ❌ | 温度（摄氏度） |
| `voltage` | number | ❌ | 电压（伏特） |
| `recorded_at` | string | ❌ | 记录时间（默认服务器时间） |
| `msg_id` | string | ❌ | 消息 ID（用于追踪） |

**成功响应**：
```json
{
  "type": "battery_report_result",
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "device_id": "550e8400-e29b-41d4-a716-446655440001",
    "battery_level": 75,
    "is_charging": false,
    "power_saving_mode": "off",
    "temperature": 25.5,
    "voltage": 3.8,
    "recorded_at": "2026-01-13T10:30:00Z",
    "created_at": "2026-01-13T10:30:01Z"
  },
  "msg_id": "req-001"
}
```

**失败响应**：
```json
{
  "type": "battery_report_result",
  "success": false,
  "error": "电量值必须在 0-100 之间",
  "msg_id": "req-001"
}
```

---

### 批量上报（设备）

**请求**：
```json
{
  "type": "batch_battery_report",
  "data": [
    {
      "battery_level": 80,
      "is_charging": true,
      "recorded_at": "2026-01-13T10:00:00Z"
    },
    {
      "battery_level": 75,
      "is_charging": false,
      "recorded_at": "2026-01-13T10:30:00Z"
    }
  ],
  "msg_id": "batch-001"
}
```

**响应**：
```json
{
  "type": "batch_battery_report_result",
  "success": true,
  "inserted_count": 2,
  "msg_id": "batch-001"
}
```

---

### 订阅设备数据（用户）

用户可以订阅一个或多个设备的实时数据推送。

**订阅请求**：
```json
{
  "type": "subscribe",
  "device_ids": [
    "550e8400-e29b-41d4-a716-446655440000",
    "550e8400-e29b-41d4-a716-446655440001"
  ]
}
```

**订阅响应**：
```json
{
  "type": "subscribe_result",
  "success": true,
  "subscribed_devices": [
    "550e8400-e29b-41d4-a716-446655440000",
    "550e8400-e29b-41d4-a716-446655440001"
  ]
}
```

**取消订阅**：
```json
{
  "type": "unsubscribe",
  "device_ids": ["550e8400-e29b-41d4-a716-446655440000"]
}
```

---

### 数据推送

当订阅的设备上报新数据时，用户会收到推送：

```json
{
  "type": "battery_push",
  "device_id": "550e8400-e29b-41d4-a716-446655440000",
  "data": {
    "device_id": "550e8400-e29b-41d4-a716-446655440000",
    "battery_level": 75,
    "is_charging": false,
    "power_saving_mode": "off",
    "recorded_at": "2026-01-13T10:30:00Z",
    "is_low_battery": false,
    "is_critical": false
  }
}
```

### 预警推送

```json
{
  "type": "alert_push",
  "device_id": "550e8400-e29b-41d4-a716-446655440000",
  "alert_type": "low_battery",
  "message": "设备电量过低 (15%)",
  "severity": "warning",
  "timestamp": "2026-01-13T10:30:00Z"
}
```

---

### 心跳

保持连接活跃：

**请求**：
```json
{
  "type": "ping"
}
```

**响应**：
```json
{
  "type": "pong"
}
```

---

### 错误消息

```json
{
  "type": "error",
  "code": "UNAUTHORIZED",
  "message": "请先完成认证"
}
```

**常见错误码**：

| 错误码 | 描述 |
|--------|------|
| `INVALID_MESSAGE` | 消息格式错误 |
| `UNAUTHORIZED` | 未认证 |
| `AUTH_TIMEOUT` | 认证超时 |
| `FORBIDDEN` | 无权限执行此操作 |
| `VALIDATION_ERROR` | 数据验证失败 |
| `INTERNAL_ERROR` | 服务器内部错误 |

---

### WebSocket TypeScript 类型定义

```typescript
// 客户端消息类型
type ClientMessage = 
  | { type: 'auth'; token: string; auth_type?: 'device_token' | 'jwt' }
  | { type: 'battery_report'; battery_level: number; is_charging?: boolean; power_saving_mode?: string; temperature?: number; voltage?: number; recorded_at?: string; msg_id?: string }
  | { type: 'batch_battery_report'; data: BatteryReportData[]; msg_id?: string }
  | { type: 'ping' }
  | { type: 'subscribe'; device_ids: string[] }
  | { type: 'unsubscribe'; device_ids?: string[] };

// 服务器消息类型
type ServerMessage =
  | { type: 'connected'; message: string; server_time: string; auth_timeout: number }
  | { type: 'auth_result'; success: boolean; message: string; device_id?: string; user_id?: string }
  | { type: 'battery_report_result'; success: boolean; data?: BatteryData; error?: string; msg_id?: string }
  | { type: 'batch_battery_report_result'; success: boolean; inserted_count?: number; error?: string; msg_id?: string }
  | { type: 'pong' }
  | { type: 'subscribe_result'; success: boolean; subscribed_devices: string[]; error?: string }
  | { type: 'battery_push'; device_id: string; data: LatestBatteryResponse }
  | { type: 'alert_push'; device_id: string; alert_type: string; message: string; severity: string; timestamp: string }
  | { type: 'error'; code: string; message: string };

interface BatteryReportData {
  battery_level: number;
  is_charging?: boolean;
  power_saving_mode?: string;
  temperature?: number;
  voltage?: number;
  recorded_at?: string;
}
```

---

### 使用示例

#### JavaScript/TypeScript 客户端

```typescript
// 设备连接示例
const ws = new WebSocket('wss://api.example.com/ws');

ws.onopen = () => {
  // 发送认证
  ws.send(JSON.stringify({
    type: 'auth',
    token: 'your-device-token',
    auth_type: 'device_token'
  }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  
  switch (msg.type) {
    case 'auth_result':
      if (msg.success) {
        console.log('认证成功');
        // 开始上报
        reportBattery();
      } else {
        console.error('认证失败:', msg.message);
      }
      break;
    case 'battery_report_result':
      if (msg.success) {
        console.log('上报成功:', msg.data);
      } else {
        console.error('上报失败:', msg.error);
      }
      break;
  }
};

function reportBattery() {
  ws.send(JSON.stringify({
    type: 'battery_report',
    battery_level: 75,
    is_charging: false,
    temperature: 25.5
  }));
}

// 心跳保活
setInterval(() => {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'ping' }));
  }
}, 30000);
```

#### Rust 客户端

```rust
use tokio_tungstenite::connect_async;
use futures_util::{SinkExt, StreamExt};

#[tokio::main]
async fn main() {
    let (ws_stream, _) = connect_async("wss://api.example.com/ws")
        .await
        .expect("连接失败");

    let (mut write, mut read) = ws_stream.split();

    // 发送认证
    let auth_msg = r#"{"type":"auth","token":"your-token","auth_type":"device_token"}"#;
    write.send(auth_msg.into()).await.unwrap();

    // 处理消息
    while let Some(msg) = read.next().await {
        if let Ok(msg) = msg {
            println!("收到: {}", msg);
        }
    }
}
```
