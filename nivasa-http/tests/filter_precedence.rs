use bytes::Bytes;
use http::{Method, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use nivasa_common::HttpException;
use nivasa_filters::{
    ArgumentsHost, ExceptionFilter, ExceptionFilterFuture, ExceptionFilterMetadata,
};
use nivasa_http::{GlobalFilterBinding, NivasaResponse, NivasaServer};
use nivasa_interceptors::{CallHandler, ExecutionContext, Interceptor, InterceptorFuture};
use nivasa_routing::RouteMethod;
use serde_json::json;
use std::net::TcpListener as StdTcpListener;
use std::time::Duration;
use tokio::{sync::oneshot, time::sleep};

struct PrecedenceErrorInterceptor;

impl Interceptor for PrecedenceErrorInterceptor {
    type Response = NivasaResponse;

    fn intercept(
        &self,
        _context: &ExecutionContext,
        _next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        Box::pin(async { Err(HttpException::bad_request("precedence check")) })
    }
}

struct PrecedenceFilter {
    label: &'static str,
}

impl ExceptionFilter<HttpException, NivasaResponse> for PrecedenceFilter {
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
                    "matched": self.label,
                }),
            )
            .with_header("x-filter-source", self.label)
        })
    }
}

impl ExceptionFilterMetadata for PrecedenceFilter {
    fn exception_type(&self) -> Option<&'static str> {
        Some(std::any::type_name::<HttpException>())
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

async fn run_case(
    handler_filters: Vec<GlobalFilterBinding>,
    controller_filters: Vec<GlobalFilterBinding>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .use_global_filter(PrecedenceFilter { label: "global" })
        .interceptor(PrecedenceErrorInterceptor)
        .route_with_filters(
            RouteMethod::Get,
            "/filters",
            |_| NivasaResponse::text("handler"),
            handler_filters,
            controller_filters,
        )?
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
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let header = response
        .headers()
        .get("x-filter-source")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body)?,
        json!({
            "statusCode": 400,
            "message": "precedence check",
            "error": "Bad Request",
            "matched": header.clone().expect("filter must set a header"),
        })
    );

    Ok(json!(header))
}

#[tokio::test]
async fn handler_filters_override_controller_and_global_filters(
) -> Result<(), Box<dyn std::error::Error>> {
    let handler = GlobalFilterBinding::new(PrecedenceFilter { label: "handler" });
    let controller = GlobalFilterBinding::new(PrecedenceFilter {
        label: "controller",
    });

    let result = run_case(vec![handler], vec![controller]).await?;
    assert_eq!(result, json!("handler"));

    let result = run_case(
        Vec::new(),
        vec![GlobalFilterBinding::new(PrecedenceFilter {
            label: "controller",
        })],
    )
    .await?;
    assert_eq!(result, json!("controller"));

    let result = run_case(Vec::new(), Vec::new()).await?;
    assert_eq!(result, json!("global"));

    Ok(())
}
