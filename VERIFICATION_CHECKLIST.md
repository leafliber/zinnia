# PWA Web Push 功能验证清单

## 验证前准备

- [ ] 已安装 Rust 工具链
- [ ] 已安装 PostgreSQL
- [ ] 已安装 Redis
- [ ] 已配置环境变量（DATABASE_URL, REDIS_URL, JWT_SECRET 等）

## 代码完整性验证

### 1. 检查所有文件已创建

```bash
# 检查新增的文件
ls -la migrations/003_add_web_push_subscriptions.sql
ls -la src/models/web_push.rs
ls -la src/repositories/web_push_repo.rs
ls -la src/services/web_push_service.rs
ls -la src/handlers/web_push_handler.rs
ls -la tests/unit/notification_tests.rs
ls -la tests/integration/web_push_tests.rs
ls -la docs/WEB_PUSH_TESTING_GUIDE.md
ls -la docs/WEB_PUSH_IMPLEMENTATION_SUMMARY.md
```

预期输出：所有文件都存在且有内容

### 2. 检查文件修改

```bash
# 检查修改的文件
git status
```

预期输出：显示所有修改和新增的文件

### 3. 检查导入完整性

```bash
# 检查是否所有模块都正确导出
grep -r "pub use web_push" src/
grep -r "WebPushService" src/main.rs
grep -r "WebPushRepository" src/repositories/mod.rs
```

预期输出：找到所有相关导出和使用

## 编译验证

### 1. 检查语法错误

```bash
cargo check
```

预期输出：
```
   Compiling zinnia v0.1.0
    Finished dev [unoptimized + debuginfo] target(s) in X.XXs
```

### 2. 编译项目

```bash
cargo build
```

预期输出：成功编译，无错误

### 3. 运行测试

```bash
# 运行单元测试
cargo test --lib

# 运行特定测试
cargo test notification_tests
```

预期输出：所有测试通过（忽略 `#[ignore]` 的测试）

## 数据库验证

### 1. 检查迁移文件

```bash
cat migrations/003_add_web_push_subscriptions.sql
```

预期输出：包含 CREATE TABLE web_push_subscriptions

### 2. 运行迁移

```bash
./scripts/dev_manage.sh migrate
```

预期输出：
```
Running migration 003_add_web_push_subscriptions.sql
Migration successful
```

### 3. 验证表结构

```sql
\d web_push_subscriptions
```

预期输出：显示表结构，包含所有字段

## 服务启动验证

### 1. 启动服务（不配置 VAPID 密钥）

```bash
cargo run
```

预期输出：
```
✅ 数据库连接成功
✅ Redis 连接成功
⚠️  Web Push 服务初始化失败（需要配置 VAPID 密钥）
✅ 安全服务初始化完成
🚀 服务启动在 http://0.0.0.0:8080
```

注意：Web Push 未启用是正常的，因为没有配置 VAPID 密钥

### 2. 生成 VAPID 密钥

```bash
# 需要 Node.js 和 npm
npx web-push generate-vapid-keys
```

保存输出的密钥

### 3. 配置 VAPID 密钥并重启

```bash
# 添加到 .env
export ZINNIA_WEB_PUSH__ENABLED=true
export WEB_PUSH_VAPID_PRIVATE_KEY="<your-private-key>"
export WEB_PUSH_VAPID_PUBLIC_KEY="<your-public-key>"
export WEB_PUSH_CONTACT_EMAIL="mailto:admin@example.com"

# 重启服务
cargo run
```

预期输出：
```
✅ Web Push 服务初始化完成
```

## API 端点验证

### 1. 检查路由注册

```bash
# 服务启动后，检查日志
```

预期：没有路由冲突错误

### 2. 测试 VAPID 公钥获取（无需认证）

```bash
curl -X GET http://localhost:8080/api/v1/web-push/vapid-key
```

预期输出：
```json
{
  "status": "success",
  "data": {
    "public_key": "BNcRdreA..."
  }
}
```

### 3. 测试订阅（需要 JWT Token）

首先登录获取 token：

```bash
# 注册/登录获取 token
curl -X POST http://localhost:8080/api/v1/users/login \
  -H "Content-Type: application/json" \
  -d '{
    "login": "test@example.com",
    "password": "password123"
  }'
```

然后测试订阅：

```bash
curl -X POST http://localhost:8080/api/v1/web-push/subscribe \
  -H "Authorization: Bearer <your-token>" \
  -H "Content-Type: application/json" \
  -d '{
    "subscription": {
      "endpoint": "https://fcm.googleapis.com/fcm/send/test",
      "keys": {
        "p256dh": "BNcRdreALRFXTkOOUHK1EtK2wtaz5Ry4YfYCA_0QTpQtUbVlUls0VJXg7A8u-Ts1XbjhazAkj7I99e8QcYP7DkM=",
        "auth": "tBHItJI5svbpez7KI4CCXg=="
      }
    }
  }'
```

