use std::any::TypeId;

use nivasa_macros::middleware;
use trybuild::TestCases;

#[middleware]
struct LoggingMiddleware;

#[test]
fn middleware_macro_emits_helper_surface() {
    assert_eq!(
        LoggingMiddleware::__nivasa_middleware_name(),
        "LoggingMiddleware"
    );
    assert_eq!(
        LoggingMiddleware::__nivasa_middleware_type_id(),
        TypeId::of::<LoggingMiddleware>()
    );
}

#[test]
fn middleware_macro_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/middleware_pass.rs");
    t.compile_fail("tests/trybuild/middleware_invalid_target.rs");
}
