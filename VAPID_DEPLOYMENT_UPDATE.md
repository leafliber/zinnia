# VAPID 支持部署更新总结

## 更新日期
2026年1月20日

## 更新内容

### 1. 环境变量配置文件

#### ✅ 更新了 `.env.example`
添加了 Web Push VAPID 配置部分：
```bash
# Web Push (PWA) 通知配置
VAPID_PUBLIC_KEY=your_vapid_public_key_here
VAPID_PRIVATE_KEY=your_vapid_private_key_here
```

#### ✅ 更新了 `.env.production.example`
添加了可选的 VAPID 配置说明：
```bash
# Web Push (PWA) 通知配置（可选）
VAPID_PUBLIC_KEY=
VAPID_PRIVATE_KEY=
```

### 2. 部署脚本更新

#### ✅ `scripts/deploy.sh`
添加了交互式 Web Push 配置向导：
- 询问是否启用 Web Push 通知
- 自动检测 npx 并生成 VAPID 密钥
- 支持手动输入密钥
- 将密钥写入 `.env.production` 文件
- 在配置摘要中显示 Web Push 状态

**新增功能**：
```bash
# Web Push VAPID 配置
print_header "Web Push (PWA) 通知配置（可选）"

read -p "是否启用 Web Push 通知？[y/N] " -r enable_vapid
if [[ $enable_vapid =~ ^[Yy]$ ]]; then
    # 自动生成或手动输入 VAPID 密钥
    # ...
fi
```

#### ✅ `scripts/dev_init.sh`
添加了开发环境 VAPID 密钥生成：
- 在初始化过程中询问是否生成 VAPID 密钥
- 使用 npx web-push 自动生成
- 自动写入 `.env` 文件
- 提供友好的错误处理

**新增功能**：
```bash
# 询问是否生成 VAPID 密钥（可选）
read -p "是否生成 Web Push VAPID 密钥？[y/N] " -r gen_vapid
if [[ $gen_vapid =~ ^[Yy]$ ]]; then
    # 使用 npx web-push 生成密钥
    # ...
fi
```

#### ✅ 新建 `scripts/generate-vapid-keys.sh`
创建了专用的 VAPID 密钥生成工具：
- 独立的密钥生成脚本
- 使用 npx web-push 生成密钥
- 支持自动写入 .env 文件
- 支持覆盖现有配置
- 提供详细的使用说明

**使用方法**：
```bash
./scripts/generate-vapid-keys.sh
```

### 3. Docker 配置更新

#### ✅ `docker-compose.prod.yml`
在 zinnia 服务的环境变量中添加：
```yaml
# Web Push VAPID 配置（可选）
VAPID_PUBLIC_KEY: ${VAPID_PUBLIC_KEY:-}
VAPID_PRIVATE_KEY: ${VAPID_PRIVATE_KEY:-}
```

### 4. 代码修复

#### ✅ `src/main.rs`
修复了 NotificationRepository 的所有权问题：

**问题**：
- `notification_repo` 被多次使用但没有正确的 Arc 包装
- `NotificationService::new` 和 `WebPushService::new` 都需要使用该 repo

**修复**：
```rust
// 修改前
let notification_repo = NotificationRepository::new((*pg_pool).clone());
// ...
let mut notification_service = NotificationService::new(
    notification_repo.clone(),  // ❌ 这里会 move
    (*device_repo).clone(),
    email_service.clone(),
);
let web_push_service_opt = match WebPushService::new(&settings, notification_repo) {
    // ❌ 这里再次使用已被 move 的值
};

// 修改后
let notification_repo = Arc::new(NotificationRepository::new((*pg_pool).clone()));
// ...
let mut notification_service = NotificationService::new(
    (*notification_repo).clone(),  // ✅ 正确 clone
    (*device_repo).clone(),
    email_service.clone(),
);
let web_push_service_opt = match WebPushService::new(&settings, notification_repo.clone()) {
    // ✅ 使用 Arc clone
};
```

## 环境变量说明

### 必需环境变量（核心功能）
```bash
DATABASE_URL=postgres://...
REDIS_URL=redis://...
JWT_SECRET=...
ENCRYPTION_KEY=...
```

### 可选环境变量（Web Push）
```bash
VAPID_PUBLIC_KEY=your_public_key_base64
VAPID_PRIVATE_KEY=your_private_key_base64
```

