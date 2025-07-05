use derive_setters::Setters;
use serde::{Deserialize, Serialize};

/// Feature flags configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct FeatureFlags {
    /// Enable distributed tracing
    pub tracing_enabled: bool,
    /// Enable metrics collection
    pub metrics_enabled: bool,
    /// Enable experimental features
    pub experimental_features: bool,
    /// Enable debug mode
    pub debug_mode: bool,
    /// Enable batch processing
    pub batch_processing: bool,
    /// Enable caching
    pub caching_enabled: bool,
    /// Enable webhooks
    pub webhooks_enabled: bool,
    /// Enable rate limiting
    pub rate_limiting_enabled: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            tracing_enabled: true,
            metrics_enabled: true,
            experimental_features: false,
            debug_mode: false,
            batch_processing: true,
            caching_enabled: true,
            webhooks_enabled: true,
            rate_limiting_enabled: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_flags_default() {
        let actual = FeatureFlags::default();
        assert!(actual.tracing_enabled);
        assert!(actual.metrics_enabled);
        assert!(!actual.experimental_features);
        assert!(!actual.debug_mode);
    }
}