预期输出：
```json
{
  "status": "success",
  "data": {
    "message": "订阅成功"
  }
}
```

### 4. 测试订阅列表

```bash
curl -X GET http://localhost:8080/api/v1/web-push/subscriptions \
  -H "Authorization: Bearer <your-token>"
```

预期输出：
```json
{
  "status": "success",
  "data": [
    {
      "id": "...",
      "endpoint": "https://fcm.googleapis.com/fcm/send/test",
      "created_at": "2026-01-20T..."
    }
  ]
}
```

### 5. 测试推送通知

```bash
curl -X POST http://localhost:8080/api/v1/web-push/test \
  -H "Authorization: Bearer <your-token>" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "测试通知",
    "body": "这是一条测试消息",
    "icon": "/icons/test.png"
  }'
```

预期输出：
```json
{
  "status": "success",
  "data": {
    "message": "测试通知已发送",
    "sent_count": 1
  }
}
```

## 集成验证

### 1. 配置通知偏好

```bash
curl -X PUT http://localhost:8080/api/v1/notifications/preferences \
  -H "Authorization: Bearer <your-token>" \
  -H "Content-Type: application/json" \
  -d '{
    "enabled": true,
    "notify_critical": true
  }'
```

### 2. 创建预警规则

```bash
curl -X POST http://localhost:8080/api/v1/alerts/rules \
  -H "Authorization: Bearer <your-token>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "低电量预警",
    "alert_type": "low_battery",
    "level": "critical",
    "cooldown_minutes": 1,
    "enabled": true
  }'
```

### 3. 触发预警

```bash
curl -X POST http://localhost:8080/api/v1/battery/report \
  -H "Authorization: Bearer <your-device-api-key>" \
  -H "Content-Type: application/json" \
  -d '{
    "level": 5.0,
    "temperature": 25.0
  }'
```

### 4. 检查通知历史

```bash
# 检查数据库
psql $DATABASE_URL -c "
SELECT * FROM notification_history 
WHERE channel = 'push' 
ORDER BY created_at DESC 
LIMIT 5;
"
```

预期：看到新的 push 通知记录

## 前端验证（可选）

### 1. 创建测试 HTML 页面

创建 `test-push.html`：

```html
<!DOCTYPE html>
<html>
<head>
  <title>Web Push Test</title>
</head>
<body>
  <h1>Web Push 测试</h1>
  <button id="subscribe">订阅推送</button>
  <button id="test">发送测试通知</button>
  <div id="status"></div>

  <script>
    const API_BASE = 'http://localhost:8080/api/v1';
    const TOKEN = 'YOUR_JWT_TOKEN'; // 替换为实际 token

    async function subscribe() {
      try {
        // 注册 Service Worker
        const registration = await navigator.serviceWorker.register('/sw.js');
        
        // 获取 VAPID 公钥
        const keyRes = await fetch(`${API_BASE}/web-push/vapid-key`);
        const { data } = await keyRes.json();
        
        // 订阅
        const subscription = await registration.pushManager.subscribe({
          userVisibleOnly: true,
          applicationServerKey: urlBase64ToUint8Array(data.public_key)
        });
        
        // 发送到后端
        await fetch(`${API_BASE}/web-push/subscribe`, {
          method: 'POST',
          headers: {
            'Authorization': `Bearer ${TOKEN}`,
            'Content-Type': 'application/json'
          },
          body: JSON.stringify({ subscription })
        });
        
        document.getElementById('status').textContent = '订阅成功！';
      } catch (error) {
        document.getElementById('status').textContent = `错误: ${error.message}`;
      }
    }

    async function sendTest() {
      try {
        const res = await fetch(`${API_BASE}/web-push/test`, {
          method: 'POST',
          headers: {
            'Authorization': `Bearer ${TOKEN}`,
            'Content-Type': 'application/json'
          },
          body: JSON.stringify({
            title: '测试通知',
            body: '这是一条测试消息'
          })
        });
        
        const result = await res.json();
        document.getElementById('status').textContent = 
          `测试通知已发送: ${result.data.message}`;
      } catch (error) {
        document.getElementById('status').textContent = `错误: ${error.message}`;
      }
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

    document.getElementById('subscribe').onclick = subscribe;
    document.getElementById('test').onclick = sendTest;
  </script>
</body>
</html>
```

