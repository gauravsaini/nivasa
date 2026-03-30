use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderValue, Method, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use nivasa_http::{
    Body, HelmetMiddleware, NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse,
    NivasaServer, RequestIdMiddleware,
};
use nivasa_routing::RouteMethod;
use std::net::TcpListener as StdTcpListener;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tokio::{sync::oneshot, time::sleep};
use uuid::Uuid;

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

struct ModuleScopedMiddleware;

#[async_trait]
impl NivasaMiddleware for ModuleScopedMiddleware {
    async fn use_(&self, mut req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        req.body_mut().clone_from(&Body::text("module"));
        next.run(req).await
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
async fn request_id_middleware_propagates_an_existing_request_id() {
    let middleware = RequestIdMiddleware::new();
    let next = NextMiddleware::new(|request: NivasaRequest| async move {
        let request_id = request
            .header("x-request-id")
            .expect("request id should be present")
            .to_str()
            .expect("request id should be valid ascii");

        assert_eq!(request_id, "req-123");
        NivasaResponse::text("ok")
    });

    let mut request = NivasaRequest::new(Method::GET, "/middleware", Body::empty());
    request.set_header("x-request-id", "req-123");

    let response = middleware.use_(request, next).await;
    let response_id = response
        .headers()
        .get("x-request-id")
        .expect("response should include the request id")
        .to_str()
        .expect("response request id should be valid ascii");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body(), &Body::text("ok"));
    assert_eq!(response_id, "req-123");
}

#[tokio::test]
async fn helmet_middleware_adds_security_headers_without_altering_response() {
    let middleware = HelmetMiddleware::new();
    let next = NextMiddleware::new(|request: NivasaRequest| async move {
        assert_eq!(request.path(), "/helmet");
        NivasaResponse::new(StatusCode::OK, Body::text("safe"))
    });

    let response = middleware
        .use_(
            NivasaRequest::new(Method::GET, "/helmet", Body::empty()),
            next,
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body(), &Body::text("safe"));
    assert_eq!(
        response.headers().get("content-security-policy"),
        Some(&HeaderValue::from_static(
            "default-src 'self'; base-uri 'self'; frame-ancestors 'none'"
        ))
    );
    assert_eq!(
        response.headers().get("referrer-policy"),
        Some(&HeaderValue::from_static("no-referrer"))
    );
    assert_eq!(
        response.headers().get("strict-transport-security"),
        Some(&HeaderValue::from_static(
            "max-age=31536000; includeSubDomains"
        ))
    );
    assert_eq!(
        response.headers().get("x-content-type-options"),
        Some(&HeaderValue::from_static("nosniff"))
    );
    assert_eq!(
        response.headers().get("x-frame-options"),
        Some(&HeaderValue::from_static("DENY"))
    );
}

#[tokio::test]
async fn request_id_middleware_generates_and_echoes_a_request_id(
) -> Result<(), Box<dyn std::error::Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .middleware(RequestIdMiddleware::new())
        .route(RouteMethod::Get, "/request-id", |request| {
            let request_id = request
                .header("x-request-id")
                .and_then(|value| value.to_str().ok())
                .expect("request id should be injected into the request");

            NivasaResponse::text(request_id.to_owned())
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("http://127.0.0.1:{port}/request-id"))
        .body(Full::new(Bytes::new()))?;

    let response = client.request(request).await?;
    let body = response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    let response_id = String::from_utf8(body.to_vec())?;
    Uuid::parse_str(&response_id).expect("generated request id should be a UUID");
    Ok(())
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
async fn server_runs_global_middleware_on_every_request() -> Result<(), Box<dyn std::error::Error>>
{
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let request_count = Arc::new(AtomicUsize::new(0));

    let middleware_request_count = Arc::clone(&request_count);
    let server = NivasaServer::builder()
        .middleware(move |request: NivasaRequest, next: NextMiddleware| {
            let request_count = Arc::clone(&middleware_request_count);
            async move {
                request_count.fetch_add(1, Ordering::SeqCst);
                next.run(request).await
            }
        })
        .route(RouteMethod::Get, "/middleware", |_| {
            NivasaResponse::text("ok")
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();

    for _ in 0..2 {
        let request = http::Request::builder()
            .method(Method::GET)
            .uri(format!("http://127.0.0.1:{port}/middleware"))
            .body(Full::new(Bytes::new()))?;

        let response = client.request(request).await?;
        let status = response.status();
        let body = response.into_body().collect().await?.to_bytes();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_ref(), b"ok");
    }

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(request_count.load(Ordering::SeqCst), 2);
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
            handler_events.lock().expect("events lock").push("handler");
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
async fn server_applies_module_middleware_only_to_bound_routes(
) -> Result<(), Box<dyn std::error::Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route_with_module_middlewares(
            RouteMethod::Post,
            "/module",
            vec![ModuleScopedMiddleware],
            |request| NivasaResponse::new(StatusCode::OK, request.body().clone()),
        )?
        .route(RouteMethod::Post, "/plain", |request| {
            NivasaResponse::new(StatusCode::OK, request.body().clone())
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();

    let module_request = http::Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/module"))
        .body(Full::new(Bytes::from_static(b"original")))?;
    let module_response = client.request(module_request).await?;
    let module_status = module_response.status();
    let module_body = module_response.into_body().collect().await?.to_bytes();

    let plain_request = http::Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/plain"))
        .body(Full::new(Bytes::from_static(b"original")))?;
    let plain_response = client.request(plain_request).await?;
    let plain_status = plain_response.status();
    let plain_body = plain_response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(module_status, StatusCode::OK);
    assert_eq!(module_body.as_ref(), b"module");
    assert_eq!(plain_status, StatusCode::OK);
    assert_eq!(plain_body.as_ref(), b"original");
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
