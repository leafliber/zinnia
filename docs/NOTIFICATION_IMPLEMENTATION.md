# 邮箱预警通知功能实现文档

## 项目概述

本次实现完成了 Zinnia 设备电量监控系统的邮箱预警通知功能，并设计了可扩展的通知架构，支持未来添加 Webhook、短信、推送等多种通知方式。

## 架构设计

### 1. 分层架构

```
┌─────────────────────────────────────────┐
│         API Layer (Handlers)            │
│  - notification_handler.rs              │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│       Service Layer (Services)          │
│  - NotificationService                  │
│  - EmailService (增强)                  │
│  - AlertService (集成通知)              │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│    Repository Layer (Data Access)       │
│  - NotificationRepository               │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│           Database Layer                │
│  - user_notification_preferences        │
│  - notification_history                 │
└─────────────────────────────────────────┘
```

### 2. 核心组件

#### 2.1 数据模型 (`models/notification.rs`)

- **NotificationChannel**: 通知渠道枚举（Email, Webhook, SMS, Push）
- **UserNotificationPreference**: 用户通知偏好配置
- **NotificationHistory**: 通知发送历史记录
- **EmailNotificationConfig**: 邮件通知配置
- **WebhookNotificationConfig**: Webhook通知配置（预留）

#### 2.2 数据库迁移 (`migrations/002_add_notification_preferences.sql`)

**新增表结构：**

1. **user_notification_preferences** - 用户通知偏好
   - 全局通知开关
   - 各渠道配置（JSONB存储，灵活扩展）
   - 预警级别过滤（Info/Warning/Critical）
   - 安静时段配置
   - 通知频率控制

2. **notification_history** - 通知历史记录
   - 关联预警事件
   - 记录发送状态
   - 支持审计追踪

**特性：**
- 自动为新用户创建默认通知偏好（触发器）
- 为现有用户创建默认配置

#### 2.3 仓库层 (`repositories/notification_repo.rs`)

提供数据访问方法：
- `get_user_preference()` - 获取用户通知偏好
- `upsert_user_preference()` - 创建或更新偏好
- `create_notification_history()` - 创建通知记录
- `update_notification_status()` - 更新发送状态
- `get_last_notification_time()` - 频率控制检查

#### 2.4 服务层

##### NotificationService (`services/notification_service.rs`)

**核心功能：**
- 统一的通知发送接口
- 多渠道通知支持（Email, Webhook等）
- 智能通知控制：
  - 预警级别过滤
  - 安静时段检测
  - 发送频率限制
  - 失败重试和状态记录

**实现的 NotificationSender trait：**
```rust
#[async_trait::async_trait]
pub trait NotificationSender: Send + Sync {
    async fn send_alert_notification(&self, alert_event: &AlertEvent, user_id: Uuid) -> Result<(), AppError>;
}
```

##### EmailService 增强 (`services/email_service.rs`)

**新增方法：**
- `send_alert_notification()` - 发送预警邮件
  - 根据预警级别定制邮件主题
  - 详细的预警信息
  - 智能建议（基于预警类型）

##### AlertService 集成 (`services/alert_service.rs`)

**改进：**
- 支持通知服务依赖注入（避免循环依赖）
- 预警触发时自动发送通知
- 通知发送失败不影响预警记录

#### 2.5 API 层 (`handlers/notification_handler.rs`)

**新增接口：**
- `GET /api/v1/notifications/preferences` - 获取通知偏好
- `PUT /api/v1/notifications/preferences` - 更新通知偏好

## 主要特性

### 1. 多渠道支持

设计采用可扩展架构，当前实现：
- ✅ **Email 通知** - 完整实现
- 🔄 **Webhook 通知** - 框架就绪，待实现HTTP调用
- 📋 **SMS 通知** - 预留接口
- 📋 **Push 通知** - 预留接口

### 2. 智能通知控制

#### 预警级别过滤
用户可配置接收哪些级别的预警：
- Info（信息）
- Warning（警告）
- Critical（严重）

#### 安静时段
- 支持配置安静时段（如22:00-08:00）
- 时区感知（支持不同时区）
- 跨午夜时段处理

#### 频率限制
- 可配置最小通知间隔（默认5分钟）
- 防止通知轰炸
- 记录跳过的通知

### 3. 邮件通知增强

#### 智能建议系统
根据预警类型和级别提供针对性建议：
- 低电量 → 充电建议
- 高温 → 散热建议
- 设备离线 → 连接检查
- 快速耗电 → 性能优化

#### 邮件内容
- 设备信息
- 预警详情（类型、级别、消息）
- 当前值 vs 阈值
- 触发时间
- 智能建议

### 4. 历史记录与审计

所有通知都会记录：
- 发送时间
- 接收方
- 状态（pending/sent/failed/skipped）
- 失败原因
- 关联的预警事件

## 使用示例

### 1. 配置通知偏好

