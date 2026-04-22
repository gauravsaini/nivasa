use trybuild::TestCases;

#[test]
fn schedule_macro_validation() {
    let t = TestCases::new();
    t.compile_fail("tests/trybuild/schedule_interval_static_method.rs");
    t.compile_fail("tests/trybuild/schedule_interval_zero.rs");
    t.compile_fail("tests/trybuild/schedule_timeout_invalid_literal.rs");
    t.compile_fail("tests/trybuild/schedule_timeout_invalid_target.rs");
}
