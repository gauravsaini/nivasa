use trybuild::TestCases;

#[test]
fn module_macro_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/module_pass.rs");
    t.compile_fail("tests/trybuild/module_unknown_key.rs");
    t.compile_fail("tests/trybuild/module_duplicate_key.rs");
}
