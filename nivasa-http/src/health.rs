use async_trait::async_trait;
use serde_json::Value;

/// Health status reported by a custom health indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Up,
    Down,
}

/// Result returned by a custom health indicator check.
#[derive(Debug, Clone, PartialEq)]
pub struct HealthIndicatorResult {
    pub status: HealthStatus,
    pub details: Option<Value>,
}

impl HealthIndicatorResult {
    pub const fn up() -> Self {
        Self {
            status: HealthStatus::Up,
            details: None,
        }
    }

    pub const fn down() -> Self {
        Self {
            status: HealthStatus::Down,
            details: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Trait implemented by custom health indicators.
#[async_trait]
pub trait HealthIndicator: Send + Sync {
    async fn check(&self) -> HealthIndicatorResult;
}

#[cfg(test)]
mod tests {
    use super::{HealthIndicator, HealthIndicatorResult, HealthStatus};
    use async_trait::async_trait;
    use serde_json::json;

    struct DatabaseIndicator;

    #[async_trait]
    impl HealthIndicator for DatabaseIndicator {
        async fn check(&self) -> HealthIndicatorResult {
            HealthIndicatorResult::up().with_details(json!({
                "name": "database",
                "latency_ms": 12
            }))
        }
    }

    struct FailingIndicator;

    #[async_trait]
    impl HealthIndicator for FailingIndicator {
        async fn check(&self) -> HealthIndicatorResult {
            HealthIndicatorResult::down()
        }
    }

    #[tokio::test]
    async fn custom_indicators_can_report_up_with_details() {
        let indicator = DatabaseIndicator;
        let result = indicator.check().await;

        assert_eq!(result.status, HealthStatus::Up);
        assert_eq!(
            result.details,
            Some(json!({
                "name": "database",
                "latency_ms": 12
            }))
        );
    }

    #[tokio::test]
    async fn custom_indicators_can_report_down_without_details() {
        let indicator = FailingIndicator;
        let result = indicator.check().await;

        assert_eq!(result.status, HealthStatus::Down);
        assert_eq!(result.details, None);
    }
}
