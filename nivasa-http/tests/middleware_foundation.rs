use async_trait::async_trait;
use bytes::Bytes;
use http::{Method, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use nivasa_http::{
    Body, NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse, NivasaServer,
};
use nivasa_routing::RouteMethod;
use std::net::TcpListener as StdTcpListener;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tokio::{sync::oneshot, time::sleep};

struct PassThroughMiddleware;

#[async_trait]
impl NivasaMiddleware for PassThroughMiddleware {
    async fn use_(&self, req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        next.run(req).await
    }
}

struct HeaderInjectingMiddleware;

#[async_trait]
impl NivasaMiddleware for HeaderInjectingMiddleware {
    async fn use_(&self, mut req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        req.body_mut().clone_from(&Body::text("middleware"));
        next.run(req).await
    }
}

struct BlockingMiddleware;

#[async_trait]
impl NivasaMiddleware for BlockingMiddleware {
    async fn use_(&self, _req: NivasaRequest, _next: NextMiddleware) -> NivasaResponse {
        NivasaResponse::new(StatusCode::FORBIDDEN, Body::text("blocked by middleware"))
    }
}

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
async fn next_middleware_runs_the_terminal_handler() {
    let seen = Arc::new(AtomicBool::new(false));
    let flag = seen.clone();
    let next = NextMiddleware::new(move |request: NivasaRequest| {
        let flag = flag.clone();
        async move {
            flag.store(true, Ordering::SeqCst);
            assert_eq!(request.path(), "/middleware");
            NivasaResponse::text("ok")
        }
    });

    let response = next
        .run(NivasaRequest::new(
            Method::GET,
            "/middleware",
            Body::empty(),
        ))
        .await;

    assert!(seen.load(Ordering::SeqCst));
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body(), &Body::text("ok"));
}

#[tokio::test]
async fn middleware_can_delegate_to_the_next_handler() {
    let middleware = PassThroughMiddleware;
    let next = NextMiddleware::new(|request: NivasaRequest| async move {
        assert_eq!(request.path(), "/delegate");
        NivasaResponse::text("delegated")
    });

    let response = middleware
        .use_(
            NivasaRequest::new(Method::GET, "/delegate", Body::empty()),
            next,
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body(), &Body::text("delegated"));
}

#[tokio::test]
async fn middleware_can_mutate_the_request_before_forwarding() {
    let middleware = HeaderInjectingMiddleware;
    let next = NextMiddleware::new(|request: NivasaRequest| async move {
        assert_eq!(request.body(), &Body::text("middleware"));
        NivasaResponse::new(StatusCode::CREATED, request.body().clone())
    });

    let response = middleware
        .use_(
            NivasaRequest::new(Method::POST, "/middleware", Body::empty()),
            next,
        )
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.body(), &Body::text("middleware"));
}

