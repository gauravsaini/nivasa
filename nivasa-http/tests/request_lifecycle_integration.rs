use async_trait::async_trait;
use bytes::Bytes;
use http::header::CONTENT_TYPE;
use http::{Method, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use nivasa_common::HttpException;
use nivasa_filters::{
    ArgumentsHost, ExceptionFilter, ExceptionFilterFuture, ExceptionFilterMetadata,
};
use nivasa_guards::{ExecutionContext as GuardExecutionContext, Guard, GuardFuture};
use nivasa_http::{
    resolve_controller_guard_execution, run_controller_action_with_request, Body,
    GuardExecutionOutcome, NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse,
    NivasaServer, RequestPipeline,
};
use nivasa_interceptors::{
    CallHandler, ExecutionContext as InterceptorExecutionContext, Interceptor, InterceptorFuture,
};
use nivasa_macros::{controller, impl_controller};
use nivasa_pipes::{ArgumentMetadata, Pipe};
use nivasa_routing::{RouteDispatchOutcome, RouteDispatchRegistry, RouteMethod};
use serde_json::Value;
use serde_json::json;
use std::net::TcpListener as StdTcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::{sync::oneshot, time::sleep};

type LifecycleRouteHandler = Arc<dyn Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static>;

#[derive(Clone)]
struct LifecycleLog(Arc<Mutex<Vec<&'static str>>>);

impl LifecycleLog {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    fn push(&self, entry: &'static str) {
        self.0.lock().unwrap().push(entry);
    }

    fn snapshot(&self) -> Vec<&'static str> {
        self.0.lock().unwrap().clone()
    }
}

struct LifecycleMiddleware {
    log: LifecycleLog,
}

#[async_trait]
impl NivasaMiddleware for LifecycleMiddleware {
    async fn use_(&self, mut req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        self.log.push("middleware");
        req.body_mut().clone_from(&Body::text("from-middleware"));
        next.run(req).await
    }
}

struct LifecycleGuard {
    log: LifecycleLog,
}

impl Guard for LifecycleGuard {
    fn can_activate<'a>(&'a self, context: &'a GuardExecutionContext) -> GuardFuture<'a> {
        Box::pin(async move {
            self.log.push("guard");

            let request = context
                .request::<NivasaRequest>()
                .expect("guard context must carry the request");
            assert_eq!(request.body(), &Body::text("from-middleware"));
            assert_eq!(request.path(), "/lifecycle/flow");

            Ok(true)
        })
    }
}

struct LifecycleInterceptor {
    log: LifecycleLog,
}

impl Interceptor for LifecycleInterceptor {
    type Response = NivasaResponse;

    fn intercept(
        &self,
        context: &InterceptorExecutionContext,
        next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        let log = self.log.clone();
        assert_eq!(context.request_method(), Some("GET"));
        match context.request_path() {
            Some("/lifecycle/flow") => {
                assert_eq!(context.handler_name(), Some("flow"));
            }
            Some("/lifecycle/mapping") => {
                assert_eq!(context.handler_name(), None);
            }
            other => panic!("unexpected lifecycle path: {other:?}"),
        }

        Box::pin(async move {
            log.push("interceptor.pre");
            let response = next.handle().await?;
            log.push("interceptor.post");
            Ok(response.with_header("x-interceptor", "applied"))
        })
    }
}

struct RequestHeaderInterceptor {
    log: LifecycleLog,
    request: Arc<Mutex<NivasaRequest>>,
}

impl Interceptor for RequestHeaderInterceptor {
    type Response = NivasaResponse;

    fn intercept(
        &self,
        _context: &InterceptorExecutionContext,
        next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        let log = self.log.clone();
        let request = Arc::clone(&self.request);

        Box::pin(async move {
            log.push("interceptor.pre");
            request
                .lock()
                .unwrap()
                .set_header("x-pre-processing", "applied");
            let response = next.handle().await?;
            log.push("interceptor.post");
            Ok(response)
        })
    }
}

struct FullLifecycleMiddleware {
    log: LifecycleLog,
}

#[async_trait]
impl NivasaMiddleware for FullLifecycleMiddleware {
    async fn use_(&self, mut req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        self.log.push("middleware");
        req.body_mut().clone_from(&Body::text("  from-middleware  "));
        next.run(req).await
    }
}

struct FullLifecycleGuard {
    log: LifecycleLog,
}

impl Guard for FullLifecycleGuard {
    fn can_activate<'a>(&'a self, context: &'a GuardExecutionContext) -> GuardFuture<'a> {
        Box::pin(async move {
            self.log.push("guard");

            let request = context
                .request::<NivasaRequest>()
                .expect("guard context must carry the request");
            assert_eq!(request.path(), "/lifecycle/full");
            assert_eq!(request.body(), &Body::text("  from-middleware  "));

            Ok(true)
        })
    }
}

struct FullLifecyclePipe {
    log: LifecycleLog,
}

impl Pipe for FullLifecyclePipe {
    fn transform(&self, value: Value, _metadata: ArgumentMetadata) -> Result<Value, HttpException> {
        self.log.push("pipe");

        let body = value
            .as_str()
            .ok_or_else(|| HttpException::bad_request("FullLifecyclePipe expects a string body"))?;

        Ok(Value::String(body.trim().to_owned()))
    }
}

struct FullLifecycleInterceptor {
    log: LifecycleLog,
}

impl Interceptor for FullLifecycleInterceptor {
    type Response = NivasaResponse;

    fn intercept(
        &self,
        context: &InterceptorExecutionContext,
        next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        let log = self.log.clone();

        assert_eq!(context.request_method(), Some("POST"));
        assert_eq!(context.request_path(), Some("/lifecycle/full"));
        assert_eq!(context.handler_name(), None);

        Box::pin(async move {
            log.push("interceptor.pre");
            let _response = next.handle().await?;
            log.push("interceptor.post");
            Err(HttpException::bad_request("full lifecycle intercepted"))
        })
    }
}

struct FullLifecycleFilter {
    log: LifecycleLog,
}

impl ExceptionFilter<HttpException, NivasaResponse> for FullLifecycleFilter {
    fn catch<'a>(
        &'a self,
        exception: HttpException,
        host: ArgumentsHost,
    ) -> ExceptionFilterFuture<'a, NivasaResponse> {
        let log = self.log.clone();

        Box::pin(async move {
            log.push("filter");

            let request = host
                .request::<NivasaRequest>()
                .expect("filter context must expose the request");

            NivasaResponse::new(
                StatusCode::from_u16(exception.status_code)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                json!({
                    "statusCode": exception.status_code,
                    "message": exception.message,
                    "error": exception.error,
                    "requestPath": request.path(),
                    "lifecycle": log.snapshot(),
                }),
            )
        })
    }
}

