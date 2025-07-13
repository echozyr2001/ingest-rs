//! Rate limiting middleware and utilities
//!
//! This module provides rate limiting functionality to protect the API
//! from abuse and ensure fair usage across clients.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
};
use std::{collections::HashMap, net::IpAddr, num::NonZeroU32, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::error::{ApiError, Result};

/// Type alias for a rate limiter instance
type RateLimiterInstance = Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>;

/// Type alias for IP-based rate limiter storage
type IpLimiterMap = Arc<RwLock<HashMap<IpAddr, RateLimiterInstance>>>;

/// Type alias for user-based rate limiter storage
type UserLimiterMap = Arc<RwLock<HashMap<String, RateLimiterInstance>>>;

/// Type alias for endpoint-based rate limiter storage
type EndpointLimiterMap = Arc<RwLock<HashMap<String, RateLimiterInstance>>>;

/// Rate limiter configuration
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// Requests per minute for anonymous users
    pub anonymous_requests_per_minute: u32,
    /// Requests per minute for authenticated users
    pub authenticated_requests_per_minute: u32,
    /// Burst size for rate limiting
    pub burst_size: u32,
    /// Whether to use IP-based limiting
    pub enable_ip_limiting: bool,
    /// Whether to use user-based limiting
    pub enable_user_limiting: bool,
    /// Custom rate limits for specific endpoints
    pub endpoint_limits: HashMap<String, u32>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            anonymous_requests_per_minute: 60,
            authenticated_requests_per_minute: 1000,
            burst_size: 10,
            enable_ip_limiting: true,
            enable_user_limiting: true,
            endpoint_limits: HashMap::new(),
        }
    }
}

/// Rate limiter state
#[derive(Clone)]
pub struct RateLimitState {
    config: RateLimitConfig,
    ip_limiters: IpLimiterMap,
    user_limiters: UserLimiterMap,
    endpoint_limiters: EndpointLimiterMap,
}

impl RateLimitState {
    /// Create new rate limit state
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            ip_limiters: Arc::new(RwLock::new(HashMap::new())),
            user_limiters: Arc::new(RwLock::new(HashMap::new())),
            endpoint_limiters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create rate limiter for IP address
    async fn get_ip_limiter(&self, ip: IpAddr) -> RateLimiterInstance {
        let mut limiters = self.ip_limiters.write().await;

        if let Some(limiter) = limiters.get(&ip) {
            return limiter.clone();
        }

        let quota = Quota::per_minute(
            NonZeroU32::new(self.config.anonymous_requests_per_minute)
                .unwrap_or(NonZeroU32::new(60).unwrap()),
        )
        .allow_burst(std::num::NonZeroU32::new(self.config.burst_size).unwrap());
        let limiter = Arc::new(RateLimiter::direct(quota));

        limiters.insert(ip, limiter.clone());
        limiter
    }

    /// Get or create rate limiter for user
    async fn get_user_limiter(&self, user_id: &str) -> RateLimiterInstance {
        let mut limiters = self.user_limiters.write().await;

        if let Some(limiter) = limiters.get(user_id) {
            return limiter.clone();
        }

        let quota = Quota::per_minute(
            NonZeroU32::new(self.config.authenticated_requests_per_minute)
                .unwrap_or(NonZeroU32::new(120).unwrap()),
        )
        .allow_burst(std::num::NonZeroU32::new(self.config.burst_size).unwrap());
        let limiter = Arc::new(RateLimiter::direct(quota));

        limiters.insert(user_id.to_string(), limiter.clone());
        limiter
    }

    /// Get or create rate limiter for endpoint
    async fn get_endpoint_limiter(&self, endpoint: &str) -> Option<RateLimiterInstance> {
        if let Some(&limit) = self.config.endpoint_limits.get(endpoint) {
            let mut limiters = self.endpoint_limiters.write().await;

            if let Some(limiter) = limiters.get(endpoint) {
                return Some(limiter.clone());
            }

            let quota =
                Quota::per_minute(NonZeroU32::new(limit).unwrap_or(NonZeroU32::new(60).unwrap()))
                    .allow_burst(std::num::NonZeroU32::new(self.config.burst_size).unwrap());
            let limiter = Arc::new(RateLimiter::direct(quota));

            limiters.insert(endpoint.to_string(), limiter.clone());
            Some(limiter)
        } else {
            None
        }
    }

