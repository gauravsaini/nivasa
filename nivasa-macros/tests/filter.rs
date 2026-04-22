use nivasa_filters::ExceptionFilterMetadata;
use nivasa_macros::{catch, catch_all};
use trybuild::TestCases;

struct HttpException;

#[catch(HttpException)]
struct HttpExceptionFilter;

#[catch_all]
struct DefaultExceptionFilter;

#[test]
fn filter_macros_emit_helper_surface() {
    let typed_filter = HttpExceptionFilter;
    let catch_all_filter = DefaultExceptionFilter;

    assert_eq!(
        HttpExceptionFilter::__nivasa_filter_exception(),
        "HttpException"
    );
    assert_eq!(
        HttpExceptionFilter::__nivasa_filter_exception_type(),
        std::any::type_name::<HttpException>()
    );
    assert_eq!(
        typed_filter.exception_type(),
        Some(std::any::type_name::<HttpException>())
    );

    assert!(DefaultExceptionFilter::__nivasa_filter_catch_all());
    assert!(catch_all_filter.is_catch_all());
}

#[test]
fn filter_macro_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/filter_catch_pass.rs");
    t.pass("tests/trybuild/filter_catch_all_pass.rs");
    t.compile_fail("tests/trybuild/filter_catch_invalid.rs");
    t.compile_fail("tests/trybuild/filter_catch_all_invalid.rs");
    t.compile_fail("tests/trybuild/filter_non_struct.rs");
}
