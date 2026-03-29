use http::{Method, Request};
use nivasa_common::HttpException;
use nivasa_http::{
    run_controller_action, run_controller_action_with_body, run_controller_action_with_file,
    run_controller_action_with_files, run_controller_action_with_param,
    run_controller_action_with_query, run_controller_action_with_request,
    resolve_controller_guard_execution, GuardExecutionOutcome,
    upload::{FileInterceptor, FilesInterceptor, UploadedFile},
    Body, ControllerResponse, FromRequest, Json, NivasaRequest, NivasaResponse, Query,
    RequestPipeline,
};
use nivasa_guards::{ExecutionContext, Guard, GuardFuture, RolesGuard};
use nivasa_macros::{controller, impl_controller};
use nivasa_routing::{
    Controller, RouteDispatchOutcome, RouteDispatchRegistry, RouteMethod, RoutePathCaptures,
    RoutePattern,
};
use serde::Deserialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

type GuardedRouteHandler = Arc<dyn Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static>;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct CreateUser {
    name: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct UserSearch {
    page: u32,
    active: bool,
}

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/create")]
    fn create(&self) {}

    #[nivasa_macros::get("/:id")]
    fn show(&self) {}
}

#[test]
fn post_route_registration_supports_json_and_query_extraction() {
    let mut routes = RouteDispatchRegistry::new();
    for (method, path, handler) in UsersController::__nivasa_controller_routes() {
        let pattern = RoutePattern::parse(&path).expect("controller route pattern must parse");
        routes
            .register(RouteMethod::from(method), pattern, handler)
            .expect("controller route must register");
    }

    let controller = UsersController;
    controller.create();

    let request = Request::builder()
        .method(Method::POST)
        .uri("/users/create?page=2&active=true")
        .header("x-request-id", "abc123")
        .header("x-retry-count", "3")
        .body(Body::json(serde_json::json!({ "name": "Ada" })))
        .expect("request must build");
    let request = NivasaRequest::from_http(request);

    assert!(routes.resolve_match("POST", request.path()).is_some());
    assert_eq!(request.query_typed::<u32>("page").unwrap(), 2);
    assert_eq!(request.query_typed::<bool>("active").unwrap(), true);
    assert_eq!(
        request.header_typed::<String>("x-request-id").unwrap(),
        "abc123"
    );
    assert_eq!(request.header_typed::<u32>("x-retry-count").unwrap(), 3);

    let body = Json::<CreateUser>::from_request(&request).unwrap();
    assert_eq!(
        body.into_inner(),
        CreateUser {
            name: "Ada".to_string(),
        }
    );

    let query = Query::<UserSearch>::from_request(&request).unwrap();
    assert_eq!(
        query.into_inner(),
        UserSearch {
            page: 2,
            active: true,
        }
    );

    let body_json = request.extract::<Json<CreateUser>>().unwrap();
    assert_eq!(
        body_json.into_inner(),
        CreateUser {
            name: "Ada".to_string(),
        }
    );
}