    /// Check rate limits for a request
    pub async fn check_rate_limit(
        &self,
        ip: Option<IpAddr>,
        user_id: Option<&str>,
        endpoint: &str,
    ) -> Result<()> {
        // Check endpoint-specific rate limit first
        if let Some(endpoint_limiter) = self.get_endpoint_limiter(endpoint).await {
            if endpoint_limiter.check().is_err() {
                warn!(endpoint = endpoint, "Endpoint rate limit exceeded");
                return Err(ApiError::RateLimitExceeded(format!(
                    "Rate limit exceeded for endpoint: {endpoint}"
                )));
            }
        }

        // Check user-based rate limit
        if self.config.enable_user_limiting {
            if let Some(user_id) = user_id {
                let user_limiter = self.get_user_limiter(user_id).await;
                if user_limiter.check().is_err() {
                    warn!(user_id = user_id, "User rate limit exceeded");
                    return Err(ApiError::RateLimitExceeded(format!(
                        "Rate limit exceeded for user: {user_id}"
                    )));
                }
                debug!(user_id = user_id, "User rate limit check passed");
                return Ok(());
            }
        }

        // Check IP-based rate limit
        if self.config.enable_ip_limiting {
            if let Some(ip) = ip {
                let ip_limiter = self.get_ip_limiter(ip).await;
                if ip_limiter.check().is_err() {
                    warn!(ip = %ip, "IP rate limit exceeded");
                    return Err(ApiError::RateLimitExceeded(format!(
                        "Rate limit exceeded for IP: {ip}"
                    )));
                }
                debug!(ip = %ip, "IP rate limit check passed");
            }
        }

        Ok(())
    }

    /// Get rate limit information for response headers
    pub async fn get_rate_limit_info(
        &self,
        ip: Option<IpAddr>,
        user_id: Option<&str>,
    ) -> RateLimitInfo {
        let mut info = RateLimitInfo::default();

        if let Some(user_id) = user_id {
            let _user_limiter = self.get_user_limiter(user_id).await;
            // Note: governor doesn't expose remaining count directly
            // This would need to be implemented with a custom rate limiter
            info.limit = self.config.authenticated_requests_per_minute;
            info.remaining = self.config.authenticated_requests_per_minute; // Placeholder
            info.reset_time = Duration::from_secs(60); // Placeholder
        } else if let Some(_ip) = ip {
            info.limit = self.config.anonymous_requests_per_minute;
            info.remaining = self.config.anonymous_requests_per_minute; // Placeholder
            info.reset_time = Duration::from_secs(60); // Placeholder
        }

        info
    }
}

/// Rate limit information for response headers
#[derive(Debug, Default)]
pub struct RateLimitInfo {
    /// Rate limit per minute
    pub limit: u32,
    /// Remaining requests
    pub remaining: u32,
    /// Time until reset
    pub reset_time: Duration,
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(rate_limit_state): State<RateLimitState>,
    request: Request,
    next: Next,
) -> std::result::Result<Response, crate::error::ApiError> {
    let ip = extract_client_ip(&request);
    let user_id = extract_user_id(&request);
    let endpoint = request.uri().path();

    // Check rate limits
    match rate_limit_state
        .check_rate_limit(ip, user_id.as_deref(), endpoint)
        .await
    {
        Ok(()) => {
            // Rate limit passed, proceed with request
            let mut response = next.run(request).await;

            // Add rate limit headers
            let rate_limit_info = rate_limit_state
                .get_rate_limit_info(ip, user_id.as_deref())
                .await;
            add_rate_limit_headers(&mut response, &rate_limit_info);

            Ok(response)
        }
        Err(ApiError::RateLimitExceeded(message)) => {
            warn!(
                ip = ?ip,
                user_id = ?user_id,
                endpoint = endpoint,
                message = message,
                "Rate limit exceeded"
            );

            Err(crate::error::ApiError::RateLimitExceeded(
                "Rate limit exceeded".to_string(),
            ))
        }
        Err(_) => {
            // Other errors should not happen in rate limiting
            Err(crate::error::ApiError::Internal(anyhow::anyhow!(
                "Rate limiting error"
            )))
        }
    }
}

/// Extract client IP from request
fn extract_client_ip(request: &Request) -> Option<IpAddr> {
    // Try X-Forwarded-For header first
    if let Some(forwarded) = request.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(ip_str) = forwarded_str.split(',').next() {
                if let Ok(ip) = ip_str.trim().parse() {
                    return Some(ip);
                }
            }
        }
    }

    // Try X-Real-IP header
    if let Some(real_ip) = request.headers().get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            if let Ok(ip) = ip_str.parse() {
                return Some(ip);
            }
        }
    }

    // Fall back to connection info (would need to be passed through extensions)
    None
}

