extern crate self as nivasa_core;

pub mod di {
    use std::any::{Any, TypeId};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ProviderScope {
        Singleton,
        Scoped,
        Transient,
    }

    pub mod error {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum DiError {
            ProviderNotFound(&'static str),
            TypeMismatch(&'static str),
        }
    }

    pub mod provider {
        use super::error::DiError;
        use async_trait::async_trait;
        use std::any::TypeId;

        #[async_trait]
        pub trait Injectable: Sized + Send + Sync + 'static {
            async fn build(
                container: &super::container::DependencyContainer,
            ) -> Result<Self, DiError>;

            fn dependencies() -> Vec<TypeId>;
        }
    }

    pub mod container {
        use super::error::DiError;
        use super::provider::Injectable;
        use super::{Any, Arc, HashMap, Mutex, TypeId};

        #[derive(Clone, Default)]
        pub struct DependencyContainer {
            values: Arc<Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>>,
        }

        impl DependencyContainer {
            pub fn new() -> Self {
                Self::default()
            }

            pub async fn register_value<T: Send + Sync + 'static>(&self, value: T) {
                self.values
                    .lock()
                    .expect("test dependency container lock must be available")
                    .insert(TypeId::of::<T>(), Arc::new(value));
            }

            pub async fn register_injectable<T: Injectable>(
                &self,
                _scope: super::ProviderScope,
                _dependencies: Vec<TypeId>,
            ) {
                let instance = T::build(self)
                    .await
                    .expect("test injectable must build successfully");
                self.values
                    .lock()
                    .expect("test dependency container lock must be available")
                    .insert(TypeId::of::<T>(), Arc::new(instance));
            }

            pub async fn initialize(&self) -> Result<(), DiError> {
                Ok(())
            }

            pub async fn resolve<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, DiError> {
                let value = self
                    .values
                    .lock()
                    .expect("test dependency container lock must be available")
                    .get(&TypeId::of::<T>())
                    .cloned()
                    .ok_or(DiError::ProviderNotFound(std::any::type_name::<T>()))?;

                Arc::downcast::<T>(value)
                    .map_err(|_| DiError::TypeMismatch(std::any::type_name::<T>()))
            }
        }
    }

    pub use container::DependencyContainer;
}

use http::{Method, Request};
use nivasa_common::HttpException;
use nivasa_core::di::{DependencyContainer, ProviderScope};
use nivasa_guards::{ExecutionContext, Guard, GuardFuture, RolesGuard, ThrottlerGuard};
use nivasa_http::{
    apply_controller_response_metadata, resolve_controller_guard_execution, run_controller_action,
    run_controller_action_with_body, run_controller_action_with_custom_param,
    run_controller_action_with_file, run_controller_action_with_files,
    run_controller_action_with_header, run_controller_action_with_headers,
    run_controller_action_with_ip, run_controller_action_with_param,
    run_controller_action_with_query, run_controller_action_with_request,
    run_controller_action_with_session,
    upload::{FileInterceptor, FilesInterceptor, UploadedFile},
    Body, ClientIp, ControllerParamExtractor, ControllerResponse, FromRequest,
    GuardExecutionOutcome, Json, NivasaRequest, NivasaResponse, Query, RequestPipeline,
};
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
use std::time::Duration;

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

mod route_registration {
    use super::*;

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
        assert!(request.query_typed::<bool>("active").unwrap());
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
}

#[controller({ path: "/reports", version: "1" })]
struct VersionedReportsController;

