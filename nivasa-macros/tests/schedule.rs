use trybuild::TestCases;

#[test]
fn schedule_macro_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/schedule_cron_pass.rs");
    t.pass("tests/trybuild/schedule_interval_pass.rs");
    t.pass("tests/trybuild/schedule_timeout_pass.rs");
    t.compile_fail("tests/trybuild/schedule_interval_negative.rs");
    t.compile_fail("tests/trybuild/schedule_cron_empty.rs");
    t.compile_fail("tests/trybuild/schedule_cron_invalid_literal.rs");
    t.compile_fail("tests/trybuild/schedule_interval_static_method.rs");
    t.compile_fail("tests/trybuild/schedule_interval_zero.rs");
    t.compile_fail("tests/trybuild/schedule_timeout_invalid_literal.rs");
    t.compile_fail("tests/trybuild/schedule_timeout_invalid_target.rs");
}
