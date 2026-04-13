use http::Method;
use nivasa_common::HttpException;
use nivasa_guards::{ExecutionContext, Guard, GuardFuture};
use nivasa_http::{Body, RequestPipeline};
use nivasa_routing::{RouteDispatchOutcome, RouteDispatchRegistry, RouteMethod};
use std::sync::{Arc, Mutex};

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

fn assert_latest_transition(pipeline: &RequestPipeline, from: &str, event: &str, to: Option<&str>) {
    let snapshot = pipeline.snapshot();
    let transition = snapshot
        .recent_transitions
        .last()
        .expect("pipeline must record the latest SCXML transition");

    assert_eq!(transition.from, from);
    assert_eq!(transition.event, event);
    assert_eq!(transition.to.as_deref(), to);
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
    assert_latest_transition(
        &not_found_pipeline,
        "RouteMatching",
        "RouteNotFound",
        Some("ErrorHandling"),
    );

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
    assert_latest_transition(
        &not_allowed_pipeline,
        "RouteMatching",
        "RouteMethodNotAllowed",
        Some("ErrorHandling"),
    );
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
    assert_latest_transition(
        &pipeline,
        "MiddlewareChain",
        "ErrorMiddleware",
        Some("ErrorHandling"),
    );
}

#[test]
fn request_pipeline_routes_parse_errors_to_error_handling() {
    let request = nivasa_http::NivasaRequest::new(Method::GET, "/users/42", Body::empty());
    let mut pipeline = RequestPipeline::new(request);

    pipeline.fail_parse().unwrap();

    assert_eq!(pipeline.snapshot().current_state, "ErrorHandling");
    assert!(pipeline.request().path_params().is_none());
    assert_latest_transition(&pipeline, "Received", "ErrorParse", Some("ErrorHandling"));
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
    assert_latest_transition(
        &pipeline,
        "GuardChain",
        "GuardDenied",
        Some("ErrorHandling"),
    );
    pipeline.handle_filter().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "SendingResponse");
    pipeline.fail_send().unwrap();

    assert_eq!(pipeline.snapshot().current_state, "Done");
    assert_latest_transition(&pipeline, "SendingResponse", "ErrorSend", Some("Done"));
}

struct AllowGuard;

impl Guard for AllowGuard {
    fn can_activate<'a>(&'a self, _: &'a ExecutionContext) -> GuardFuture<'a> {
        Box::pin(async { Ok(true) })
    }
}

struct DenyGuard;

impl Guard for DenyGuard {
    fn can_activate<'a>(&'a self, _: &'a ExecutionContext) -> GuardFuture<'a> {
        Box::pin(async { Ok(false) })
    }
}

struct ErrorGuard;

impl Guard for ErrorGuard {
    fn can_activate<'a>(&'a self, _: &'a ExecutionContext) -> GuardFuture<'a> {
        Box::pin(async { Err(HttpException::forbidden("blocked")) })
    }
}

