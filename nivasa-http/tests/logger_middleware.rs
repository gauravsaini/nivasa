use async_trait::async_trait;
use http::{Method, StatusCode};
use nivasa_http::{
    Body, LoggerMiddleware, NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse,
    NivasaServer, TestClient,
};
use nivasa_routing::RouteMethod;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

struct BufferWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl Write for BufferWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut buffer = self.buffer.lock().expect("buffer lock");
        buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct UsersModule;

#[async_trait]
impl NivasaMiddleware for UsersModule {
    async fn use_(&self, req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        next.run(req).await
    }
}

fn run_with_buffered_subscriber<R>(buffer: Arc<Mutex<Vec<u8>>>, f: impl FnOnce() -> R) -> R {
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_target(false)
        .compact()
        .with_writer({
            let buffer = Arc::clone(&buffer);
            move || BufferWriter {
                buffer: Arc::clone(&buffer),
            }
        })
        .finish();

    let dispatch = tracing::Dispatch::new(subscriber);
    tracing::dispatcher::with_default(&dispatch, f)
}

#[test]
fn logger_middleware_global_path_uses_route_module_metadata_for_module_name() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    run_with_buffered_subscriber(Arc::clone(&buffer), || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async {
            let server = NivasaServer::builder()
                .middleware(LoggerMiddleware::new())
                .route_with_module_middlewares::<UsersModule, _>(
                    RouteMethod::Post,
                    "/logger",
                    Vec::<UsersModule>::new(),
                    |_request| NivasaResponse::new(StatusCode::CREATED, Body::text("ok")),
                )
                .expect("module route registers")
                .build();

            let response = TestClient::new(server)
                .post("/logger")
                .header("x-request-id", "req-123")
                .header("x-user-id", "user-7")
                .send()
                .await;

            let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
                .expect("logs must be utf-8");

            assert_eq!(response.status(), StatusCode::CREATED.as_u16());
            assert_eq!(response.text(), "ok");
            assert!(logs.contains("request_id=req-123"));
            assert!(logs.contains("user_id=user-7"));
            assert!(logs.contains("module_name=UsersModule"));
            assert!(logs.contains("method=POST"));
            assert!(logs.contains("path=/logger"));
            assert!(logs.contains("status="), "logs: {logs}");
            assert!(logs.contains("duration="));
        });
    });
}

#[test]
fn logger_middleware_uses_response_user_id_when_request_is_missing_it() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    run_with_buffered_subscriber(Arc::clone(&buffer), || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async {
            let middleware = LoggerMiddleware::new();
            let next = NextMiddleware::new(|request: NivasaRequest| async move {
                assert_eq!(request.path(), "/logger");
                NivasaResponse::new(StatusCode::OK, Body::text("ok"))
                    .with_header("x-user-id", "user-from-response")
            });

            let response = middleware
                .use_(
                    NivasaRequest::new(Method::GET, "/logger", Body::empty()),
                    next,
                )
                .await;

            let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
                .expect("logs must be utf-8");

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.body(), &Body::text("ok"));
            assert!(logs.contains("user_id=user-from-response"));
            assert!(logs.contains("request_id="));
            assert!(logs.contains("module_name="));
            assert!(logs.contains("method=GET"));
            assert!(logs.contains("path=/logger"));
            assert!(logs.contains("status=200"));
        });
    });
}

#[test]
fn logger_middleware_seeds_request_id_for_server_requests_without_one() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    run_with_buffered_subscriber(Arc::clone(&buffer), || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async {
            let server = NivasaServer::builder()
                .middleware(LoggerMiddleware::new())
                .route_with_module_middlewares::<UsersModule, _>(
                    RouteMethod::Get,
                    "/logger",
                    Vec::<UsersModule>::new(),
                    |_request| NivasaResponse::new(StatusCode::OK, Body::text("ok")),
                )
                .expect("module route registers")
                .build();

            let response = TestClient::new(server).get("/logger").send().await;
            let request_id = response
                .header("x-request-id")
                .expect("server should seed a request id");

            let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
                .expect("logs must be utf-8");

            assert_eq!(response.status(), StatusCode::OK.as_u16());
            assert_eq!(response.text(), "ok");
            assert!(!request_id.is_empty());
            assert!(logs.contains(&format!("request_id={request_id}")));
            assert!(logs.contains("module_name=UsersModule"));
            assert!(logs.contains("path=/logger"));
        });
    });
}