#[test]
fn path_parameter_extraction_supports_typed_values() {
    let mut routes = RouteDispatchRegistry::new();
    for (method, path, handler) in UsersController::__nivasa_controller_routes() {
        let pattern = RoutePattern::parse(&path).expect("controller route pattern must parse");
        routes
            .register(RouteMethod::from(method), pattern, handler)
            .expect("controller route must register");
    }

    let controller = UsersController;
    controller.show();

    let request = NivasaRequest::new(Method::GET, "/users/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);

    pipeline.parse_request().unwrap();
    pipeline.complete_middleware().unwrap();

    let outcome = pipeline.match_route(&routes).unwrap();
    assert!(matches!(
        outcome,
        nivasa_routing::RouteDispatchOutcome::Matched(_)
    ));

    let request = pipeline.request();
    assert_eq!(request.path_params().unwrap().get("id"), Some("42"));
    assert_eq!(request.path_param("id"), Some("42"));
    assert_eq!(request.path_param_typed::<u32>("id").unwrap(), 42);

    let captures = RoutePathCaptures::from_request(request).unwrap();
    assert_eq!(captures.get("id"), Some("42"));
}

#[controller({ path: "/reports", version: "1" })]
struct VersionedReportsController;

#[impl_controller]
impl VersionedReportsController {
    #[nivasa_macros::get("/summary")]
    #[nivasa_macros::http_code(204)]
    #[nivasa_macros::header("x-controller-version", "v1")]
    fn summary(&self) {}
}

#[controller("/responses")]
struct ResponseController;

#[impl_controller]
impl ResponseController {
    #[nivasa_macros::get("/:id")]
    fn show(
        &self,
        request: &NivasaRequest,
        #[nivasa_macros::res] response: &mut ControllerResponse,
    ) {
        let id = request
            .path_param("id")
            .expect("route matching must attach captures before controller execution");

        response
            .status(http::StatusCode::CREATED)
            .header("x-controller-mode", "res")
            .json(serde_json::json!({ "id": id }));
    }
}

#[controller("/body")]
struct BodyController;

#[impl_controller]
impl BodyController {
    #[nivasa_macros::post("/create")]
    fn create(&self, #[nivasa_macros::body] payload: Json<CreateUser>) -> NivasaResponse {
        let payload = payload.into_inner();

        NivasaResponse::new(
            http::StatusCode::CREATED,
            Body::json(serde_json::json!({ "name": payload.name })),
        )
        .with_header("x-controller-mode", "body")
    }
}

#[controller("/requests")]
struct RequestController;

#[impl_controller]
impl RequestController {
    #[nivasa_macros::get("/:id")]
    fn show(&self, request: &NivasaRequest) -> NivasaResponse {
        let id = request
            .path_param("id")
            .expect("route matching must attach captures before controller execution");

        NivasaResponse::json(serde_json::json!({
            "path": request.path(),
            "id": id,
        }))
        .with_header("x-controller-mode", "req")
    }
}

struct ControllerGuardAllow;

impl Guard for ControllerGuardAllow {
    fn can_activate<'a>(&'a self, _: &'a ExecutionContext) -> GuardFuture<'a> {
        Box::pin(async { Ok(true) })
    }
}

struct ControllerGuardDeny;

impl Guard for ControllerGuardDeny {
    fn can_activate<'a>(&'a self, _: &'a ExecutionContext) -> GuardFuture<'a> {
        Box::pin(async { Err(HttpException::forbidden("controller guard blocked")) })
    }
}

#[controller("/guarded")]
#[nivasa_macros::guard(ControllerGuard)]
#[derive(Clone, Copy)]
struct GuardedController;

#[impl_controller]
impl GuardedController {
    #[nivasa_macros::get("/first")]
    fn first(&self) -> NivasaResponse {
        NivasaResponse::text("first").with_header("x-controller-mode", "guarded-first")
    }

    #[nivasa_macros::get("/second")]
    fn second(&self) -> NivasaResponse {
        NivasaResponse::text("second").with_header("x-controller-mode", "guarded-second")
    }
}

#[controller("/roles")]
#[nivasa_macros::roles("admin")]
#[derive(Clone, Copy)]
struct RolesController;

#[impl_controller]
impl RolesController {
    #[nivasa_macros::roles("editor")]
    #[nivasa_macros::get("/dashboard")]
    fn dashboard(&self) -> NivasaResponse {
        NivasaResponse::text("dashboard").with_header("x-controller-mode", "roles")
    }
}

#[controller("/params")]
struct ParamController;

#[impl_controller]
impl ParamController {
    #[nivasa_macros::get("/:id")]
    fn show(&self, id: u32) -> NivasaResponse {
        NivasaResponse::json(serde_json::json!({ "id": id }))
            .with_header("x-controller-mode", "param")
    }
}

#[controller("/queries")]
struct QueryController;

#[impl_controller]
impl QueryController {
    #[nivasa_macros::get("/search")]
    fn search(&self, query: Query<UserSearch>) -> NivasaResponse {
        let query = query.into_inner();

        NivasaResponse::json(serde_json::json!({
            "page": query.page,
            "active": query.active,
        }))
        .with_header("x-controller-mode", "query")
    }
}

#[controller("/uploads")]
struct UploadController;

#[impl_controller]
impl UploadController {
    #[nivasa_macros::post("/avatar")]
    fn avatar(&self, file: UploadedFile) -> NivasaResponse {
        NivasaResponse::json(serde_json::json!({
            "filename": file.filename(),
            "contentType": file.content_type(),
            "size": file.len(),
        }))
        .with_header("x-controller-mode", "file")
    }

    #[nivasa_macros::post("/attachments")]
    fn attachments(&self, files: Vec<UploadedFile>) -> NivasaResponse {
        let filenames = files
            .iter()
            .map(|file| file.filename().to_string())
            .collect::<Vec<_>>();

        NivasaResponse::json(serde_json::json!({
            "count": filenames.len(),
            "filenames": filenames,
        }))
        .with_header("x-controller-mode", "files")
    }
}

fn multipart_body(parts: &[(&str, &str, Option<&str>, &[u8])]) -> (String, Vec<u8>) {
    let boundary = "X-BOUNDARY";
    let mut body = Vec::new();

    for (field_name, filename, content_type, bytes) in parts {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{field_name}\"; filename=\"{filename}\"\r\n"
            )
            .as_bytes(),
        );
        if let Some(content_type) = content_type {
            body.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

#[test]
fn versioned_controller_routes_and_response_metadata_are_exposed() {
    let controller = VersionedReportsController;
    let metadata = controller.metadata();

    assert_eq!(
        VersionedReportsController::__nivasa_controller_metadata(),
        ("/reports", Some("1"))
    );
    assert_eq!(metadata.path(), "/reports");
    assert_eq!(metadata.version(), Some("1"));
    assert_eq!(metadata.versioned_path(), "/v1/reports");

    let routes = VersionedReportsController::__nivasa_controller_routes();
    assert_eq!(
        routes,
        vec![("GET", "/reports/summary".to_string(), "summary")]
    );

    let mut registry = RouteDispatchRegistry::new();
    for (method, path, handler) in &routes {
        let relative_path = path
            .strip_prefix(VersionedReportsController::__nivasa_controller_path())
            .expect("controller routes must start with the controller prefix");

        registry
            .register_controller_route(
                RouteMethod::from(*method),
                metadata.versioned_path(),
                relative_path,
                *handler,
            )
            .expect("versioned controller route must register");
    }

    let request = NivasaRequest::new(Method::GET, "/v1/reports/summary", Body::empty());
    let mut pipeline = RequestPipeline::new(request);
    pipeline.parse_request().unwrap();
    pipeline.complete_middleware().unwrap();

    let outcome = pipeline.match_route(&registry).unwrap();
    assert!(matches!(
        outcome,
        nivasa_routing::RouteDispatchOutcome::Matched(_)
    ));

    let request = pipeline.request();
    assert_eq!(request.path_params().unwrap().len(), 0);

    assert_eq!(
        VersionedReportsController::__nivasa_controller_response_metadata(),
        vec![("summary", Some(204), vec![("x-controller-version", "v1")],)]
    );
}

#[test]
fn controller_res_runtime_runs_only_after_route_matching() {
    let mut routes = RouteDispatchRegistry::new();
    let controller = ResponseController;
    let route = ResponseController::__nivasa_controller_routes()
        .into_iter()
        .next()
        .expect("response controller must expose a route");

    routes
        .register_pattern(
            RouteMethod::from(route.0),
            route.1,
            move |request: &NivasaRequest| {
                run_controller_action(request, |request, response| {
                    controller.show(request, response)
                })
            },
        )
        .expect("controller response route must register");

    let request = NivasaRequest::new(Method::GET, "/responses/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);
    pipeline.parse_request().unwrap();
    pipeline.complete_middleware().unwrap();

    let outcome = pipeline.match_route(&routes).unwrap();
    assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));
    assert_eq!(pipeline.snapshot().current_state, "GuardChain");

    let response = match outcome {
        RouteDispatchOutcome::Matched(entry) => (entry.value)(pipeline.request()),
        _ => panic!("route must match"),
    };

    assert_eq!(response.status(), http::StatusCode::CREATED);
    assert_eq!(response.headers().get("x-controller-mode").unwrap(), "res");
    assert_eq!(
        response.body(),
        &Body::json(serde_json::json!({ "id": "42" }))
    );
}

#[test]
fn controller_body_runtime_extracts_json_only_after_route_matching() {
    let mut routes = RouteDispatchRegistry::new();
    let controller = BodyController;
    let route = BodyController::__nivasa_controller_routes()
        .into_iter()
        .next()
        .expect("body controller must expose a route");

    routes
        .register_pattern(
            RouteMethod::from(route.0),
            route.1,
            move |request: &NivasaRequest| {
                run_controller_action_with_body::<Json<CreateUser>, _, _>(request, |payload| {
                    controller.create(payload)
                })
            },
        )
        .expect("controller body route must register");

    let request = NivasaRequest::new(
        Method::POST,
        "/body/create",
        Body::json(serde_json::json!({ "name": "Ada" })),
    );
    let mut pipeline = RequestPipeline::new(request);
    pipeline.parse_request().unwrap();
    pipeline.complete_middleware().unwrap();

    let outcome = pipeline.match_route(&routes).unwrap();
    assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));
    assert_eq!(pipeline.snapshot().current_state, "GuardChain");

    let response = match outcome {
        RouteDispatchOutcome::Matched(entry) => (entry.value)(pipeline.request()),
        _ => panic!("route must match"),
    };

    assert_eq!(response.status(), http::StatusCode::CREATED);
    assert_eq!(response.headers().get("x-controller-mode").unwrap(), "body");
    assert_eq!(
        response.body(),
        &Body::json(serde_json::json!({ "name": "Ada" }))
    );
}

