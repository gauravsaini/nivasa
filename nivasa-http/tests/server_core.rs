use bytes::Bytes;
use http::{
    header::{
        ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS,
        ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
        ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD, ORIGIN,
    },
    Method,
};
use http_body_util::{BodyExt, Empty, Full};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_http::{
    run_controller_action, run_controller_action_with_body, Body, ControllerResponse, CorsOptions,
    Json, NivasaRequest, NivasaResponse, NivasaServer,
};
use nivasa_macros::{controller, impl_controller};
use nivasa_routing::RouteMethod;
use serde::Deserialize;
use std::{
    error::Error,
    net::TcpListener as StdTcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::oneshot,
    time::{sleep, timeout},
};

#[derive(Debug, Deserialize)]
struct CreateRuntimeUser {
    name: String,
}

#[controller("/controller")]
struct RuntimeResponseController;

#[impl_controller]
impl RuntimeResponseController {
    #[nivasa_macros::get("/:id")]
    fn show(
        &self,
        request: &NivasaRequest,
        #[nivasa_macros::res] response: &mut ControllerResponse,
    ) {
        let user_id = request
            .path_param("id")
            .expect("server pipeline must attach route captures before handler execution");

        response
            .status(http::StatusCode::ACCEPTED)
            .header("x-controller-runtime", "res")
            .text(format!("controller-{user_id}"));
    }
}

#[controller("/body")]
struct RuntimeBodyController;

