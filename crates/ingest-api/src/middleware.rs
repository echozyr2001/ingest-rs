//! Middleware for the API server

use crate::{Result, config::RateLimitConfig, error::ApiError};
use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tower_http::cors::CorsLayer;
use tracing::{info, warn};
use uuid::Uuid;

/// Rate limiting service
#[derive(Clone)]
pub struct RateLimitService {
    config: Arc<RateLimitConfig>,
    buckets: Arc<Mutex<HashMap<IpAddr, TokenBucket>>>,
}

/// Token bucket for rate limiting
#[derive(Debug)]
struct TokenBucket {
    tokens: u32,
    last_refill: Instant,
    capacity: u32,
}

/// Request ID middleware state
#[derive(Clone)]
pub struct RequestIdService {
    header_name: String,
}

impl RateLimitService {
    /// Create a new rate limiting service
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config: Arc::new(config),
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a request is allowed for the given IP
    pub fn is_allowed(&self, ip: IpAddr) -> bool {
        if !self.config.enabled {
            return true;
        }

        let mut buckets = self.buckets.lock().unwrap();
        let bucket = buckets.entry(ip).or_insert_with(|| TokenBucket {
            tokens: self.config.burst_capacity,
            last_refill: Instant::now(),
            capacity: self.config.burst_capacity,
        });

        bucket.refill(self.config.requests_per_minute, self.config.window_duration);

        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            true
        } else {
            false
        }
    }

    /// Clean up old buckets (should be called periodically)
    pub fn cleanup_old_buckets(&self) {
        let mut buckets = self.buckets.lock().unwrap();
        let cutoff = Instant::now() - self.config.window_duration * 2;

        buckets.retain(|_, bucket| bucket.last_refill > cutoff);
    }
}

impl TokenBucket {
    /// Refill the token bucket based on elapsed time
    fn refill(&mut self, refill_rate: u32, window_duration: Duration) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);

        if elapsed >= window_duration {
            // Calculate how many tokens to add
            let windows_passed = elapsed.as_secs_f64() / window_duration.as_secs_f64();
            let tokens_to_add = (refill_rate as f64 * windows_passed) as u32;

            self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
            self.last_refill = now;
        }
    }
}

impl RequestIdService {
    /// Create a new request ID service
    pub fn new() -> Self {
        Self {
            header_name: "x-request-id".to_string(),
        }
    }

    /// Create with custom header name
    pub fn with_header_name(header_name: String) -> Self {
        Self { header_name }
    }
}

impl Default for RequestIdService {
    fn default() -> Self {
        Self::new()
    }
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(rate_limit_service): State<RateLimitService>,
    request: Request,
    next: Next,
) -> Result<Response> {
    // Extract client IP address
    let client_ip = extract_client_ip(&request);

    // Check rate limit
    if !rate_limit_service.is_allowed(client_ip) {
        warn!(
            client_ip = %client_ip,
            "Rate limit exceeded"
        );
        return Err(ApiError::RateLimit);
    }

    Ok(next.run(request).await)
}

/// Request ID middleware
pub async fn request_id_middleware(
    State(request_id_service): State<RequestIdService>,
    mut request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    // Generate or extract request ID
    let request_id = request
        .headers()
        .get(&request_id_service.header_name)
        .and_then(|value| value.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(Uuid::new_v4);

    // Add request ID to request extensions
    request.extensions_mut().insert(request_id);

    // Run the request
    let mut response = next.run(request).await;

    // Add request ID to response headers
    response
        .headers_mut()
        .insert("x-request-id", request_id.to_string().parse().unwrap());

    Ok(response)
}

/// Logging middleware
pub async fn logging_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = Instant::now();

    // Extract request ID if available
    let request_id = request
        .extensions()
        .get::<Uuid>()
        .copied()
        .unwrap_or_else(Uuid::new_v4);

    info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        "Request started"
    );

    let response = next.run(request).await;
    let duration = start.elapsed();

    info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        status = %response.status(),
        duration_ms = duration.as_millis(),
        "Request completed"
    );

    response
}

/// Extract client IP address from request
fn extract_client_ip(request: &Request) -> IpAddr {
    // Try to get IP from X-Forwarded-For header first
    if let Some(forwarded) = request.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(first_ip) = forwarded_str.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse() {
                    return ip;
                }
            }
        }
    }

    // Try X-Real-IP header
    if let Some(real_ip) = request.headers().get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            if let Ok(ip) = ip_str.parse() {
                return ip;
            }
        }
    }

    // Fallback to connection remote address (if available)
    // In a real implementation, this would come from the connection info
    "127.0.0.1".parse().unwrap()
}

