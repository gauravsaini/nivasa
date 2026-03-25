use trybuild::TestCases;

#[test]
fn injectable_macro_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/injectable_pass.rs");
    t.pass("tests/trybuild/injectable_generic.rs");
    t.pass("tests/trybuild/injectable_lazy.rs");
    t.compile_fail("tests/trybuild/injectable_non_arc.rs");
    t.compile_fail("tests/trybuild/injectable_invalid_scope.rs");
}
