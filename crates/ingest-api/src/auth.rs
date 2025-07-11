//! Authentication and authorization for the API

use crate::{config::AuthConfig, error::ApiError, Result};
use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, HeaderMap},
    middleware::Next,
    response::Response,
};
use chrono::{Duration, Utc};
use headers::{authorization::Bearer, Authorization, HeaderMapExt};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    
    /// Issued at timestamp
    pub iat: i64,
    
    /// Expiration timestamp
    pub exp: i64,
    
    /// JWT ID
    pub jti: String,
    
    /// Issuer
    pub iss: String,
    
    /// Audience
    pub aud: String,
    
    /// Custom claims
    pub custom: std::collections::HashMap<String, serde_json::Value>,
}

/// Authentication service
#[derive(Clone)]
pub struct AuthService {
    config: Arc<AuthConfig>,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

/// Authentication context extracted from requests
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// User ID
    pub user_id: String,
    
    /// JWT claims
    pub claims: Claims,
    
    /// Authentication method used
    pub auth_method: AuthMethod,
}

/// Authentication methods
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// JWT token authentication
    Jwt,
    
    /// API key authentication
    ApiKey(String),
    
    /// No authentication (for public endpoints)
    None,
}

impl AuthService {
    /// Create a new authentication service
    pub fn new(config: AuthConfig) -> Self {
        let encoding_key = EncodingKey::from_secret(config.jwt_secret.as_ref());
        let decoding_key = DecodingKey::from_secret(config.jwt_secret.as_ref());
        
        Self {
            config: Arc::new(config),
            encoding_key,
            decoding_key,
        }
    }

    /// Generate a JWT token for a user
    pub fn generate_token(&self, user_id: &str) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.config.token_expiration.as_secs() as i64);
        
        let claims = Claims {
            sub: user_id.to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            jti: Uuid::new_v4().to_string(),
            iss: "inngest-api".to_string(),
            aud: "inngest".to_string(),
            custom: std::collections::HashMap::new(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| ApiError::Authentication(format!("Failed to generate token: {}", e)))
    }

    /// Validate a JWT token
    pub fn validate_token(&self, token: &str) -> Result<Claims> {
        let validation = Validation::default();
        
        decode::<Claims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| ApiError::Authentication(format!("Invalid token: {}", e)))
    }

    /// Validate an API key
    pub fn validate_api_key(&self, api_key: &str) -> Result<()> {
        if self.config.api_keys.contains(&api_key.to_string()) {
            Ok(())
        } else {
            Err(ApiError::Authentication("Invalid API key".to_string()))
        }
    }

    /// Extract authentication from request headers
    pub fn extract_auth(&self, headers: &HeaderMap) -> Result<Option<AuthContext>> {
        // Try JWT authentication first
        if let Some(bearer) = headers.typed_get::<Authorization<Bearer>>() {
            let token = bearer.token();
            let claims = self.validate_token(token)?;
            
            return Ok(Some(AuthContext {
                user_id: claims.sub.clone(),
                claims,
                auth_method: AuthMethod::Jwt,
            }));
        }

        // Try API key authentication
        if let Some(api_key) = headers.get("x-api-key") {
            let api_key_str = api_key
                .to_str()
                .map_err(|_| ApiError::Authentication("Invalid API key format".to_string()))?;
            
            self.validate_api_key(api_key_str)?;
            
            return Ok(Some(AuthContext {
                user_id: "api-key-user".to_string(),
                claims: Claims {
                    sub: "api-key-user".to_string(),
                    iat: Utc::now().timestamp(),
                    exp: (Utc::now() + Duration::hours(24)).timestamp(),
                    jti: Uuid::new_v4().to_string(),
                    iss: "inngest-api".to_string(),
                    aud: "inngest".to_string(),
                    custom: std::collections::HashMap::new(),
                },
                auth_method: AuthMethod::ApiKey(api_key_str.to_string()),
            }));
        }

        // No authentication found
        Ok(None)
    }
}

