use trybuild::TestCases;

#[test]
fn filter_macro_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/filter_catch_pass.rs");
    t.pass("tests/trybuild/filter_catch_all_pass.rs");
    t.compile_fail("tests/trybuild/filter_catch_invalid.rs");
    t.compile_fail("tests/trybuild/filter_catch_all_invalid.rs");
    t.compile_fail("tests/trybuild/filter_non_struct.rs");
}
