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

fn matched_pipeline() -> RequestPipeline {
    let mut pipeline = ready_pipeline();
    let mut routes = RouteDispatchRegistry::new();
    routes
        .register_pattern(RouteMethod::Get, "/users/:id", "user")
        .unwrap();

    let matched = pipeline.match_route(&routes).unwrap();
    assert!(matches!(matched, RouteDispatchOutcome::Matched(_)));
    assert_eq!(pipeline.snapshot().current_state, "GuardChain");

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
    assert!(matched_pipeline.request().path_params().is_some());
    assert!(matched_pipeline.request().path_params().unwrap().is_empty());

    let mut not_found_pipeline = ready_pipeline();
    let not_found_routes = RouteDispatchRegistry::<&str>::new();
    let not_found = not_found_pipeline.match_route(&not_found_routes).unwrap();
    assert!(matches!(not_found, RouteDispatchOutcome::NotFound));
    assert_eq!(not_found_pipeline.snapshot().current_state, "ErrorHandling");
    assert!(not_found_pipeline.request().path_params().is_none());

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
    assert_eq!(
        not_allowed_pipeline.snapshot().current_state,
        "ErrorHandling"
    );
    assert!(not_allowed_pipeline.request().path_params().is_none());
}

#[test]
fn request_pipeline_attaches_route_captures_for_parameterized_routes() {
    let request = nivasa_http::NivasaRequest::new(Method::GET, "/users/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);
    let mut routes = RouteDispatchRegistry::new();

    pipeline.parse_request().unwrap();
    pipeline.complete_middleware().unwrap();
    routes
        .register_pattern(RouteMethod::Get, "/users/:id", "user")
        .unwrap();

    let matched = pipeline.match_route(&routes).unwrap();
    assert!(matches!(matched, RouteDispatchOutcome::Matched(_)));
    assert_eq!(pipeline.snapshot().current_state, "GuardChain");
    assert_eq!(pipeline.request().path_param("id"), Some("42"));
    assert_eq!(
        pipeline.request().path_params().unwrap().get("id"),
        Some("42")
    );
}

#[test]
fn request_pipeline_routes_middleware_errors_to_error_handling() {
    let request = nivasa_http::NivasaRequest::new(Method::GET, "/users/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);

    pipeline.parse_request().unwrap();
    pipeline.fail_middleware().unwrap();

    assert_eq!(pipeline.snapshot().current_state, "ErrorHandling");
    assert!(pipeline.request().path_params().is_none());
}

#[test]
fn request_pipeline_routes_parse_errors_to_error_handling() {
    let request = nivasa_http::NivasaRequest::new(Method::GET, "/users/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);

    pipeline.fail_parse().unwrap();

    assert_eq!(pipeline.snapshot().current_state, "ErrorHandling");
    assert!(pipeline.request().path_params().is_none());
}

#[test]
fn request_pipeline_can_complete_the_scxml_happy_path() {
    let mut pipeline = matched_pipeline();

    pipeline.pass_guards().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "InterceptorPre");
    pipeline.complete_interceptors_pre().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "PipeTransform");
    pipeline.complete_pipes().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "HandlerExecution");
    pipeline.complete_handler().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "InterceptorPost");
    pipeline.complete_interceptors_post().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "SendingResponse");
    pipeline.complete_response().unwrap();

    assert_eq!(pipeline.snapshot().current_state, "Done");
}

#[test]
fn request_pipeline_short_circuits_guard_denials_via_scxml() {
    let mut pipeline = matched_pipeline();

    pipeline.deny_guards().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "ErrorHandling");
    pipeline.handle_filter().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "SendingResponse");
    pipeline.fail_send().unwrap();

    assert_eq!(pipeline.snapshot().current_state, "Done");
}

#[test]
fn request_pipeline_routes_late_stage_errors_through_error_handling() {
    let mut guard_error = matched_pipeline();
    guard_error.fail_guards().unwrap();
    assert_eq!(guard_error.snapshot().current_state, "ErrorHandling");

    let mut interceptor_error = matched_pipeline();
    interceptor_error.pass_guards().unwrap();
    interceptor_error.fail_interceptors_pre().unwrap();
    assert_eq!(interceptor_error.snapshot().current_state, "ErrorHandling");

    let mut validation_error = matched_pipeline();
    validation_error.pass_guards().unwrap();
    validation_error.complete_interceptors_pre().unwrap();
    validation_error.fail_validation().unwrap();
    assert_eq!(validation_error.snapshot().current_state, "ErrorHandling");

    let mut pipe_error = matched_pipeline();
    pipe_error.pass_guards().unwrap();
    pipe_error.complete_interceptors_pre().unwrap();
    pipe_error.fail_pipes().unwrap();
    assert_eq!(pipe_error.snapshot().current_state, "ErrorHandling");

    let mut handler_error = matched_pipeline();
    handler_error.pass_guards().unwrap();
    handler_error.complete_interceptors_pre().unwrap();
    handler_error.complete_pipes().unwrap();
    handler_error.fail_handler().unwrap();
    assert_eq!(handler_error.snapshot().current_state, "ErrorHandling");

    let mut interceptor_post_error = matched_pipeline();
    interceptor_post_error.pass_guards().unwrap();
    interceptor_post_error.complete_interceptors_pre().unwrap();
    interceptor_post_error.complete_pipes().unwrap();
    interceptor_post_error.complete_handler().unwrap();
    interceptor_post_error.fail_interceptors_post().unwrap();
    assert_eq!(
        interceptor_post_error.snapshot().current_state,
        "ErrorHandling"
    );
}

#[test]
fn request_pipeline_routes_unhandled_filter_outcomes_through_scxml() {
    let mut pipeline = matched_pipeline();

    pipeline.deny_guards().unwrap();
    pipeline.fail_filter_unhandled().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "SendingResponse");
    pipeline.complete_response().unwrap();

    assert_eq!(pipeline.snapshot().current_state, "Done");
}

#[test]
#[should_panic(expected = "SCXML violation")]
fn invalid_stage_skip_is_rejected_by_the_engine() {
    let request = nivasa_http::NivasaRequest::new(Method::GET, "/users/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);

    let _ = pipeline.complete_middleware();
}