#[tokio::test]
async fn server_executes_middleware_before_route_dispatch() -> Result<(), Box<dyn std::error::Error>>
{
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .middleware(HeaderInjectingMiddleware)
        .route(RouteMethod::Post, "/middleware", |request| {
            NivasaResponse::new(StatusCode::OK, request.body().clone())
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/middleware"))
        .body(Full::new(Bytes::from_static(b"original")))?;

    let response = client.request(request).await?;
    let status = response.status();
    let body = response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_ref(), b"middleware");
    Ok(())
}

#[tokio::test]
async fn server_orders_global_module_and_route_specific_middleware(
) -> Result<(), Box<dyn std::error::Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let events = Arc::new(Mutex::new(Vec::new()));

    let global_events = Arc::clone(&events);
    let module_events = Arc::clone(&events);
    let route_events = Arc::clone(&events);
    let handler_events = Arc::clone(&events);

    let server = NivasaServer::builder()
        .middleware(move |request: NivasaRequest, next: NextMiddleware| {
            let events = Arc::clone(&global_events);
            async move {
                events.lock().expect("events lock").push("global");
                next.run(request).await
            }
        })
        .module_middleware(move |request: NivasaRequest, next: NextMiddleware| {
            let events = Arc::clone(&module_events);
            async move {
                events.lock().expect("events lock").push("module");
                next.run(request).await
            }
        })
        .apply(move |request: NivasaRequest, next: NextMiddleware| {
            let events = Arc::clone(&route_events);
            async move {
                events.lock().expect("events lock").push("route");
                next.run(request).await
            }
        })
        .for_routes("/middleware")?
        .route(RouteMethod::Post, "/middleware", move |_| {
            handler_events
                .lock()
                .expect("events lock")
                .push("handler");
            NivasaResponse::text("ok")
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/middleware"))
        .body(Full::new(Bytes::new()))?;

    let response = client.request(request).await?;
    let status = response.status();
    let body = response.into_body().collect().await?.to_bytes();
    let recorded = events.lock().expect("events lock").clone();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_ref(), b"ok");
    assert_eq!(recorded, vec!["global", "module", "route", "handler"]);
    Ok(())
}

#[tokio::test]
async fn server_applies_route_specific_middleware_only_to_matching_route(
) -> Result<(), Box<dyn std::error::Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .apply(HeaderInjectingMiddleware)
        .for_routes("/middleware")?
        .route(RouteMethod::Post, "/middleware", |request| {
            NivasaResponse::new(StatusCode::OK, request.body().clone())
        })?
        .route(RouteMethod::Post, "/other", |request| {
            NivasaResponse::new(StatusCode::OK, request.body().clone())
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();

    let routed_request = http::Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/middleware"))
        .body(Full::new(Bytes::from_static(b"original")))?;
    let routed_response = client.request(routed_request).await?;
    let routed_status = routed_response.status();
    let routed_body = routed_response.into_body().collect().await?.to_bytes();

    let passthrough_request = http::Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/other"))
        .body(Full::new(Bytes::from_static(b"original")))?;
    let passthrough_response = client.request(passthrough_request).await?;
    let passthrough_status = passthrough_response.status();
    let passthrough_body = passthrough_response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(routed_status, StatusCode::OK);
    assert_eq!(routed_body.as_ref(), b"middleware");
    assert_eq!(passthrough_status, StatusCode::OK);
    assert_eq!(passthrough_body.as_ref(), b"original");
    Ok(())
}

#[tokio::test]
async fn server_excludes_route_specific_middleware_for_exact_path(
) -> Result<(), Box<dyn std::error::Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .apply(HeaderInjectingMiddleware)
        .exclude("/middleware/health")?
        .for_routes("/middleware/:kind")?
        .route(RouteMethod::Post, "/middleware/:kind", |request| {
            NivasaResponse::new(StatusCode::OK, request.body().clone())
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();

    let routed_request = http::Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/middleware/users"))
        .body(Full::new(Bytes::from_static(b"original")))?;
    let routed_response = client.request(routed_request).await?;
    let routed_status = routed_response.status();
    let routed_body = routed_response.into_body().collect().await?.to_bytes();

    let excluded_request = http::Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/middleware/health"))
        .body(Full::new(Bytes::from_static(b"original")))?;
    let excluded_response = client.request(excluded_request).await?;
    let excluded_status = excluded_response.status();
    let excluded_body = excluded_response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(routed_status, StatusCode::OK);
    assert_eq!(routed_body.as_ref(), b"middleware");
    assert_eq!(excluded_status, StatusCode::OK);
    assert_eq!(excluded_body.as_ref(), b"original");
    Ok(())
}

#[tokio::test]
async fn server_short_circuits_when_middleware_does_not_delegate(
) -> Result<(), Box<dyn std::error::Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let called = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&called);

    let server = NivasaServer::builder()
        .middleware(BlockingMiddleware)
        .route(RouteMethod::Get, "/middleware", move |_| {
            seen.store(true, Ordering::SeqCst);
            NivasaResponse::text("handler")
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("http://127.0.0.1:{port}/middleware"))
        .body(Full::new(Bytes::new()))?;

    let response = client.request(request).await?;
    let status = response.status();
    let body = response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body.as_ref(), b"blocked by middleware");
    assert!(!called.load(Ordering::SeqCst));
    Ok(())
}

#[tokio::test]
async fn server_supports_functional_middleware() -> Result<(), Box<dyn std::error::Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .middleware(|mut req: NivasaRequest, next: NextMiddleware| async move {
            req.body_mut().clone_from(&Body::text("functional"));
            next.run(req).await
        })
        .route(RouteMethod::Post, "/middleware", |request| {
            NivasaResponse::new(StatusCode::OK, request.body().clone())
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/middleware"))
        .body(Full::new(Bytes::from_static(b"original")))?;

    let response = client.request(request).await?;
    let status = response.status();
    let body = response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_ref(), b"functional");
    Ok(())
}
