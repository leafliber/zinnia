//! Cookie 工具模块
//!
//! 提供 httpOnly cookie 的设置、清除和读取功能

use actix_web::{
    cookie::{time::Duration, Cookie, SameSite},
    HttpRequest, HttpResponse,
};

/// Cookie 配置常量
pub const ACCESS_TOKEN_COOKIE: &str = "access_token";
pub const REFRESH_TOKEN_COOKIE: &str = "refresh_token";
pub const COOKIE_PATH: &str = "/";
pub const COOKIE_DOMAIN: Option<&str> = None; // 生产环境应设置为实际域名

/// Access Token Cookie 配置
///
/// - httpOnly: JavaScript 无法访问，防止 XSS 攻击
/// - secure: 仅通过 HTTPS 传输（生产环境）
/// - same_site: CSRF 保护
/// - max_age: 15 分钟（与 access token 过期时间一致）
pub const ACCESS_TOKEN_MAX_AGE: Duration = Duration::seconds(900); // 15 分钟

/// Refresh Token Cookie 配置
///
/// - httpOnly: JavaScript 无法访问，防止 XSS 攻击
/// - secure: 仅通过 HTTPS 传输（生产环境）
/// - same_site: CSRF 保护
/// - max_age: 7 天（与 refresh token 过期时间一致）
pub const REFRESH_TOKEN_MAX_AGE: Duration = Duration::days(7); // 7 天

/// Cookie 构建器
#[derive(Clone)]
pub struct CookieBuilder {
    pub http_only: bool,
    pub secure: bool,
    pub same_site: SameSite,
    pub path: &'static str,
}

impl Default for CookieBuilder {
    fn default() -> Self {
        Self {
            http_only: true,
            secure: cfg!(not(debug_assertions)), // 生产环境启用 secure
            same_site: SameSite::Lax, // Lax 允许同站导航，比 Strict 更宽松但仍提供 CSRF 保护
            path: COOKIE_PATH,
        }
    }
}

impl CookieBuilder {
    /// 创建开发环境配置
    pub fn development() -> Self {
        Self {
            http_only: true,
            secure: false, // 开发环境不强制 HTTPS
            same_site: SameSite::Lax,
            path: COOKIE_PATH,
        }
    }

    /// 创建生产环境配置
    pub fn production() -> Self {
        Self {
            http_only: true,
            secure: true,                // 生产环境必须 HTTPS
            same_site: SameSite::Strict, // Strict 更严格
            path: COOKIE_PATH,
        }
    }

    /// 设置自定义 domain
    pub fn with_domain(self, domain: &'static str) -> CookieWithDomain {
        CookieWithDomain {
            builder: self,
            domain: Some(domain),
        }
    }

    /// 构建 access token cookie
    pub fn build_access_cookie(&self, token: &str) -> Cookie<'static> {
        let mut cookie = Cookie::new(ACCESS_TOKEN_COOKIE, token.to_string());
        cookie.set_http_only(self.http_only);
        cookie.set_secure(self.secure);
        cookie.set_same_site(self.same_site);
        cookie.set_path(self.path);
        cookie.set_max_age(ACCESS_TOKEN_MAX_AGE);

        if let Some(domain) = COOKIE_DOMAIN {
            cookie.set_domain(domain);
        }

