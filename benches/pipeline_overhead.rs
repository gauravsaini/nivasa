use async_trait::async_trait;
use bytes::Bytes;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use http::{StatusCode, Uri};
use http_body_util::{BodyExt, Empty};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_guards::{ExecutionContext as GuardExecutionContext, Guard, GuardFuture};
use nivasa_http::{NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse, NivasaServer};
use nivasa_interceptors::{
    CallHandler, ExecutionContext as InterceptorExecutionContext, Interceptor, InterceptorFuture,
};
use nivasa_routing::RouteMethod;
use serde_json::json;
use std::{hint::black_box, net::TcpListener as StdTcpListener, time::Duration};
use tokio::{
    runtime::Runtime,
    sync::oneshot,
    time::{sleep, timeout},
};

const BASELINE_PORTS: &[u16] = &[32123, 32124, 32125];
const FULL_PORTS: &[u16] = &[32133, 32134, 32135];

fn pick_port(candidates: &[u16]) -> u16 {
    for port in candidates {
        if StdTcpListener::bind(("127.0.0.1", *port)).is_ok() {
            return *port;
        }
    }

    panic!("benchmark could not reserve a fixed loopback port");
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

fn pipeline_uri(port: u16, path: &str) -> Uri {
    format!("http://127.0.0.1:{port}{path}")
        .parse()
        .expect("benchmark URI must parse")
}

async fn assert_pipeline_response(
    client: &Client<HttpConnector, Empty<Bytes>>,
    uri: &Uri,
    expect_interceptor_header: bool,
) {
    let response = client
        .get(uri.clone())
        .await
        .expect("benchmark request must succeed");
    assert_eq!(response.status(), StatusCode::OK);

    if expect_interceptor_header {
        let header = response
            .headers()
            .get("x-pipeline-interceptor")
            .and_then(|value| value.to_str().ok());
        assert_eq!(header, Some("applied"));
    }

    let body = response
        .into_body()
        .collect()
        .await
        .expect("benchmark body must collect")
        .to_bytes();
    let payload: serde_json::Value =
        serde_json::from_slice(&body).expect("benchmark body must be JSON");
    assert_eq!(
        payload,
        json!({
            "message": "pipeline ok",
        })
    );
}

struct PipelineMiddleware {
    stage: &'static str,
    header: &'static str,
}

#[async_trait]
impl NivasaMiddleware for PipelineMiddleware {
    async fn use_(&self, mut req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        debug_assert_eq!(req.method(), http::Method::GET);
        debug_assert!(req.path().starts_with("/pipeline"));
        req.set_header(self.header, self.stage);
        next.run(req).await
    }
}

struct PipelineGuard;

impl Guard for PipelineGuard {
    fn can_activate<'a>(&'a self, context: &'a GuardExecutionContext) -> GuardFuture<'a> {
        Box::pin(async move {
            let request = context
                .request::<NivasaRequest>()
                .expect("guard context must carry the request");

            debug_assert_eq!(request.method(), http::Method::GET);
            debug_assert_eq!(request.path(), "/pipeline/full");
            debug_assert_eq!(
                request
                    .header("x-global-middleware")
                    .and_then(|value| value.to_str().ok()),
                Some("applied")
            );
            debug_assert_eq!(
                request
                    .header("x-module-middleware")
                    .and_then(|value| value.to_str().ok()),
                Some("applied")
            );
            debug_assert_eq!(
                request
                    .header("x-route-middleware")
                    .and_then(|value| value.to_str().ok()),
                Some("applied")
            );

            Ok(true)
        })
    }
}

struct PipelineInterceptor;

impl Interceptor for PipelineInterceptor {
    type Response = NivasaResponse;

    fn intercept(
        &self,
        context: &InterceptorExecutionContext,
        next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        debug_assert_eq!(context.request_method(), Some("GET"));
        debug_assert_eq!(context.request_path(), Some("/pipeline/full"));

        Box::pin(async move {
            let response = next.handle().await?;
            Ok(response.with_header("x-pipeline-interceptor", "applied"))
        })
    }
}

fn build_baseline_server(
    shutdown: oneshot::Receiver<()>,
) -> Result<NivasaServer, String> {
    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/pipeline/baseline", |_request| {
            NivasaResponse::json(json!({
                "message": "pipeline ok",
            }))
        })
        .map_err(|err| err.to_string())?
        .shutdown_signal(shutdown)
        .build();

    Ok(server)
}

