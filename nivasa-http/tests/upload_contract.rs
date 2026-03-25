use nivasa_http::UploadedFile;

#[test]
fn uploaded_file_supports_public_contract_accessors() {
    let file = UploadedFile::new("avatar.png", Some("image/png".to_string()), vec![1, 2, 3, 4]);

    assert_eq!(file.filename(), "avatar.png");
    assert_eq!(file.content_type(), Some("image/png"));
    assert_eq!(file.bytes(), &[1, 2, 3, 4]);
    assert_eq!(file.len(), 4);
    assert!(!file.is_empty());
}

#[test]
fn uploaded_file_round_trips_parts_and_bytes() {
    let file = UploadedFile::new("notes.txt", None, b"hello".to_vec());

    assert_eq!(file.clone().into_bytes(), b"hello".to_vec());
    assert_eq!(
        file.into_parts(),
        ("notes.txt".to_string(), None, b"hello".to_vec())
    );
}

#[test]
fn uploaded_file_preserves_empty_payloads() {
    let file = UploadedFile::new("empty.bin", None, Vec::<u8>::new());

    assert!(file.is_empty());
    assert_eq!(file.len(), 0);
    assert_eq!(file.bytes(), &[] as &[u8]);
}
