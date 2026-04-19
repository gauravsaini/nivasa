use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_http::{Body, NivasaResponse, NivasaServer};
use nivasa_routing::RouteMethod;
use std::{error::Error, net::TcpListener as StdTcpListener, sync::atomic::{AtomicBool, Ordering}, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
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

#[tokio::test]
async fn server_prefers_header_version_over_accept_fallback() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/versions", |_| {
            NivasaResponse::text("default")
        })
        .expect("default route must register")
        .route_header_versioned(RouteMethod::Get, "1", "/versions", |_| {
            NivasaResponse::text("header-versioned")
        })
        .expect("header versioned route must register")
        .route_media_type_versioned(RouteMethod::Get, "2", "/versions", |_| {
            NivasaResponse::text("accept-versioned")
        })
        .expect("media type versioned route must register")
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
    let request = http::Request::builder()
        .method(http::Method::GET)
        .uri(format!("http://127.0.0.1:{port}/versions"))
        .header("X-API-Version", "1")
        .header(http::header::ACCEPT, "application/vnd.nivasa.v2+json")
        .body(Empty::<Bytes>::new())?;

    let response = client.request(request).await?;
    let status = response.status();
    let body = response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    timeout(Duration::from_secs(2), server_task).await??;

    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(body, Bytes::from_static(b"header-versioned"));
    Ok(())
}

#[tokio::test]
async fn server_returns_bad_request_for_truncated_request_body() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let handler_called = Arc::new(AtomicBool::new(false));
    let handler_called_for_route = Arc::clone(&handler_called);

    let server = NivasaServer::builder()
        .route(RouteMethod::Post, "/truncated", move |_| {
            handler_called_for_route.store(true, Ordering::SeqCst);
            NivasaResponse::new(http::StatusCode::OK, Body::text("ok"))
        })
        .expect("route must register")
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        server
            .listen("127.0.0.1", port)
            .await
            .expect("server must stop cleanly");
    });

    wait_for_server(port).await;

    let mut stream = TcpStream::connect(("127.0.0.1", port)).await?;
    stream
        .write_all(
            b"POST /truncated HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 5\r\n\r\nhel",
        )
        .await?;
    stream.shutdown().await?;

    let mut response = Vec::new();
    timeout(Duration::from_secs(2), stream.read_to_end(&mut response)).await??;
    let response = String::from_utf8(response)?;

    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;

    assert!(response.starts_with("HTTP/1.1 400") || response.starts_with("HTTP/1.0 400"));
    assert!(response.contains("invalid request body"));
    assert!(!handler_called.load(Ordering::SeqCst));
    Ok(())
}
