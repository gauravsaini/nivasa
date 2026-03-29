use bytes::Bytes;
use http::{Method, StatusCode};
use http_body_util::{BodyExt, Empty};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_http::{Body, NivasaResponse, NivasaServer};
use nivasa_routing::RouteMethod;
use std::{
    error::Error,
    net::TcpListener as StdTcpListener,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};
use tokio::{sync::oneshot, time::timeout};

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

        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    panic!("server did not become ready");
}

#[tokio::test]
async fn graceful_shutdown_completes_in_flight_requests() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let (started_tx, started_rx) = oneshot::channel();
    let started_tx = Arc::new(Mutex::new(Some(started_tx)));
    let started_tx_for_handler = Arc::clone(&started_tx);
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let release_rx_for_handler = Arc::clone(&release_rx);

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/slow", move |_| {
            started_tx_for_handler
                .lock()
                .expect("must lock start gate")
                .take()
                .expect("must have a start sender")
                .send(())
                .expect("must signal request start");
            release_rx_for_handler
                .lock()
                .expect("must lock release gate")
                .recv()
                .expect("must wait for release");
            NivasaResponse::new(StatusCode::OK, Body::text("finished"))
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
        .uri(format!("http://127.0.0.1:{port}/slow"))
        .body(Empty::<Bytes>::new())?;

    let response_task = tokio::spawn(async move { client.request(request).await });

    started_rx
        .await
        .expect("handler must start before shutdown");
    let _ = shutdown_tx.send(());
    release_tx.send(()).expect("must release in-flight request");

    let response = timeout(Duration::from_secs(2), response_task).await??;
    let response = response?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"finished"));

    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}