#[test]
fn controller_body_runtime_maps_missing_body_to_bad_request() {
    let response = run_controller_action_with_body::<Json<CreateUser>, _, _>(
        &NivasaRequest::new(Method::POST, "/body/create", Body::empty()),
        |_| NivasaResponse::text("unreachable"),
    );

    assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);

    let body: serde_json::Value =
        serde_json::from_slice(&response.body().as_bytes()).expect("error payload must be json");
    assert_eq!(body["statusCode"], 400);
    assert_eq!(body["error"], "Bad Request");
    assert_eq!(body["message"], "request body is empty");
}

#[test]
fn controller_req_runtime_exposes_raw_request_only_after_route_matching() {
    let mut routes = RouteDispatchRegistry::new();
    let controller = RequestController;
    let route = RequestController::__nivasa_controller_routes()
        .into_iter()
        .next()
        .expect("request controller must expose a route");

    routes
        .register_pattern(
            RouteMethod::from(route.0),
            route.1,
            move |request: &NivasaRequest| {
                run_controller_action_with_request(request, |request| controller.show(request))
            },
        )
        .expect("controller request route must register");

    let request = NivasaRequest::new(Method::GET, "/requests/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);
    pipeline.parse_request().unwrap();
    pipeline.complete_middleware().unwrap();

    let outcome = pipeline.match_route(&routes).unwrap();
    assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));
    assert_eq!(pipeline.snapshot().current_state, "GuardChain");

    let response = match outcome {
        RouteDispatchOutcome::Matched(entry) => (entry.value)(pipeline.request()),
        _ => panic!("route must match"),
    };

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(response.headers().get("x-controller-mode").unwrap(), "req");
    assert_eq!(
        response.body(),
        &Body::json(serde_json::json!({
            "path": "/requests/42",
            "id": "42",
        }))
    );
}

