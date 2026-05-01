use trybuild::TestCases;

#[test]
fn controller_additional_pass_fixtures() {
    let t = TestCases::new();
    t.pass("tests/trybuild/controller_root_join_pass.rs");
    t.pass("tests/trybuild/controller_optional_extractors_pass.rs");
}
