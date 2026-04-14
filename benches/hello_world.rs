use bytes::Bytes;
use criterion::{criterion_group, criterion_main, Criterion};
use http::{StatusCode, Uri};
use http_body_util::{BodyExt, Empty};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_http::{NivasaResponse, NivasaServer, NivasaServerBuilder};
use nivasa_routing::RouteMethod;
use serde_json::json;
use std::{net::TcpListener as StdTcpListener, time::Duration};
use tokio::{
    runtime::Runtime,
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

    panic!("benchmark server did not become ready");
}

fn build_hello_world_server() -> NivasaServerBuilder {
    NivasaServer::builder()
        .route(RouteMethod::Get, "/hello", |_| {
            NivasaResponse::json(json!({
                "message": "hello world"
            }))
        })
        .expect("benchmark route must register")
}

fn bench_hello_world_get_json_response(c: &mut Criterion) {
    let runtime = Runtime::new().expect("benchmark runtime must build");
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = build_hello_world_server()
        .shutdown_signal(shutdown_rx)
        .build();
    let server_task = runtime.spawn(async move {
        server
            .listen("127.0.0.1", port)
            .await
            .expect("benchmark server must stop cleanly");
    });

    runtime.block_on(wait_for_server(port));

    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let uri: Uri = format!("http://127.0.0.1:{port}/hello")
        .parse()
        .expect("benchmark URI must parse");

    c.bench_function("hello_world_get_json_response", |bench| {
        bench.iter(|| {
            runtime.block_on(async {
                let response = client
                    .get(uri.clone())
                    .await
                    .expect("benchmark request must succeed");
                assert_eq!(response.status(), StatusCode::OK);

                let body = response
                    .into_body()
                    .collect()
                    .await
                    .expect("benchmark body must collect")
                    .to_bytes();
                let payload: serde_json::Value =
                    serde_json::from_slice(&body).expect("benchmark body must be JSON");
                assert_eq!(payload, json!({ "message": "hello world" }));
            });
        });
    });

    drop(client);
    let _ = shutdown_tx.send(());
    runtime.block_on(async {
        timeout(Duration::from_secs(2), server_task)
            .await
            .expect("server task must finish in time")
            .expect("server task must not error");
    });
}

criterion_group!(benches, bench_hello_world_get_json_response);
criterion_main!(benches);
