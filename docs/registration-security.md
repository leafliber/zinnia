# 用户注册安全 API 文档

本文档描述了 Zinnia 系统中用户注册安全相关的 API 接口，包括邮箱验证码、reCAPTCHA 验证、密码重置等功能。

## 目录

- [概述](#概述)
- [配置说明](#配置说明)
- [API 端点](#api-端点)
  - [获取 reCAPTCHA 配置](#获取-recaptcha-配置)
  - [获取注册安全配置](#获取注册安全配置)
  - [发送注册验证码](#发送注册验证码)
  - [验证验证码](#验证验证码)
  - [发送密码重置验证码](#发送密码重置验证码)
  - [确认密码重置](#确认密码重置)
  - [用户注册](#用户注册)
- [错误响应](#错误响应)
- [前端集成指南](#前端集成指南)

---

## 概述

Zinnia 提供多层安全防护机制保护用户注册流程：

1. **邮箱验证码**：6 位数字验证码，有效期 10 分钟，最多验证 5 次
2. **Google reCAPTCHA**：支持 v2 和 v3，有效防止机器人注册
3. **IP 限制**：每 IP 每小时/每天注册次数限制
4. **可疑行为检测**：自动标记和限制异常注册行为

---

## 配置说明

### 环境变量

```bash
# SMTP 配置
SMTP_ENABLED=true
SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_USERNAME=your-email@example.com
SMTP_PASSWORD=your-smtp-password
SMTP_FROM_EMAIL=noreply@example.com
SMTP_FROM_NAME=Zinnia
SMTP_TLS=true
EMAIL_VERIFICATION_CODE_EXPIRY_SECONDS=600
EMAIL_MAX_SENDS_PER_HOUR=10

# reCAPTCHA 配置
RECAPTCHA_ENABLED=true
RECAPTCHA_SITE_KEY=your-site-key
RECAPTCHA_SECRET_KEY=your-secret-key
RECAPTCHA_SCORE_THRESHOLD=0.5

# 注册限制配置
REGISTRATION_MAX_PER_IP_PER_HOUR=5
REGISTRATION_MAX_PER_IP_PER_DAY=10
REGISTRATION_REQUIRE_EMAIL_VERIFICATION=true
REGISTRATION_REQUIRE_RECAPTCHA=true
```

---

## API 端点

### 获取 reCAPTCHA 配置

获取前端初始化 reCAPTCHA 所需的配置信息。

**请求**

```http
GET /api/v1/auth/recaptcha/config
```

**响应示例**

```json
{
  "success": true,
  "data": {
    "enabled": true,
    "site_key": "6LeIxAcTAAAAAJcZVRqyHh71UMIEGNQ_MXjiZKhI"
  }
}
```

**字段说明**

| 字段 | 类型 | 说明 |
|------|------|------|
| `enabled` | boolean | reCAPTCHA 是否启用 |
| `site_key` | string \| null | reCAPTCHA 站点密钥（前端使用） |

---

### 获取注册安全配置

获取注册流程需要的安全配置信息。

**请求**

```http
GET /api/v1/auth/registration/config
```

**响应示例**

```json
{
  "success": true,
  "data": {
    "require_email_verification": true,
    "require_recaptcha": true,
    "recaptcha_site_key": "6LeIxAcTAAAAAJcZVRqyHh71UMIEGNQ_MXjiZKhI"
  }
}
```

**字段说明**

| 字段 | 类型 | 说明 |
|------|------|------|
| `require_email_verification` | boolean | 是否需要邮箱验证 |
| `require_recaptcha` | boolean | 是否需要 reCAPTCHA 验证 |
| `recaptcha_site_key` | string \| null | reCAPTCHA 站点密钥 |

---

### 发送注册验证码

向指定邮箱发送 6 位数字验证码。

**请求**

```http
POST /api/v1/auth/verification/send
Content-Type: application/json
```

**请求体**

```json
{
  "email": "user@example.com",
  "recaptcha_token": "03AGdBq26..."  // 可选，如启用 reCAPTCHA 则必填
}
```

**字段说明**

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `email` | string | 是 | 接收验证码的邮箱地址 |
| `recaptcha_token` | string | 条件必填 | reCAPTCHA 验证令牌 |

**响应示例**

成功：
```json
{
  "success": true,
  "data": {
    "message": "验证码已发送到您的邮箱",
    "expires_in_minutes": 10
  }
}
```

**限制规则**

- 同一邮箱每小时最多发送 10 次
- 两次发送间隔至少 60 秒
- 验证码有效期 10 分钟

---

### 验证验证码

验证用户输入的验证码是否正确（不消耗验证码）。

**请求**

```http
POST /api/v1/auth/verification/verify
Content-Type: application/json
```

**请求体**

```json
{
  "email": "user@example.com",
  "code": "123456"
}
```

**字段说明**

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `email` | string | 是 | 邮箱地址 |
| `code` | string | 是 | 6 位数字验证码 |

**响应示例**

成功：
```json
{
  "success": true,
  "message": "验证码正确"
}
```

**限制规则**

- 每个验证码最多验证 5 次
- 超过限制后验证码失效

---

### 发送密码重置验证码

向用户邮箱发送密码重置验证码。

**请求**

```http
POST /api/v1/auth/password-reset/send
Content-Type: application/json
```

**请求体**

```json
{
  "email": "user@example.com",
  "recaptcha_token": "03AGdBq26..."  // 可选
}
```

**响应示例**

```json
{
  "success": true,
  "data": {
    "message": "验证码已发送到您的邮箱",
    "expires_in_minutes": 10
  }
}
```

---

### 确认密码重置

使用验证码重置用户密码。

**请求**

```http
POST /api/v1/auth/password-reset/confirm
Content-Type: application/json
```

**请求体**

```json
{
  "email": "user@example.com",
  "code": "123456",
  "new_password": "NewSecurePassword123!",
  "confirm_password": "NewSecurePassword123!"
}
```

**字段说明**

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `email` | string | 是 | 邮箱地址 |
| `code` | string | 是 | 6 位数字验证码 |
| `new_password` | string | 是 | 新密码（8-128 字符） |
| `confirm_password` | string | 是 | 确认密码 |

**密码要求**

- 长度：8-128 字符
- 必须包含：大小写字母、数字

**响应示例**

```json
{
  "success": true,
  "message": "密码已重置，请使用新密码登录"
}
```

---

### 用户注册

创建新用户账户。

**请求**

```http
POST /api/v1/users/register
Content-Type: application/json
```

**请求体**

```json
{
  "email": "user@example.com",
  "username": "johndoe",
  "password": "SecurePassword123!",
  "recaptcha_token": "03AGdBq26...",  // 可选
  "verification_code": "123456"        // 可选
}
```

**字段说明**

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `email` | string | 是 | 邮箱地址 |
| `username` | string | 是 | 用户名（3-50 字符） |
| `password` | string | 是 | 密码（8-128 字符） |
| `recaptcha_token` | string | 条件必填 | reCAPTCHA 验证令牌 |
| `verification_code` | string | 条件必填 | 邮箱验证码 |

**响应示例**

成功：
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "user@example.com",
    "username": "johndoe",
    "role": "user",
    "is_active": true,
    "created_at": "2025-01-12T10:30:00Z"
  }
}
```

---

## 错误响应

### 常见错误码

| HTTP 状态码 | 错误类型 | 说明 |
|-------------|----------|------|
| 400 | ValidationError | 请求参数验证失败 |
| 401 | Unauthorized | 验证失败（reCAPTCHA/验证码） |
| 429 | RateLimitExceeded | 请求过于频繁 |
| 500 | InternalServerError | 服务器内部错误 |

### 错误响应格式

```json
{
  "success": false,
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "邮箱格式无效"
  }
}
```

### 典型错误场景

**邮箱已注册**
```json
{
  "success": false,
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "邮箱已被注册"
  }
}
```

**验证码错误**
```json
{
  "success": false,
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "验证码错误或已过期"
  }
}
```

**请求过于频繁**
```json
{
  "success": false,
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "注册过于频繁，请稍后再试"
  }
}
```

**reCAPTCHA 验证失败**
```json
{
  "success": false,
  "error": {
    "code": "UNAUTHORIZED",
    "message": "人机验证失败，请重试"
  }
}
```

---

## 前端集成指南

### 1. 初始化 reCAPTCHA

```html
<!-- 在 HTML 中引入 reCAPTCHA -->
<script src="https://www.google.com/recaptcha/api.js?render=YOUR_SITE_KEY"></script>
```

```javascript
// 获取配置
const configResponse = await fetch('/api/v1/auth/registration/config');
const config = await configResponse.json();

if (config.data.require_recaptcha && config.data.recaptcha_site_key) {
  // 初始化 reCAPTCHA v3
  grecaptcha.ready(() => {
    grecaptcha.execute(config.data.recaptcha_site_key, { action: 'register' })
      .then(token => {
        // 使用 token 进行后续请求
      });
  });
}
```

### 2. 完整注册流程

```javascript
async function registerUser(email, username, password) {
  // 1. 获取配置
  const config = await getRegistrationConfig();
  
  // 2. 发送验证码（如需要）
  if (config.require_email_verification) {
    const recaptchaToken = await getRecaptchaToken('send_verification');
    await sendVerificationCode(email, recaptchaToken);
    
    // 等待用户输入验证码...
    const code = await promptForCode();
  }
  
  // 3. 获取注册用的 reCAPTCHA token
  let registerRecaptchaToken = null;
  if (config.require_recaptcha) {
    registerRecaptchaToken = await getRecaptchaToken('register');
  }
  
  // 4. 提交注册
  const response = await fetch('/api/v1/users/register', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      email,
      username,
      password,
      recaptcha_token: registerRecaptchaToken,
      verification_code: code
    })
  });
  
  return response.json();
}
```

### 3. 密码重置流程

```javascript
async function resetPassword(email, newPassword) {
  // 1. 发送重置验证码
  await fetch('/api/v1/auth/password-reset/send', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email })
  });
  
  // 2. 等待用户输入验证码
  const code = await promptForCode();
  
  // 3. 确认重置
  const response = await fetch('/api/v1/auth/password-reset/confirm', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      email,
      code,
      new_password: newPassword,
      confirm_password: newPassword
    })
  });
  
  return response.json();
}
```

---

## 安全建议

1. **HTTPS**：所有 API 请求必须通过 HTTPS 进行
2. **验证码安全**：验证码仅显示部分内容（如前2位），防止邮件被截获
3. **密码策略**：建议使用强密码，包含大小写字母、数字和特殊字符
4. **登录锁定**：连续登录失败 5 次后锁定账户 15 分钟
5. **会话管理**：密码重置后自动登出所有设备

---

## 更新日志

| 版本 | 日期 | 变更说明 |
|------|------|----------|
| 1.0.0 | 2025-01-12 | 初始版本，包含邮箱验证、reCAPTCHA、IP 限制等功能 |