async fn evaluate_controller_guard<'a, G: Guard>(
    pipeline: &mut RequestPipeline,
    guard: &'a G,
    handler: &'static str,
    controller_guards: &[&'static str],
    handler_guard_metadata: &[(&'static str, Vec<&'static str>)],
) -> GuardExecutionOutcome {
    let contract = resolve_controller_guard_execution(
        handler,
        controller_guards,
        handler_guard_metadata,
    )
    .expect("controller guard contract must exist");

    assert_eq!(contract.handler(), handler);
    assert_eq!(contract.guards(), &["ControllerGuard"]);

    pipeline
        .evaluate_guard(guard, &ExecutionContext::new(()))
        .await
        .expect("guard evaluation must advance the request pipeline")
}

#[tokio::test]
async fn controller_guard_runtime_allows_all_routes_when_the_guard_passes() {
    let controller = GuardedController;
    let controller_guards = GuardedController::__nivasa_controller_guards();
    let handler_guard_metadata = GuardedController::__nivasa_controller_guard_metadata();
    let routes = GuardedController::__nivasa_controller_routes();

    let first_called = Arc::new(AtomicBool::new(false));
    let second_called = Arc::new(AtomicBool::new(false));
    let mut registry: RouteDispatchRegistry<GuardedRouteHandler> = RouteDispatchRegistry::new();

    assert_eq!(controller_guards, vec!["ControllerGuard"]);
    assert_eq!(handler_guard_metadata.len(), routes.len());
    assert!(handler_guard_metadata
        .iter()
        .all(|(_, guards)| guards.is_empty()));

    for (method, path, handler) in &routes {
        match *handler {
            "first" => {
                let called = Arc::clone(&first_called);
                registry
                    .register_pattern(
                        RouteMethod::from(*method),
                        path.clone(),
                        Arc::new(move |request: &NivasaRequest| {
                            called.store(true, Ordering::SeqCst);
                            run_controller_action_with_request(request, |_| controller.first())
                        }),
                    )
                    .expect("guarded controller route must register");
            }
            "second" => {
                let called = Arc::clone(&second_called);
                registry
                    .register_pattern(
                        RouteMethod::from(*method),
                        path.clone(),
                        Arc::new(move |request: &NivasaRequest| {
                            called.store(true, Ordering::SeqCst);
                            run_controller_action_with_request(request, |_| controller.second())
                        }),
                    )
                    .expect("guarded controller route must register");
            }
            other => panic!("unexpected guarded controller handler `{other}`"),
        }
    }

    for (method, path, handler) in routes {
        let request = NivasaRequest::new(Method::from_bytes(method.as_bytes()).unwrap(), path, Body::empty());
        let mut pipeline = RequestPipeline::new(request);
        pipeline.parse_request().unwrap();
        pipeline.complete_middleware().unwrap();

        let outcome = pipeline.match_route(&registry).unwrap();
        assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));
        assert_eq!(pipeline.snapshot().current_state, "GuardChain");

        let guard_outcome = evaluate_controller_guard(
            &mut pipeline,
            &ControllerGuardAllow,
            handler,
            &controller_guards,
            &handler_guard_metadata,
        )
        .await;

        assert!(matches!(guard_outcome, GuardExecutionOutcome::Passed));
        assert_eq!(pipeline.snapshot().current_state, "InterceptorPre");
        pipeline.complete_interceptors_pre().unwrap();
        assert_eq!(pipeline.snapshot().current_state, "PipeTransform");
        pipeline.complete_pipes().unwrap();
        assert_eq!(pipeline.snapshot().current_state, "HandlerExecution");

        let response = match outcome {
            RouteDispatchOutcome::Matched(entry) => entry.value.as_ref()(pipeline.request()),
            _ => panic!("guarded controller route must match"),
        };

        pipeline.complete_handler().unwrap();
        assert_eq!(pipeline.snapshot().current_state, "InterceptorPost");
        pipeline.complete_interceptors_post().unwrap();
        assert_eq!(pipeline.snapshot().current_state, "SendingResponse");
        pipeline.complete_response().unwrap();
        assert_eq!(pipeline.snapshot().current_state, "Done");

        assert_eq!(response.status(), http::StatusCode::OK);
        assert_eq!(
            response.headers().get("x-controller-mode").unwrap(),
            if handler == "first" {
                "guarded-first"
            } else {
                "guarded-second"
            }
        );
        assert_eq!(
            response.body(),
            &Body::text(if handler == "first" { "first" } else { "second" })
        );
    }

    assert!(first_called.load(Ordering::SeqCst));
    assert!(second_called.load(Ordering::SeqCst));
}