/// Authentication middleware
pub async fn auth_middleware(
    State(auth_service): State<AuthService>,
    mut request: Request,
    next: Next,
) -> Result<Response> {
    let headers = request.headers();
    
    // Extract authentication context
    let auth_context = auth_service.extract_auth(headers)?;
    
    // Check if authentication is required
    if auth_service.config.required && auth_context.is_none() {
        return Err(ApiError::Authentication(
            "Authentication required".to_string(),
        ));
    }

    // Add auth context to request extensions
    if let Some(context) = auth_context {
        request.extensions_mut().insert(context);
    }

    Ok(next.run(request).await)
}

/// Optional authentication middleware (doesn't require auth)
pub async fn optional_auth_middleware(
    State(auth_service): State<AuthService>,
    mut request: Request,
    next: Next,
) -> Result<Response> {
    let headers = request.headers();
    
    // Extract authentication context if present
    if let Ok(Some(auth_context)) = auth_service.extract_auth(headers) {
        request.extensions_mut().insert(auth_context);
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::time::Duration as StdDuration;

    fn create_test_auth_service() -> AuthService {
        let config = AuthConfig {
            jwt_secret: "test-secret".to_string(),
            token_expiration: StdDuration::from_secs(3600),
            required: false,
            api_keys: vec!["test-api-key".to_string()],
        };
        AuthService::new(config)
    }

    #[test]
    fn test_generate_and_validate_token() {
        let auth_service = create_test_auth_service();
        let user_id = "test-user";
        
        // Generate token
        let token = auth_service.generate_token(user_id).unwrap();
        assert!(!token.is_empty());
        
        // Validate token
        let claims = auth_service.validate_token(&token).unwrap();
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.iss, "inngest-api");
        assert_eq!(claims.aud, "inngest");
    }

    #[test]
    fn test_invalid_token() {
        let auth_service = create_test_auth_service();
        let result = auth_service.validate_token("invalid-token");
        assert!(result.is_err());
    }

    #[test]
    fn test_api_key_validation() {
        let auth_service = create_test_auth_service();
        
        // Valid API key
        assert!(auth_service.validate_api_key("test-api-key").is_ok());
        
        // Invalid API key
        assert!(auth_service.validate_api_key("invalid-key").is_err());
    }

    #[test]
    fn test_extract_jwt_auth() {
        let auth_service = create_test_auth_service();
        let token = auth_service.generate_token("test-user").unwrap();
        
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            format!("Bearer {}", token).parse().unwrap(),
        );
        
        let auth_context = auth_service.extract_auth(&headers).unwrap();
        assert!(auth_context.is_some());
        
        let context = auth_context.unwrap();
        assert_eq!(context.user_id, "test-user");
        assert!(matches!(context.auth_method, AuthMethod::Jwt));
    }

    #[test]
    fn test_extract_api_key_auth() {
        let auth_service = create_test_auth_service();
        
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", "test-api-key".parse().unwrap());
        
        let auth_context = auth_service.extract_auth(&headers).unwrap();
        assert!(auth_context.is_some());
        
        let context = auth_context.unwrap();
        assert_eq!(context.user_id, "api-key-user");
        assert!(matches!(context.auth_method, AuthMethod::ApiKey(_)));
    }

    #[test]
    fn test_no_auth_headers() {
        let auth_service = create_test_auth_service();
        let headers = HeaderMap::new();
        
        let auth_context = auth_service.extract_auth(&headers).unwrap();
        assert!(auth_context.is_none());
    }

    #[test]
    fn test_claims_structure() {
        let auth_service = create_test_auth_service();
        let token = auth_service.generate_token("test-user").unwrap();
        let claims = auth_service.validate_token(&token).unwrap();
        
        assert_eq!(claims.sub, "test-user");
        assert!(claims.iat > 0);
        assert!(claims.exp > claims.iat);
        assert!(!claims.jti.is_empty());
        assert_eq!(claims.iss, "inngest-api");
        assert_eq!(claims.aud, "inngest");
        assert!(claims.custom.is_empty());
    }
}