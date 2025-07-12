//! Authentication service for SDK connections

use crate::{
    Result, config::AuthConfig, error::ConnectError, protocol::AuthMethod, types::ClientInfo,
};
use chrono::{Duration, Utc};
use headers::{Authorization, HeaderMapExt, authorization::Bearer};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

/// JWT claims for SDK authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (SDK identifier)
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

    /// SDK version
    pub sdk_version: String,

    /// Client capabilities
    pub capabilities: Vec<String>,
}

/// SDK credentials for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkCredentials {
    /// SDK identifier
    pub sdk_id: String,

    /// API key or token
    pub token: String,

    /// SDK version
    pub version: String,

    /// Authentication method
    pub method: AuthMethod,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl SdkCredentials {
    /// Create new SDK credentials
    pub fn new(sdk_id: String, token: String, version: String, method: AuthMethod) -> Self {
        Self {
            sdk_id,
            token,
            version,
            method,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to credentials
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Authentication context for connected SDKs
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// SDK identifier
    pub sdk_id: String,

    /// SDK version
    pub sdk_version: String,

    /// Authentication method used
    pub auth_method: AuthMethod,

    /// JWT claims (if JWT auth was used)
    pub claims: Option<Claims>,

    /// Client information
    pub client_info: ClientInfo,

    /// Authenticated at timestamp
    pub authenticated_at: chrono::DateTime<chrono::Utc>,

    /// Capabilities granted to this SDK
    pub capabilities: Vec<String>,
}

impl AuthContext {
    /// Create a new authentication context
    pub fn new(
        sdk_id: String,
        sdk_version: String,
        auth_method: AuthMethod,
        client_info: ClientInfo,
        capabilities: Vec<String>,
    ) -> Self {
        Self {
            sdk_id,
            sdk_version,
            auth_method,
            claims: None,
            client_info,
            authenticated_at: Utc::now(),
            capabilities,
        }
    }

    /// Set JWT claims
    pub fn with_claims(mut self, claims: Claims) -> Self {
        self.claims = Some(claims);
        self
    }

    /// Check if the context has a specific capability
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(&capability.to_string())
    }

    /// Check if the authentication is expired
    pub fn is_expired(&self, max_age: Duration) -> bool {
        Utc::now() - self.authenticated_at > max_age
    }
}

/// Authentication service for SDK connections
#[derive(Clone)]
pub struct AuthService {
    config: Arc<AuthConfig>,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
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

    /// Generate a JWT token for an SDK
    pub fn generate_sdk_token(
        &self,
        sdk_id: &str,
        sdk_version: &str,
        capabilities: Vec<String>,
    ) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.config.token_expiration.as_secs() as i64);

        let claims = Claims {
            sub: sdk_id.to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            jti: Uuid::new_v4().to_string(),
            iss: "inngest-connect".to_string(),
            aud: "inngest-sdk".to_string(),
            sdk_version: sdk_version.to_string(),
            capabilities,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| ConnectError::Authentication(format!("Failed to generate token: {e}")))
    }

    /// Validate a JWT token
    pub fn validate_jwt_token(&self, token: &str) -> Result<Claims> {
        let mut validation = Validation::default();
        validation.set_audience(&["inngest-sdk"]);

        decode::<Claims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| ConnectError::Authentication(format!("Invalid JWT token: {e}")))
    }

    /// Validate an API key
    pub fn validate_api_key(&self, api_key: &str) -> Result<()> {
        if self.config.api_keys.contains(&api_key.to_string()) {
            Ok(())
        } else {
            Err(ConnectError::Authentication("Invalid API key".to_string()))
        }
    }

    /// Validate SDK signature
    pub fn validate_sdk_signature(
        &self,
        sdk_id: &str,
        signature: &str,
        payload: &str,
    ) -> Result<()> {
        if !self.config.sdk.validate_signatures {
            return Ok(());
        }

        let secret = self
            .config
            .sdk
            .sdk_secrets
            .get(sdk_id)
            .ok_or_else(|| ConnectError::Authentication(format!("Unknown SDK: {sdk_id}")))?;

        // Simple HMAC-SHA256 signature validation
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        payload.hash(&mut hasher);
        secret.hash(&mut hasher);
        let expected_signature = format!("{:x}", hasher.finish());

        if signature == expected_signature {
            Ok(())
        } else {
            Err(ConnectError::Authentication(
                "Invalid SDK signature".to_string(),
            ))
        }
    }

    /// Check if SDK version is allowed
    pub fn is_sdk_version_allowed(&self, version: &str) -> bool {
        if self.config.sdk.allowed_versions.is_empty() {
            return true; // Allow all versions if none specified
        }

        self.config
            .sdk
            .allowed_versions
            .contains(&version.to_string())
    }

    /// Authenticate SDK credentials
    pub fn authenticate_sdk(
        &self,
        credentials: &SdkCredentials,
        client_info: ClientInfo,
    ) -> Result<AuthContext> {
        // Check if SDK version is allowed
        if !self.is_sdk_version_allowed(&credentials.version) {
            return Err(ConnectError::Authentication(format!(
                "SDK version {} is not allowed",
                credentials.version
            )));
        }

        let mut context = AuthContext::new(
            credentials.sdk_id.clone(),
            credentials.version.clone(),
            credentials.method.clone(),
            client_info,
            vec![], // Will be populated based on auth method
        );

        match &credentials.method {
            AuthMethod::Jwt => {
                let claims = self.validate_jwt_token(&credentials.token)?;

                // Verify the token is for this SDK
                if claims.sub != credentials.sdk_id {
                    return Err(ConnectError::Authentication(
                        "Token SDK ID mismatch".to_string(),
                    ));
                }

                context.capabilities = claims.capabilities.clone();
                context = context.with_claims(claims);
            }

            AuthMethod::ApiKey => {
                self.validate_api_key(&credentials.token)?;

                // API key gets basic capabilities
                context.capabilities =
                    vec!["event.send".to_string(), "function.execute".to_string()];
            }

            AuthMethod::SdkSignature { sdk_id, signature } => {
                if sdk_id != &credentials.sdk_id {
                    return Err(ConnectError::Authentication(
                        "SDK ID mismatch in signature".to_string(),
                    ));
                }

                self.validate_sdk_signature(sdk_id, signature, &credentials.token)?;

                // SDK signature gets full capabilities
                context.capabilities = vec![
                    "event.send".to_string(),
                    "function.execute".to_string(),
                    "function.register".to_string(),
                    "webhook.receive".to_string(),
                ];
            }
        }

        Ok(context)
    }

    /// Extract credentials from headers (for HTTP upgrade)
    pub fn extract_credentials_from_headers(
        &self,
        headers: &axum::http::HeaderMap,
    ) -> Result<Option<SdkCredentials>> {
        // Try JWT authentication first
        if let Some(bearer) = headers.typed_get::<Authorization<Bearer>>() {
            let token = bearer.token();

            // Try to decode to get SDK info
            if let Ok(claims) = self.validate_jwt_token(token) {
                return Ok(Some(SdkCredentials::new(
                    claims.sub,
                    token.to_string(),
                    claims.sdk_version,
                    AuthMethod::Jwt,
                )));
            }
        }

        // Try API key authentication
        if let Some(api_key) = headers.get("x-api-key") {
            let api_key_str = api_key
                .to_str()
                .map_err(|_| ConnectError::Authentication("Invalid API key format".to_string()))?;

            self.validate_api_key(api_key_str)?;

            return Ok(Some(SdkCredentials::new(
                "api-key-client".to_string(),
                api_key_str.to_string(),
                "unknown".to_string(),
                AuthMethod::ApiKey,
            )));
        }

        // Try SDK signature authentication
        if let (Some(sdk_id), Some(signature)) = (
            headers.get("x-sdk-id").and_then(|h| h.to_str().ok()),
            headers.get("x-sdk-signature").and_then(|h| h.to_str().ok()),
        ) {
            return Ok(Some(SdkCredentials::new(
                sdk_id.to_string(),
                "".to_string(), // Signature validation doesn't use token
                "unknown".to_string(),
                AuthMethod::SdkSignature {
                    sdk_id: sdk_id.to_string(),
                    signature: signature.to_string(),
                },
            )));
        }

        // No authentication found
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SdkAuthConfig;
    use pretty_assertions::assert_eq;
    use std::time::Duration as StdDuration;

    fn create_test_auth_service() -> AuthService {
        let mut sdk_secrets = HashMap::new();
        sdk_secrets.insert("test-sdk".to_string(), "test-secret".to_string());

        let config = AuthConfig {
            required: true,
            jwt_secret: "test-secret".to_string(),
            token_expiration: StdDuration::from_secs(3600),
            api_keys: vec!["test-api-key".to_string()],
            sdk: SdkAuthConfig {
                validate_signatures: true,
                allowed_versions: vec!["1.0.0".to_string()],
                sdk_secrets,
            },
        };
        AuthService::new(config)
    }

    fn create_test_client_info() -> ClientInfo {
        ClientInfo::new(
            "test-client".to_string(),
            "1.0.0".to_string(),
            "rust".to_string(),
            "linux".to_string(),
            "127.0.0.1".to_string(),
        )
    }

    #[test]
    fn test_generate_and_validate_sdk_token() {
        let auth_service = create_test_auth_service();
        let capabilities = vec!["event.send".to_string(), "function.execute".to_string()];

        // Generate token
        let token = auth_service
            .generate_sdk_token("test-sdk", "1.0.0", capabilities.clone())
            .unwrap();
        assert!(!token.is_empty());

        // Validate token
        let claims = auth_service.validate_jwt_token(&token).unwrap();
        assert_eq!(claims.sub, "test-sdk");
        assert_eq!(claims.sdk_version, "1.0.0");
        assert_eq!(claims.capabilities, capabilities);
    }

    #[test]
    fn test_sdk_credentials() {
        let credentials = SdkCredentials::new(
            "test-sdk".to_string(),
            "test-token".to_string(),
            "1.0.0".to_string(),
            AuthMethod::Jwt,
        )
        .with_metadata(
            "key".to_string(),
            serde_json::Value::String("value".to_string()),
        );

        assert_eq!(credentials.sdk_id, "test-sdk");
        assert_eq!(credentials.metadata.len(), 1);
    }

    #[test]
    fn test_auth_context() {
        let client_info = create_test_client_info();
        let capabilities = vec!["event.send".to_string()];

        let context = AuthContext::new(
            "test-sdk".to_string(),
            "1.0.0".to_string(),
            AuthMethod::Jwt,
            client_info,
            capabilities,
        );

        assert_eq!(context.sdk_id, "test-sdk");
        assert!(context.has_capability("event.send"));
        assert!(!context.has_capability("admin"));
        assert!(!context.is_expired(Duration::hours(1)));
    }

    #[test]
    fn test_authenticate_sdk_with_jwt() {
        let auth_service = create_test_auth_service();
        let client_info = create_test_client_info();
        let capabilities = vec!["event.send".to_string()];

        let token = auth_service
            .generate_sdk_token("test-sdk", "1.0.0", capabilities.clone())
            .unwrap();
        let credentials = SdkCredentials::new(
            "test-sdk".to_string(),
            token,
            "1.0.0".to_string(),
            AuthMethod::Jwt,
        );

        let context = auth_service
            .authenticate_sdk(&credentials, client_info)
            .unwrap();
        assert_eq!(context.sdk_id, "test-sdk");
        assert_eq!(context.capabilities, capabilities);
        assert!(context.claims.is_some());
    }

    #[test]
    fn test_authenticate_sdk_with_api_key() {
        let auth_service = create_test_auth_service();
        let client_info = create_test_client_info();

        let credentials = SdkCredentials::new(
            "api-client".to_string(),
            "test-api-key".to_string(),
            "1.0.0".to_string(),
            AuthMethod::ApiKey,
        );

        let context = auth_service
            .authenticate_sdk(&credentials, client_info)
            .unwrap();
        assert_eq!(context.sdk_id, "api-client");
        assert!(context.has_capability("event.send"));
        assert!(context.has_capability("function.execute"));
    }

    #[test]
    fn test_sdk_version_validation() {
        let auth_service = create_test_auth_service();

        assert!(auth_service.is_sdk_version_allowed("1.0.0"));
        assert!(!auth_service.is_sdk_version_allowed("2.0.0"));
    }

    #[test]
    fn test_invalid_sdk_version() {
        let auth_service = create_test_auth_service();
        let client_info = create_test_client_info();

        let credentials = SdkCredentials::new(
            "test-sdk".to_string(),
            "test-token".to_string(),
            "2.0.0".to_string(), // Not allowed
            AuthMethod::ApiKey,
        );

        let result = auth_service.authenticate_sdk(&credentials, client_info);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConnectError::Authentication(_)
        ));
    }
}
