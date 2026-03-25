use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_http::{Body, NivasaResponse, NivasaServer};
use nivasa_routing::RouteMethod;
use std::{error::Error, net::TcpListener as StdTcpListener};
use tokio::{sync::oneshot, time::{sleep, timeout, Duration}};

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
async fn server_dispatches_through_request_pipeline() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/users/:id", |request| {
            let user_id = request
                .path_param("id")
                .expect("pipeline must attach route captures");

            NivasaResponse::new(
                http::StatusCode::OK,
                Body::text(format!("user-{user_id}")),
            )
        })
        .expect("route must register")
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        server.listen("127.0.0.1", port).await.expect("server must stop cleanly");
    });

    wait_for_server(port).await;

    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let uri = format!("http://127.0.0.1:{port}/users/42").parse()?;
    let response = client.get(uri).await?;
    assert_eq!(response.status(), http::StatusCode::OK);

    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"user-42"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(std::time::Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_dispatches_header_versioned_routes_through_request_pipeline() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/users", |_| {
            NivasaResponse::new(http::StatusCode::OK, Body::text("default"))
        })
        .expect("default route must register")
        .route_header_versioned(RouteMethod::Get, "1", "/users", |_| {
            NivasaResponse::new(http::StatusCode::OK, Body::text("versioned"))
        })
        .expect("header versioned route must register")
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        server.listen("127.0.0.1", port).await.expect("server must stop cleanly");
    });

    wait_for_server(port).await;

    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let versioned_request = http::Request::builder()
        .method(http::Method::GET)
        .uri(format!("http://127.0.0.1:{port}/users"))
        .header("X-API-Version", "1")
        .body(Empty::<Bytes>::new())?;

    let versioned_response = client.request(versioned_request).await?;
    assert_eq!(versioned_response.status(), http::StatusCode::OK);
    let body = versioned_response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"versioned"));

    let fallback_uri = format!("http://127.0.0.1:{port}/users").parse()?;
    let fallback_response = client.get(fallback_uri).await?;
    assert_eq!(fallback_response.status(), http::StatusCode::OK);
    let body = fallback_response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"default"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(std::time::Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_dispatches_media_type_versioned_routes_through_request_pipeline() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/reports", |_| {
            NivasaResponse::new(http::StatusCode::OK, Body::text("default"))
        })
        .expect("default route must register")
        .route_media_type_versioned(RouteMethod::Get, "2", "/reports", |_| {
            NivasaResponse::new(http::StatusCode::OK, Body::text("media-versioned"))
        })
        .expect("media type versioned route must register")
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        server.listen("127.0.0.1", port).await.expect("server must stop cleanly");
    });

    wait_for_server(port).await;

    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let versioned_request = http::Request::builder()
        .method(http::Method::GET)
        .uri(format!("http://127.0.0.1:{port}/reports"))
        .header(http::header::ACCEPT, "application/vnd.nivasa.v2+json")
        .body(Empty::<Bytes>::new())?;

    let versioned_response = client.request(versioned_request).await?;
    assert_eq!(versioned_response.status(), http::StatusCode::OK);
    let body = versioned_response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"media-versioned"));

    let fallback_uri = format!("http://127.0.0.1:{port}/reports").parse()?;
    let fallback_response = client.get(fallback_uri).await?;
    assert_eq!(fallback_response.status(), http::StatusCode::OK);
    let body = fallback_response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"default"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(std::time::Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_shutdown_signal_stops_accepting_connections() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        server.listen("127.0.0.1", port).await.expect("server must stop cleanly");
    });

    let _ = shutdown_tx.send(());
    timeout(std::time::Duration::from_secs(2), server_task).await??;
    Ok(())
}
