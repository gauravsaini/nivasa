use trybuild::TestCases;

#[test]
fn scxml_handler_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/scxml_handler_pass.rs");
    t.compile_fail("tests/trybuild/scxml_handler_missing_state.rs");
    t.compile_fail("tests/trybuild/scxml_handler_missing_file.rs");
    t.compile_fail("tests/trybuild/scxml_handler_invalid_statechart.rs");
    t.compile_fail("tests/trybuild/scxml_handler_duplicate_statechart.rs");
    t.compile_fail("tests/trybuild/scxml_handler_duplicate_state.rs");
    t.compile_fail("tests/trybuild/scxml_handler_empty_statechart.rs");
    t.compile_fail("tests/trybuild/scxml_handler_empty_state.rs");
    t.compile_fail("tests/trybuild/scxml_handler_invalid_statechart_chars.rs");
    t.compile_fail("tests/trybuild/scxml_handler_invalid_state_path.rs");
    t.compile_fail("tests/trybuild/scxml_handler_unknown_arg.rs");
    t.compile_fail("tests/trybuild/scxml_handler_missing_statechart_arg.rs");
    t.compile_fail("tests/trybuild/scxml_handler_missing_state_arg.rs");
}
