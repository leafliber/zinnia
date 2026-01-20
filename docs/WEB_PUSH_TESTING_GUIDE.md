# Web Push API 测试指南

## 前置条件

### 1. 配置 VAPID 密钥

首先生成 VAPID 密钥对（可以使用 web-push 工具）：

```bash
# 使用 Node.js web-push 库生成
npx web-push generate-vapid-keys
```

将生成的密钥添加到环境变量：

```bash
# .env 文件
ZINNIA_WEB_PUSH__ENABLED=true
WEB_PUSH_VAPID_PRIVATE_KEY=your-private-key-here
WEB_PUSH_VAPID_PUBLIC_KEY=your-public-key-here
WEB_PUSH_CONTACT_EMAIL=mailto:admin@example.com
```

### 2. 启动服务

```bash
cargo run
```

## API 测试流程

### 步骤 1: 获取 VAPID 公钥

前端需要先获取公钥用于订阅：

```bash
curl -X GET http://localhost:8080/api/v1/web-push/vapid-key
```

**响应示例：**
```json
{
  "status": "success",
  "data": {
    "public_key": "BNcRdreALRFXTkOOUHK1EtK2wtaz5Ry4YfYCA_0QTpQtUbVlUls0VJXg7A8u-Ts1XbjhazAkj7I99e8QcYP7DkM="
  }
}
```

### 步骤 2: 订阅 Web Push

使用前端 Service Worker 获取订阅后，发送到后端：

```bash
curl -X POST http://localhost:8080/api/v1/web-push/subscribe \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "subscription": {
      "endpoint": "https://fcm.googleapis.com/fcm/send/cBw...:APA91...",
      "keys": {
        "p256dh": "BNcRdreALRFXTkOOUHK1EtK2wtaz5Ry4YfYCA_0QTpQtUbVlUls0VJXg7A8u-Ts1XbjhazAkj7I99e8QcYP7DkM=",
        "auth": "tBHItJI5svbpez7KI4CCXg=="
      }
    }
  }'
```

**响应示例：**
```json
{
  "status": "success",
  "data": {
    "message": "订阅成功"
  }
}
```

### 步骤 3: 测试推送通知

手动发送测试通知（仅用于测试）：

```bash
curl -X POST http://localhost:8080/api/v1/web-push/test \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "测试通知",
    "body": "这是一条测试推送消息",
    "icon": "/icons/test.png"
  }'
```

**响应示例：**
```json
{
  "status": "success",
  "data": {
    "message": "测试通知已发送",
    "sent_count": 1
  }
}
```

### 步骤 4: 查看订阅列表

```bash
curl -X GET http://localhost:8080/api/v1/web-push/subscriptions \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"
```

**响应示例：**
```json
{
  "status": "success",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "endpoint": "https://fcm.googleapis.com/fcm/send/cBw...",
      "created_at": "2026-01-20T10:00:00Z"
    }
  ]
}
```

### 步骤 5: 取消订阅

```bash
curl -X POST http://localhost:8080/api/v1/web-push/unsubscribe \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "endpoint": "https://fcm.googleapis.com/fcm/send/cBw..."
  }'
```

**响应示例：**
```json
{
  "status": "success",
  "data": {
    "message": "取消订阅成功"
  }
}
```

## 前端集成示例

### 1. Service Worker 注册

```javascript
// public/sw.js
self.addEventListener('push', function(event) {
  const data = event.data.json();
  
  const options = {
    body: data.body,
    icon: data.icon || '/icons/default.png',
    badge: data.badge || '/icons/badge.png',
    tag: data.tag || 'notification',
    data: data.data,
    actions: data.actions || [
      { action: 'view', title: '查看' },
      { action: 'dismiss', title: '关闭' }
    ]
  };

  event.waitUntil(
    self.registration.showNotification(data.title, options)
  );
});

self.addEventListener('notificationclick', function(event) {
  event.notification.close();
  
  if (event.action === 'view' && event.notification.data?.url) {
    event.waitUntil(
      clients.openWindow(event.notification.data.url)
    );
  }
});
```

### 2. 订阅逻辑

```javascript
// src/utils/webPush.js
async function subscribeWebPush(token) {
  // 1. 注册 Service Worker
  const registration = await navigator.serviceWorker.register('/sw.js');
  await navigator.serviceWorker.ready;

  // 2. 获取 VAPID 公钥
  const response = await fetch('http://localhost:8080/api/v1/web-push/vapid-key');
  const { data } = await response.json();
  const publicKey = data.public_key;

  // 3. 订阅推送
  const subscription = await registration.pushManager.subscribe({
    userVisibleOnly: true,
    applicationServerKey: urlBase64ToUint8Array(publicKey)
  });

  // 4. 发送订阅到后端
  await fetch('http://localhost:8080/api/v1/web-push/subscribe', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${token}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({ subscription })
  });

  return subscription;
}

function urlBase64ToUint8Array(base64String) {
  const padding = '='.repeat((4 - base64String.length % 4) % 4);
  const base64 = (base64String + padding)
    .replace(/\-/g, '+')
    .replace(/_/g, '/');

  const rawData = window.atob(base64);
  const outputArray = new Uint8Array(rawData.length);

  for (let i = 0; i < rawData.length; ++i) {
    outputArray[i] = rawData.charCodeAt(i);
  }
  return outputArray;
}
```