impl ExceptionFilterMetadata for FullLifecycleFilter {
    fn exception_type(&self) -> Option<&'static str> {
        Some(std::any::type_name::<HttpException>())
    }
}

#[controller("/lifecycle")]
struct LifecycleController;

#[impl_controller]
impl LifecycleController {
    #[nivasa_macros::get("/flow")]
    #[nivasa_macros::guard(LifecycleGuard)]
    #[nivasa_macros::interceptor(LifecycleInterceptor)]
    fn flow(&self, request: &NivasaRequest) -> NivasaResponse {
        assert_eq!(request.body(), &Body::text("from-middleware"));

        NivasaResponse::text("handler").with_header("x-handler", "applied")
    }
}

#[tokio::test]
async fn request_lifecycle_runs_middleware_guard_interceptor_and_handler_in_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let log = LifecycleLog::new();
    let middleware = LifecycleMiddleware { log: log.clone() };
    let guard = LifecycleGuard { log: log.clone() };
    let interceptor = LifecycleInterceptor { log: log.clone() };
    let controller = LifecycleController;

    let mut registry: RouteDispatchRegistry<LifecycleRouteHandler> = RouteDispatchRegistry::new();
    let (method, path, handler_name) = LifecycleController::__nivasa_controller_routes()
        .into_iter()
        .next()
        .expect("controller must expose a route");

    assert_eq!(handler_name, "flow");
    registry
        .register_pattern(
            RouteMethod::from(method),
            path.clone(),
            Arc::new({
                let log = log.clone();
                move |request: &NivasaRequest| {
                    log.push("handler");
                    run_controller_action_with_request(request, |request| controller.flow(request))
                }
            }),
        )
        .expect("lifecycle route must register");

    let controller_guards = LifecycleController::__nivasa_controller_guards();
    let handler_guard_metadata = LifecycleController::__nivasa_controller_guard_metadata();
    let handler_interceptor_metadata =
        LifecycleController::__nivasa_controller_interceptor_metadata();

    assert_eq!(controller_guards, Vec::<&'static str>::new());
    assert_eq!(
        handler_guard_metadata,
        vec![("flow", vec!["LifecycleGuard"])]
    );
    assert_eq!(
        handler_interceptor_metadata,
        vec![("flow", vec!["LifecycleInterceptor"])]
    );

    let request = NivasaRequest::new(Method::GET, "/lifecycle/flow", Body::empty());
    let forwarded = Arc::new(tokio::sync::Mutex::new(None));
    let capture = Arc::clone(&forwarded);
    let middleware_response = middleware
        .use_(
            request,
            NextMiddleware::new(move |request: NivasaRequest| {
                let capture = Arc::clone(&capture);
                async move {
                    *capture.lock().await = Some(request);
                    NivasaResponse::new(StatusCode::NO_CONTENT, Body::empty())
                }
            }),
        )
        .await;

    assert_eq!(middleware_response.status(), StatusCode::NO_CONTENT);

    let request = forwarded
        .lock()
        .await
        .take()
        .expect("middleware must forward the request");
    let mut pipeline = RequestPipeline::new(request);
    pipeline.parse_request()?;
    pipeline.complete_middleware()?;

    let outcome = pipeline.match_route(&registry)?;
    let matched_entry = match outcome {
        RouteDispatchOutcome::Matched(entry) => entry,
        _ => panic!("expected route match"),
    };
    assert_eq!(pipeline.snapshot().current_state, "GuardChain");

    let contract = resolve_controller_guard_execution(
        handler_name,
        &controller_guards,
        &handler_guard_metadata,
    )
    .expect("guard contract must exist");
    assert_eq!(contract.handler(), "flow");
    assert_eq!(contract.guards(), &["LifecycleGuard"]);

    let guard_context = GuardExecutionContext::new(pipeline.request().clone());
    let guard_outcome = pipeline
        .evaluate_guard(&guard, &guard_context)
        .await
        .expect("guard execution must advance the request pipeline");
    assert!(matches!(guard_outcome, GuardExecutionOutcome::Passed));
    assert_eq!(pipeline.snapshot().current_state, "InterceptorPre");

    let handler = Arc::clone(&matched_entry.value);
    let request = pipeline.request().clone();
    let interceptor_context = InterceptorExecutionContext::new()
        .with_request("GET", "/lifecycle/flow")
        .with_handler_name("flow")
        .with_class_name("LifecycleController");
    let response = interceptor
        .intercept(
            &interceptor_context,
            CallHandler::new(move || {
                let handler = Arc::clone(&handler);
                let request = request.clone();
                async move { Ok((handler)(&request)) }
            }),
        )
        .await?;

    pipeline.complete_interceptors_pre()?;
    pipeline.complete_pipes()?;
    pipeline.complete_handler()?;
    pipeline.complete_interceptors_post()?;
    pipeline.complete_response()?;

    assert_eq!(pipeline.snapshot().current_state, "Done");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body(), &Body::text("handler"));
    assert_eq!(
        response
            .headers()
            .get("x-handler")
            .and_then(|value| value.to_str().ok()),
        Some("applied")
    );
    assert_eq!(
        response
            .headers()
            .get("x-interceptor")
            .and_then(|value| value.to_str().ok()),
        Some("applied")
    );
    assert_eq!(
        log.snapshot(),
        vec![
            "middleware",
            "guard",
            "interceptor.pre",
            "handler",
            "interceptor.post"
        ]
    );

    Ok(())
}