#[test]
fn logger_middleware_uses_response_module_name_when_request_is_missing_it() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    run_with_buffered_subscriber(Arc::clone(&buffer), || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async {
            let middleware = LoggerMiddleware::new();
            let next = NextMiddleware::new(|request: NivasaRequest| async move {
                assert_eq!(request.path(), "/logger");
                NivasaResponse::new(StatusCode::OK, Body::text("ok"))
                    .with_header("x-module-name", "UsersModule")
            });

            let response = middleware
                .use_(
                    NivasaRequest::new(Method::GET, "/logger", Body::empty()),
                    next,
                )
                .await;

            let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
                .expect("logs must be utf-8");

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.body(), &Body::text("ok"));
            assert!(logs.contains("module_name=UsersModule"));
            assert!(logs.contains("request_id="));
            assert!(logs.contains("user_id="));
        });
    });
}

#[test]
fn logger_middleware_route_scoped_path_propagates_module_name_and_request_user_id() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    run_with_buffered_subscriber(Arc::clone(&buffer), || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async {
            let server = NivasaServer::builder()
                .route_with_module_middlewares::<UsersModule, _>(
                    RouteMethod::Post,
                    "/logger",
                    vec![UsersModule],
                    |_request| NivasaResponse::new(StatusCode::CREATED, Body::text("ok")),
                )
                .expect("module route registers")
                .apply(LoggerMiddleware::new())
                .for_routes("/logger")
                .expect("logger middleware registers")
                .build();

            let response = TestClient::new(server)
                .post("/logger")
                .header("x-request-id", "req-123")
                .header("x-user-id", "user-7")
                .send()
                .await;

            let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
                .expect("logs must be utf-8");

            assert_eq!(response.status(), StatusCode::CREATED.as_u16());
            assert_eq!(response.text(), "ok");
            assert!(logs.contains("request_id=req-123"));
            assert!(logs.contains("user_id=user-7"));
            assert!(logs.contains("module_name=UsersModule"));
            assert!(logs.contains("method=POST"));
            assert!(logs.contains("path=/logger"));
            assert!(logs.contains("status="), "logs: {logs}");
            assert!(logs.contains("duration="));
        });
    });
}

#[test]
fn logger_middleware_prefers_request_user_id_over_response_user_id() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    run_with_buffered_subscriber(Arc::clone(&buffer), || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async {
            let middleware = LoggerMiddleware::new();
            let next = NextMiddleware::new(|request: NivasaRequest| async move {
                assert_eq!(request.path(), "/logger");
                NivasaResponse::new(StatusCode::OK, Body::text("ok"))
                    .with_header("x-user-id", "user-from-response")
            });

            let mut request = NivasaRequest::new(Method::GET, "/logger", Body::empty());
            request.set_header("x-user-id", "user-from-request");

            let response = middleware.use_(request, next).await;

            let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
                .expect("logs must be utf-8");

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.body(), &Body::text("ok"));
            assert!(logs.contains("user_id=user-from-request"));
            assert!(!logs.contains("user_id=user-from-response"));
        });
    });
}

#[test]
fn logger_middleware_respects_stricter_log_filter() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    run_with_buffered_subscriber(Arc::clone(&buffer), || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async {
            let middleware = LoggerMiddleware::new();
            let next = NextMiddleware::new(|request: NivasaRequest| async move {
                assert_eq!(request.path(), "/logger");
                NivasaResponse::new(StatusCode::CREATED, Body::text("ok"))
            });

            let response = middleware
                .use_(
                    NivasaRequest::new(Method::POST, "/logger", Body::empty()),
                    next,
                )
                .await;

            assert_eq!(response.status(), StatusCode::CREATED);
            assert_eq!(response.body(), &Body::text("ok"));
            let _ = buffer;
        });
    });
}
