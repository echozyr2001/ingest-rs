//! Security middleware and utilities
//!
//! This module provides security enhancements including CORS, security headers,
//! input validation, and other security hardening measures.

use axum::{
    extract::Request,
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use std::collections::HashSet;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{debug, warn};

/// Security configuration
#[derive(Clone, Debug)]
pub struct SecurityConfig {
    /// CORS configuration
    pub cors: CorsConfig,
    /// Security headers configuration
    pub headers: SecurityHeadersConfig,
    /// Content Security Policy
    pub csp: Option<String>,
    /// Whether to enable security headers
    pub enable_security_headers: bool,
    /// Maximum request body size in bytes
    pub max_request_size: usize,
    /// Allowed file upload types
    pub allowed_upload_types: HashSet<String>,
}

/// CORS configuration
#[derive(Clone, Debug)]
pub struct CorsConfig {
    /// Allowed origins
    pub allowed_origins: Vec<String>,
    /// Allowed methods
    pub allowed_methods: Vec<Method>,
    /// Allowed headers
    pub allowed_headers: Vec<String>,
    /// Whether to allow credentials
    pub allow_credentials: bool,
    /// Max age for preflight requests
    pub max_age: Option<std::time::Duration>,
}

/// Security headers configuration
#[derive(Clone, Debug)]
pub struct SecurityHeadersConfig {
    /// X-Frame-Options header value
    pub frame_options: Option<String>,
    /// X-Content-Type-Options header value
    pub content_type_options: Option<String>,
    /// X-XSS-Protection header value
    pub xss_protection: Option<String>,
    /// Strict-Transport-Security header value
    pub hsts: Option<String>,
    /// Referrer-Policy header value
    pub referrer_policy: Option<String>,
    /// Permissions-Policy header value
    pub permissions_policy: Option<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            cors: CorsConfig::default(),
            headers: SecurityHeadersConfig::default(),
            csp: Some(
                "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self'; connect-src 'self'; frame-ancestors 'none';"
                    .to_string(),
            ),
            enable_security_headers: true,
            max_request_size: 10 * 1024 * 1024, // 10MB
            allowed_upload_types: [
                "application/json".to_string(),
                "text/plain".to_string(),
                "application/octet-stream".to_string(),
            ]
            .into_iter()
            .collect(),
        }
    }
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["http://localhost:3000".to_string()],
            allowed_methods: vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::PATCH,
                Method::OPTIONS,
            ],
            allowed_headers: vec![
                "authorization".to_string(),
                "content-type".to_string(),
                "x-api-key".to_string(),
                "x-requested-with".to_string(),
            ],
            allow_credentials: true,
            max_age: Some(std::time::Duration::from_secs(3600)),
        }
    }
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            frame_options: Some("DENY".to_string()),
            content_type_options: Some("nosniff".to_string()),
            xss_protection: Some("1; mode=block".to_string()),
            hsts: Some("max-age=31536000; includeSubDomains; preload".to_string()),
            referrer_policy: Some("strict-origin-when-cross-origin".to_string()),
            permissions_policy: Some(
                "camera=(), microphone=(), geolocation=(), payment=()".to_string(),
            ),
        }
    }
}

/// Create CORS layer from configuration
pub fn create_cors_layer(config: &CorsConfig) -> CorsLayer {
    let origins = if config.allowed_origins.contains(&"*".to_string()) {
        AllowOrigin::any()
    } else {
        AllowOrigin::list(
            config
                .allowed_origins
                .iter()
                .filter_map(|origin| origin.parse().ok())
                .collect::<Vec<_>>(),
        )
    };

    let mut cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods(config.allowed_methods.clone())
        .allow_credentials(config.allow_credentials);

    // Add allowed headers
    if !config.allowed_headers.is_empty() {
        cors = cors.allow_headers(
            config
                .allowed_headers
                .iter()
                .filter_map(|header| header.parse().ok())
                .collect::<Vec<HeaderName>>(),
        );
    }

    // Set max age
    if let Some(max_age) = config.max_age {
        cors = cors.max_age(max_age);
    }

    cors
}

/// Security headers middleware
pub async fn security_headers_middleware(
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    let mut response = next.run(request).await;

    // Add security headers
    add_security_headers(response.headers_mut());

    Ok(response)
}

