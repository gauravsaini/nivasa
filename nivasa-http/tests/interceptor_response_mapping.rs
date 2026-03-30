use bytes::Bytes;
use http::Method;
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use nivasa_http::{NivasaResponse, NivasaServer};
use nivasa_interceptors::{CallHandler, ExecutionContext, Interceptor, InterceptorFuture};
use nivasa_routing::RouteMethod;
use serde_json::json;
use std::net::TcpListener as StdTcpListener;
use tokio::{sync::oneshot, time::sleep};

struct ResponseMappingInterceptor;

impl Interceptor for ResponseMappingInterceptor {
    type Response = NivasaResponse;

    fn intercept(
        &self,
        _context: &ExecutionContext,
        next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        Box::pin(async move {
            let response = next.handle().await?;
            Ok(response.with_header("x-interceptor", "mapped"))
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

        sleep(std::time::Duration::from_millis(20)).await;
    }

    panic!("server did not become ready");
}

#[tokio::test]
async fn interceptor_post_processing_maps_response_body_into_a_data_envelope(
) -> Result<(), Box<dyn std::error::Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .interceptor(ResponseMappingInterceptor)
        .route(RouteMethod::Get, "/mapping", |_| {
            NivasaResponse::json(json!({ "message": "handler" }))
                .with_header("x-handler", "applied")
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("http://127.0.0.1:{port}/mapping"))
        .body(Full::new(Bytes::new()))?;

    let response = client.request(request).await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let handler_header = response
        .headers()
        .get("x-handler")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let interceptor_header = response
        .headers()
        .get("x-interceptor")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(status, http::StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_eq!(handler_header.as_deref(), Some("applied"));
    assert_eq!(interceptor_header.as_deref(), Some("mapped"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body)?,
        json!({ "data": { "message": "handler" } })
    );

    Ok(())
}