### 2. 创建 Service Worker

创建 `sw.js`：

```javascript
self.addEventListener('push', function(event) {
  const data = event.data.json();
  
  const options = {
    body: data.body,
    icon: data.icon || '/icons/default.png',
    badge: '/icons/badge.png',
    tag: data.tag || 'notification',
    data: data.data
  };

  event.waitUntil(
    self.registration.showNotification(data.title, options)
  );
});

self.addEventListener('notificationclick', function(event) {
  event.notification.close();
  if (event.notification.data?.url) {
    event.waitUntil(
      clients.openWindow(event.notification.data.url)
    );
  }
});
```

### 3. 使用浏览器测试

```bash
# 启动简单 HTTP 服务器
python3 -m http.server 8000
```

访问 `http://localhost:8000/test-push.html`

## 验证结果记录

### ✅ 编译验证
- [ ] cargo check 通过
- [ ] cargo build 成功
- [ ] cargo test 测试通过

### ✅ 数据库验证
- [ ] 迁移成功运行
- [ ] web_push_subscriptions 表创建成功

### ✅ 服务验证
- [ ] 服务正常启动
- [ ] Web Push 服务初始化成功（配置 VAPID 后）

### ✅ API 验证
- [ ] VAPID 公钥获取成功
- [ ] 订阅接口工作正常
- [ ] 订阅列表查询成功
- [ ] 测试推送发送成功
- [ ] 取消订阅工作正常

### ✅ 集成验证
- [ ] 通知偏好配置成功
- [ ] 预警规则创建成功
- [ ] 预警触发推送通知
- [ ] 通知历史记录正确

### ✅ 前端验证（可选）
- [ ] Service Worker 注册成功
- [ ] 浏览器订阅成功
- [ ] 推送通知显示正常

## 常见问题处理

### 问题 1: cargo check 失败

**症状**: 编译错误

**解决**:
```bash
# 查看详细错误
cargo check --verbose

# 检查依赖
cargo tree | grep web-push

# 更新依赖
cargo update
```

### 问题 2: 迁移失败

**症状**: 数据库迁移报错

**解决**:
```bash
# 检查数据库连接
psql $DATABASE_URL -c "SELECT 1"

# 手动运行迁移
psql $DATABASE_URL < migrations/003_add_web_push_subscriptions.sql

# 检查迁移状态
psql $DATABASE_URL -c "SELECT * FROM _sqlx_migrations"
```

### 问题 3: VAPID 密钥错误

**症状**: Web Push 服务初始化失败

**解决**:
```bash
# 检查环境变量
echo $WEB_PUSH_VAPID_PUBLIC_KEY
echo $WEB_PUSH_VAPID_PRIVATE_KEY

# 重新生成密钥
npx web-push generate-vapid-keys

# 确保密钥格式正确（base64 编码）
```

### 问题 4: 订阅失败

**症状**: 前端订阅返回错误

**解决**:
1. 检查 HTTPS（生产环境必需）
2. 检查 Service Worker 注册
3. 检查浏览器通知权限
4. 检查 VAPID 公钥格式

### 问题 5: 收不到推送

**症状**: 测试推送无响应

**解决**:
1. 检查订阅是否保存到数据库
2. 检查后端日志错误信息
3. 检查浏览器开发者工具 Console
4. 验证 Service Worker 是否激活

## 性能基准测试

### 1. 订阅性能

```bash
# 使用 ab 工具测试订阅接口
ab -n 100 -c 10 -T application/json -H "Authorization: Bearer TOKEN" \
  http://localhost:8080/api/v1/web-push/subscribe
```

预期：平均响应时间 < 100ms

### 2. 推送性能

```bash
# 测试推送接口
ab -n 50 -c 5 -T application/json -H "Authorization: Bearer TOKEN" \
  http://localhost:8080/api/v1/web-push/test
```

预期：平均响应时间 < 500ms（取决于订阅数量）

## 验证完成确认

- [ ] 所有代码文件已创建且格式正确
- [ ] 所有修改文件已更新
- [ ] cargo check 通过无错误
- [ ] cargo build 成功编译
- [ ] 单元测试通过
- [ ] 数据库迁移成功
- [ ] 服务正常启动
- [ ] VAPID 密钥配置成功
- [ ] 所有 API 端点测试通过
- [ ] 集成测试验证成功
- [ ] 文档完整且准确

## 下一步

✅ **验证完成后**，可以：

1. 提交代码到版本控制
2. 部署到测试环境
3. 进行完整的端到端测试
4. 准备生产环境部署

---

**验证状态**: ⏳ 待验证
**验证日期**: 2026年1月20日
**验证人**: _______