/// Add security headers to response
fn add_security_headers(headers: &mut HeaderMap) {
    let config = SecurityHeadersConfig::default();

    // X-Frame-Options
    if let Some(frame_options) = &config.frame_options {
        if let Ok(value) = HeaderValue::from_str(frame_options) {
            headers.insert("X-Frame-Options", value);
        }
    }

    // X-Content-Type-Options
    if let Some(content_type_options) = &config.content_type_options {
        if let Ok(value) = HeaderValue::from_str(content_type_options) {
            headers.insert("X-Content-Type-Options", value);
        }
    }

    // X-XSS-Protection
    if let Some(xss_protection) = &config.xss_protection {
        if let Ok(value) = HeaderValue::from_str(xss_protection) {
            headers.insert("X-XSS-Protection", value);
        }
    }

    // Strict-Transport-Security
    if let Some(hsts) = &config.hsts {
        if let Ok(value) = HeaderValue::from_str(hsts) {
            headers.insert("Strict-Transport-Security", value);
        }
    }

    // Referrer-Policy
    if let Some(referrer_policy) = &config.referrer_policy {
        if let Ok(value) = HeaderValue::from_str(referrer_policy) {
            headers.insert("Referrer-Policy", value);
        }
    }

    // Permissions-Policy
    if let Some(permissions_policy) = &config.permissions_policy {
        if let Ok(value) = HeaderValue::from_str(permissions_policy) {
            headers.insert("Permissions-Policy", value);
        }
    }

    // Content-Security-Policy
    let default_csp = "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self'; connect-src 'self'; frame-ancestors 'none';";
    if let Ok(value) = HeaderValue::from_str(default_csp) {
        headers.insert("Content-Security-Policy", value);
    }

    // Additional security headers
    headers.insert("X-Powered-By", HeaderValue::from_static("Inngest"));
    headers.insert("Server", HeaderValue::from_static("Inngest/1.0"));
}

