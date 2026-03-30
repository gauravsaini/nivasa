use trybuild::TestCases;

#[test]
fn validation_macro_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/validation_dto_pass.rs");
    t.pass("tests/trybuild/validation_is_string_pass.rs");
    t.compile_fail("tests/trybuild/validation_min_length_invalid.rs");
    t.compile_fail("tests/trybuild/validation_max_length_invalid.rs");
    t.compile_fail("tests/trybuild/validation_is_string_invalid.rs");
}