/// Extract user ID from request (from JWT token or API key)
fn extract_user_id(request: &Request) -> Option<String> {
    // Try Authorization header
    if let Some(auth_header) = request.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                // TODO: Decode JWT token and extract user ID
                // For now, return a placeholder
                return Some("user_from_jwt".to_string());
            }
        }
    }

    // Try API key header
    if let Some(api_key) = request.headers().get("x-api-key") {
        if let Ok(key_str) = api_key.to_str() {
            // TODO: Look up user ID from API key
            // For now, return the key as user ID
            return Some(format!("api_key_{key_str}"));
        }
    }

    None
}

/// Add rate limit headers to response
fn add_rate_limit_headers(response: &mut Response, info: &RateLimitInfo) {
    let headers = response.headers_mut();

    headers.insert("X-RateLimit-Limit", info.limit.to_string().parse().unwrap());
    headers.insert(
        "X-RateLimit-Remaining",
        info.remaining.to_string().parse().unwrap(),
    );
    headers.insert(
        "X-RateLimit-Reset",
        info.reset_time.as_secs().to_string().parse().unwrap(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{HeaderMap, Method, Request, StatusCode},
        middleware,
        routing::get,
    };
    use pretty_assertions::assert_eq;
    use std::net::{IpAddr, Ipv4Addr};
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "OK"
    }

    #[tokio::test]
    async fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();

        assert_eq!(config.anonymous_requests_per_minute, 60);
        assert_eq!(config.authenticated_requests_per_minute, 1000);
        assert_eq!(config.burst_size, 10);
        assert!(config.enable_ip_limiting);
        assert!(config.enable_user_limiting);
    }

    #[tokio::test]
    async fn test_rate_limit_state_creation() {
        let config = RateLimitConfig::default();
        let state = RateLimitState::new(config);

        // Test that state is created successfully
        assert!(state.ip_limiters.read().await.is_empty());
        assert!(state.user_limiters.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_ip_rate_limiting() {
        let config = RateLimitConfig {
            anonymous_requests_per_minute: 2, // Very low limit for testing
            burst_size: 2,                    // Small burst size for testing
            ..Default::default()
        };
        let state = RateLimitState::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        // First request should pass
        let result = state.check_rate_limit(Some(ip), None, "/test").await;
        assert!(result.is_ok());

        // Second request should pass
        let result = state.check_rate_limit(Some(ip), None, "/test").await;
        assert!(result.is_ok());

        // Third request should fail (exceeds burst)
        let result = state.check_rate_limit(Some(ip), None, "/test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_user_rate_limiting() {
        let config = RateLimitConfig {
            authenticated_requests_per_minute: 2, // Very low limit for testing
            burst_size: 2,                        // Small burst size for testing
            ..Default::default()
        };
        let state = RateLimitState::new(config);
        let user_id = "test_user";

        // First request should pass
        let result = state.check_rate_limit(None, Some(user_id), "/test").await;
        assert!(result.is_ok());

        // Second request should pass
        let result = state.check_rate_limit(None, Some(user_id), "/test").await;
        assert!(result.is_ok());

        // Third request should fail
        let result = state.check_rate_limit(None, Some(user_id), "/test").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_client_ip() {
        let mut request = Request::builder()
            .header("x-forwarded-for", "192.168.1.1, 10.0.0.1")
            .body(Body::empty())
            .unwrap();

        let ip = extract_client_ip(&request);
        assert_eq!(ip, Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));

        // Test X-Real-IP header
        *request.headers_mut() = HeaderMap::new();
        request
            .headers_mut()
            .insert("x-real-ip", "10.0.0.2".parse().unwrap());

        let ip = extract_client_ip(&request);
        assert_eq!(ip, Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2))));
    }

    #[test]
    fn test_extract_user_id() {
        let request = Request::builder()
            .header("authorization", "Bearer jwt_token_here")
            .body(Body::empty())
            .unwrap();

        let user_id = extract_user_id(&request);
        assert_eq!(user_id, Some("user_from_jwt".to_string()));

        let request = Request::builder()
            .header("x-api-key", "api_key_123")
            .body(Body::empty())
            .unwrap();

        let user_id = extract_user_id(&request);
        assert_eq!(user_id, Some("api_key_api_key_123".to_string()));
    }

    #[tokio::test]
    async fn test_rate_limit_middleware_integration() {
        let config = RateLimitConfig::default();
        let state = RateLimitState::new(config);

        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(middleware::from_fn_with_state(state, rate_limit_middleware));

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Check that rate limit headers are present
        assert!(response.headers().contains_key("X-RateLimit-Limit"));
        assert!(response.headers().contains_key("X-RateLimit-Remaining"));
        assert!(response.headers().contains_key("X-RateLimit-Reset"));
    }
}