```bash
# 获取当前配置
curl -X GET http://localhost:8080/api/v1/notifications/preferences \
  -H "Authorization: Bearer <token>"

# 更新配置
curl -X PUT http://localhost:8080/api/v1/notifications/preferences \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "enabled": true,
    "email_config": {
      "enabled": true,
      "email": "user@example.com"
    },
    "notify_info": false,
    "notify_warning": true,
    "notify_critical": true,
    "quiet_hours_start": "22:00",
    "quiet_hours_end": "08:00",
    "quiet_hours_timezone": "Asia/Shanghai",
    "min_notification_interval": 10
  }'
```

### 2. 邮件通知示例

当设备触发预警时，用户会收到类似这样的邮件：

```
主题：【Zinnia】🔴 严重预警 - CriticalBattery

您好！

您的设备触发了预警：

📱 设备名称：iPhone 14 Pro
⚠️  预警类型：CriticalBattery
📊 预警级别：critical
📝 预警信息：设备电量临界: 5%

详细信息：
• 当前值：5.00
• 阈值：10.00
• 触发时间：2026-01-20 15:30:45 UTC

建议：
• 请立即为设备充电
• 关闭非必要应用以延长续航

如需查看更多详情或管理预警，请登录 Zinnia 控制台。

此邮件由系统自动发送，请勿直接回复。

——Zinnia 团队
```

## 环境配置

### SMTP 配置（环境变量）

```bash
# 启用SMTP
ZINNIA_SMTP__ENABLED=true

# SMTP服务器配置
ZINNIA_SMTP__HOST=smtp.gmail.com
ZINNIA_SMTP__PORT=465
ZINNIA_SMTP__USERNAME=your-email@gmail.com
SMTP_PASSWORD=your-app-password

# 发件人信息
ZINNIA_SMTP__FROM_EMAIL=noreply@example.com
ZINNIA_SMTP__FROM_NAME=Zinnia

# 限制配置
ZINNIA_SMTP__MAX_SENDS_PER_HOUR=30
```

## 数据库迁移

运行新的迁移脚本：

```bash
# 开发环境
./scripts/dev_manage.sh migrate

# 生产环境
./scripts/manage.sh migrate
```

迁移会自动：
1. 创建通知相关表
2. 为现有用户生成默认通知配置
3. 设置触发器为新用户自动创建配置

## 扩展指南

### 添加新的通知渠道

以添加 SMS 通知为例：

1. **在 `models/notification.rs` 中定义配置：**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsNotificationConfig {
    pub enabled: bool,
    pub phone_number: String,
    pub provider: String,
}
```

2. **在 `NotificationService` 中实现发送逻辑：**
```rust
async fn send_sms_notification(
    &self,
    preference: &UserNotificationPreference,
    alert_event: &AlertEvent,
    device_name: &str,
) -> Result<(), AppError> {
    // 实现 SMS 发送逻辑
}
```

3. **在 `send_alert_notification` 中调用：**
```rust
// SMS 通知
if let Err(e) = self.send_sms_notification(&preference, alert_event, &device.name).await {
    tracing::error!("SMS通知发送失败: {}", e);
}
```

## 测试建议

### 单元测试
- NotificationRepository 数据访问
- 通知级别过滤逻辑
- 安静时段判断
- 频率限制检查

### 集成测试
- 端到端通知流程
- 多渠道并发发送
- 失败重试机制
- 历史记录准确性

### 手动测试清单
- [ ] 创建预警规则
- [ ] 配置通知偏好
- [ ] 触发不同级别的预警
- [ ] 验证邮件接收
- [ ] 测试安静时段
- [ ] 测试频率限制
- [ ] 检查通知历史记录

## 注意事项

1. **SMTP 配置**：确保 SMTP 密码正确配置，使用应用专用密码
2. **频率限制**：避免短时间内大量通知
3. **安全性**：Webhook secret 应妥善保管
4. **性能**：通知发送采用异步非阻塞，不影响预警记录
5. **监控**：关注通知发送失败率，及时处理问题

## 相关文件清单

### 新增文件
- `migrations/002_add_notification_preferences.sql`
- `src/models/notification.rs`
- `src/repositories/notification_repo.rs`
- `src/services/notification_service.rs`
- `src/handlers/notification_handler.rs`

### 修改文件
- `src/models/mod.rs`
- `src/repositories/mod.rs`
- `src/services/mod.rs`
- `src/services/alert_service.rs`
- `src/services/email_service.rs`
- `src/handlers/mod.rs`
- `src/routes/mod.rs`
- `src/main.rs`
- `Cargo.toml`

## 未来改进方向

1. **Webhook 通知完整实现**
   - HTTP 请求发送
   - 签名验证
   - 重试机制

2. **通知模板系统**
   - 支持自定义邮件模板
   - HTML 邮件支持
   - 多语言支持

3. **批量通知**
   - 相同设备多个预警合并
   - 定期摘要报告

4. **通知统计**
   - 发送成功率
   - 用户参与度分析
   - 通道效果对比

5. **高级过滤**
   - 按设备过滤
   - 按时间段过滤
   - 自定义规则引擎

## 参考资料

- [Lettre 邮件库文档](https://lettre.rs/)
- [PostgreSQL JSONB](https://www.postgresql.org/docs/current/datatype-json.html)
- [Actix-web 中间件](https://actix.rs/docs/middleware/)
