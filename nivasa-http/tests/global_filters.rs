use bytes::Bytes;
use http::{Method, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use nivasa_common::HttpException;
use nivasa_filters::{ArgumentsHost, ExceptionFilter, ExceptionFilterFuture};
use nivasa_http::{NivasaRequest, NivasaResponse, NivasaServer};
use nivasa_interceptors::{CallHandler, ExecutionContext, Interceptor, InterceptorFuture};
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

impl Interceptor for ErrorInterceptor {
    type Response = NivasaResponse;

    fn intercept(
        &self,
        _context: &ExecutionContext,
        _next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        Box::pin(async { Err(HttpException::bad_request("global filter intercepted")) })
    }
}

struct RequestAwareGlobalFilter;

impl ExceptionFilter<HttpException, NivasaResponse> for RequestAwareGlobalFilter {
    fn catch<'a>(
        &'a self,
        exception: HttpException,
        host: ArgumentsHost,
    ) -> ExceptionFilterFuture<'a, NivasaResponse> {
        Box::pin(async move {
            let request = host
                .request::<NivasaRequest>()
                .expect("global filter must receive the request context");

            NivasaResponse::new(
                StatusCode::from_u16(exception.status_code)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                json!({
                    "statusCode": exception.status_code,
                    "message": exception.message,
                    "error": exception.error,
                    "requestPath": request.path(),
                }),
            )
            .with_header("x-global-filter", "applied")
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
async fn global_filter_handles_http_exception_and_sees_request_context(
) -> Result<(), Box<dyn std::error::Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let called = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&called);

    let server = NivasaServer::builder()
        .use_global_filter(RequestAwareGlobalFilter)
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
    assert_eq!(headers.get("x-global-filter").unwrap(), "applied");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        json!({
            "statusCode": 400,
            "message": "global filter intercepted",
            "error": "Bad Request",
            "requestPath": "/filters"
        })
    );
    assert!(!called.load(Ordering::SeqCst));
    Ok(())
}
