use async_trait::async_trait;
use http::{Method, StatusCode};
use nivasa_guards::{ExecutionContext as GuardExecutionContext, Guard, GuardFuture};
use nivasa_http::{
    resolve_controller_guard_execution, run_controller_action_with_request, Body,
    GuardExecutionOutcome, NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse,
    RequestPipeline,
};
use nivasa_interceptors::{
    CallHandler, ExecutionContext as InterceptorExecutionContext, Interceptor, InterceptorFuture,
};
use nivasa_macros::{controller, impl_controller};
use nivasa_routing::{RouteDispatchOutcome, RouteDispatchRegistry, RouteMethod};
use std::sync::{Arc, Mutex};

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
        let handler_name = context
            .handler_name()
            .expect("interceptor context must carry a handler name");
        assert_eq!(handler_name, "flow");
        assert_eq!(context.request_method(), Some("GET"));
        assert_eq!(context.request_path(), Some("/lifecycle/flow"));

        Box::pin(async move {
            log.push("interceptor.pre");
            let response = next.handle().await?;
            log.push("interceptor.post");
            Ok(response.with_header("x-interceptor", "applied"))
        })
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
