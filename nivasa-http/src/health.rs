use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

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

/// Reports a stable disk health payload.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiskHealthIndicator;

/// Reports a stable memory health payload.
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryHealthIndicator;

/// Probe-backed database health indicator.
pub struct DatabaseHealthIndicator<P> {
    probe: P,
}

impl<P> DatabaseHealthIndicator<P> {
    pub fn new(probe: P) -> Self {
        Self { probe }
    }
}

/// Probe-backed HTTP health indicator.
pub struct HttpHealthIndicator<P> {
    probe: P,
}

impl<P> HttpHealthIndicator<P> {
    pub fn new(probe: P) -> Self {
        Self { probe }
    }
}

#[async_trait]
impl HealthIndicator for DiskHealthIndicator {
    async fn check(&self) -> HealthIndicatorResult {
        HealthIndicatorResult::up().with_details(serde_json::json!({
            "name": "disk",
            "status": "up",
        }))
    }
}

#[async_trait]
impl HealthIndicator for MemoryHealthIndicator {
    async fn check(&self) -> HealthIndicatorResult {
        HealthIndicatorResult::up().with_details(serde_json::json!({
            "name": "memory",
            "status": "up",
        }))
    }
}

#[async_trait]
impl<P> HealthIndicator for DatabaseHealthIndicator<P>
where
    P: Fn() -> bool + Send + Sync,
{
    async fn check(&self) -> HealthIndicatorResult {
        let status = if (self.probe)() {
            HealthStatus::Up
        } else {
            HealthStatus::Down
        };

        let details = serde_json::json!({
            "name": "database",
            "status": match status {
                HealthStatus::Up => "up",
                HealthStatus::Down => "down",
            },
        });

        match status {
            HealthStatus::Up => HealthIndicatorResult::up().with_details(details),
            HealthStatus::Down => HealthIndicatorResult::down().with_details(details),
        }
    }
}

#[async_trait]
impl<P> HealthIndicator for HttpHealthIndicator<P>
where
    P: Fn() -> bool + Send + Sync,
{
    async fn check(&self) -> HealthIndicatorResult {
        let status = if (self.probe)() {
            HealthStatus::Up
        } else {
            HealthStatus::Down
        };

        let details = serde_json::json!({
            "name": "http",
            "status": match status {
                HealthStatus::Up => "up",
                HealthStatus::Down => "down",
            },
        });

        match status {
            HealthStatus::Up => HealthIndicatorResult::up().with_details(details),
            HealthStatus::Down => HealthIndicatorResult::down().with_details(details),
        }
    }
}

/// Aggregate result returned by [`HealthCheckService`].
#[derive(Debug, Clone, PartialEq)]
pub struct HealthCheckResult {
    pub status: HealthStatus,
    pub details: Vec<HealthIndicatorResult>,
}

/// Runs a list of health indicators and aggregates their status.
#[derive(Clone, Default)]
pub struct HealthCheckService {
    indicators: Vec<Arc<dyn HealthIndicator>>,
}

impl HealthCheckService {
    pub fn new(indicators: Vec<Arc<dyn HealthIndicator>>) -> Self {
        Self { indicators }
    }

    pub async fn check(&self) -> HealthCheckResult {
        let mut details = Vec::with_capacity(self.indicators.len());
        let mut status = HealthStatus::Up;

        for indicator in &self.indicators {
            let result = indicator.check().await;
            if matches!(result.status, HealthStatus::Down) {
                status = HealthStatus::Down;
            }
            details.push(result);
        }

        HealthCheckResult { status, details }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DatabaseHealthIndicator, DiskHealthIndicator, HealthCheckService, HealthIndicator,
        HealthIndicatorResult, HealthStatus, HttpHealthIndicator, MemoryHealthIndicator,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::Arc;

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

    #[tokio::test]
    async fn database_indicator_uses_probe_result_and_stable_details() {
        let indicator = DatabaseHealthIndicator::new(|| true);
        let result = indicator.check().await;

        assert_eq!(result.status, HealthStatus::Up);
        assert_eq!(
            result.details,
            Some(json!({
                "name": "database",
                "status": "up"
            }))
        );
    }

    #[tokio::test]
    async fn http_indicator_uses_probe_result_and_stable_details() {
        let indicator = HttpHealthIndicator::new(|| false);
        let result = indicator.check().await;

        assert_eq!(result.status, HealthStatus::Down);
        assert_eq!(
            result.details,
            Some(json!({
                "name": "http",
                "status": "down"
            }))
        );
    }

    #[tokio::test]
    async fn disk_indicator_reports_stable_up_payload() {
        let result = DiskHealthIndicator.check().await;

        assert_eq!(result.status, HealthStatus::Up);
        assert_eq!(
            result.details,
            Some(json!({
                "name": "disk",
                "status": "up"
            }))
        );
    }

    #[tokio::test]
    async fn memory_indicator_reports_stable_up_payload() {
        let result = MemoryHealthIndicator.check().await;

        assert_eq!(result.status, HealthStatus::Up);
        assert_eq!(
            result.details,
            Some(json!({
                "name": "memory",
                "status": "up"
            }))
        );
    }

    #[tokio::test]
    async fn health_check_service_aggregates_status_and_details() {
        let service = HealthCheckService::new(vec![
            Arc::new(DiskHealthIndicator),
            Arc::new(MemoryHealthIndicator),
            Arc::new(FailingIndicator),
        ]);

        let result = service.check().await;

        assert_eq!(result.status, HealthStatus::Down);
        assert_eq!(result.details.len(), 3);
        assert_eq!(result.details[0].status, HealthStatus::Up);
        assert_eq!(
            result.details[0].details,
            Some(json!({
                "name": "disk",
                "status": "up"
            }))
        );
        assert_eq!(result.details[1].status, HealthStatus::Up);
        assert_eq!(
            result.details[1].details,
            Some(json!({
                "name": "memory",
                "status": "up"
            }))
        );
        assert_eq!(result.details[2].status, HealthStatus::Down);
        assert_eq!(result.details[2].details, None);
    }
}