#[impl_controller]
impl VersionedReportsController {
    #[allow(dead_code)]
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

#[controller("/throttle")]
#[nivasa_macros::guard(ThrottlerGuard)]
#[derive(Clone, Copy)]
struct ThrottledController;

#[impl_controller]
impl ThrottledController {
    #[nivasa_macros::get("/allow")]
    fn allow(&self) -> NivasaResponse {
        NivasaResponse::text("allowed").with_header("x-controller-mode", "throttled")
    }
}

#[derive(Debug)]
struct GuardAllowance {
    allowed: bool,
}

#[nivasa_macros::injectable]
struct InjectableGuard {
    #[inject]
    allowance: Arc<GuardAllowance>,
}

impl Guard for InjectableGuard {
    fn can_activate<'a>(&'a self, _: &'a ExecutionContext) -> GuardFuture<'a> {
        Box::pin(async move { Ok(self.allowance.allowed) })
    }
}

#[controller("/injected-guard")]
#[nivasa_macros::guard(InjectableGuard)]
#[derive(Clone, Copy)]
struct InjectableGuardController;

#[impl_controller]
impl InjectableGuardController {
    #[nivasa_macros::get("/check")]
    fn check(&self) -> NivasaResponse {
        NivasaResponse::text("guarded").with_header("x-controller-mode", "injectable-guard")
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestSession {
    user_id: u32,
}

struct TenantExtractor;

impl ControllerParamExtractor<String> for TenantExtractor {
    fn extract(&self, request: &NivasaRequest) -> Result<String, HttpException> {
        request
            .header("x-tenant")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned)
            .ok_or_else(|| HttpException::bad_request("request tenant is missing"))
    }
}

#[controller("/context")]
struct ContextController;

#[impl_controller]
impl ContextController {
    #[nivasa_macros::get("/headers")]
    fn headers(&self, headers: http::HeaderMap) -> NivasaResponse {
        let trace_id = headers
            .get("x-trace-id")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("missing");

        NivasaResponse::json(serde_json::json!({ "traceId": trace_id }))
            .with_header("x-controller-mode", "headers")
    }

    #[nivasa_macros::get("/header")]
    fn header(&self, tenant: String) -> NivasaResponse {
        NivasaResponse::json(serde_json::json!({ "tenant": tenant }))
            .with_header("x-controller-mode", "header")
    }

    #[nivasa_macros::get("/ip")]
    fn ip(&self, ip: String) -> NivasaResponse {
        NivasaResponse::json(serde_json::json!({ "ip": ip })).with_header("x-controller-mode", "ip")
    }

    #[nivasa_macros::get("/session")]
    fn session(&self, session: TestSession) -> NivasaResponse {
        NivasaResponse::json(serde_json::json!({ "userId": session.user_id }))
            .with_header("x-controller-mode", "session")
    }

