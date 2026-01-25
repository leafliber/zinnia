//! 路由配置模块

use crate::handlers;
use crate::middleware::{JwtAuth, JwtOrApiKeyAuth};
use actix_web::web;

/// 配置所有路由
///
/// 需要传入认证中间件实例
pub fn configure(
    cfg: &mut web::ServiceConfig,
    jwt_auth: JwtAuth,
    jwt_or_apikey_auth: JwtOrApiKeyAuth,
) {
    cfg
        // 健康检查路由（公开）
        .service(
            web::scope("/health")
                .route("", web::get().to(handlers::health))
                .route("/detailed", web::get().to(handlers::health_detailed))
                .route("/ready", web::get().to(handlers::ready))
                .route("/live", web::get().to(handlers::live)),
        )
        // API v1 路由
        .service(
            web::scope("/api/v1")
                // 认证路由（公开）
                .service(
                    web::scope("/auth")
                        .route("/token", web::post().to(handlers::authenticate))
                        .route("/exchange", web::post().to(handlers::authenticate)) // API Key → JWT 交换（推荐）
                        .route("/refresh", web::post().to(handlers::refresh_token))
                        .route("/revoke", web::post().to(handlers::revoke_token))
                        .route("/logout", web::post().to(handlers::logout))
                        // 验证相关路由（公开）
                        .route(
                            "/recaptcha/config",
                            web::get().to(handlers::get_recaptcha_config),
                        )
                        .route(
                            "/registration/config",
                            web::get().to(handlers::get_registration_security_config),
                        )
                        .route(
                            "/verification/send",
                            web::post().to(handlers::send_verification_code),
                        )
                        .route(
                            "/verification/verify",
                            web::post().to(handlers::verify_code),
                        )
                        .route(
                            "/password-reset/send",
                            web::post().to(handlers::send_password_reset_code),
                        )
                        .route(
                            "/password-reset/confirm",
                            web::post().to(handlers::confirm_password_reset),
                        ),
                )
                // 用户路由
                .service(
                    web::scope("/users")
                        // 公开路由
                        .route("/register", web::post().to(handlers::register))
                        .route("/login", web::post().to(handlers::login))
                        .route("/refresh", web::post().to(handlers::user_refresh_token))
                        // 需要认证的路由（使用 JWT 认证）
                        .service(
                            web::scope("")
                                .wrap(jwt_auth.clone())
                                .route("/logout", web::post().to(handlers::user_logout))
                                .route("/me", web::get().to(handlers::get_me))
                                .route("/me", web::put().to(handlers::update_me))
                                .route("/me/password", web::put().to(handlers::change_password))
                                .route("/logout-all", web::post().to(handlers::logout_all))
                                // 设备共享路由（需要认证）
                                .route(
                                    "/devices/{device_id}/share",
                                    web::post().to(handlers::share_device),
                                )
                                .route(
                                    "/devices/{device_id}/share/{target_user_id}",
                                    web::delete().to(handlers::remove_device_share),
                                )
                                .route(
                                    "/devices/{device_id}/shares",
                                    web::get().to(handlers::get_device_shares),
                                )
                                // 管理员路由
                                .route("", web::get().to(handlers::list_users))
                                .route("/{user_id}", web::get().to(handlers::get_user))
                                .route("/{user_id}", web::put().to(handlers::update_user))
                                .route("/{user_id}", web::delete().to(handlers::delete_user))
                                .route("/{user_id}/role", web::put().to(handlers::update_user_role))
                                .route(
                                    "/{user_id}/active",
                                    web::put().to(handlers::set_user_active),
                                ),
                        ),
                )
                // 电量路由（需要认证 - 支持 JWT 和 API Key）
                .service(
                    web::scope("/battery")
                        .wrap(jwt_or_apikey_auth.clone())
                        .route("/report", web::post().to(handlers::report_battery))
                        .route(
                            "/batch-report",
                            web::post().to(handlers::batch_report_battery),
                        )
                        .route(
                            "/latest/{device_id}",
                            web::get().to(handlers::get_latest_battery),
                        )
                        .route(
                            "/history/{device_id}",
                            web::get().to(handlers::get_battery_history),
                        )
                        .route(
                            "/aggregated/{device_id}",
                            web::get().to(handlers::get_battery_aggregated),
                        )
                        .route(
                            "/stats/{device_id}",
                            web::get().to(handlers::get_battery_stats),
                        ),
                )
                // 设备路由（需要认证/管理员权限）
                .service(
                    web::scope("/devices")
                        .wrap(jwt_auth.clone())
                        .route("", web::post().to(handlers::create_device))
                        .route("", web::get().to(handlers::list_devices))
                        .route("/{id}", web::get().to(handlers::get_device))
                        .route("/{id}", web::put().to(handlers::update_device))
                        .route("/{id}", web::delete().to(handlers::delete_device))
                        .route("/{id}/config", web::get().to(handlers::get_device_config))
                        .route(
                            "/{id}/config",
                            web::put().to(handlers::update_device_config),
                        )
                        .route(
                            "/{id}/rotate-key",
                            web::post().to(handlers::rotate_device_api_key),
                        )
                        // 设备访问令牌管理
                        .route(
                            "/{id}/tokens",
                            web::post().to(handlers::create_device_token),
                        )
                        .route("/{id}/tokens", web::get().to(handlers::list_device_tokens))
                        .route(
                            "/{id}/tokens",
                            web::delete().to(handlers::revoke_all_device_tokens),
                        )
                        .route(
                            "/{id}/tokens/{token_id}",
                            web::delete().to(handlers::revoke_device_token),
                        ),
                )
                // 兼容模式路由（无需请求头认证，通过 URL 参数认证）
                .service(
                    web::scope("/compat")
                        .route(
                            "/battery/report",
                            web::get().to(handlers::compat_report_battery),
                        )
                        .route(
                            "/battery/report",
                            web::post().to(handlers::compat_report_battery),
                        )
                        .route(
                            "/battery/simple",
                            web::get().to(handlers::compat_simple_report),
                        )
                        .route(
                            "/battery/latest",
                            web::get().to(handlers::compat_get_latest_battery),
                        )
                        .route("/ping", web::get().to(handlers::compat_ping)),
                )
                // 预警路由（需要认证）
                .service(
                    web::scope("/alerts")
                        .wrap(jwt_auth.clone())
                        .route("/rules", web::post().to(handlers::create_alert_rule))
                        .route("/rules", web::get().to(handlers::list_alert_rules))
                        .route("/rules/{id}", web::put().to(handlers::update_alert_rule))
                        .route("/rules/{id}", web::delete().to(handlers::delete_alert_rule))
                        .route("/events", web::get().to(handlers::list_alert_events))
                        .route(
                            "/events/{id}/acknowledge",
                            web::post().to(handlers::acknowledge_alert),
                        )
                        .route(
                            "/events/{id}/resolve",
                            web::post().to(handlers::resolve_alert),
                        )
                        .route(
                            "/events/{id}/status",
                            web::put().to(handlers::update_alert_status),
                        )
                        .route(
                            "/devices/{device_id}/count",
                            web::get().to(handlers::count_active_alerts),
                        ),
                )
                // 通知偏好路由（需要认证）
                .service(
                    web::scope("/notifications")
                        .wrap(jwt_auth.clone())
                        .route(
                            "/preferences",
                            web::get().to(handlers::get_notification_preference),
                        )
                        .route(
                            "/preferences",
                            web::put().to(handlers::update_notification_preference),
                        )
                        // Web Push 订阅管理
                        .route(
                            "/web-push/vapid-key",
                            web::get().to(handlers::get_vapid_public_key),
                        )
                        .route(
                            "/web-push/subscribe",
                            web::post().to(handlers::subscribe_web_push),
                        )
                        .route(
                            "/web-push/subscriptions",
                            web::get().to(handlers::list_web_push_subscriptions),
                        )
                        .route(
                            "/web-push/subscriptions/{id}",
                            web::delete().to(handlers::unsubscribe_web_push),
                        ),
                ),
        );
}