/// Input validation middleware
pub async fn input_validation_middleware(
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    // Validate request size
    if let Some(content_length) = request.headers().get("content-length") {
        if let Ok(length_str) = content_length.to_str() {
            if let Ok(length) = length_str.parse::<usize>() {
                let max_size = 10 * 1024 * 1024; // 10MB
                if length > max_size {
                    warn!(
                        content_length = length,
                        max_size = max_size,
                        "Request body too large"
                    );
                    return Err(StatusCode::PAYLOAD_TOO_LARGE);
                }
            }
        }
    }

    // Validate content type for POST/PUT requests
    if matches!(
        request.method(),
        &Method::POST | &Method::PUT | &Method::PATCH
    ) {
        if let Some(content_type) = request.headers().get("content-type") {
            if let Ok(content_type_str) = content_type.to_str() {
                if !is_allowed_content_type(content_type_str) {
                    warn!(content_type = content_type_str, "Unsupported content type");
                    return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
                }
            }
        } else {
            // Require content-type for POST/PUT requests
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Validate request headers
    validate_request_headers(request.headers())?;

    debug!("Input validation passed");
    Ok(next.run(request).await)
}

/// Check if content type is allowed
fn is_allowed_content_type(content_type: &str) -> bool {
    let allowed_types = [
        "application/json",
        "application/x-www-form-urlencoded",
        "multipart/form-data",
        "text/plain",
        "application/octet-stream",
    ];

    allowed_types
        .iter()
        .any(|&allowed| content_type.starts_with(allowed))
}

/// Validate request headers for security issues
fn validate_request_headers(headers: &HeaderMap) -> std::result::Result<(), StatusCode> {
    // Check for suspicious headers
    let suspicious_headers = ["x-forwarded-host", "x-original-url", "x-rewrite-url"];

    for suspicious in &suspicious_headers {
        if headers.contains_key(*suspicious) {
            warn!(header = suspicious, "Suspicious header detected");
            // Log but don't block - these might be legitimate in some setups
        }
    }

    // Validate User-Agent header
    if let Some(user_agent) = headers.get("user-agent") {
        if let Ok(ua_str) = user_agent.to_str() {
            if is_suspicious_user_agent(ua_str) {
                warn!(user_agent = ua_str, "Suspicious user agent detected");
                return Err(StatusCode::FORBIDDEN);
            }
        }
    }

    // Validate Authorization header format
    if let Some(auth) = headers.get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if !is_valid_authorization_header(auth_str) {
                warn!("Invalid authorization header format");
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    }

    Ok(())
}

/// Check if user agent looks suspicious
fn is_suspicious_user_agent(user_agent: &str) -> bool {
    let suspicious_patterns = [
        "sqlmap",
        "nmap",
        "nikto",
        "dirb",
        "gobuster",
        "burpsuite",
        "owasp",
        "scanner",
        "bot",
    ];

    let ua_lower = user_agent.to_lowercase();
    suspicious_patterns
        .iter()
        .any(|&pattern| ua_lower.contains(pattern))
}

/// Validate authorization header format
fn is_valid_authorization_header(auth: &str) -> bool {
    // Check for common authorization schemes
    auth.starts_with("Bearer ")
        || auth.starts_with("Basic ")
        || auth.starts_with("Digest ")
        || auth.starts_with("ApiKey ")
}

/// SQL injection detection middleware
pub async fn sql_injection_detection_middleware(
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    // Check query parameters for SQL injection patterns
    if let Some(query) = request.uri().query() {
        if contains_sql_injection_patterns(query) {
            warn!(query = query, "Potential SQL injection detected in query");
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Check path for SQL injection patterns
    let path = request.uri().path();
    if contains_sql_injection_patterns(path) {
        warn!(path = path, "Potential SQL injection detected in path");
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(next.run(request).await)
}

/// Check for SQL injection patterns
fn contains_sql_injection_patterns(input: &str) -> bool {
    let sql_patterns = [
        "' or '1'='1",
        "' or 1=1",
        "union select",
        "drop table",
        "delete from",
        "insert into",
        "update set",
        "exec(",
        "execute(",
        "sp_",
        "xp_",
        "--",
        "/*",
        "*/",
        "@@",
        "char(",
        "nchar(",
        "varchar(",
        "nvarchar(",
        "alter table",
        "create table",
        "truncate table",
    ];

    let input_lower = input.to_lowercase();

    // Check original input
    if sql_patterns
        .iter()
        .any(|&pattern| input_lower.contains(pattern))
    {
        return true;
    }

    // Also check URL-decoded input to catch encoded attacks
    // Simple percent-decoding for common cases
    let decoded = input
        .replace("%27", "'")
        .replace("%20", " ")
        .replace("%3D", "=")
        .replace("%3d", "=");

    let decoded_lower = decoded.to_lowercase();
    sql_patterns
        .iter()
        .any(|&pattern| decoded_lower.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{Method, Request},
        middleware,
        routing::get,
    };
    use pretty_assertions::assert_eq;
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "OK"
    }

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();

        assert!(config.enable_security_headers);
        assert_eq!(config.max_request_size, 10 * 1024 * 1024);
        assert!(config.allowed_upload_types.contains("application/json"));
    }

    #[test]
    fn test_cors_config_default() {
        let config = CorsConfig::default();

        assert_eq!(config.allowed_origins, vec!["http://localhost:3000"]);
        assert!(config.allowed_methods.contains(&Method::GET));
        assert!(config.allowed_methods.contains(&Method::POST));
        assert!(config.allow_credentials);
    }

    #[test]
    fn test_is_allowed_content_type() {
        assert!(is_allowed_content_type("application/json"));
        assert!(is_allowed_content_type("application/json; charset=utf-8"));
        assert!(is_allowed_content_type("text/plain"));
        assert!(!is_allowed_content_type("application/xml"));
        assert!(!is_allowed_content_type("image/jpeg"));
    }

    #[test]
    fn test_is_suspicious_user_agent() {
        assert!(is_suspicious_user_agent("sqlmap/1.0"));
        assert!(is_suspicious_user_agent(
            "Mozilla/5.0 (compatible; Nmap Scripting Engine)"
        ));
        assert!(!is_suspicious_user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64)"
        ));
        assert!(!is_suspicious_user_agent("curl/7.68.0"));
    }

    #[test]
    fn test_is_valid_authorization_header() {
        assert!(is_valid_authorization_header(
            "Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9"
        ));
        assert!(is_valid_authorization_header("Basic dXNlcjpwYXNz"));
        assert!(is_valid_authorization_header("ApiKey abc123"));
        assert!(!is_valid_authorization_header("Invalid format"));
        assert!(!is_valid_authorization_header(""));
    }

    #[test]
    fn test_contains_sql_injection_patterns() {
        assert!(contains_sql_injection_patterns("' or '1'='1"));
        assert!(contains_sql_injection_patterns("union select * from users"));
        assert!(contains_sql_injection_patterns("DROP TABLE users"));
        assert!(!contains_sql_injection_patterns("normal query string"));
        assert!(!contains_sql_injection_patterns("user@example.com"));
    }

    #[tokio::test]
    async fn test_security_headers_middleware() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(security_headers_middleware));

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("X-Frame-Options"));
        assert!(response.headers().contains_key("X-Content-Type-Options"));
        assert!(response.headers().contains_key("Content-Security-Policy"));
    }

    #[tokio::test]
    async fn test_input_validation_middleware_large_request() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(input_validation_middleware));

        let request = Request::builder()
            .method(Method::POST)
            .uri("/test")
            .header("content-length", "20971520") // 20MB
            .header("content-type", "application/json")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn test_input_validation_middleware_invalid_content_type() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(input_validation_middleware));

        let request = Request::builder()
            .method(Method::POST)
            .uri("/test")
            .header("content-type", "application/xml")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn test_sql_injection_detection_middleware() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn(sql_injection_detection_middleware));

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test?id=%27%20or%20%271%27=%271")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_create_cors_layer() {
        let config = CorsConfig::default();
        let _cors_layer = create_cors_layer(&config);

        // Test that CORS layer is created without panicking
        // Detailed testing would require more complex setup
    }
}
