use nivasa_http::upload::{
    FileInterceptor, FilesInterceptor, MultipartLimits, UploadInterceptError,
};

fn multipart_body(parts: &[(&str, &str, Option<&str>, &[u8])]) -> (String, Vec<u8>) {
    let boundary = "X-BOUNDARY";
    let mut body = Vec::new();

    for (field_name, filename, content_type, bytes) in parts {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{field_name}\"; filename=\"{filename}\"\r\n"
            )
            .as_bytes(),
        );
        if let Some(content_type) = content_type {
            body.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

#[test]
fn file_interceptor_extracts_public_uploaded_file_contract() {
    let payload = [0_u8, 1, 2, 0xFF, b'P', b'N', b'G'];
    let (content_type, body) =
        multipart_body(&[("avatar", "avatar.png", Some("image/png"), &payload)]);

    let file = FileInterceptor::new("avatar")
        .extract_from_bytes(&content_type, &body)
        .expect("single file should parse");

    assert_eq!(file.filename(), "avatar.png");
    assert_eq!(file.content_type(), Some("image/png"));
    assert_eq!(file.bytes(), payload);
}

#[test]
fn files_interceptor_extracts_multiple_files_for_the_same_field() {
    let (content_type, body) = multipart_body(&[
        ("attachments", "one.txt", Some("text/plain"), b"first"),
        ("attachments", "two.txt", Some("text/plain"), b"second"),
    ]);

    let files = FilesInterceptor::new("attachments")
        .extract_from_bytes(&content_type, &body)
        .expect("multiple files should parse");

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].filename(), "one.txt");
    assert_eq!(files[1].filename(), "two.txt");
}

#[test]
fn file_interceptor_rejects_unknown_fields_when_restricted() {
    let (content_type, body) =
        multipart_body(&[("avatar", "avatar.png", Some("image/png"), b"png-data")]);

    let error = FileInterceptor::new("avatar")
        .with_limits(MultipartLimits::new().allowed_fields(["documents"]))
        .extract_from_bytes(&content_type, &body)
        .expect_err("unexpected field should be rejected");

    assert_eq!(
        error,
        UploadInterceptError::UnknownField {
            name: "avatar".to_string(),
        }
    );
}
