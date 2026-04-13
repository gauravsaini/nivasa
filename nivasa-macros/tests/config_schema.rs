use trybuild::TestCases;

#[test]
fn config_schema_derive() {
    let t = TestCases::new();
    t.pass("tests/trybuild/config_schema_pass.rs");
    t.compile_fail("tests/trybuild/config_schema_tuple_struct.rs");
    t.compile_fail("tests/trybuild/config_schema_unknown_key.rs");
    t.compile_fail("tests/trybuild/config_schema_duplicate_default.rs");
}
