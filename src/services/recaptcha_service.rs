//! reCAPTCHA 验证服务模块
//!
//! 提供 Google reCAPTCHA v2/v3 验证功能

use crate::config::{RecaptchaSettings, Settings};
use crate::errors::AppError;
use reqwest::Client;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};

/// Google reCAPTCHA 验证响应
#[derive(Debug, Deserialize)]
struct RecaptchaResponse {
    /// 验证是否成功
    success: bool,
    /// 验证分数 (0.0 - 1.0，仅 v3)
    #[serde(default)]
    score: Option<f64>,
    /// 操作名称 (仅 v3)
    #[serde(default)]
    action: Option<String>,
    /// 错误代码
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
    /// 主机名
    #[serde(default)]
    hostname: Option<String>,
}

/// reCAPTCHA 验证结果
#[derive(Debug, Clone, Serialize)]
pub struct RecaptchaVerifyResult {
    pub success: bool,
    pub score: Option<f64>,
    pub action: Option<String>,
}

/// reCAPTCHA 服务
pub struct RecaptchaService {
    client: Client,
    secret_key: Option<String>,
    settings: RecaptchaSettings,
}

impl RecaptchaService {
    /// 创建新的 reCAPTCHA 服务实例
    pub fn new(settings: &Settings) -> Self {
        let secret_key = if settings.recaptcha.enabled {
            Settings::recaptcha_secret_key().map(|s| s.expose_secret().clone())
        } else {
            None
        };

        if settings.recaptcha.enabled && secret_key.is_none() {
            tracing::warn!("reCAPTCHA 已启用但 RECAPTCHA_SECRET_KEY 未设置");
        }

        Self {
            client: Client::new(),
            secret_key,
            settings: settings.recaptcha.clone(),
        }
    }

    /// 检查 reCAPTCHA 是否启用
    pub fn is_enabled(&self) -> bool {
        self.settings.enabled && self.secret_key.is_some()
    }

    /// 获取站点密钥（供前端使用）
    pub fn get_site_key(&self) -> Option<&str> {
        if self.settings.enabled {
            Some(&self.settings.site_key)
        } else {
            None
        }
    }

    /// 验证 reCAPTCHA 响应
    pub async fn verify(
        &self,
        response_token: &str,
        remote_ip: Option<&str>,
    ) -> Result<RecaptchaVerifyResult, AppError> {
        // 如果未启用，直接返回成功
        if !self.is_enabled() {
            tracing::debug!("reCAPTCHA 未启用，跳过验证");
            return Ok(RecaptchaVerifyResult {
                success: true,
                score: None,
                action: None,
            });
        }

        let secret = self
            .secret_key
            .as_ref()
            .ok_or_else(|| AppError::ConfigError("reCAPTCHA 密钥未配置".to_string()))?;

        // 构建请求参数
        let mut params = vec![("secret", secret.as_str()), ("response", response_token)];

        if let Some(ip) = remote_ip {
            params.push(("remoteip", ip));
        }

        // 发送验证请求
        let response = self
            .client
            .post("https://www.google.com/recaptcha/api/siteverify")
            .form(&params)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "reCAPTCHA 验证请求失败");
                AppError::InternalError("验证服务暂时不可用".to_string())
            })?;

        let recaptcha_response: RecaptchaResponse = response.json().await.map_err(|e| {
            tracing::error!(error = %e, "reCAPTCHA 响应解析失败");
            AppError::InternalError("验证服务响应异常".to_string())
        })?;

        // 检查验证结果
        if !recaptcha_response.success {
            tracing::warn!(
                error_codes = ?recaptcha_response.error_codes,
                "reCAPTCHA 验证失败"
            );
            return Err(AppError::ValidationError(
                "人机验证失败，请重试".to_string(),
            ));
        }

        // 对于 v3，检查分数
        if let Some(score) = recaptcha_response.score {
            if score < self.settings.score_threshold {
                tracing::warn!(
                    score = score,
                    threshold = self.settings.score_threshold,
                    "reCAPTCHA 分数过低"
                );
                return Err(AppError::ValidationError(
                    "安全验证未通过，请重试".to_string(),
                ));
            }
        }

        tracing::debug!(
            success = recaptcha_response.success,
            score = ?recaptcha_response.score,
            hostname = ?recaptcha_response.hostname,
            "reCAPTCHA 验证成功"
        );

        Ok(RecaptchaVerifyResult {
            success: true,
            score: recaptcha_response.score,
            action: recaptcha_response.action,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recaptcha_disabled() {
        // 当禁用时，应该直接返回成功
        let settings = Settings {
            server: crate::config::ServerSettings {
                host: "127.0.0.1".to_string(),
                port: 8080,
                workers: 1,
            },
            database: crate::config::DatabaseSettings {
                max_connections: 10,
                min_connections: 1,
                connect_timeout_seconds: 30,
                idle_timeout_seconds: 600,
                require_ssl: false,
            },
            redis: crate::config::RedisSettings {
                pool_size: 10,
                connect_timeout_seconds: 5,
            },
            jwt: crate::config::JwtSettings {
                expiry_seconds: 900,
                refresh_expiry_days: 7,
                issuer: "zinnia".to_string(),
                audience: "zinnia".to_string(),
            },
            rate_limit: crate::config::RateLimitSettings {
                requests_per_minute: 60,
                burst_size: 10,
                login_attempts_per_minute: 5,
            },
            logging: crate::config::LoggingSettings {
                level: "info".to_string(),
                format: "json".to_string(),
            },
            smtp: Default::default(),
            recaptcha: RecaptchaSettings {
                enabled: false,
                site_key: String::new(),
                score_threshold: 0.5,
            },
            registration: Default::default(),
        };

        let service = RecaptchaService::new(&settings);
        assert!(!service.is_enabled());
    }
}
