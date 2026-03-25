use nivasa_http::upload::MultipartLimits;

#[test]
fn multipart_limits_keep_the_builder_shape() {
    let limits = MultipartLimits::new()
        .whole_stream(4096)
        .per_field(1024)
        .field_limit("avatar", 256)
        .allowed_fields(["avatar", "resume"])
        .allowed_mime_types(["image/png", "image/jpeg"]);

    assert_eq!(limits.whole_stream_limit(), Some(4096));
    assert_eq!(limits.per_field_limit(), Some(1024));
    assert_eq!(limits.field_limit_for("avatar"), Some(256));
    assert_eq!(
        limits.allowed_fields_list(),
        &["avatar".to_string(), "resume".to_string()]
    );
    assert_eq!(
        limits.allowed_mime_types_list(),
        &["image/png".to_string(), "image/jpeg".to_string()]
    );
    assert!(limits.allows_mime_type(Some("image/png")));
    assert!(!limits.allows_mime_type(Some("text/plain")));
}

#[test]
fn multipart_limits_convert_into_multer_constraints() {
    let constraints = MultipartLimits::new()
        .whole_stream(8192)
        .per_field(2048)
        .field_limit("avatar", 512)
        .allowed_fields(["avatar"])
        .into_constraints();

    let debug = format!("{constraints:?}");
    assert!(debug.contains("avatar"));
    assert!(debug.contains("8192"));
    assert!(debug.contains("2048"));
}