/// Create CORS layer from configuration
pub fn create_cors_layer(cors_config: &crate::config::CorsConfig) -> CorsLayer {
    let mut cors = CorsLayer::new();

    // Configure allowed origins
    if cors_config.allowed_origins.contains(&"*".to_string()) {
        cors = cors.allow_origin(tower_http::cors::Any);
    } else {
        for origin in &cors_config.allowed_origins {
            if let Ok(origin_header) = origin.parse::<HeaderValue>() {
                cors = cors.allow_origin(origin_header);
            }
        }
    }

    // Configure allowed methods
    let methods: Vec<axum::http::Method> = cors_config
        .allowed_methods
        .iter()
        .filter_map(|method| method.parse().ok())
        .collect();
    cors = cors.allow_methods(methods);

    // Configure allowed headers
    let headers: Vec<axum::http::HeaderName> = cors_config
        .allowed_headers
        .iter()
        .filter_map(|header| header.parse().ok())
        .collect();
    cors = cors.allow_headers(headers);

    // Configure credentials
    if cors_config.allow_credentials {
        cors = cors.allow_credentials(true);
    }

    // Configure max age
    cors = cors.max_age(cors_config.max_age);

    cors
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::time::Duration as StdDuration;

    fn create_test_rate_limit_config() -> RateLimitConfig {
        RateLimitConfig {
            enabled: true,
            requests_per_minute: 60,
            burst_capacity: 10,
            window_duration: StdDuration::from_secs(60),
        }
    }

    #[test]
    fn test_rate_limit_service() {
        let config = create_test_rate_limit_config();
        let service = RateLimitService::new(config);
        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        // Should allow initial requests up to burst capacity
        for _ in 0..10 {
            assert!(service.is_allowed(ip));
        }

        // Should deny the next request
        assert!(!service.is_allowed(ip));
    }

    #[test]
    fn test_rate_limit_disabled() {
        let mut config = create_test_rate_limit_config();
        config.enabled = false;

        let service = RateLimitService::new(config);
        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        // Should always allow when disabled
        for _ in 0..100 {
            assert!(service.is_allowed(ip));
        }
    }

    #[test]
    fn test_token_bucket_refill() {
        let mut bucket = TokenBucket {
            tokens: 0,
            last_refill: Instant::now() - StdDuration::from_secs(60),
            capacity: 10,
        };

        bucket.refill(60, StdDuration::from_secs(60));

        // Should have refilled to capacity
        assert_eq!(bucket.tokens, 10);
    }

    #[test]
    fn test_request_id_service() {
        let service = RequestIdService::new();
        assert_eq!(service.header_name, "x-request-id");

        let service = RequestIdService::with_header_name("custom-id".to_string());
        assert_eq!(service.header_name, "custom-id");
    }

    #[test]
    fn test_extract_client_ip() {
        let mut request = Request::builder()
            .uri("/")
            .body(axum::body::Body::empty())
            .unwrap();

        // Test X-Forwarded-For header
        request
            .headers_mut()
            .insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());

        let ip = extract_client_ip(&request);
        assert_eq!(ip, "192.168.1.1".parse::<IpAddr>().unwrap());

        // Test X-Real-IP header
        let mut request = Request::builder()
            .uri("/")
            .body(axum::body::Body::empty())
            .unwrap();

        request
            .headers_mut()
            .insert("x-real-ip", "192.168.1.2".parse().unwrap());

        let ip = extract_client_ip(&request);
        assert_eq!(ip, "192.168.1.2".parse::<IpAddr>().unwrap());

        // Test fallback
        let request = Request::builder()
            .uri("/")
            .body(axum::body::Body::empty())
            .unwrap();

        let ip = extract_client_ip(&request);
        assert_eq!(ip, "127.0.0.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_cleanup_old_buckets() {
        let config = create_test_rate_limit_config();
        let service = RateLimitService::new(config);
        let ip1: IpAddr = "127.0.0.1".parse().unwrap();
        let ip2: IpAddr = "127.0.0.2".parse().unwrap();

        // Create some buckets
        service.is_allowed(ip1);
        service.is_allowed(ip2);

        // Verify buckets exist
        {
            let buckets = service.buckets.lock().unwrap();
            assert_eq!(buckets.len(), 2);
        }

        // Cleanup shouldn't remove recent buckets
        service.cleanup_old_buckets();
        {
            let buckets = service.buckets.lock().unwrap();
            assert_eq!(buckets.len(), 2);
        }
    }
}
