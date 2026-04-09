use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_http::NivasaServer;
use std::{error::Error, net::TcpListener as StdTcpListener, time::Duration};
use tokio::{
    sync::oneshot,
    time::{sleep, timeout},
};

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

fn spawn_server(
    server: NivasaServer,
    port: u16,
) -> tokio::task::JoinHandle<Result<(), std::io::Error>> {
    tokio::spawn(async move { server.listen("127.0.0.1", port).await })
}

async fn fetch_json(
    path: &str,
    expected: serde_json::Value,
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .openapi_spec_json(path, expected.clone())?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = spawn_server(server, port);
    wait_for_server(port).await;

    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let uri = format!("http://127.0.0.1:{port}{path}").parse()?;
    let response = client.get(uri).await?;

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        response.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "application/json"
    );

    let body = response.into_body().collect().await?.to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(value, expected);

    drop(client);
    let _ = shutdown_tx.send(());
    let _ = timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn openapi_spec_endpoint_serves_json_at_default_path() -> Result<(), Box<dyn Error>> {
    fetch_json(
        "/api/docs/openapi.json",
        serde_json::json!({
            "openapi": "3.0.0",
            "info": { "title": "Nivasa API", "version": "1.0.0" },
            "paths": {},
            "components": { "schemas": {}, "securitySchemes": {} }
        }),
    )
    .await
}

#[tokio::test]
async fn openapi_spec_endpoint_serves_json_at_custom_path() -> Result<(), Box<dyn Error>> {
    fetch_json(
        "/docs/openapi.json",
        serde_json::json!({
            "openapi": "3.0.0",
            "info": { "title": "Custom API", "version": "2.0.0" },
            "paths": { "/users": { "get": { "tags": ["Users"] } } },
            "components": { "schemas": {}, "securitySchemes": {} }
        }),
    )
    .await
}