**注意**：
- 如果不配置 VAPID 密钥，Web Push 功能将被禁用
- 应用仍可正常运行，只是无法发送 Web Push 通知
- 日志中会显示警告："Web Push 服务初始化失败（需要配置 VAPID 密钥）"

## 生成 VAPID 密钥的方法

### 方法 1：使用专用脚本
```bash
./scripts/generate-vapid-keys.sh
```

### 方法 2：使用 npx
```bash
npx web-push generate-vapid-keys
```

### 方法 3：部署时自动生成
运行 `./scripts/deploy.sh`，在交互式配置中选择启用 Web Push

### 方法 4：开发环境初始化
运行 `./scripts/dev_init.sh`，在询问时选择生成 VAPID 密钥

## 部署检查清单

- [x] 更新 .env.example 文件
- [x] 更新 .env.production.example 文件
- [x] 更新 deploy.sh 脚本
- [x] 更新 dev_init.sh 脚本
- [x] 创建 generate-vapid-keys.sh 脚本
- [x] 更新 docker-compose.prod.yml
- [x] 修复 main.rs 中的所有权问题
- [x] 添加脚本可执行权限

## 向后兼容性

✅ **完全向后兼容**

- 不配置 VAPID 密钥时，应用正常运行
- Web Push API 端点会返回配置错误
- 其他通知渠道（邮件）不受影响
- 现有部署无需修改即可升级

## 测试建议

### 1. 开发环境测试
```bash
# 清理现有配置
rm -f .env

# 重新初始化
./scripts/dev_init.sh

# 选择生成 VAPID 密钥
# 启动应用
cargo run

# 测试 API
curl http://localhost:8080/api/v1/web-push/vapid-key
```

### 2. 生产环境部署
```bash
# 运行部署脚本
./scripts/deploy.sh

# 在 Web Push 配置环节选择启用
# 脚本会自动生成密钥并写入配置
# 继续完成部署流程
```

### 3. 手动配置
```bash
# 生成密钥
./scripts/generate-vapid-keys.sh

# 密钥会自动添加到 .env 或手动复制到环境变量
# 重启应用
```

## 安全注意事项

1. **保护 VAPID 私钥**
   - 不要提交到版本控制
   - 使用环境变量或密钥管理服务
   - 定期轮换密钥

2. **密钥格式**
   - 公钥和私钥都是 Base64 编码
   - 使用 URL-safe 编码（无填充）
   - 不要手动修改格式

3. **部署环境**
   - 生产环境必须配置 HTTPS
   - Web Push 要求安全上下文
   - 使用 SSL/TLS 证书

## 故障排查

### 问题 1：Web Push 服务未启动
**症状**：日志显示 "Web Push 服务初始化失败"

**解决方案**：
1. 检查是否配置了 VAPID_PUBLIC_KEY 和 VAPID_PRIVATE_KEY
2. 验证密钥格式是否正确（Base64 编码）
3. 确保没有多余的空格或换行符

### 问题 2：密钥生成失败
**症状**：generate-vapid-keys.sh 报错 "npx 未找到"

**解决方案**：
1. 安装 Node.js：https://nodejs.org/
2. 或使用在线工具：https://web-push-codelab.glitch.me/

### 问题 3：订阅失败
**症状**：前端订阅时返回 401 或 403

**解决方案**：
1. 确保使用 HTTPS（本地可用 localhost）
2. 检查浏览器是否支持 Push API
3. 验证公钥是否正确传递给前端

## 相关文档

- [Web Push 测试指南](./docs/WEB_PUSH_TESTING_GUIDE.md)
- [Web Push 实现总结](./docs/WEB_PUSH_IMPLEMENTATION_SUMMARY.md)
- [验证检查清单](./VERIFICATION_CHECKLIST.md)
- [通知功能总结](./docs/NOTIFICATION_FEATURE_SUMMARY.md)

## 后续优化建议

1. **密钥管理**
   - 考虑使用密钥管理服务（如 AWS KMS、HashiCorp Vault）
   - 实现密钥轮换机制
   - 添加密钥过期检查

2. **监控和告警**
   - 监控 Web Push 发送成功率
   - 记录订阅失败的原因
   - 设置推送失败告警

3. **性能优化**
   - 批量推送优化
   - 失败重试机制
   - 连接池管理

4. **用户体验**
   - 前端提供订阅状态提示
   - 允许用户管理订阅设备
   - 提供通知预览功能