### 3. React 组件示例

```tsx
// src/components/NotificationSettings.tsx
import React, { useState, useEffect } from 'react';

export function NotificationSettings() {
  const [isPushEnabled, setIsPushEnabled] = useState(false);
  const [isSupported, setIsSupported] = useState(false);

  useEffect(() => {
    // 检查浏览器支持
    setIsSupported(
      'serviceWorker' in navigator &&
      'PushManager' in window
    );

    // 检查当前订阅状态
    checkSubscription();
  }, []);

  async function checkSubscription() {
    if (!isSupported) return;

    const registration = await navigator.serviceWorker.getRegistration();
    if (!registration) return;

    const subscription = await registration.pushManager.getSubscription();
    setIsPushEnabled(!!subscription);
  }

  async function togglePush() {
    if (isPushEnabled) {
      // 取消订阅
      await unsubscribeWebPush();
      setIsPushEnabled(false);
    } else {
      // 订阅
      const token = localStorage.getItem('jwt_token');
      await subscribeWebPush(token);
      setIsPushEnabled(true);
    }
  }

  return (
    <div className="notification-settings">
      <h3>推送通知设置</h3>
      
      {!isSupported && (
        <p className="warning">您的浏览器不支持推送通知</p>
      )}

      {isSupported && (
        <label>
          <input
            type="checkbox"
            checked={isPushEnabled}
            onChange={togglePush}
          />
          启用浏览器推送通知
        </label>
      )}
    </div>
  );
}
```

## 实际预警触发测试

### 1. 配置通知偏好

确保启用 Web Push 通知：

```bash
curl -X PUT http://localhost:8080/api/v1/notifications/preferences \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "enabled": true,
    "notify_critical": true
  }'
```

### 2. 创建预警规则

```bash
curl -X POST http://localhost:8080/api/v1/alerts/rules \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "低电量预警",
    "alert_type": "low_battery",
    "level": "critical",
    "cooldown_minutes": 5,
    "enabled": true
  }'
```

### 3. 上报触发预警的数据

```bash
curl -X POST http://localhost:8080/api/v1/battery/report \
  -H "Authorization: Bearer YOUR_DEVICE_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "level": 5.0,
    "temperature": 25.0,
    "is_charging": false
  }'
```

如果配置正确，你应该会在浏览器中收到推送通知！

## 调试建议

### 1. 检查浏览器控制台

```javascript
// 查看 Service Worker 状态
navigator.serviceWorker.getRegistrations().then(registrations => {
  console.log('Service Workers:', registrations);
});

// 查看订阅状态
navigator.serviceWorker.ready.then(registration => {
  registration.pushManager.getSubscription().then(subscription => {
    console.log('Push Subscription:', subscription);
  });
});
```

### 2. 检查后端日志

```bash
# 查看通知发送日志
grep "Web Push" zinnia.log

# 查看订阅相关日志
grep "subscribe" zinnia.log
```

### 3. 使用浏览器开发工具

- Chrome DevTools > Application > Service Workers
- Chrome DevTools > Application > Notifications
- Firefox DevTools > Application > Service Workers

## 常见问题

### Q: 为什么我收不到推送通知？

检查以下项：
1. 浏览器通知权限是否授予
2. Service Worker 是否正确注册
3. 订阅是否成功保存到后端
4. VAPID 密钥配置是否正确
5. 后端日志是否有错误

### Q: 如何在本地测试 HTTPS？

Web Push 需要 HTTPS（localhost 除外）。开发环境可以：
1. 使用 localhost（无需 HTTPS）
2. 使用 ngrok 等工具创建 HTTPS 隧道
3. 配置自签名证书

### Q: 推送通知不显示图标？

确保：
1. 图标路径正确
2. 图标文件可访问
3. 图标大小合适（建议 192x192）

## 性能监控

### 推送通知指标

```sql
-- 查看推送通知发送统计
SELECT 
  channel,
  status,
  COUNT(*) as count
FROM notification_history
WHERE channel = 'push'
  AND created_at > NOW() - INTERVAL '24 hours'
GROUP BY channel, status;

-- 查看推送失败率
SELECT 
  COUNT(CASE WHEN status = 'failed' THEN 1 END) * 100.0 / COUNT(*) as failure_rate
FROM notification_history
WHERE channel = 'push'
  AND created_at > NOW() - INTERVAL '24 hours';
```

## 安全建议

1. **VAPID 密钥管理**：
   - 使用环境变量存储
   - 定期轮换密钥
   - 不要提交到版本控制

2. **订阅验证**：
   - 验证订阅端点的有效性
   - 限制每用户的订阅数量
   - 定期清理无效订阅

3. **通知内容**：
   - 不要在通知中包含敏感信息
   - 使用通知数据的 URL 跳转获取详情
   - 加密敏感负载

## 参考资料

- [Web Push Protocol](https://datatracker.ietf.org/doc/html/rfc8030)
- [VAPID](https://datatracker.ietf.org/doc/html/rfc8292)
- [Push API MDN](https://developer.mozilla.org/en-US/docs/Web/API/Push_API)
- [Service Worker API](https://developer.mozilla.org/en-US/docs/Web/API/Service_Worker_API)