#[tokio::test]
async fn request_lifecycle_allows_pre_processing_interceptors_to_add_request_headers(
) -> Result<(), Box<dyn std::error::Error>> {
    let log = LifecycleLog::new();
    let guard = LifecycleGuard { log: log.clone() };
    let request = Arc::new(Mutex::new(NivasaRequest::new(
        Method::GET,
        "/lifecycle/flow",
        Body::text("from-middleware"),
    )));
    let interceptor = RequestHeaderInterceptor {
        log: log.clone(),
        request: Arc::clone(&request),
    };

    let mut registry: RouteDispatchRegistry<LifecycleRouteHandler> = RouteDispatchRegistry::new();
    let (method, path, handler_name) = LifecycleController::__nivasa_controller_routes()
        .into_iter()
        .next()
        .expect("controller must expose a route");

    assert_eq!(handler_name, "flow");
    registry
        .register_pattern(
            RouteMethod::from(method),
            path.clone(),
            Arc::new({
                move |request: &NivasaRequest| {
                    let pre_processing_header = request
                        .header("x-pre-processing")
                        .and_then(|value| value.to_str().ok());
                    assert_eq!(pre_processing_header, Some("applied"));
                    assert_eq!(request.path(), "/lifecycle/flow");
                    NivasaResponse::text("handler")
                }
            }),
        )
        .expect("lifecycle route must register");

    let mut pipeline = RequestPipeline::new(
        request
            .lock()
            .expect("test request lock must be available")
            .clone(),
    );
    pipeline.parse_request()?;
    pipeline.complete_middleware()?;

    let outcome = pipeline.match_route(&registry)?;
    let matched_entry = match outcome {
        RouteDispatchOutcome::Matched(entry) => entry,
        _ => panic!("expected route match"),
    };
    assert_eq!(pipeline.snapshot().current_state, "GuardChain");

    let guard_context = GuardExecutionContext::new(pipeline.request().clone());
    let guard_outcome = pipeline
        .evaluate_guard(&guard, &guard_context)
        .await
        .expect("guard execution must advance the request pipeline");
    assert!(matches!(guard_outcome, GuardExecutionOutcome::Passed));
    assert_eq!(pipeline.snapshot().current_state, "InterceptorPre");

    let handler = Arc::clone(&matched_entry.value);
    let handler_request = Arc::clone(&request);
    let interceptor_context = InterceptorExecutionContext::new()
        .with_request("GET", "/lifecycle/flow")
        .with_handler_name("flow")
        .with_class_name("LifecycleController");
    let response = interceptor
        .intercept(
            &interceptor_context,
            CallHandler::new(move || {
                let handler = Arc::clone(&handler);
                let request = Arc::clone(&handler_request);
                async move {
                    let request = request
                        .lock()
                        .expect("test request lock must be available")
                        .clone();
                    Ok((handler)(&request))
                }
            }),
        )
        .await?;

    pipeline.complete_interceptors_pre()?;
    pipeline.complete_pipes()?;
    pipeline.complete_handler()?;
    pipeline.complete_interceptors_post()?;
    pipeline.complete_response()?;

    assert_eq!(pipeline.snapshot().current_state, "Done");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        log.snapshot(),
        vec!["guard", "interceptor.pre", "interceptor.post"]
    );

    Ok(())
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
async fn request_lifecycle_maps_interceptor_responses_into_a_data_envelope(
) -> Result<(), Box<dyn std::error::Error>> {
    let log = LifecycleLog::new();
    let interceptor = LifecycleInterceptor { log: log.clone() };
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .interceptor(interceptor)
        .route(RouteMethod::Get, "/lifecycle/mapping", {
            let log = log.clone();
            move |_| {
                log.push("handler");
                NivasaResponse::json(json!({ "message": "handler" }))
                    .with_header("x-handler", "applied")
            }
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("http://127.0.0.1:{port}/lifecycle/mapping"))
        .body(Full::new(Bytes::new()))?;

    let response = client.request(request).await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
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

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_eq!(handler_header.as_deref(), Some("applied"));
    assert_eq!(interceptor_header.as_deref(), Some("applied"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body)?,
        json!({ "data": { "message": "handler" } })
    );
    assert_eq!(
        log.snapshot(),
        vec!["interceptor.pre", "handler", "interceptor.post"]
    );

    Ok(())
}

#[tokio::test]
async fn request_lifecycle_runs_through_middleware_guard_pipe_handler_and_filter(
) -> Result<(), Box<dyn std::error::Error>> {
    let log = LifecycleLog::new();
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .middleware(FullLifecycleMiddleware { log: log.clone() })
        .use_global_guard(FullLifecycleGuard { log: log.clone() })
        .use_global_pipe(FullLifecyclePipe { log: log.clone() })
        .interceptor(FullLifecycleInterceptor { log: log.clone() })
        .use_global_filter(FullLifecycleFilter { log: log.clone() })
        .route(RouteMethod::Post, "/lifecycle/full", {
            let log = log.clone();
            move |request| {
                log.push("handler");
                assert_eq!(request.path(), "/lifecycle/full");
                assert_eq!(request.body(), &Body::text("from-middleware"));
                NivasaResponse::text("handler")
            }
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move { server.listen("127.0.0.1", port).await });
    wait_for_server(port).await;

    let client = Client::builder(TokioExecutor::new()).build_http();
    let request = http::Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/lifecycle/full"))
        .header(CONTENT_TYPE, "text/plain")
        .body(Full::new(Bytes::from_static(b"ignored")))?;

    let response = client.request(request).await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response.into_body().collect().await?.to_bytes();

    let _ = shutdown_tx.send(());
    drop(client);
    server_task.await??;

    let body = serde_json::from_slice::<serde_json::Value>(&body)?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_eq!(
        body,
        json!({
            "statusCode": 400,
            "message": "full lifecycle intercepted",
            "error": "Bad Request",
            "requestPath": "/lifecycle/full",
            "lifecycle": [
                "middleware",
                "guard",
                "pipe",
                "interceptor.pre",
                "handler",
                "interceptor.post",
                "filter"
            ]
        })
    );
    assert_eq!(
        log.snapshot(),
        vec![
            "middleware",
            "guard",
            "pipe",
            "interceptor.pre",
            "handler",
            "interceptor.post",
            "filter"
        ]
    );

    Ok(())
}