#[derive(Clone)]
struct SequenceGuard {
    label: &'static str,
    outcome: SequenceGuardOutcome,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

#[derive(Clone, Copy)]
enum SequenceGuardOutcome {
    Allow,
    Deny,
    Error(&'static str),
}

impl Guard for SequenceGuard {
    fn can_activate<'a>(&'a self, _: &'a ExecutionContext) -> GuardFuture<'a> {
        let label = self.label;
        let outcome = self.outcome;
        let calls = Arc::clone(&self.calls);

        Box::pin(async move {
            calls.lock().unwrap().push(label);

            match outcome {
                SequenceGuardOutcome::Allow => Ok(true),
                SequenceGuardOutcome::Deny => Ok(false),
                SequenceGuardOutcome::Error(message) => Err(HttpException::forbidden(message)),
            }
        })
    }
}

#[tokio::test]
async fn request_pipeline_can_evaluate_guard_results_through_scxml() {
    let context = ExecutionContext::new(());

    let mut passed = matched_pipeline();
    let passed_outcome = passed.evaluate_guard(&AllowGuard, &context).await.unwrap();
    assert!(matches!(
        passed_outcome,
        nivasa_http::GuardExecutionOutcome::Passed
    ));
    assert_eq!(passed.snapshot().current_state, "InterceptorPre");

    let mut denied = matched_pipeline();
    let denied_outcome = denied.evaluate_guard(&DenyGuard, &context).await.unwrap();
    assert!(matches!(
        denied_outcome,
        nivasa_http::GuardExecutionOutcome::Denied
    ));
    assert_eq!(denied.snapshot().current_state, "ErrorHandling");
    assert_latest_transition(&denied, "GuardChain", "GuardDenied", Some("ErrorHandling"));

    let mut errored = matched_pipeline();
    let error_outcome = errored.evaluate_guard(&ErrorGuard, &context).await.unwrap();
    match error_outcome {
        nivasa_http::GuardExecutionOutcome::Error(error) => {
            assert_eq!(error.status_code, 403);
            assert_eq!(error.message, "blocked");
        }
        other => panic!("expected guard error outcome, got {other:?}"),
    }
    assert_eq!(errored.snapshot().current_state, "ErrorHandling");
    assert_latest_transition(&errored, "GuardChain", "ErrorGuard", Some("ErrorHandling"));
}

#[tokio::test]
async fn request_pipeline_evaluates_multiple_guards_as_an_and_chain() {
    let context = ExecutionContext::new(());
    let calls = Arc::new(Mutex::new(Vec::new()));
    let first = SequenceGuard {
        label: "first",
        outcome: SequenceGuardOutcome::Allow,
        calls: Arc::clone(&calls),
    };
    let second = SequenceGuard {
        label: "second",
        outcome: SequenceGuardOutcome::Allow,
        calls: Arc::clone(&calls),
    };

    let mut pipeline = matched_pipeline();
    let outcome = pipeline
        .evaluate_guard_chain(&[&first, &second], &context)
        .await
        .unwrap();

    assert!(matches!(
        outcome,
        nivasa_http::GuardExecutionOutcome::Passed
    ));
    assert_eq!(pipeline.snapshot().current_state, "InterceptorPre");
    assert_eq!(*calls.lock().unwrap(), vec!["first", "second"]);
}

#[tokio::test]
async fn request_pipeline_short_circuits_multiple_guards_on_the_first_failure() {
    let context = ExecutionContext::new(());
    let calls = Arc::new(Mutex::new(Vec::new()));
    let allow = SequenceGuard {
        label: "allow",
        outcome: SequenceGuardOutcome::Allow,
        calls: Arc::clone(&calls),
    };
    let deny = SequenceGuard {
        label: "deny",
        outcome: SequenceGuardOutcome::Deny,
        calls: Arc::clone(&calls),
    };
    let error = SequenceGuard {
        label: "error",
        outcome: SequenceGuardOutcome::Error("blocked"),
        calls: Arc::clone(&calls),
    };

    let mut denied = matched_pipeline();
    let denied_outcome = denied
        .evaluate_guard_chain(&[&allow, &deny, &error], &context)
        .await
        .unwrap();
    assert!(matches!(
        denied_outcome,
        nivasa_http::GuardExecutionOutcome::Denied
    ));
    assert_eq!(denied.snapshot().current_state, "ErrorHandling");
    assert_eq!(*calls.lock().unwrap(), vec!["allow", "deny"]);

    calls.lock().unwrap().clear();

    let mut errored = matched_pipeline();
    let errored_outcome = errored
        .evaluate_guard_chain(&[&allow, &error, &deny], &context)
        .await
        .unwrap();
    match errored_outcome {
        nivasa_http::GuardExecutionOutcome::Error(error) => {
            assert_eq!(error.status_code, 403);
            assert_eq!(error.message, "blocked");
        }
        other => panic!("expected guard error outcome, got {other:?}"),
    }
    assert_eq!(errored.snapshot().current_state, "ErrorHandling");
    assert_eq!(*calls.lock().unwrap(), vec!["allow", "error"]);
}

#[test]
fn request_pipeline_routes_late_stage_errors_through_error_handling() {
    let mut guard_error = matched_pipeline();
    guard_error.fail_guards().unwrap();
    assert_eq!(guard_error.snapshot().current_state, "ErrorHandling");
    assert_latest_transition(
        &guard_error,
        "GuardChain",
        "ErrorGuard",
        Some("ErrorHandling"),
    );

    let mut interceptor_error = matched_pipeline();
    interceptor_error.pass_guards().unwrap();
    interceptor_error.fail_interceptors_pre().unwrap();
    assert_eq!(interceptor_error.snapshot().current_state, "ErrorHandling");
    assert_latest_transition(
        &interceptor_error,
        "InterceptorPre",
        "ErrorInterceptor",
        Some("ErrorHandling"),
    );

    let mut validation_error = matched_pipeline();
    validation_error.pass_guards().unwrap();
    validation_error.complete_interceptors_pre().unwrap();
    validation_error.fail_validation().unwrap();
    assert_eq!(validation_error.snapshot().current_state, "ErrorHandling");
    assert_latest_transition(
        &validation_error,
        "PipeTransform",
        "ErrorValidation",
        Some("ErrorHandling"),
    );

    let mut pipe_error = matched_pipeline();
    pipe_error.pass_guards().unwrap();
    pipe_error.complete_interceptors_pre().unwrap();
    pipe_error.fail_pipes().unwrap();
    assert_eq!(pipe_error.snapshot().current_state, "ErrorHandling");
    assert_latest_transition(
        &pipe_error,
        "PipeTransform",
        "ErrorPipe",
        Some("ErrorHandling"),
    );

    let mut handler_error = matched_pipeline();
    handler_error.pass_guards().unwrap();
    handler_error.complete_interceptors_pre().unwrap();
    handler_error.complete_pipes().unwrap();
    handler_error.fail_handler().unwrap();
    assert_eq!(handler_error.snapshot().current_state, "ErrorHandling");
    assert_latest_transition(
        &handler_error,
        "HandlerExecution",
        "ErrorHandler",
        Some("ErrorHandling"),
    );

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
    assert_latest_transition(
        &interceptor_post_error,
        "InterceptorPost",
        "ErrorInterceptorPost",
        Some("ErrorHandling"),
    );
}

#[test]
fn request_pipeline_routes_unhandled_filter_outcomes_through_scxml() {
    let mut pipeline = matched_pipeline();

    pipeline.deny_guards().unwrap();
    pipeline.fail_filter_unhandled().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "SendingResponse");
    assert_latest_transition(
        &pipeline,
        "ErrorHandling",
        "ErrorFilterUnhandled",
        Some("SendingResponse"),
    );
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