#[tokio::test]
async fn controller_guard_metadata_applies_to_every_route() {
    let controller_guards = GuardedController::__nivasa_controller_guards();
    let handler_guard_metadata = GuardedController::__nivasa_controller_guard_metadata();

    assert_eq!(controller_guards, vec!["ControllerGuard"]);
    assert_eq!(
        handler_guard_metadata.len(),
        GuardedController::__nivasa_controller_routes().len()
    );
    assert!(handler_guard_metadata
        .iter()
        .all(|(_, guards)| guards.is_empty()));

    for (method, path, handler) in GuardedController::__nivasa_controller_routes() {
        let contract = resolve_controller_guard_execution(
            handler,
            &controller_guards,
            &handler_guard_metadata,
        )
        .expect("controller guard contract must exist");

        assert_eq!(contract.handler(), handler);
        assert_eq!(contract.guards(), &["ControllerGuard"]);
        assert_eq!(method, "GET");
        assert!(path.starts_with("/guarded/"));
    }
}

#[tokio::test]
async fn controller_roles_guard_uses_handler_then_class_metadata() {
    let controller = RolesController;
    let route = RolesController::__nivasa_controller_routes()
        .into_iter()
        .next()
        .expect("roles controller must expose a route");
    let controller_roles = RolesController::__nivasa_controller_roles();
    let handler_roles = RolesController::__nivasa_controller_role_metadata()
        .into_iter()
        .find(|(handler, _)| *handler == route.2)
        .expect("roles controller must expose handler roles")
        .1;

    let mut registry: RouteDispatchRegistry<GuardedRouteHandler> = RouteDispatchRegistry::new();
    registry
        .register_pattern(
            RouteMethod::from(route.0),
            route.1.clone(),
            Arc::new(move |request: &NivasaRequest| {
                run_controller_action_with_request(request, |_| controller.dashboard())
            }),
        )
        .expect("roles controller route must register");

    let request = NivasaRequest::new(Method::GET, "/roles/dashboard", Body::empty());
    let mut pipeline = RequestPipeline::new(request);
    pipeline.parse_request().unwrap();
    pipeline.complete_middleware().unwrap();

    let outcome = pipeline.match_route(&registry).unwrap();
    assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));
    assert_eq!(pipeline.snapshot().current_state, "GuardChain");

    let mut request_context = nivasa_common::RequestContext::new();
    request_context.set_handler_metadata("roles", serde_json::json!(handler_roles));
    request_context.set_class_metadata("roles", serde_json::json!(controller_roles));
    request_context.set_custom_data("roles", serde_json::json!(["editor"]));

    let guard_outcome = pipeline
        .evaluate_guard(
            &RolesGuard::new(),
            &ExecutionContext::new(()).with_request_context(request_context),
        )
        .await
        .expect("roles guard evaluation must advance the request pipeline");

    assert!(matches!(guard_outcome, GuardExecutionOutcome::Passed));
    assert_eq!(pipeline.snapshot().current_state, "InterceptorPre");
    pipeline.complete_interceptors_pre().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "PipeTransform");
    pipeline.complete_pipes().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "HandlerExecution");

    let response = match outcome {
        RouteDispatchOutcome::Matched(entry) => (entry.value)(pipeline.request()),
        _ => panic!("roles controller route must match"),
    };

    pipeline.complete_handler().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "InterceptorPost");
    pipeline.complete_interceptors_post().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "SendingResponse");
    pipeline.complete_response().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "Done");

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(response.headers().get("x-controller-mode").unwrap(), "roles");
    assert_eq!(response.body(), &Body::text("dashboard"));

    let mut fallback_registry: RouteDispatchRegistry<GuardedRouteHandler> =
        RouteDispatchRegistry::new();
    fallback_registry
        .register_pattern(
            RouteMethod::from(route.0),
            route.1.clone(),
            Arc::new(move |request: &NivasaRequest| {
                run_controller_action_with_request(request, |_| controller.dashboard())
            }),
        )
        .expect("roles controller fallback route must register");

    let mut fallback_context = nivasa_common::RequestContext::new();
    fallback_context.set_class_metadata("roles", serde_json::json!(controller_roles));
    fallback_context.set_custom_data("roles", serde_json::json!(["admin"]));

    let mut fallback_pipeline = RequestPipeline::new(NivasaRequest::new(
        Method::GET,
        "/roles/dashboard",
        Body::empty(),
    ));
    fallback_pipeline.parse_request().unwrap();
    fallback_pipeline.complete_middleware().unwrap();

    let fallback_outcome = fallback_pipeline.match_route(&fallback_registry).unwrap();
    assert!(matches!(fallback_outcome, RouteDispatchOutcome::Matched(_)));
    assert_eq!(fallback_pipeline.snapshot().current_state, "GuardChain");

    let fallback_guard_outcome = fallback_pipeline
        .evaluate_guard(
            &RolesGuard::new(),
            &ExecutionContext::new(()).with_request_context(fallback_context),
        )
        .await
        .expect("roles guard fallback evaluation must advance the request pipeline");

    assert!(matches!(fallback_guard_outcome, GuardExecutionOutcome::Passed));
    assert_eq!(fallback_pipeline.snapshot().current_state, "InterceptorPre");
}

