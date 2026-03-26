use http::{Method, Request};
use nivasa_http::{
    run_controller_action, run_controller_action_with_body, run_controller_action_with_param,
    run_controller_action_with_query, run_controller_action_with_request, Body, ControllerResponse,
    FromRequest, Json, NivasaRequest, NivasaResponse, Query, RequestPipeline,
};
use nivasa_macros::{controller, impl_controller};
use nivasa_routing::{
    Controller, RouteDispatchOutcome, RouteDispatchRegistry, RouteMethod, RoutePathCaptures,
    RoutePattern,
};
use serde::Deserialize;

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
