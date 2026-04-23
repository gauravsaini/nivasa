use bytes::Bytes;
use http::{
    header::{ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_REQUEST_METHOD, ORIGIN},
    Method, StatusCode,
};
use http_body_util::{BodyExt, Empty};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_http::NivasaServer;
use std::{error::Error, net::TcpListener as StdTcpListener, time::Duration};
use tokio::{
    sync::oneshot,
    time::{sleep, timeout},
};

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
async fn cors_preflight_omits_allow_methods_when_requested_method_is_blank(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
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
        .method(Method::OPTIONS)
        .uri(format!("http://127.0.0.1:{port}/preflight"))
        .header(ORIGIN, "https://frontend.example")
        .header(ACCESS_CONTROL_REQUEST_METHOD, "   ")
        .body(Empty::<Bytes>::new())?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(response.headers().get(ACCESS_CONTROL_ALLOW_METHODS).is_none());

    let body = response.into_body().collect().await?.to_bytes();
    assert!(body.is_empty());

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}