#[tokio::test]
async fn controller_guard_runtime_blocks_all_routes_when_the_guard_errors() {
    let controller = GuardedController;
    let controller_guards = GuardedController::__nivasa_controller_guards();
    let handler_guard_metadata = GuardedController::__nivasa_controller_guard_metadata();
    let routes = GuardedController::__nivasa_controller_routes();

    let first_called = Arc::new(AtomicBool::new(false));
    let second_called = Arc::new(AtomicBool::new(false));
    let mut registry: RouteDispatchRegistry<GuardedRouteHandler> = RouteDispatchRegistry::new();

    for (method, path, handler) in &routes {
        match *handler {
            "first" => {
                let called = Arc::clone(&first_called);
                registry
                    .register_pattern(
                        RouteMethod::from(*method),
                        path.clone(),
                        Arc::new(move |request: &NivasaRequest| {
                            called.store(true, Ordering::SeqCst);
                            run_controller_action_with_request(request, |_| controller.first())
                        }),
                    )
                    .expect("guarded controller route must register");
            }
            "second" => {
                let called = Arc::clone(&second_called);
                registry
                    .register_pattern(
                        RouteMethod::from(*method),
                        path.clone(),
                        Arc::new(move |request: &NivasaRequest| {
                            called.store(true, Ordering::SeqCst);
                            run_controller_action_with_request(request, |_| controller.second())
                        }),
                    )
                    .expect("guarded controller route must register");
            }
            other => panic!("unexpected guarded controller handler `{other}`"),
        }
    }

    for (method, path, handler) in routes {
        let request = NivasaRequest::new(Method::from_bytes(method.as_bytes()).unwrap(), path, Body::empty());
        let mut pipeline = RequestPipeline::new(request);
        pipeline.parse_request().unwrap();
        pipeline.complete_middleware().unwrap();

        let outcome = pipeline.match_route(&registry).unwrap();
        assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));
        assert_eq!(pipeline.snapshot().current_state, "GuardChain");

        let guard_outcome = evaluate_controller_guard(
            &mut pipeline,
            &ControllerGuardDeny,
            handler,
            &controller_guards,
            &handler_guard_metadata,
        )
        .await;

        match guard_outcome {
            GuardExecutionOutcome::Error(error) => {
                assert_eq!(error.status_code, 403);
                assert_eq!(error.message, "controller guard blocked");
            }
            other => panic!("expected controller guard error, got {other:?}"),
        }
        assert_eq!(pipeline.snapshot().current_state, "ErrorHandling");
        pipeline.fail_filter_unhandled().unwrap();
        assert_eq!(pipeline.snapshot().current_state, "SendingResponse");
        pipeline.complete_response().unwrap();
        assert_eq!(pipeline.snapshot().current_state, "Done");
    }

    assert!(!first_called.load(Ordering::SeqCst));
    assert!(!second_called.load(Ordering::SeqCst));
}