#[impl_controller]
impl RuntimeBodyController {
    #[nivasa_macros::post("/create")]
    fn create(&self, #[nivasa_macros::body] payload: Json<CreateRuntimeUser>) -> NivasaResponse {
        let payload = payload.into_inner();

        NivasaResponse::new(
            http::StatusCode::CREATED,
            Body::json(serde_json::json!({ "name": payload.name })),
        )
        .with_header("x-controller-runtime", "body")
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
async fn server_dispatches_through_request_pipeline() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/users/:id", |request| {
            let user_id = request
                .path_param("id")
                .expect("pipeline must attach route captures");

            NivasaResponse::new(http::StatusCode::OK, Body::text(format!("user-{user_id}")))
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
async fn server_dispatches_controller_res_runtime_through_request_pipeline(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let controller = RuntimeResponseController;
    let route = RuntimeResponseController::__nivasa_controller_routes()
        .into_iter()
        .next()
        .expect("controller runtime route must exist");

    let server = NivasaServer::builder()
        .route(route.0, route.1, move |request| {
            run_controller_action(request, |request, response| {
                controller.show(request, response)
            })
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
    let uri = format!("http://127.0.0.1:{port}/controller/42").parse()?;
    let response = client.get(uri).await?;
    assert_eq!(response.status(), http::StatusCode::ACCEPTED);
    assert_eq!(
        response
            .headers()
            .get("x-controller-runtime")
            .expect("response header must exist"),
        "res"
    );

    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"controller-42"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_dispatches_controller_body_runtime_through_request_pipeline(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let controller = RuntimeBodyController;
    let route = RuntimeBodyController::__nivasa_controller_routes()
        .into_iter()
        .next()
        .expect("controller body route must exist");

    let server = NivasaServer::builder()
        .route(route.0, route.1, move |request| {
            run_controller_action_with_body::<Json<CreateRuntimeUser>, _, _>(request, |payload| {
                controller.create(payload)
            })
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

    let client: Client<HttpConnector, Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/body/create"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from_static(br#"{ "name": "Ada" }"#)))?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), http::StatusCode::CREATED);
    assert_eq!(
        response
            .headers()
            .get("x-controller-runtime")
            .expect("response header must exist"),
        "body"
    );

    let body = response.into_body().collect().await?.to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(body, serde_json::json!({ "name": "Ada" }));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_returns_bad_request_for_controller_body_runtime_extraction_failures(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let controller = RuntimeBodyController;
    let route = RuntimeBodyController::__nivasa_controller_routes()
        .into_iter()
        .next()
        .expect("controller body route must exist");

    let server = NivasaServer::builder()
        .route(route.0, route.1, move |request| {
            run_controller_action_with_body::<Json<CreateRuntimeUser>, _, _>(request, |payload| {
                controller.create(payload)
            })
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

    let client: Client<HttpConnector, Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/body/create"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from_static(br#"{"name":"Ada""#)))?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
    assert_eq!(
        response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .expect("error content type must exist"),
        "application/json"
    );

    let body = response.into_body().collect().await?.to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(body["statusCode"], 400);
    assert_eq!(body["error"], "Bad Request");
    assert!(body["message"]
        .as_str()
        .expect("error message must be a string")
        .starts_with("invalid request body:"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_returns_internal_server_error_for_panicking_handlers(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/panic", |_| {
            panic!("non-http exception");
        })
        .expect("panic route must register")
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
    let response = client
        .get(format!("http://127.0.0.1:{port}/panic").parse()?)
        .await?;
    assert_eq!(response.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"request handler failed"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_dispatches_header_versioned_routes_through_request_pipeline(
) -> Result<(), Box<dyn Error>> {
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
        server
            .listen("127.0.0.1", port)
            .await
            .expect("server must stop cleanly");
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
async fn server_dispatches_media_type_versioned_routes_through_request_pipeline(
) -> Result<(), Box<dyn Error>> {
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
        server
            .listen("127.0.0.1", port)
            .await
            .expect("server must stop cleanly");
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
async fn server_enforces_request_body_size_limit() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let handler_called = Arc::new(AtomicBool::new(false));
    let handler_called_for_route = handler_called.clone();

    let server = NivasaServer::builder()
        .route(RouteMethod::Post, "/uploads", move |_| {
            handler_called_for_route.store(true, Ordering::SeqCst);
            NivasaResponse::new(http::StatusCode::OK, Body::text("accepted"))
        })
        .expect("route must register")
        .request_body_size_limit(4)
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        server
            .listen("127.0.0.1", port)
            .await
            .expect("server must stop cleanly");
    });

    wait_for_server(port).await;

    let client: Client<HttpConnector, Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(http::Method::POST)
        .uri(format!("http://127.0.0.1:{port}/uploads"))
        .body(Full::new(Bytes::from_static(b"hello")))?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), http::StatusCode::PAYLOAD_TOO_LARGE);
    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"request body too large"));
    assert!(!handler_called.load(Ordering::SeqCst));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_times_out_slow_handlers() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/slow", |_| {
            std::thread::sleep(Duration::from_millis(100));
            NivasaResponse::new(http::StatusCode::OK, Body::text("late"))
        })
        .expect("route must register")
        .request_timeout(Duration::from_millis(25))
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
        .uri(format!("http://127.0.0.1:{port}/slow"))
        .body(Empty::<Bytes>::new())?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), http::StatusCode::REQUEST_TIMEOUT);
    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"request timed out"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_shutdown_signal_stops_accepting_connections() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder().shutdown_signal(shutdown_rx).build();

    let server_task = tokio::spawn(async move {
        server
            .listen("127.0.0.1", port)
            .await
            .expect("server must stop cleanly");
    });

    let _ = shutdown_tx.send(());
    timeout(std::time::Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_adds_cors_headers_to_pipeline_responses_when_enabled() -> Result<(), Box<dyn Error>>
{
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/users/:id", |request| {
            let user_id = request
                .path_param("id")
                .expect("pipeline must attach route captures before handler execution");

            NivasaResponse::new(http::StatusCode::OK, Body::text(format!("user-{user_id}")))
        })
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
    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("http://127.0.0.1:{port}/users/42"))
        .header(ORIGIN, "https://frontend.example")
        .body(Empty::<Bytes>::new())?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("CORS header must be present"),
        "*"
    );

    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"user-42"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_handles_cors_preflight_without_running_route_handler() -> Result<(), Box<dyn Error>>
{
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let handler_called = Arc::new(AtomicBool::new(false));
    let handler_called_for_route = Arc::clone(&handler_called);

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/users/:id", move |_| {
            handler_called_for_route.store(true, Ordering::SeqCst);
            NivasaResponse::new(http::StatusCode::OK, Body::text("user"))
        })
        .expect("route must register")
        .cors(true)
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
        .method(Method::OPTIONS)
        .uri(format!("http://127.0.0.1:{port}/users/42"))
        .header(ORIGIN, "https://frontend.example")
        .header(ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .header(
            ACCESS_CONTROL_REQUEST_HEADERS,
            "x-api-version, content-type",
        )
        .body(Empty::<Bytes>::new())?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), http::StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("CORS header must be present"),
        "*"
    );
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_METHODS)
            .expect("allow methods header must be present"),
        "GET, OPTIONS"
    );
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_HEADERS)
            .expect("allow headers header must be present"),
        "x-api-version, content-type"
    );
    let body = response.into_body().collect().await?.to_bytes();
    assert!(body.is_empty());
    assert!(!handler_called.load(Ordering::SeqCst));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_does_not_add_cors_headers_when_disabled() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/users/:id", |request| {
            let user_id = request
                .path_param("id")
                .expect("pipeline must attach route captures before handler execution");

            NivasaResponse::new(http::StatusCode::OK, Body::text(format!("user-{user_id}")))
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
        .uri(format!("http://127.0.0.1:{port}/users/7"))
        .header(ORIGIN, "https://frontend.example")
        .body(Empty::<Bytes>::new())?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), http::StatusCode::OK);
    assert!(response
        .headers()
        .get(ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());

    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"user-7"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn server_applies_configured_cors_options_to_preflight_and_responses()
-> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let handler_called = Arc::new(AtomicBool::new(false));
    let handler_called_for_route = Arc::clone(&handler_called);
    let cors = CorsOptions::permissive()
        .allow_origins(["https://frontend.example"])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(["x-api-version", "content-type"])
        .allow_credentials(true);

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/users/:id", move |request| {
            handler_called_for_route.store(true, Ordering::SeqCst);
            let user_id = request
                .path_param("id")
                .expect("pipeline must attach route captures before handler execution");

            NivasaResponse::new(http::StatusCode::OK, Body::text(format!("user-{user_id}")))
        })
        .expect("route must register")
        .cors_options(cors)
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
        .header(ORIGIN, "https://frontend.example")
        .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(
            ACCESS_CONTROL_REQUEST_HEADERS,
            "x-api-version, content-type",
        )
        .body(Empty::<Bytes>::new())?;

    let response = client.request(preflight).await?;
    assert_eq!(response.status(), http::StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("CORS origin must be present"),
        "https://frontend.example"
    );
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_METHODS)
            .expect("allow methods header must be present"),
        "GET, POST, OPTIONS"
    );
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_HEADERS)
            .expect("allow headers header must be present"),
        "x-api-version, content-type"
    );
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .expect("allow credentials header must be present"),
        "true"
    );
    let body = response.into_body().collect().await?.to_bytes();
    assert!(body.is_empty());
    assert!(!handler_called.load(Ordering::SeqCst));

    let response = client
        .request(
            http::Request::builder()
                .method(Method::GET)
                .uri(format!("http://127.0.0.1:{port}/users/42"))
                .header(ORIGIN, "https://frontend.example")
                .body(Empty::<Bytes>::new())?,
        )
        .await?;
    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("CORS origin must be present"),
        "https://frontend.example"
    );
    assert_eq!(
        response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .expect("allow credentials header must be present"),
        "true"
    );

    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"user-42"));
    assert!(handler_called.load(Ordering::SeqCst));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}
