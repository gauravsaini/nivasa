use bytes::Bytes;
use http::{
    header::{
        ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS,
        ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_HEADERS,
        ACCESS_CONTROL_REQUEST_METHOD, ORIGIN,
    },
    Method, StatusCode,
};
use http_body_util::{BodyExt, Empty};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_http::{Body, CorsOptions, NivasaResponse, NivasaServer};
use nivasa_routing::RouteMethod;
use std::{
    error::Error,
    net::TcpListener as StdTcpListener,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::oneshot,
    time::{sleep, timeout},
};
use uuid::Uuid;

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
async fn server_omits_allow_origin_for_disallowed_cors_origins_on_preflight_and_response(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let handler_calls = Arc::new(AtomicUsize::new(0));
    let handler_calls_for_route = Arc::clone(&handler_calls);

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/users/:id", move |request| {
            handler_calls_for_route.fetch_add(1, Ordering::SeqCst);
            let user_id = request
                .path_param("id")
                .expect("pipeline must attach route captures before handler execution");

            NivasaResponse::new(StatusCode::OK, Body::text(format!("user-{user_id}")))
        })
        .expect("route must register")
        .cors_options(
            CorsOptions::permissive()
                .allow_origins(["https://allowed.example"])
                .allow_methods([Method::GET, Method::OPTIONS]),
        )
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

    let preflight = http::Request::builder()
        .method(Method::OPTIONS)
        .uri(format!("http://127.0.0.1:{port}/users/42"))
        .header(ORIGIN, "https://blocked.example")
        .header(ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .body(Empty::<Bytes>::new())?;

    let response = client.request(preflight).await?;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(response
        .headers()
        .get(ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_METHODS)
            .expect("allow methods header must still be present"),
        "GET, OPTIONS"
    );
    let body = response.into_body().collect().await?.to_bytes();
    assert!(body.is_empty());
    assert_eq!(handler_calls.load(Ordering::SeqCst), 0);

    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("http://127.0.0.1:{port}/users/42"))
        .header(ORIGIN, "https://blocked.example")
        .body(Empty::<Bytes>::new())?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get(ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());
    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"user-42"));
    assert_eq!(handler_calls.load(Ordering::SeqCst), 1);

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_preserves_request_request_id_over_handler_supplied_response_header(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/request-id", |_| {
            NivasaResponse::text("ok")
                .with_header("x-request-id", "handler-id")
                .with_header("x-handler", "set")
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

    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("http://127.0.0.1:{port}/request-id"))
        .header("x-request-id", "req-123")
        .body(Empty::<Bytes>::new())?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-request-id")
            .expect("response request id must exist"),
        "req-123"
    );
    assert_eq!(
        response
            .headers()
            .get("x-handler")
            .expect("handler header must survive finalization"),
        "set"
    );
    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"ok"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_generates_request_id_for_success_responses_even_when_handler_sets_one(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/request-id", |_| {
            NivasaResponse::text("ok").with_header("x-request-id", "handler-id")
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

    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("http://127.0.0.1:{port}/request-id"))
        .body(Empty::<Bytes>::new())?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .expect("response request id must exist")
        .to_owned();
    assert_ne!(request_id, "handler-id");
    Uuid::parse_str(&request_id).expect("server should seed a UUID request id");
    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"ok"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn credentialed_cors_without_origin_sets_credentials_but_not_allow_origin(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/cors", |_| NivasaResponse::text("ok"))
        .expect("route must register")
        .cors_options(CorsOptions::permissive().allow_credentials(true))
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
        .method(Method::GET)
        .uri(format!("http://127.0.0.1:{port}/cors"))
        .body(Empty::<Bytes>::new())?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get(ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .expect("credentialed cors header should be present"),
        "true"
    );

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn cors_preflight_echoes_options_and_requested_headers_when_not_pinned(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/cors", |_| NivasaResponse::text("ok"))
        .expect("route must register")
        .enable_cors()
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
    let preflight = http::Request::builder()
        .method(Method::OPTIONS)
        .uri(format!("http://127.0.0.1:{port}/cors"))
        .header(ORIGIN, "https://app.example")
        .header(ACCESS_CONTROL_REQUEST_METHOD, "OPTIONS")
        .header(ACCESS_CONTROL_REQUEST_HEADERS, "x-demo, authorization")
        .body(Empty::<Bytes>::new())?;

    let response = client.request(preflight).await?;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_METHODS)
            .expect("allow methods header should echo OPTIONS"),
        "OPTIONS"
    );
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_HEADERS)
            .expect("allow headers should echo requested headers"),
        "x-demo, authorization"
    );

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn cors_preflight_empty_allow_lists_suppress_method_and_header_echo(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/cors", |_| NivasaResponse::text("ok"))
        .expect("route must register")
        .cors_options(
            CorsOptions::permissive()
                .allow_methods(Vec::<Method>::new())
                .allow_headers(Vec::<String>::new()),
        )
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
    let preflight = http::Request::builder()
        .method(Method::OPTIONS)
        .uri(format!("http://127.0.0.1:{port}/cors"))
        .header(ORIGIN, "https://app.example")
        .header(ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .header(ACCESS_CONTROL_REQUEST_HEADERS, "x-demo")
        .body(Empty::<Bytes>::new())?;

    let response = client.request(preflight).await?;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(response
        .headers()
        .get(ACCESS_CONTROL_ALLOW_METHODS)
        .is_none());
    assert!(response
        .headers()
        .get(ACCESS_CONTROL_ALLOW_HEADERS)
        .is_none());

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}
