use bytes::Bytes;
use http::{Method, Request, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_common::HttpException;
use nivasa_core::di::DependencyContainer;
use nivasa_http::{NivasaResponse, NivasaServer};
use nivasa_macros::injectable;
use nivasa_pipes::{ArgumentMetadata, Pipe};
use nivasa_routing::RouteMethod;
use serde_json::Value;
use std::error::Error;
use std::net::TcpListener as StdTcpListener;
use std::sync::Arc;
use std::time::Duration;
use tokio::{sync::oneshot, time::{sleep, timeout}};

struct PipePrefixConfig {
    prefix: &'static str,
}

#[derive(Clone)]
#[injectable]
struct PrefixBodyPipe {
    #[inject]
    config: Arc<PipePrefixConfig>,
}

impl Pipe for PrefixBodyPipe {
    fn transform(&self, value: Value, _metadata: ArgumentMetadata) -> Result<Value, HttpException> {
        let body = value
            .as_str()
            .ok_or_else(|| HttpException::bad_request("PrefixBodyPipe expects a string value"))?;

        Ok(Value::String(format!("{}{}", self.config.prefix, body)))
    }
}

fn free_port() -> u16 {
    StdTcpListener::bind("127.0.0.1:0")
        .expect("must bind an ephemeral port")
        .local_addr()
        .expect("must inspect bound address")
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
async fn injectable_pipe_struct_can_be_resolved_and_used_as_global_pipe(
) -> Result<(), Box<dyn Error>> {
    let container = DependencyContainer::new();
    container
        .register_value(PipePrefixConfig { prefix: "di-pipe: " })
        .await;
    PrefixBodyPipe::__nivasa_register(&container).await;

    let pipe = container
        .resolve::<PrefixBodyPipe>()
        .await
        .expect("injectable pipe must resolve from the container");

    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .use_global_pipe(pipe.as_ref().clone())
        .route(RouteMethod::Post, "/pipe-di", |request| {
            NivasaResponse::text(
                request
                    .extract::<String>()
                    .expect("global pipe must rewrite the request body"),
            )
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client: Client<HttpConnector, Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let request = Request::post(format!("http://127.0.0.1:{port}/pipe-di"))
        .method(Method::POST)
        .header(http::header::CONTENT_TYPE, "text/plain")
        .body(Full::new(Bytes::from_static(b"payload")))?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"di-pipe: payload"));

    drop(client);
    let _ = shutdown_tx.send(());
    let _ = timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}
