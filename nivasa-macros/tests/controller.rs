use trybuild::TestCases;

#[test]
fn controller_macro_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/controller_pass.rs");
    t.pass("tests/trybuild/controller_version.rs");
    t.compile_fail("tests/trybuild/controller_invalid_key.rs");
    t.compile_fail("tests/trybuild/controller_missing_path.rs");
}
