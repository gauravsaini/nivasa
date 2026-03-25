use trybuild::TestCases;

#[test]
fn controller_macro_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/controller_pass.rs");
    t.pass("tests/trybuild/controller_version.rs");
    t.pass("tests/trybuild/controller_impl_pass.rs");
    t.pass("tests/trybuild/controller_parameter_extractors_pass.rs");
    t.pass("tests/trybuild/controller_response_metadata_pass.rs");
    t.compile_fail("tests/trybuild/controller_parameter_extractors_headers_no_route.rs");
    t.compile_fail("tests/trybuild/controller_invalid_key.rs");
    t.compile_fail("tests/trybuild/controller_missing_path.rs");
    t.compile_fail("tests/trybuild/controller_parameter_extractors_invalid.rs");
    t.compile_fail("tests/trybuild/controller_custom_param_invalid.rs");
    t.compile_fail("tests/trybuild/controller_parameter_extractors_flag_invalid.rs");
    t.compile_fail("tests/trybuild/controller_parameter_extractors_req_namevalue_invalid.rs");
    t.compile_fail("tests/trybuild/controller_parameter_extractors_res_namevalue_invalid.rs");
    t.compile_fail("tests/trybuild/controller_response_metadata_invalid.rs");
    t.compile_fail("tests/trybuild/controller_response_metadata_no_route.rs");
    t.compile_fail("tests/trybuild/controller_parameter_extractors_no_route.rs");
}
