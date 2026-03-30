use bytes::Bytes;
use http::{Method, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use nivasa_core::di::{DependencyContainer, ProviderScope};
use nivasa_http::{NivasaResponse, NivasaServer};
use nivasa_interceptors::{
    CallHandler, ExecutionContext, Interceptor, InterceptorFuture,
};
use nivasa_macros::injectable;
use nivasa_routing::RouteMethod;
use std::net::TcpListener as StdTcpListener;
use std::sync::Arc;
use std::time::Duration;
use tokio::{sync::oneshot, time::sleep};

#[derive(Clone)]
#[injectable]
struct PrefixHeaderInterceptor {
    #[inject]
    prefix: Arc<String>,
}

impl Interceptor for PrefixHeaderInterceptor {
    type Response = NivasaResponse;

    fn intercept(
        &self,
        _context: &ExecutionContext,
        next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        let prefix = Arc::clone(&self.prefix);

        Box::pin(async move {
            let response = next.handle().await?;
            Ok(response.with_header("x-interceptor-prefix", prefix.as_str()))
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
async fn injectable_interceptor_receives_dependencies_from_the_container(
) -> Result<(), Box<dyn std::error::Error>> {
    let container = DependencyContainer::new();
    container.register_value(String::from("di-ready")).await;
    container
        .register_injectable::<PrefixHeaderInterceptor>(
            ProviderScope::Singleton,
            <PrefixHeaderInterceptor as nivasa_core::di::provider::Injectable>::dependencies(),
        )
        .await;
    container.initialize().await?;

    let interceptor = container.resolve::<PrefixHeaderInterceptor>().await?;
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .interceptor(interceptor.as_ref().clone())
        .route(RouteMethod::Get, "/injectable-interceptor", |_| {
            NivasaResponse::text("handler")
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("http://127.0.0.1:{port}/injectable-interceptor"))
        .body(Full::new(Bytes::new()))?;

    let response = client.request(request).await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers
            .get("x-interceptor-prefix")
            .expect("injectable interceptor must add its header")
            .to_str()?,
        "di-ready"
    );
    assert_eq!(body.as_ref(), b"{\"data\":\"handler\"}");
    Ok(())
}
