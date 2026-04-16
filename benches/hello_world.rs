use actix_web::{web, App, HttpResponse, HttpServer};
use axum::{routing::get, Json, Router};
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

fn build_client() -> Client<HttpConnector, Empty<Bytes>> {
    Client::builder(TokioExecutor::new()).build_http()
}

fn hello_world_uri(port: u16) -> Uri {
    format!("http://127.0.0.1:{port}/hello")
        .parse()
        .expect("benchmark URI must parse")
}

async fn assert_hello_world_response(
    client: &Client<HttpConnector, Empty<Bytes>>,
    uri: &Uri,
) {
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
}

fn build_nivasa_server() -> NivasaServerBuilder {
    NivasaServer::builder()
        .route(RouteMethod::Get, "/hello", |_| {
            NivasaResponse::json(json!({
                "message": "hello world"
            }))
        })
        .expect("benchmark route must register")
}

fn bench_nivasa_hello_world_get_json_response(c: &mut Criterion) {
    let runtime = Runtime::new().expect("benchmark runtime must build");
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = build_nivasa_server()
        .shutdown_signal(shutdown_rx)
        .build();
    let server_task = runtime.spawn(async move {
        server
            .listen("127.0.0.1", port)
            .await
            .expect("benchmark server must stop cleanly");
    });

    runtime.block_on(wait_for_server(port));

    let client = build_client();
    let uri = hello_world_uri(port);

    c.bench_function("nivasa_hello_world_get_json_response", |bench| {
        bench.iter(|| {
            runtime.block_on(async {
                assert_hello_world_response(&client, &uri).await;
            });
        });
    });

    drop(client);
    let _ = shutdown_tx.send(());
    runtime.block_on(async {
        let _ = timeout(Duration::from_secs(2), server_task)
            .await
            .expect("server task must finish in time")
            .expect("server task must not error");
    });
}

fn bench_actix_hello_world_get_json_response(c: &mut Criterion) {
    let runtime = Runtime::new().expect("benchmark runtime must build");
    let port = free_port();
    let server = HttpServer::new(|| {
        App::new().route(
            "/hello",
            web::get().to(|| async {
                HttpResponse::Ok().json(json!({
                    "message": "hello world"
                }))
            }),
        )
    })
    .disable_signals()
    .bind(("127.0.0.1", port))
    .expect("benchmark actix server must bind")
    .run();
    let handle = server.handle();
    let server_task = runtime.spawn(server);

    runtime.block_on(wait_for_server(port));

    let client = build_client();
    let uri = hello_world_uri(port);

    c.bench_function("actix_hello_world_get_json_response", |bench| {
        bench.iter(|| {
            runtime.block_on(async {
                assert_hello_world_response(&client, &uri).await;
            });
        });
    });

    drop(client);
    runtime.block_on(async {
        handle.stop(true).await;
        let _ = timeout(Duration::from_secs(2), server_task)
            .await
            .expect("server task must finish in time")
            .expect("server task must not error");
    });
}

fn bench_axum_hello_world_get_json_response(c: &mut Criterion) {
    let runtime = Runtime::new().expect("benchmark runtime must build");
    let port = free_port();
    let app = Router::new().route(
        "/hello",
        get(|| async {
            Json(json!({
                "message": "hello world"
            }))
        }),
    );
    let listener = runtime.block_on(async {
        tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .expect("benchmark axum listener must bind")
    });
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server_task = runtime.spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("benchmark axum server must stop cleanly");
    });

    runtime.block_on(wait_for_server(port));

    let client = build_client();
    let uri = hello_world_uri(port);

    c.bench_function("axum_hello_world_get_json_response", |bench| {
        bench.iter(|| {
            runtime.block_on(async {
                assert_hello_world_response(&client, &uri).await;
            });
        });
    });

    drop(client);
    let _ = shutdown_tx.send(());
    runtime.block_on(async {
        let _ = timeout(Duration::from_secs(2), server_task)
            .await
            .expect("server task must finish in time")
            .expect("server task must not error");
    });
}

criterion_group!(
    benches,
    bench_nivasa_hello_world_get_json_response,
    bench_actix_hello_world_get_json_response,
    bench_axum_hello_world_get_json_response
);
criterion_main!(benches);