    #[nivasa_macros::get("/custom")]
    fn custom(&self, tenant: String) -> NivasaResponse {
        NivasaResponse::json(serde_json::json!({ "tenant": tenant }))
            .with_header("x-controller-mode", "custom")
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

mod runtime_extraction {
    use super::*;

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

        let response = apply_controller_response_metadata(
            NivasaResponse::text("summary"),
            "summary",
            &VersionedReportsController::__nivasa_controller_response_metadata(),
        );
        assert_eq!(response.status(), http::StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers().get("x-controller-version").unwrap(),
            "v1"
        );
        assert_eq!(response.body(), &Body::text("summary"));

        let unchanged = apply_controller_response_metadata(
            NivasaResponse::text("plain"),
            "missing",
            &VersionedReportsController::__nivasa_controller_response_metadata(),
        );
        assert_eq!(unchanged.status(), http::StatusCode::OK);
        assert_eq!(unchanged.headers().get("x-controller-version"), None);
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

        let body: serde_json::Value = serde_json::from_slice(&response.body().as_bytes())
            .expect("error payload must be json");
        assert_eq!(body["statusCode"], 400);
        assert_eq!(body["error"], "Bad Request");
        assert_eq!(body["message"], "request body is empty");
    }

    #[test]
    fn controller_body_runtime_maps_invalid_json_to_bad_request() {
        let response = run_controller_action_with_body::<Json<CreateUser>, _, _>(
            &NivasaRequest::new(Method::POST, "/body/create", Body::text(r#"{"name":"Ada""#)),
            |_| NivasaResponse::text("unreachable"),
        );

        assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);

        let body: serde_json::Value = serde_json::from_slice(&response.body().as_bytes())
            .expect("error payload must be json");
        assert_eq!(body["statusCode"], 400);
        assert_eq!(body["error"], "Bad Request");
        assert!(body["message"]
            .as_str()
            .expect("error message must be a string")
            .starts_with("invalid request body:"));
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
}

mod guards {
    use super::*;

    async fn evaluate_controller_guard<G: Guard>(
        pipeline: &mut RequestPipeline,
        guard: &G,
        handler: &'static str,
        controller_guards: &[&'static str],
        handler_guard_metadata: &[(&'static str, Vec<&'static str>)],
    ) -> GuardExecutionOutcome {
        let contract =
            resolve_controller_guard_execution(handler, controller_guards, handler_guard_metadata)
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
            let request = NivasaRequest::new(
                Method::from_bytes(method.as_bytes()).unwrap(),
                path,
                Body::empty(),
            );
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
                &Body::text(if handler == "first" {
                    "first"
                } else {
                    "second"
                })
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
    async fn throttler_guard_controller_proof_compiles_and_runs_when_configured() {
        let controller = ThrottledController;
        let controller_guards = ThrottledController::__nivasa_controller_guards();
        let handler_guard_metadata = ThrottledController::__nivasa_controller_guard_metadata();
        let route = ThrottledController::__nivasa_controller_routes()
            .into_iter()
            .next()
            .expect("throttled controller must expose a route");

        assert_eq!(controller_guards, vec!["ThrottlerGuard"]);
        assert_eq!(handler_guard_metadata.len(), 1);
        assert!(handler_guard_metadata
            .iter()
            .all(|(_, guards)| guards.is_empty()));

        let mut registry: RouteDispatchRegistry<GuardedRouteHandler> = RouteDispatchRegistry::new();
        registry
            .register_pattern(
                RouteMethod::from(route.0),
                route.1.clone(),
                Arc::new(move |request: &NivasaRequest| {
                    run_controller_action_with_request(request, |_| controller.allow())
                }),
            )
            .expect("throttled controller route must register");

        let request = NivasaRequest::new(Method::GET, "/throttle/allow", Body::empty());
        let mut pipeline = RequestPipeline::new(request);
        pipeline.parse_request().unwrap();
        pipeline.complete_middleware().unwrap();

        let outcome = pipeline.match_route(&registry).unwrap();
        assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));
        assert_eq!(pipeline.snapshot().current_state, "GuardChain");

        let contract = resolve_controller_guard_execution(
            route.2,
            &controller_guards,
            &handler_guard_metadata,
        )
        .expect("throttler guard contract must exist");
        assert_eq!(contract.handler(), route.2);
        assert_eq!(contract.guards(), &["ThrottlerGuard"]);

        let guard_outcome = pipeline
            .evaluate_guard(
                &ThrottlerGuard::new(10, Duration::from_secs(60)),
                &ExecutionContext::new(()),
            )
            .await
            .expect("throttler guard evaluation must advance the request pipeline");

        assert!(matches!(guard_outcome, GuardExecutionOutcome::Passed));
        assert_eq!(pipeline.snapshot().current_state, "InterceptorPre");
        pipeline.complete_interceptors_pre().unwrap();
        assert_eq!(pipeline.snapshot().current_state, "PipeTransform");
        pipeline.complete_pipes().unwrap();
        assert_eq!(pipeline.snapshot().current_state, "HandlerExecution");

        let response = match outcome {
            RouteDispatchOutcome::Matched(entry) => (entry.value)(pipeline.request()),
            _ => panic!("throttled controller route must match"),
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
            "throttled"
        );
        assert_eq!(response.body(), &Body::text("allowed"));
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
        assert_eq!(
            response.headers().get("x-controller-mode").unwrap(),
            "roles"
        );
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

        assert!(matches!(
            fallback_guard_outcome,
            GuardExecutionOutcome::Passed
        ));
        assert_eq!(fallback_pipeline.snapshot().current_state, "InterceptorPre");
    }

    #[tokio::test]
    async fn controller_guard_resolves_from_dependency_container() {
        let controller = InjectableGuardController;
        let controller_guards = InjectableGuardController::__nivasa_controller_guards();
        let handler_guard_metadata =
            InjectableGuardController::__nivasa_controller_guard_metadata();
        let route = InjectableGuardController::__nivasa_controller_routes()
            .into_iter()
            .next()
            .expect("injectable guard controller must expose a route");

        let container = DependencyContainer::new();
        container
            .register_value(GuardAllowance { allowed: true })
            .await;
        container
            .register_injectable::<InjectableGuard>(
                ProviderScope::Singleton,
                <InjectableGuard as nivasa_core::di::provider::Injectable>::dependencies(),
            )
            .await;
        container.initialize().await.unwrap();

        let guard = container.resolve::<InjectableGuard>().await.unwrap();

        let mut registry: RouteDispatchRegistry<GuardedRouteHandler> = RouteDispatchRegistry::new();
        registry
            .register_pattern(
                RouteMethod::from(route.0),
                route.1.clone(),
                Arc::new(move |request: &NivasaRequest| {
                    run_controller_action_with_request(request, |_| controller.check())
                }),
            )
            .expect("injectable guard controller route must register");

        let request = NivasaRequest::new(Method::GET, "/injected-guard/check", Body::empty());
        let mut pipeline = RequestPipeline::new(request);
        pipeline.parse_request().unwrap();
        pipeline.complete_middleware().unwrap();

        let outcome = pipeline.match_route(&registry).unwrap();
        assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));
        assert_eq!(pipeline.snapshot().current_state, "GuardChain");

        let contract = resolve_controller_guard_execution(
            route.2,
            &controller_guards,
            &handler_guard_metadata,
        )
        .expect("injectable guard contract must exist");
        assert_eq!(contract.guards(), &["InjectableGuard"]);

        let guard_outcome = pipeline
            .evaluate_guard(guard.as_ref(), &ExecutionContext::new(()))
            .await
            .expect("injectable guard evaluation must advance the request pipeline");

        assert!(matches!(guard_outcome, GuardExecutionOutcome::Passed));
        assert_eq!(pipeline.snapshot().current_state, "InterceptorPre");
        pipeline.complete_interceptors_pre().unwrap();
        assert_eq!(pipeline.snapshot().current_state, "PipeTransform");
        pipeline.complete_pipes().unwrap();
        assert_eq!(pipeline.snapshot().current_state, "HandlerExecution");

        let response = match outcome {
            RouteDispatchOutcome::Matched(entry) => (entry.value)(pipeline.request()),
            _ => panic!("injectable guard controller route must match"),
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
            "injectable-guard"
        );
        assert_eq!(response.body(), &Body::text("guarded"));

        assert!(
            container
                .resolve::<InjectableGuard>()
                .await
                .unwrap()
                .allowance
                .allowed
        );
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
            let request = NivasaRequest::new(
                Method::from_bytes(method.as_bytes()).unwrap(),
                path,
                Body::empty(),
            );
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
}

mod params_and_queries {
    use super::*;

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

        let body: serde_json::Value = serde_json::from_slice(&response.body().as_bytes())
            .expect("error payload must be json");
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

        let body: serde_json::Value = serde_json::from_slice(&response.body().as_bytes())
            .expect("error payload must be json");
        assert_eq!(body["statusCode"], 400);
        assert_eq!(body["error"], "Bad Request");
        assert!(body["message"]
            .as_str()
            .expect("message must be string")
            .starts_with("invalid query string: field `page`:"));
    }
}

mod request_context_extractors {
    use super::*;