#[test]
fn controller_param_runtime_extracts_typed_path_values_after_route_matching() {
    let mut routes = RouteDispatchRegistry::new();
    let controller = ParamController;
    let route = ParamController::__nivasa_controller_routes()
        .into_iter()
        .next()
        .expect("param controller must expose a route");

    routes
        .register_pattern(
            RouteMethod::from(route.0),
            route.1,
            move |request: &NivasaRequest| {
                run_controller_action_with_param::<u32, _, _>(request, "id", |id| {
                    controller.show(id)
                })
            },
        )
        .expect("controller param route must register");

    let request = NivasaRequest::new(Method::GET, "/params/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);
    pipeline.parse_request().unwrap();
    pipeline.complete_middleware().unwrap();

    let outcome = pipeline.match_route(&routes).unwrap();
    assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));
    assert_eq!(pipeline.snapshot().current_state, "GuardChain");

    let response = match outcome {
        RouteDispatchOutcome::Matched(entry) => (entry.value)(pipeline.request()),
        _ => panic!("route must match"),
    };

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        response.headers().get("x-controller-mode").unwrap(),
        "param"
    );
    assert_eq!(
        response.body(),
        &Body::json(serde_json::json!({ "id": 42 }))
    );
}

#[test]
fn controller_param_runtime_maps_missing_capture_to_bad_request() {
    let response = run_controller_action_with_param::<u32, _, _>(
        &NivasaRequest::new(Method::GET, "/params/42", Body::empty()),
        "id",
        |_| NivasaResponse::text("unreachable"),
    );

    assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);

    let body: serde_json::Value =
        serde_json::from_slice(&response.body().as_bytes()).expect("error payload must be json");
    assert_eq!(body["statusCode"], 400);
    assert_eq!(body["error"], "Bad Request");
    assert_eq!(body["message"], "request is missing path parameter `id`");
}

#[test]
fn controller_query_runtime_extracts_typed_query_dto_after_route_matching() {
    let mut routes = RouteDispatchRegistry::new();
    let controller = QueryController;
    let route = QueryController::__nivasa_controller_routes()
        .into_iter()
        .next()
        .expect("query controller must expose a route");

    routes
        .register_pattern(
            RouteMethod::from(route.0),
            route.1,
            move |request: &NivasaRequest| {
                run_controller_action_with_query::<UserSearch, _, _>(request, |query| {
                    controller.search(query)
                })
            },
        )
        .expect("controller query route must register");

    let request = NivasaRequest::new(
        Method::GET,
        "/queries/search?page=2&active=true",
        Body::empty(),
    );
    let mut pipeline = RequestPipeline::new(request);
    pipeline.parse_request().unwrap();
    pipeline.complete_middleware().unwrap();

    let outcome = pipeline.match_route(&routes).unwrap();
    assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));
    assert_eq!(pipeline.snapshot().current_state, "GuardChain");

    let response = match outcome {
        RouteDispatchOutcome::Matched(entry) => (entry.value)(pipeline.request()),
        _ => panic!("route must match"),
    };

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        response.headers().get("x-controller-mode").unwrap(),
        "query"
    );
    assert_eq!(
        response.body(),
        &Body::json(serde_json::json!({
            "page": 2,
            "active": true,
        }))
    );
}

#[test]
fn controller_query_runtime_maps_invalid_query_to_bad_request() {
    let response = run_controller_action_with_query::<UserSearch, _, _>(
        &NivasaRequest::new(
            Method::GET,
            "/queries/search?page=two&active=true",
            Body::empty(),
        ),
        |_| NivasaResponse::text("unreachable"),
    );

    assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);

    let body: serde_json::Value =
        serde_json::from_slice(&response.body().as_bytes()).expect("error payload must be json");
    assert_eq!(body["statusCode"], 400);
    assert_eq!(body["error"], "Bad Request");
    assert!(body["message"]
        .as_str()
        .expect("message must be string")
        .starts_with("invalid query string:"));
}

