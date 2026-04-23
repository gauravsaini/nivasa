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

    let file = match FileInterceptor::new("avatar").extract_from_bytes(&content_type, &body) {
        Ok(file) => file,
        Err(err) => panic!("single file should parse: {err}"),
    };

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

    let files = match FilesInterceptor::new("attachments").extract_from_bytes(&content_type, &body)
    {
        Ok(files) => files,
        Err(err) => panic!("multiple files should parse: {err}"),
    };

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

#[test]
fn file_interceptor_rejects_whole_stream_limits_before_parsing() {
    let (content_type, body) = multipart_body(&[(
        "avatar",
        "avatar.png",
        Some("image/png"),
        b"png-data-too-big",
    )]);

    let error = FileInterceptor::new("avatar")
        .with_limits(MultipartLimits::new().whole_stream(8))
        .extract_from_bytes(&content_type, &body)
        .expect_err("oversized multipart stream should be rejected");

    assert_eq!(
        error,
        UploadInterceptError::StreamTooLarge {
            limit: 8,
            actual: body.len(),
        }
    );
}

#[test]
fn file_interceptor_rejects_missing_mime_type_when_restricted() {
    let (content_type, body) = multipart_body(&[("avatar", "avatar.bin", None, b"binary")]);

    let error = FileInterceptor::new("avatar")
        .with_limits(MultipartLimits::new().allowed_mime_types(["image/png"]))
        .extract_from_bytes(&content_type, &body)
        .expect_err("missing mime type should be rejected when restricted");

    assert_eq!(
        error,
        UploadInterceptError::DisallowedMimeType { mime_type: None }
    );
}

#[test]
fn file_interceptor_rejects_field_too_large_when_per_field_limit_is_exceeded() {
    let (content_type, body) =
        multipart_body(&[("avatar", "avatar.png", Some("image/png"), b"too-large")]);

    let error = FileInterceptor::new("avatar")
        .with_limits(MultipartLimits::new().field_limit("avatar", 4))
        .extract_from_bytes(&content_type, &body)
        .expect_err("oversized field should be rejected");

    assert_eq!(
        error,
        UploadInterceptError::FieldTooLarge {
            field: "avatar".to_string(),
            limit: 4,
            actual: "too-large".len(),
        }
    );
}

#[test]
fn file_interceptor_reports_too_many_files_for_a_single_field() {
    let (content_type, body) = multipart_body(&[
        ("avatar", "one.png", Some("image/png"), b"first"),
        ("avatar", "two.png", Some("image/png"), b"second"),
    ]);

    let error = FileInterceptor::new("avatar")
        .extract_from_bytes(&content_type, &body)
        .expect_err("multiple files should be rejected for single-file extraction");

    assert_eq!(
        error,
        UploadInterceptError::TooManyFiles {
            field: "avatar".to_string(),
            count: 2,
        }
    );
}

#[test]
fn files_interceptor_reports_missing_file_when_no_matching_field_exists() {
    let (content_type, body) =
        multipart_body(&[("resume", "resume.pdf", Some("application/pdf"), b"pdf")]);

    let error = FilesInterceptor::new("avatar")
        .extract_from_bytes(&content_type, &body)
        .expect_err("missing matching file should be rejected");

    assert_eq!(
        error,
        UploadInterceptError::MissingFile {
            field: "avatar".to_string(),
        }
    );
}

#[test]
fn upload_error_display_covers_public_messages() {
    let errors = [
        UploadInterceptError::MissingMultipartBoundary,
        UploadInterceptError::InvalidMultipart("bad boundary".into()),
        UploadInterceptError::UnknownField {
            name: "avatar".into(),
        },
        UploadInterceptError::MissingFile {
            field: "avatar".into(),
        },
        UploadInterceptError::TooManyFiles {
            field: "avatar".into(),
            count: 2,
        },
        UploadInterceptError::FieldTooLarge {
            field: "avatar".into(),
            limit: 4,
            actual: 9,
        },
        UploadInterceptError::StreamTooLarge {
            limit: 8,
            actual: 16,
        },
        UploadInterceptError::DisallowedMimeType {
            mime_type: Some("text/plain".into()),
        },
        UploadInterceptError::DisallowedMimeType { mime_type: None },
    ];

    let rendered = errors.map(|error| error.to_string());

    assert_eq!(rendered[0], "missing multipart boundary in content type");
    assert_eq!(rendered[1], "invalid multipart payload: bad boundary");
    assert_eq!(rendered[2], "multipart field `avatar` is not allowed");
    assert_eq!(rendered[3], "missing uploaded file for field `avatar`");
    assert_eq!(rendered[4], "expected one file for field `avatar`, found 2");
    assert_eq!(
        rendered[5],
        "multipart field `avatar` exceeded size limit 4 bytes with 9 bytes"
    );
    assert_eq!(
        rendered[6],
        "multipart payload exceeded size limit 8 bytes with 16 bytes"
    );
    assert_eq!(
        rendered[7],
        "multipart MIME type `text/plain` is not allowed"
    );
    assert_eq!(
        rendered[8],
        "multipart file is missing an allowed MIME type"
    );
}

#[test]
fn file_interceptor_reports_malformed_multipart_edges() {
    let content_type = "multipart/form-data; boundary=X-BOUNDARY";

    let cases: &[(&[u8], &str)] = &[
        (
            b"--X-BOUNDARY\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.png\"\r\n",
            "multipart payload is missing a closing boundary",
        ),
        (
            b"--X-BOUNDARY\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.png\"\r\n--X-BOUNDARY--\r\n",
            "missing header separator",
        ),
        (
            b"--X-BOUNDARY\r\nContent-Disposition form-data; name=\"avatar\"; filename=\"a.png\"\r\n\r\nbody\r\n--X-BOUNDARY--\r\n",
            "invalid header line",
        ),
        (
            b"--X-BOUNDARY\r\nContent-Disposition: form-data; filename=\"a.png\"\r\n\r\nbody\r\n--X-BOUNDARY--\r\n",
            "multipart field is missing a name",
        ),
    ];

    for (body, expected) in cases {
        let error = FileInterceptor::new("avatar")
            .extract_from_bytes(content_type, body)
            .expect_err("malformed multipart body should fail");

        assert!(
            error.to_string().contains(expected),
            "expected `{expected}` in `{error}`"
        );
    }

    let invalid_utf8 = b"--X-BOUNDARY\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.png\"\xff\r\n\r\nbody\r\n--X-BOUNDARY--\r\n";
    let error = FileInterceptor::new("avatar")
        .extract_from_bytes(content_type, invalid_utf8)
        .expect_err("invalid UTF-8 headers should fail");
    assert!(error
        .to_string()
        .contains("multipart headers must be valid UTF-8"));
}
