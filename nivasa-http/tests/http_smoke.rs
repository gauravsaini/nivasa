use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_http::{Body, NivasaResponse, NivasaServer};
use nivasa_routing::RouteMethod;
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

#[tokio::test]
async fn server_responds_to_get_and_serializes_existing_response_types(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/", |_| {
            NivasaResponse::text("root-ready")
        })
        .expect("root route must register")
        .route(RouteMethod::Get, "/json", |_| {
            NivasaResponse::json(serde_json::json!({
                "ok": true,
                "service": "nivasa-http",
            }))
        })
        .expect("json route must register")
        .route(RouteMethod::Get, "/html", |_| {
            NivasaResponse::html("<strong>ready</strong>")
        })
        .expect("html route must register")
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = spawn_server(server, port);
    wait_for_server(port).await;

    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();

    let root_uri = format!("http://127.0.0.1:{port}/").parse()?;
    let root_response = client.get(root_uri).await?;
    assert_eq!(root_response.status(), http::StatusCode::OK);
    assert_eq!(
        root_response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .unwrap(),
        "text/plain; charset=utf-8"
    );
    let root_body = root_response.into_body().collect().await?.to_bytes();
    assert_eq!(root_body, Bytes::from_static(b"root-ready"));

    let json_uri = format!("http://127.0.0.1:{port}/json").parse()?;
    let json_response = client.get(json_uri).await?;
    assert_eq!(json_response.status(), http::StatusCode::OK);
    assert_eq!(
        json_response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .unwrap(),
        "application/json"
    );
    let json_body = json_response.into_body().collect().await?.to_bytes();
    let json_value: serde_json::Value = serde_json::from_slice(&json_body)?;
    assert_eq!(
        json_value,
        serde_json::json!({
            "ok": true,
            "service": "nivasa-http",
        })
    );

    let html_uri = format!("http://127.0.0.1:{port}/html").parse()?;
    let html_response = client.get(html_uri).await?;
    assert_eq!(html_response.status(), http::StatusCode::OK);
    assert_eq!(
        html_response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .unwrap(),
        "text/html; charset=utf-8"
    );
    let html_body = html_response.into_body().collect().await?.to_bytes();
    assert_eq!(html_body, Bytes::from_static(b"<strong>ready</strong>"));

    drop(client);
    let _ = shutdown_tx.send(());
    let _ = timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_returns_404_for_unknown_routes() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/health", |_| {
            NivasaResponse::new(http::StatusCode::OK, Body::text("ok"))
        })
        .expect("health route must register")
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = spawn_server(server, port);
    wait_for_server(port).await;

    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let missing_uri = format!("http://127.0.0.1:{port}/missing").parse()?;
    let response = client.get(missing_uri).await?;

    assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"not found"));

    drop(client);
    let _ = shutdown_tx.send(());
    let _ = timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}
