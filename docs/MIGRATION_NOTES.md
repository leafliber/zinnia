# 迁移笔记

## actix-web-actors 弃用迁移计划

### 背景

`actix-web-actors` 包已被标记为弃用（deprecated），推荐迁移到 `actix-ws`。

当前版本：`actix-web-actors = "4.3.1+deprecated"`

### 影响范围

以下文件使用了 actix-web-actors：

- `src/websocket/handler.rs` - WebSocket 路由处理器
- `src/websocket/session.rs` - WebSocket Session Actor
- `src/websocket/messages.rs` - WebSocket 消息定义

### 迁移方案

#### 方案 A：迁移到 actix-ws（推荐）

`actix-ws` 是 actix-web 团队推荐的替代方案，提供更简洁的 API。

**优点**:
- 官方推荐
- API 更简洁
- 持续维护

**缺点**:
- 需要重写 WebSocket 逻辑
- 丢失 Actor 模型的自动状态管理

**工作量**: ~2-3 天

#### 方案 B：迁移到 tokio-tungstenite

直接使用 tokio 生态的 WebSocket 库。

**优点**:
- 纯 tokio 生态
- 不依赖 actix Actor 系统

**缺点**:
- 需要更多手动状态管理
- 与 actix-web 集成需要额外代码

**工作量**: ~3-4 天

### 迁移步骤（方案 A）

1. **添加依赖**
   ```toml
   actix-ws = "0.3"
   ```

2. **重构 handler.rs**
   ```rust
   use actix_ws::Message;
   
   pub async fn ws_handler(
       req: HttpRequest,
       body: web::Payload,
   ) -> Result<HttpResponse, Error> {
       let (response, session, stream) = actix_ws::handle(&req, body)?;
       
       // 手动处理消息流
       actix_web::rt::spawn(handle_ws_messages(session, stream));
       
       Ok(response)
   }
   ```

3. **重构 session.rs**
   - 将 Actor 模式改为异步任务模式
   - 使用 tokio channels 管理状态

4. **更新测试**

### 时间表

| 阶段 | 任务 | 预计时间 |
|------|------|----------|
| Phase 1 | 评估和设计 | 0.5 天 |
| Phase 2 | 核心逻辑迁移 | 1.5 天 |
| Phase 3 | 测试和修复 | 1 天 |
| Phase 4 | 文档更新 | 0.5 天 |

### 当前状态

⏳ **待处理** - 当前代码功能正常，弃用警告不影响运行。可在下一个大版本迭代时处理。

### 参考资料

- [actix-ws 文档](https://docs.rs/actix-ws)
- [actix-web-actors 弃用公告](https://github.com/actix/actix-web/issues/2960)
- [WebSocket 迁移示例](https://github.com/actix/examples/tree/master/websockets)