        cookie
    }

    /// 构建 refresh token cookie
    pub fn build_refresh_cookie(&self, token: &str) -> Cookie<'static> {
        let mut cookie = Cookie::new(REFRESH_TOKEN_COOKIE, token.to_string());
        cookie.set_http_only(self.http_only);
        cookie.set_secure(self.secure);
        cookie.set_same_site(self.same_site);
        cookie.set_path(self.path);
        cookie.set_max_age(REFRESH_TOKEN_MAX_AGE);

        if let Some(domain) = COOKIE_DOMAIN {
            cookie.set_domain(domain);
        }

        cookie
    }

    /// 构建过期的 access token cookie（用于清除）
    pub fn build_clear_access_cookie(&self) -> Cookie<'static> {
        let mut cookie = Cookie::new(ACCESS_TOKEN_COOKIE, "");
        cookie.set_http_only(self.http_only);
        cookie.set_secure(self.secure);
        cookie.set_same_site(self.same_site);
        cookie.set_path(self.path);
        cookie.set_max_age(Duration::ZERO);

        if let Some(domain) = COOKIE_DOMAIN {
            cookie.set_domain(domain);
        }

        cookie
    }

    /// 构建过期的 refresh token cookie（用于清除）
    pub fn build_clear_refresh_cookie(&self) -> Cookie<'static> {
        let mut cookie = Cookie::new(REFRESH_TOKEN_COOKIE, "");
        cookie.set_http_only(self.http_only);
        cookie.set_secure(self.secure);
        cookie.set_same_site(self.same_site);
        cookie.set_path(self.path);
        cookie.set_max_age(Duration::ZERO);

        if let Some(domain) = COOKIE_DOMAIN {
            cookie.set_domain(domain);
        }

        cookie
    }
}

/// 带 Domain 的 Cookie 构建器
pub struct CookieWithDomain {
    builder: CookieBuilder,
    domain: Option<&'static str>,
}

impl CookieWithDomain {
    /// 构建 access token cookie
    pub fn build_access_cookie(&self, token: &str) -> Cookie<'static> {
        let mut cookie = Cookie::new(ACCESS_TOKEN_COOKIE, token.to_string());
        cookie.set_http_only(self.builder.http_only);
        cookie.set_secure(self.builder.secure);
        cookie.set_same_site(self.builder.same_site);
        cookie.set_path(self.builder.path);
        cookie.set_max_age(ACCESS_TOKEN_MAX_AGE);

        if let Some(domain) = self.domain {
            cookie.set_domain(domain);
        }

        cookie
    }

    /// 构建 refresh token cookie
    pub fn build_refresh_cookie(&self, token: &str) -> Cookie<'static> {
        let mut cookie = Cookie::new(REFRESH_TOKEN_COOKIE, token.to_string());
        cookie.set_http_only(self.builder.http_only);
        cookie.set_secure(self.builder.secure);
        cookie.set_same_site(self.builder.same_site);
        cookie.set_path(self.builder.path);
        cookie.set_max_age(REFRESH_TOKEN_MAX_AGE);

        if let Some(domain) = self.domain {
            cookie.set_domain(domain);
        }

        cookie
    }

    /// 构建过期的 access token cookie（用于清除）
    pub fn build_clear_access_cookie(&self) -> Cookie<'static> {
        let mut cookie = Cookie::new(ACCESS_TOKEN_COOKIE, "");
        cookie.set_http_only(self.builder.http_only);
        cookie.set_secure(self.builder.secure);
        cookie.set_same_site(self.builder.same_site);
        cookie.set_path(self.builder.path);
        cookie.set_max_age(Duration::ZERO);

        if let Some(domain) = self.domain {
            cookie.set_domain(domain);
        }

        cookie
    }

    /// 构建过期的 refresh token cookie（用于清除）
    pub fn build_clear_refresh_cookie(&self) -> Cookie<'static> {
        let mut cookie = Cookie::new(REFRESH_TOKEN_COOKIE, "");
        cookie.set_http_only(self.builder.http_only);
        cookie.set_secure(self.builder.secure);
        cookie.set_same_site(self.builder.same_site);
        cookie.set_path(self.builder.path);
        cookie.set_max_age(Duration::ZERO);

        if let Some(domain) = self.domain {
            cookie.set_domain(domain);
        }

        cookie
    }
}

