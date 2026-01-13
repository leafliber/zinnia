//! 配置管理模块

mod settings;

pub use settings::{
	Settings,
	ServerSettings,
	DatabaseSettings,
	RedisSettings,
	JwtSettings,
	RateLimitSettings,
	LoggingSettings,
	SmtpSettings,
	RecaptchaSettings,
	RegistrationSettings,
};