#[test]
fn controller_file_runtime_extracts_uploaded_file_after_route_matching() {
    let mut routes = RouteDispatchRegistry::new();
    let controller = UploadController;
    let route = UploadController::__nivasa_controller_routes()
        .into_iter()
        .find(|(_, path, _)| path == "/uploads/avatar")
        .expect("upload controller must expose an avatar route");

    routes
        .register_pattern(
            RouteMethod::from(route.0),
            route.1,
            move |request: &NivasaRequest| {
                let interceptor = FileInterceptor::new("avatar");
                run_controller_action_with_file(request, &interceptor, |file| {
                    controller.avatar(file)
                })
            },
        )
        .expect("controller file route must register");

    let (content_type, body) =
        multipart_body(&[("avatar", "avatar.png", Some("image/png"), b"png-data")]);
    let request = Request::builder()
        .method(Method::POST)
        .uri("/uploads/avatar")
        .header("content-type", content_type)
        .body(Body::bytes(body))
        .expect("request must build");
    let request = NivasaRequest::from_http(request);
    let mut pipeline = RequestPipeline::new(request);
    pipeline.parse_request().unwrap();
    pipeline.complete_middleware().unwrap();

    let outcome = pipeline.match_route(&routes).unwrap();
    assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));
    assert_eq!(pipeline.snapshot().current_state, "GuardChain");

    let response = match outcome {
        RouteDispatchOutcome::Matched(entry) => (entry.value)(pipeline.request()),
        _ => panic!("route must match"),
    };

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(response.headers().get("x-controller-mode").unwrap(), "file");
    assert_eq!(
        response.body(),
        &Body::json(serde_json::json!({
            "filename": "avatar.png",
            "contentType": "image/png",
            "size": 8,
        }))
    );
}

#[test]
fn controller_files_runtime_extracts_multiple_uploaded_files_after_route_matching() {
    let mut routes = RouteDispatchRegistry::new();
    let controller = UploadController;
    let route = UploadController::__nivasa_controller_routes()
        .into_iter()
        .find(|(_, path, _)| path == "/uploads/attachments")
        .expect("upload controller must expose an attachments route");

    routes
        .register_pattern(
            RouteMethod::from(route.0),
            route.1,
            move |request: &NivasaRequest| {
                let interceptor = FilesInterceptor::new("attachments");
                run_controller_action_with_files(request, &interceptor, |files| {
                    controller.attachments(files)
                })
            },
        )
        .expect("controller files route must register");

    let (content_type, body) = multipart_body(&[
        ("attachments", "one.txt", Some("text/plain"), b"first"),
        ("attachments", "two.txt", Some("text/plain"), b"second"),
    ]);
    let request = Request::builder()
        .method(Method::POST)
        .uri("/uploads/attachments")
        .header("content-type", content_type)
        .body(Body::bytes(body))
        .expect("request must build");
    let request = NivasaRequest::from_http(request);
    let mut pipeline = RequestPipeline::new(request);
    pipeline.parse_request().unwrap();
    pipeline.complete_middleware().unwrap();

    let outcome = pipeline.match_route(&routes).unwrap();
    assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));
    assert_eq!(pipeline.snapshot().current_state, "GuardChain");

    let response = match outcome {
        RouteDispatchOutcome::Matched(entry) => (entry.value)(pipeline.request()),
        _ => panic!("route must match"),
    };

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        response.headers().get("x-controller-mode").unwrap(),
        "files"
    );
    assert_eq!(
        response.body(),
        &Body::json(serde_json::json!({
            "count": 2,
            "filenames": ["one.txt", "two.txt"],
        }))
    );
}

#[test]
fn controller_file_runtime_maps_missing_content_type_to_bad_request() {
    let response = run_controller_action_with_file(
        &NivasaRequest::new(
            Method::POST,
            "/uploads/avatar",
            Body::bytes(b"raw".to_vec()),
        ),
        &FileInterceptor::new("avatar"),
        |_| NivasaResponse::text("unreachable"),
    );

    assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);

    let body: serde_json::Value =
        serde_json::from_slice(&response.body().as_bytes()).expect("error payload must be json");
    assert_eq!(body["statusCode"], 400);
    assert_eq!(body["error"], "Bad Request");
    assert_eq!(body["message"], "request is missing header `content-type`");
}