/// 设置认证 cookie 到 HTTP 响应
///
/// # 参数
/// - `res`: HTTP 响应
/// - `access_token`: Access token
/// - `refresh_token`: Refresh token
///
/// # 返回
/// 带有 cookie 的 HTTP 响应
pub fn set_auth_cookies(
    mut res: HttpResponse,
    access_token: &str,
    refresh_token: &str,
) -> HttpResponse {
    let builder = if cfg!(debug_assertions) {
        CookieBuilder::development()
    } else {
        CookieBuilder::production()
    };

    let access_cookie = builder.build_access_cookie(access_token);
    let refresh_cookie = builder.build_refresh_cookie(refresh_token);

    res.add_cookie(&access_cookie)
        .expect("failed to add access cookie");
    res.add_cookie(&refresh_cookie)
        .expect("failed to add refresh cookie");

    res
}

/// 清除认证 cookie
///
/// # 参数
/// - `res`: HTTP 响应
///
/// # 返回
/// 带有清除 cookie 指令的 HTTP 响应
pub fn clear_auth_cookies(mut res: HttpResponse) -> HttpResponse {
    let builder = if cfg!(debug_assertions) {
        CookieBuilder::development()
    } else {
        CookieBuilder::production()
    };

    let clear_access_cookie = builder.build_clear_access_cookie();
    let clear_refresh_cookie = builder.build_clear_refresh_cookie();

    res.add_cookie(&clear_access_cookie)
        .expect("failed to add clear access cookie");
    res.add_cookie(&clear_refresh_cookie)
        .expect("failed to add clear refresh cookie");

    res
}

/// 从请求中提取 access token
///
/// 优先级：
/// 1. Authorization header (Bearer token)
/// 2. Cookie (access_token)
pub fn extract_access_token(req: &HttpRequest) -> Option<String> {
    // 首先尝试从 Authorization header 获取
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(header_str) = auth_header.to_str() {
            if let Some(token) = header_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }

    // 然后尝试从 cookie 获取
    if let Some(cookie_header) = req.headers().get("Cookie") {
        if let Ok(cookie_str) = cookie_header.to_str() {
            // 解析 cookie 字符串
            for pair in cookie_str.split(';') {
                let pair = pair.trim();
                if pair.starts_with(&format!("{}=", ACCESS_TOKEN_COOKIE)) {
                    let token = pair[(ACCESS_TOKEN_COOKIE.len() + 1)..].to_string();
                    return Some(token);
                }
            }
        }
    }

    None
}

/// 从请求中提取 refresh token
///
/// 仅从 cookie 中获取 refresh token（refresh token 不应通过 header 传递）
pub fn extract_refresh_token(req: &HttpRequest) -> Option<String> {
    if let Some(cookie_header) = req.headers().get("Cookie") {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for pair in cookie_str.split(';') {
                let pair = pair.trim();
                if pair.starts_with(&format!("{}=", REFRESH_TOKEN_COOKIE)) {
                    let token = pair[(REFRESH_TOKEN_COOKIE.len() + 1)..].to_string();
                    return Some(token);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::{header::SET_COOKIE, StatusCode};

    #[test]
    fn test_cookie_builder_development() {
        let builder = CookieBuilder::development();
        assert!(builder.http_only);
        assert!(!builder.secure);
        assert_eq!(builder.same_site, SameSite::Lax);
    }

    #[test]
    fn test_cookie_builder_production() {
        let builder = CookieBuilder::production();
        assert!(builder.http_only);
        assert!(builder.secure);
        assert_eq!(builder.same_site, SameSite::Strict);
    }

    #[test]
    fn test_set_and_extract_tokens() {
        let access_token = "test_access_token_123";
        let refresh_token = "test_refresh_token_456";

        // 创建响应并设置 cookie
        let res = HttpResponse::Ok().body("test");
        let res = set_auth_cookies(res, access_token, refresh_token);

        // 验证响应状态码
        assert_eq!(res.status(), StatusCode::OK);

        // 验证 cookie 已添加
        let cookies = res.headers().get_all(SET_COOKIE);
        assert_eq!(cookies.count(), 2);
    }

    #[test]
    fn test_clear_auth_cookies() {
        let res = HttpResponse::Ok().body("test");
        let res = clear_auth_cookies(res);

        let cookies = res.headers().get_all(SET_COOKIE);
        assert_eq!(cookies.count(), 2);
    }
}
