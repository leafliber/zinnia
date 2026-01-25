//! Zinnia - 高性能时间序列后端服务
//!
//! 设备电量监控与预警系统

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use zinnia::{
    config::Settings,
    db::{PostgresPool, RedisPool},
    middleware::{JwtAuth, JwtOrApiKeyAuth, RequestLogger, RequestValidator, SecurityHeaders},
    repositories::{
        AlertRepository, BatteryRepository, DeviceAccessTokenRepository, DeviceRepository,
        NotificationRepository, UserRepository,
    },
    routes,
    security::{JwtManager, Secrets},
    services::{
        AlertService, AuthService, BatteryService, CacheService, DeviceAccessTokenService,
        DeviceService, EmailService, NotificationService, RecaptchaService,
        RegistrationSecurityService, UserService, VerificationService, WebPushService,
    },
    websocket,
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 加载环境变量
    dotenvy::dotenv().ok();

    // 初始化日志
    init_tracing();

    info!("🌱 Zinnia 服务启动中...");

    // 加载配置
    let settings = match Settings::load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("❌ 配置加载失败: {}", e);
            std::process::exit(1);
        }
    };
    info!("✅ 配置加载完成");

    // 初始化密钥
    match Secrets::init() {
        Ok(_) => {}
        Err(e) => {
            eprintln!("❌ 密钥初始化失败: {}", e);
            std::process::exit(1);
        }
    };
    info!("✅ 密钥初始化完成");

    // 连接数据库
    let pg_pool = Arc::new(match PostgresPool::new(&settings).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ 数据库连接失败: {}", e);
            std::process::exit(1);
        }
    });
    info!("✅ 数据库连接成功");

    // 连接 Redis
    let redis_pool = Arc::new(match RedisPool::new(&settings).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("❌ Redis 连接失败: {}", e);
            std::process::exit(1);
        }
    });
    info!("✅ Redis 连接成功");

    // 初始化 JWT 管理器
    let jwt_manager = Arc::new(match JwtManager::new(&settings) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("❌ JWT 初始化失败: {}", e);
            std::process::exit(1);
        }
    });

    // 初始化仓库
    let device_repo = Arc::new(DeviceRepository::new((*pg_pool).clone()));
    let battery_repo = BatteryRepository::new((*pg_pool).clone());
    let alert_repo = AlertRepository::new((*pg_pool).clone());
    let user_repo = UserRepository::new((*pg_pool).clone());
    let device_token_repo = DeviceAccessTokenRepository::new((*pg_pool).clone());
    let notification_repo = Arc::new(NotificationRepository::new((*pg_pool).clone()));

    // 初始化服务
    let cache_service = Arc::new(CacheService::new(redis_pool.clone()));
    let mut alert_service = AlertService::new(alert_repo);
    let device_service = Arc::new(DeviceService::new(
        (*device_repo).clone(),
        redis_pool.clone(),
    ));

    let user_service = Arc::new(UserService::new(
        user_repo,
        jwt_manager.clone(),
        redis_pool.clone(),
    ));
    let auth_service = Arc::new(AuthService::new(
        jwt_manager.clone(),
        device_service.clone(),
        cache_service.clone(),
    ));
    let device_token_service = Arc::new(DeviceAccessTokenService::new(
        device_token_repo,
        device_repo.clone(),
        redis_pool.clone(),
    ));

    // 初始化注册安全服务
    let email_service = Arc::new(match EmailService::new(&settings, redis_pool.clone()) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("❌ 邮件服务初始化失败: {}", err);
            std::process::exit(1);
        }
    });
    let verification_service = Arc::new(VerificationService::new(
        redis_pool.clone(),
        email_service.clone(),
        &settings,
    ));
    let recaptcha_service = Arc::new(RecaptchaService::new(&settings));
    let registration_security_service = Arc::new(RegistrationSecurityService::new(
        redis_pool.clone(),
        &settings,
    ));

    // 初始化通知服务
    let mut notification_service = NotificationService::new(
        (*notification_repo).clone(),
        (*device_repo).clone(),
        email_service.clone(),
    );

    // 尝试初始化 Web Push 服务（需要 VAPID 密钥）
    let web_push_service_opt = match WebPushService::new(&settings, notification_repo.clone()) {
        Ok(service) => {
            let service = Arc::new(service);
            notification_service.set_web_push_service(service.clone());
            info!("✅ Web Push 服务初始化完成");
            Some(service)
        }
        Err(e) => {
            tracing::warn!(error = %e, "Web Push 服务初始化失败（需要配置 VAPID 密钥）");
            None
        }
    };

    let notification_service = Arc::new(notification_service);

    // 设置 AlertService 的通知服务（避免循环依赖）
    alert_service.set_notification_service(notification_service.clone());
    let alert_service = Arc::new(alert_service);

    // 现在初始化 BatteryService（需要 alert_service 的 Arc）
    let battery_service = Arc::new(BatteryService::new(
        battery_repo,
        (*device_repo).clone(),
        alert_service.clone(),
        redis_pool.clone(),
    ));

    info!("✅ 安全服务初始化完成");

    let server_addr = settings.server_addr();
    let workers = if settings.server.workers == 0 {
        num_cpus::get()
    } else {
        settings.server.workers
    };

    info!("🚀 服务启动在 http://{}", server_addr);
    info!("📊 工作线程数: {}", workers);

    // 启动 HTTP 服务器
    HttpServer::new(move || {
        // 配置 CORS
        let cors = Cors::default()
            .allowed_origin_fn(|origin, _req_head| {
                // 开发环境允许所有来源，生产环境应配置白名单
                origin.as_bytes().starts_with(b"http://localhost")
                    || origin.as_bytes().starts_with(b"https://")
            })
            .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE"])
            .allowed_headers(vec![
                "Authorization",
                "Content-Type",
                "X-API-Key",
                "X-Request-ID",
            ])
            .expose_headers(vec!["Set-Cookie"])
            .allow_any_header()
            .supports_credentials() // 允许携带凭证（cookie）
            .max_age(3600);

        // 创建认证中间件实例
        let jwt_auth = JwtAuth::new(jwt_manager.clone(), redis_pool.clone());
        let jwt_or_apikey_auth = JwtOrApiKeyAuth::new(
            jwt_manager.clone(),
            redis_pool.clone(),
            device_service.clone(),
        );

        App::new()
            // 全局中间件
            .wrap(cors)
            .wrap(SecurityHeaders::new())
            .wrap(RequestLogger::new())
            .wrap(RequestValidator::default())
            .wrap(middleware::Compress::default())
            // 注入服务
            .app_data(web::Data::new(pg_pool.clone()))
            .app_data(web::Data::new(redis_pool.clone()))
            .app_data(web::Data::new(jwt_manager.clone()))
            .app_data(web::Data::new(device_repo.clone()))
            .app_data(web::Data::new(device_service.clone()))
            .app_data(web::Data::new(battery_service.clone()))
            .app_data(web::Data::new(alert_service.clone()))
            .app_data(web::Data::new(auth_service.clone()))
            .app_data(web::Data::new(cache_service.clone()))
            .app_data(web::Data::new(user_service.clone()))
            .app_data(web::Data::new(device_token_service.clone()))
            .app_data(web::Data::new(email_service.clone()))
            .app_data(web::Data::new(verification_service.clone()))
            .app_data(web::Data::new(recaptcha_service.clone()))
            .app_data(web::Data::new(registration_security_service.clone()))
            .app_data(web::Data::new(notification_service.clone()))
            .app_data(web::Data::new(web_push_service_opt.clone()))
            // 配置 HTTP 路由
            .configure(|cfg| routes::configure(cfg, jwt_auth.clone(), jwt_or_apikey_auth.clone()))
            // 配置 WebSocket 路由
            .configure(websocket::configure_ws_routes)
    })
    .workers(workers)
    .bind(&server_addr)?
    .run()
    .await
}

/// 初始化日志系统
fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,zinnia=debug"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();
}
