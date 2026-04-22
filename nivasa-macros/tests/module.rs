use trybuild::TestCases;

#[test]
fn module_macro_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/module_pass.rs");
    t.compile_fail("tests/trybuild/module_unknown_key.rs");
    t.compile_fail("tests/trybuild/module_duplicate_key.rs");
    t.compile_fail("tests/trybuild/module_guard_empty.rs");
    t.compile_fail("tests/trybuild/module_interceptor_empty.rs");
    t.compile_fail("tests/trybuild/module_roles_empty.rs");
    t.compile_fail("tests/trybuild/module_set_metadata_missing_key.rs");
    t.compile_fail("tests/trybuild/module_set_metadata_empty_value.rs");
}
