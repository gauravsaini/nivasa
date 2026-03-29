use bytes::Bytes;
use http::{Method, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use nivasa_common::HttpException;
use nivasa_filters::{ArgumentsHost, ExceptionFilter, ExceptionFilterFuture};
use nivasa_http::{NivasaResponse, NivasaServer};
use nivasa_macros::{catch, catch_all};
use nivasa_routing::RouteMethod;
use serde_json::json;
use std::net::TcpListener as StdTcpListener;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::{sync::oneshot, time::sleep};

struct ErrorInterceptor;

impl nivasa_interceptors::Interceptor for ErrorInterceptor {
    type Response = NivasaResponse;

    fn intercept(
        &self,
        _context: &nivasa_interceptors::ExecutionContext,
        _next: nivasa_interceptors::CallHandler<Self::Response>,
    ) -> nivasa_interceptors::InterceptorFuture<Self::Response> {
        Box::pin(async { Err(HttpException::bad_request("match me")) })
    }
}

#[catch(HttpException)]
struct ExactHttpExceptionFilter;

impl ExceptionFilter<HttpException, NivasaResponse> for ExactHttpExceptionFilter {
    fn catch<'a>(
        &'a self,
        exception: HttpException,
        _host: ArgumentsHost,
    ) -> ExceptionFilterFuture<'a, NivasaResponse> {
        Box::pin(async move {
            NivasaResponse::new(
                StatusCode::from_u16(exception.status_code)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                json!({
                    "statusCode": exception.status_code,
                    "message": exception.message,
                    "error": exception.error,
                    "matched": "exact",
                }),
            )
            .with_header("x-matched-filter", "exact")
        })
    }
}

#[catch_all]
struct CatchAllHttpExceptionFilter;

impl ExceptionFilter<HttpException, NivasaResponse> for CatchAllHttpExceptionFilter {
    fn catch<'a>(
        &'a self,
        exception: HttpException,
        _host: ArgumentsHost,
    ) -> ExceptionFilterFuture<'a, NivasaResponse> {
        Box::pin(async move {
            NivasaResponse::new(
                StatusCode::from_u16(exception.status_code)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                json!({
                    "statusCode": exception.status_code,
                    "message": exception.message,
                    "error": exception.error,
                    "matched": "catch-all",
                }),
            )
            .with_header("x-matched-filter", "catch-all")
        })
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
async fn global_filter_prefers_exact_type_over_catch_all() -> Result<(), Box<dyn std::error::Error>>
{
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let called = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&called);

    let server = NivasaServer::builder()
        .use_global_filter(CatchAllHttpExceptionFilter)
        .use_global_filter(ExactHttpExceptionFilter)
        .interceptor(ErrorInterceptor)
        .route(RouteMethod::Get, "/filters", move |_| {
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
        .uri(format!("http://127.0.0.1:{port}/filters"))
        .body(Full::new(Bytes::new()))?;

    let response = client.request(request).await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(headers.get("x-matched-filter").unwrap(), "exact");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        json!({
            "statusCode": 400,
            "message": "match me",
            "error": "Bad Request",
            "matched": "exact"
        })
    );
    assert!(!called.load(Ordering::SeqCst));
    Ok(())
}

#[tokio::test]
async fn global_filter_uses_catch_all_when_no_exact_match_exists(
) -> Result<(), Box<dyn std::error::Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let called = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&called);

    let server = NivasaServer::builder()
        .use_global_filter(CatchAllHttpExceptionFilter)
        .interceptor(ErrorInterceptor)
        .route(RouteMethod::Get, "/filters", move |_| {
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
        .uri(format!("http://127.0.0.1:{port}/filters"))
        .body(Full::new(Bytes::new()))?;

    let response = client.request(request).await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(headers.get("x-matched-filter").unwrap(), "catch-all");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        json!({
            "statusCode": 400,
            "message": "match me",
            "error": "Bad Request",
            "matched": "catch-all"
        })
    );
    assert!(!called.load(Ordering::SeqCst));
    Ok(())
}