fn build_full_pipeline_server(
    shutdown: oneshot::Receiver<()>,
) -> Result<NivasaServer, String> {
    let server = NivasaServer::builder()
        .middleware(PipelineMiddleware {
            stage: "applied",
            header: "x-global-middleware",
        })
        .module_middleware(PipelineMiddleware {
            stage: "applied",
            header: "x-module-middleware",
        })
        .use_global_guard(PipelineGuard)
        .interceptor(PipelineInterceptor);

    let server = server
        .apply(PipelineMiddleware {
            stage: "applied",
            header: "x-route-middleware",
        })
        .for_routes("/pipeline/full")
        .map_err(|err| err.to_string())?
        .route(RouteMethod::Get, "/pipeline/full", |_request| {
            NivasaResponse::json(json!({
                "message": "pipeline ok",
            }))
        })
        .map_err(|err| err.to_string())?
        .shutdown_signal(shutdown)
        .build();

    Ok(server)
}

fn bench_pipeline_overhead(c: &mut Criterion) {
    let runtime = Runtime::new().expect("benchmark runtime must build");
    let client = build_client();
    let mut group = c.benchmark_group("pipeline_overhead");

    let baseline_port = pick_port(BASELINE_PORTS);
    let (baseline_shutdown_tx, baseline_shutdown_rx) = oneshot::channel();
    let baseline_server =
        build_baseline_server(baseline_shutdown_rx).expect("baseline server must build");
    let baseline_task = runtime.spawn(async move {
        baseline_server
            .listen("127.0.0.1", baseline_port)
            .await
            .expect("baseline benchmark server must stop cleanly");
    });
    runtime.block_on(wait_for_server(baseline_port));
    let baseline_uri = pipeline_uri(baseline_port, "/pipeline/baseline");

    group.bench_with_input(
        BenchmarkId::new("route_only_roundtrip", "baseline"),
        &baseline_uri,
        |bench, uri| {
            bench.iter(|| {
                runtime.block_on(async {
                    assert_pipeline_response(&client, uri, false).await;
                });
            });
        },
    );

    let full_port = pick_port(FULL_PORTS);
    let (full_shutdown_tx, full_shutdown_rx) = oneshot::channel();
    let full_server =
        build_full_pipeline_server(full_shutdown_rx).expect("full pipeline server must build");
    let full_task = runtime.spawn(async move {
        full_server
            .listen("127.0.0.1", full_port)
            .await
            .expect("full pipeline benchmark server must stop cleanly");
    });
    runtime.block_on(wait_for_server(full_port));
    let full_uri = pipeline_uri(full_port, "/pipeline/full");

    group.bench_with_input(
        BenchmarkId::new("full_stack_roundtrip", "middleware_guard_interceptor"),
        &full_uri,
        |bench, uri| {
            bench.iter(|| {
                runtime.block_on(async {
                    assert_pipeline_response(&client, uri, true).await;
                });
            });
        },
    );

    group.finish();

    let _ = baseline_shutdown_tx.send(());
    let _ = full_shutdown_tx.send(());
    runtime.block_on(async {
        let _ = timeout(Duration::from_secs(2), baseline_task)
            .await
            .expect("baseline server task must finish in time")
            .expect("baseline server task must not error");
        let _ = timeout(Duration::from_secs(2), full_task)
            .await
            .expect("full pipeline server task must finish in time")
            .expect("full pipeline server task must not error");
    });

    black_box(());
}

criterion_group!(benches, bench_pipeline_overhead);
criterion_main!(benches);
