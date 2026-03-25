use http::Method;
use nivasa_http::{Body, RequestPipeline};
use nivasa_routing::{RouteDispatchOutcome, RouteDispatchRegistry, RouteMethod};

fn ready_pipeline() -> RequestPipeline {
    let request = nivasa_http::NivasaRequest::new(Method::GET, "/users/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);

    assert_eq!(pipeline.snapshot().current_state, "Received");
    pipeline.parse_request().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "MiddlewareChain");
    pipeline.complete_middleware().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "RouteMatching");

    pipeline
}

#[test]
fn request_pipeline_advances_through_initial_scxml_stages() {
    let pipeline = ready_pipeline();

    assert_eq!(pipeline.request().path(), "/users/42");
    assert_eq!(format!("{:?}", pipeline.current_state()), "RouteMatching");
    assert_eq!(pipeline.snapshot().current_state, "RouteMatching");
}

#[test]
fn request_pipeline_drives_route_matching_outcomes() {
    let mut matched_pipeline = ready_pipeline();
    let mut matched_routes = RouteDispatchRegistry::new();
    matched_routes
        .register_static(RouteMethod::Get, "/users/42", "user")
        .unwrap();

    let matched = matched_pipeline.match_route(&matched_routes).unwrap();
    assert!(matches!(matched, RouteDispatchOutcome::Matched(_)));
    assert_eq!(matched_pipeline.snapshot().current_state, "GuardChain");

    let mut not_found_pipeline = ready_pipeline();
    let not_found_routes = RouteDispatchRegistry::<&str>::new();
    let not_found = not_found_pipeline.match_route(&not_found_routes).unwrap();
    assert!(matches!(not_found, RouteDispatchOutcome::NotFound));
    assert_eq!(not_found_pipeline.snapshot().current_state, "ErrorHandling");

    let mut not_allowed_pipeline = ready_pipeline();
    let mut not_allowed_routes = RouteDispatchRegistry::new();
    not_allowed_routes
        .register_static(RouteMethod::Post, "/users/42", "user")
        .unwrap();

    let not_allowed = not_allowed_pipeline
        .match_route(&not_allowed_routes)
        .unwrap();
    assert!(matches!(
        not_allowed,
        RouteDispatchOutcome::MethodNotAllowed { .. }
    ));
    assert_eq!(not_allowed_pipeline.snapshot().current_state, "ErrorHandling");
}

#[test]
fn request_pipeline_routes_middleware_errors_to_error_handling() {
    let request = nivasa_http::NivasaRequest::new(Method::GET, "/users/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);

    pipeline.parse_request().unwrap();
    pipeline.fail_middleware().unwrap();

    assert_eq!(pipeline.snapshot().current_state, "ErrorHandling");
}

#[test]
fn request_pipeline_routes_parse_errors_to_error_handling() {
    let request = nivasa_http::NivasaRequest::new(Method::GET, "/users/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);

    pipeline.fail_parse().unwrap();

    assert_eq!(pipeline.snapshot().current_state, "ErrorHandling");
}

#[test]
#[should_panic(expected = "SCXML violation")]
fn invalid_stage_skip_is_rejected_by_the_engine() {
    let request = nivasa_http::NivasaRequest::new(Method::GET, "/users/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);

    let _ = pipeline.complete_middleware();
}
