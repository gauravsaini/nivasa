use http::{Method, Request};
use nivasa_http::{Body, FromRequest, Json, NivasaRequest, Query, RequestPipeline};
use nivasa_macros::{controller, impl_controller};
use nivasa_routing::{Controller, RouteDispatchRegistry, RouteMethod, RoutePattern, RoutePathCaptures};
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
    assert!(matches!(outcome, nivasa_routing::RouteDispatchOutcome::Matched(_)));

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
    assert!(matches!(outcome, nivasa_routing::RouteDispatchOutcome::Matched(_)));

    let request = pipeline.request();
    assert_eq!(request.path_params().unwrap().len(), 0);

    assert_eq!(
        VersionedReportsController::__nivasa_controller_response_metadata(),
        vec![(
            "summary",
            Some(204),
            vec![("x-controller-version", "v1")],
        )]
    );
}
