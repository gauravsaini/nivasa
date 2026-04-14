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
    let listener = match StdTcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(err) => panic!("must bind an ephemeral port: {err}"),
    };
    match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(err) => panic!("must read ephemeral port: {err}"),
    }
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
            let mut started_tx = match started_tx_for_handler.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let Some(started_tx) = started_tx.take() else {
                panic!("must have a start sender");
            };
            if started_tx.send(()).is_err() {
                panic!("must notify handler start");
            }

            let release_rx = match release_rx_for_handler.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Err(err) = release_rx.recv() {
                panic!("must wait for release signal: {err}");
            }
            NivasaResponse::new(StatusCode::OK, Body::text("finished"))
        })
        .unwrap_or_else(|err| panic!("route must register: {err}"))
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        if let Err(err) = server.listen("127.0.0.1", port).await {
            panic!("server must stop cleanly: {err}");
        }
    });

    wait_for_server(port).await;

    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("http://127.0.0.1:{port}/slow"))
        .body(Empty::<Bytes>::new())?;

    let response_task = tokio::spawn(async move { client.request(request).await });

    if let Err(err) = started_rx.await {
        panic!("handler must start before shutdown: {err}");
    }
    let _ = shutdown_tx.send(());
    if release_tx.send(()).is_err() {
        panic!("must release handler");
    }

    let response = timeout(Duration::from_secs(2), response_task).await??;
    let response = response?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"finished"));

    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}