    #[test]
    fn controller_headers_runtime_extracts_full_header_map_after_route_matching() {
        let mut routes = RouteDispatchRegistry::new();
        let controller = ContextController;
        let route = ContextController::__nivasa_controller_routes()
            .into_iter()
            .find(|(_, path, _)| path == "/context/headers")
            .expect("context controller must expose a headers route");

        routes
            .register_pattern(
                RouteMethod::from(route.0),
                route.1,
                move |request: &NivasaRequest| {
                    run_controller_action_with_headers(request, |headers| {
                        controller.headers(headers)
                    })
                },
            )
            .expect("controller headers route must register");

        let request = Request::builder()
            .method(Method::GET)
            .uri("/context/headers")
            .header("x-trace-id", "trace-123")
            .body(Body::empty())
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
            "headers"
        );
        assert_eq!(
            response.body(),
            &Body::json(serde_json::json!({ "traceId": "trace-123" }))
        );
    }

    #[test]
    fn controller_header_runtime_extracts_one_typed_header() {
        let controller = ContextController;
        let request = Request::builder()
            .method(Method::GET)
            .uri("/context/header")
            .header("x-tenant", "acme")
            .body(Body::empty())
            .expect("request must build");
        let request = NivasaRequest::from_http(request);

        let response =
            run_controller_action_with_header::<String, _, _>(&request, "x-tenant", |tenant| {
                controller.header(tenant)
            });

        assert_eq!(response.status(), http::StatusCode::OK);
        assert_eq!(
            response.body(),
            &Body::json(serde_json::json!({ "tenant": "acme" }))
        );
    }

