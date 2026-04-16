use bytes::Bytes;
use http::{StatusCode, Uri};
use http_body_util::{BodyExt, Empty};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_http::{
    Body, DatabaseHealthIndicator, HealthCheckService, HealthIndicator, HealthIndicatorResult,
    HealthStatus, HttpHealthIndicator, NivasaResponse, NivasaServer,
};
use nivasa_routing::RouteMethod;
use std::{error::Error, net::TcpListener as StdTcpListener, sync::Arc, time::Duration};
use tokio::{
    sync::oneshot,
    time::{sleep, timeout},
};
use serde_json::json;

fn free_port() -> u16 {
    StdTcpListener::bind("127.0.0.1:0")
        .expect("must bind an ephemeral port")
        .local_addr()
        .expect("must inspect ephemeral addr")
        .port()
}

async fn wait_for_server(port: u16) {
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return;
        }

        sleep(Duration::from_millis(20)).await;
    }

    panic!("server did not become ready");
}

async fn assert_health_endpoint_status(
    indicators: Vec<Arc<dyn HealthIndicator>>,
    expected_status: StatusCode,
    expected_body: &'static str,
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let service = Arc::new(HealthCheckService::new(indicators));

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/health", {
            let service = Arc::clone(&service);
            move |_| {
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async { service.check().await })
                });

                let status = match result.status {
                    HealthStatus::Up => StatusCode::OK,
                    HealthStatus::Down => StatusCode::SERVICE_UNAVAILABLE,
                };

                let body = match result.status {
                    HealthStatus::Up => Body::text("up"),
                    HealthStatus::Down => Body::text("down"),
                };

                NivasaResponse::new(status, body)
            }
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        server
            .listen("127.0.0.1", port)
            .await
            .expect("server must stop cleanly");
    });

    wait_for_server(port).await;

    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let uri: Uri = format!("http://127.0.0.1:{port}/health").parse()?;
    let response = client.get(uri).await?;

    assert_eq!(response.status(), expected_status);
    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(expected_body.as_bytes()));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

struct FailingIndicator;

#[async_trait::async_trait]
impl HealthIndicator for FailingIndicator {
    async fn check(&self) -> HealthIndicatorResult {
        HealthIndicatorResult::down()
    }
}

#[tokio::test]
async fn health_endpoint_returns_ok_when_all_indicators_are_up() -> Result<(), Box<dyn Error>> {
    assert_health_endpoint_status(
        vec![
            Arc::new(nivasa_http::DiskHealthIndicator),
            Arc::new(nivasa_http::MemoryHealthIndicator),
        ],
        StatusCode::OK,
        "up",
    )
    .await
}

#[tokio::test]
async fn health_endpoint_returns_service_unavailable_when_any_indicator_is_down(
) -> Result<(), Box<dyn Error>> {
    assert_health_endpoint_status(
        vec![Arc::new(FailingIndicator)],
        StatusCode::SERVICE_UNAVAILABLE,
        "down",
    )
    .await
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
