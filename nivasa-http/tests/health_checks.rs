use nivasa_http::{
    DatabaseHealthIndicator, HealthCheckService, HealthIndicator, HealthIndicatorResult,
    HealthStatus, HttpHealthIndicator,
};
use serde_json::json;
use std::sync::Arc;

struct FailingIndicator;

#[async_trait::async_trait]
impl HealthIndicator for FailingIndicator {
    async fn check(&self) -> HealthIndicatorResult {
        HealthIndicatorResult::down()
    }
}

#[tokio::test]
async fn health_check_service_returns_up_when_all_indicators_are_up() {
    let service = HealthCheckService::new(vec![
        Arc::new(nivasa_http::DiskHealthIndicator),
        Arc::new(nivasa_http::MemoryHealthIndicator),
    ]);

    let result = service.check().await;

    assert_eq!(result.status, HealthStatus::Up);
    assert_eq!(result.details.len(), 2);
}

#[tokio::test]
async fn health_check_service_returns_down_when_any_indicator_is_down() {
    let service = HealthCheckService::new(vec![Arc::new(FailingIndicator)]);

    let result = service.check().await;

    assert_eq!(result.status, HealthStatus::Down);
    assert_eq!(result.details.len(), 1);
}

#[tokio::test]
async fn probe_based_health_indicators_cover_up_and_down_details() {
    let database_up = DatabaseHealthIndicator::new(|| true).check().await;
    assert_eq!(database_up.status, HealthStatus::Up);
    assert_eq!(
        database_up.details,
        Some(json!({
            "name": "database",
            "status": "up",
        }))
    );

    let database_down = DatabaseHealthIndicator::new(|| false).check().await;
    assert_eq!(database_down.status, HealthStatus::Down);
    assert_eq!(
        database_down.details,
        Some(json!({
            "name": "database",
            "status": "down",
        }))
    );

    let http_up = HttpHealthIndicator::new(|| true).check().await;
    assert_eq!(http_up.status, HealthStatus::Up);
    assert_eq!(
        http_up.details,
        Some(json!({
            "name": "http",
            "status": "up",
        }))
    );

    let http_down = HttpHealthIndicator::new(|| false).check().await;
    assert_eq!(http_down.status, HealthStatus::Down);
    assert_eq!(
        http_down.details,
        Some(json!({
            "name": "http",
            "status": "down",
        }))
    );
}
