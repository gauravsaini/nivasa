use nivasa_macros::{controller, impl_controller};

#[allow(dead_code)]
struct DocGuard;
#[allow(dead_code)]
struct DocPipe;
#[allow(dead_code)]
struct DocInterceptor;
#[allow(dead_code)]
struct DocFilter;

/// nivasa-guard: DocGuard
/// nivasa-roles: Admin, Auditor
/// nivasa-interceptor: DocInterceptor
/// nivasa-filter: DocFilter
/// nivasa-pipe: DocPipe
/// nivasa-set-metadata: scope=docs
/// nivasa-throttle: limit=3,ttl=45
/// nivasa-skip-throttle:
#[controller("/docs")]
struct DocMarkerController;

/// nivasa-throttle: limit=9,ttl=90
#[controller("/active")]
struct ActiveThrottleDocController;

#[impl_controller]
impl DocMarkerController {
    /// nivasa-route: GET /summary
    /// nivasa-response: http_code 202
    /// nivasa-response: header x-doc ok
    /// nivasa-guard: DocGuard
    /// nivasa-roles: Reader
    /// nivasa-interceptor: DocInterceptor
    /// nivasa-filter: DocFilter
    /// nivasa-set-metadata: handler=summary
    /// nivasa-throttle: limit=1,ttl=5
    fn summary(&self) -> &'static str {
        "summary"
    }
}

#[impl_controller]
impl ActiveThrottleDocController {
    /// nivasa-route: POST /work
    /// nivasa-throttle: limit=2,ttl=20
    #[allow(dead_code)]
    fn work(&self) {}
}

#[test]
fn controller_macro_parses_doc_marker_surface() {
    assert_eq!(
        DocMarkerController::__nivasa_controller_guards(),
        vec!["DocGuard"]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_roles(),
        vec!["Admin", "Auditor"]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_interceptors(),
        vec!["DocInterceptor"]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_filters(),
        vec!["DocFilter"]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_pipes(),
        vec!["DocPipe"]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_set_metadata(),
        vec![("scope", "docs")]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_throttle_default(),
        Some((3, 45))
    );
    assert!(DocMarkerController::__nivasa_controller_skip_throttle());

    assert_eq!(
        DocMarkerController::__nivasa_controller_routes(),
        vec![("GET", "/docs/summary".to_string(), "summary")]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_response_metadata(),
        vec![("summary", Some(202), vec![("x-doc", "ok")])]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_guard_metadata(),
        vec![("summary", vec!["DocGuard"])]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_role_metadata(),
        vec![("summary", vec!["Reader"])]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_set_metadata_metadata(),
        vec![("summary", vec![("handler", "summary")])]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_interceptor_metadata(),
        vec![("summary", vec!["DocInterceptor"])]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_filter_metadata(),
        vec![("summary", vec!["DocFilter"])]
    );
    assert_eq!(
        DocMarkerController::__nivasa_controller_throttle_metadata(),
        vec![("summary", None, true)]
    );

    let controller = DocMarkerController;
    assert_eq!(controller.summary(), "summary");
}

#[test]
fn controller_doc_marker_throttle_applies_when_not_skipped() {
    assert_eq!(
        ActiveThrottleDocController::__nivasa_controller_throttle_default(),
        Some((9, 90))
    );
    assert_eq!(
        ActiveThrottleDocController::__nivasa_controller_throttle_metadata(),
        vec![("work", Some((2, 20)), false)]
    );
}
