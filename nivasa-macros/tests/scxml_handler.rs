use trybuild::TestCases;

#[test]
fn scxml_handler_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/scxml_handler_pass.rs");
    t.compile_fail("tests/trybuild/scxml_handler_missing_state.rs");
    t.compile_fail("tests/trybuild/scxml_handler_missing_file.rs");
    t.compile_fail("tests/trybuild/scxml_handler_invalid_statechart.rs");
}