    #[test]
    fn controller_ip_runtime_prefers_extension_then_forwarded_headers() {
        let controller = ContextController;
        let mut extension_request = NivasaRequest::new(Method::GET, "/context/ip", Body::empty());
        extension_request.insert_extension(ClientIp::new("10.0.0.9"));

        let extension_response =
            run_controller_action_with_ip(&extension_request, |ip| controller.ip(ip));
        assert_eq!(
            extension_response.body(),
            &Body::json(serde_json::json!({ "ip": "10.0.0.9" }))
        );

        let forwarded_request = Request::builder()
            .method(Method::GET)
            .uri("/context/ip")
            .header("x-forwarded-for", "203.0.113.7, 10.0.0.1")
            .body(Body::empty())
            .expect("request must build");
        let forwarded_request = NivasaRequest::from_http(forwarded_request);

        let forwarded_response =
            run_controller_action_with_ip(&forwarded_request, |ip| controller.ip(ip));
        assert_eq!(
            forwarded_response.body(),
            &Body::json(serde_json::json!({ "ip": "203.0.113.7" }))
        );
    }

    #[test]
    fn controller_session_runtime_reads_typed_request_extension() {
        let controller = ContextController;
        let mut request = NivasaRequest::new(Method::GET, "/context/session", Body::empty());
        request.insert_extension(TestSession { user_id: 42 });

        let response =
            run_controller_action_with_session::<TestSession, _, _>(&request, |session| {
                controller.session(session)
            });

        assert_eq!(response.status(), http::StatusCode::OK);
        assert_eq!(
            response.body(),
            &Body::json(serde_json::json!({ "userId": 42 }))
        );
    }

    #[test]
    fn controller_custom_param_runtime_uses_custom_extractor() {
        let controller = ContextController;
        let request = Request::builder()
            .method(Method::GET)
            .uri("/context/custom")
            .header("x-tenant", "acme")
            .body(Body::empty())
            .expect("request must build");
        let request = NivasaRequest::from_http(request);

        let response =
            run_controller_action_with_custom_param(&request, TenantExtractor, |tenant| {
                controller.custom(tenant)
            });

        assert_eq!(response.status(), http::StatusCode::OK);
        assert_eq!(
            response.body(),
            &Body::json(serde_json::json!({ "tenant": "acme" }))
        );
    }

    #[test]
    fn controller_context_extractors_map_missing_values_to_bad_request() {
        let request = NivasaRequest::new(Method::GET, "/context/ip", Body::empty());
        let ip_response =
            run_controller_action_with_ip(&request, |_| NivasaResponse::text("unreachable"));
        assert_eq!(ip_response.status(), http::StatusCode::BAD_REQUEST);

        let session_response =
            run_controller_action_with_session::<TestSession, _, _>(&request, |_| {
                NivasaResponse::text("unreachable")
            });
        assert_eq!(session_response.status(), http::StatusCode::BAD_REQUEST);

        let custom_response =
            run_controller_action_with_custom_param(&request, TenantExtractor, |_| {
                NivasaResponse::text("unreachable")
            });
        assert_eq!(custom_response.status(), http::StatusCode::BAD_REQUEST);
    }
}

mod uploads {
    use super::*;

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

        let body: serde_json::Value = serde_json::from_slice(&response.body().as_bytes())
            .expect("error payload must be json");
        assert_eq!(body["statusCode"], 400);
        assert_eq!(body["error"], "Bad Request");
        assert_eq!(body["message"], "request is missing header `content-type`");
    }
}
