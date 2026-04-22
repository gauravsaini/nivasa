use http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use nivasa_http::NivasaResponse;

#[test]
fn download_helper_escapes_quotes_and_backslashes_in_filename() {
    let response = NivasaResponse::download(r#"folder\report"final".csv"#, b"csv".to_vec());

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(response.body().as_bytes(), b"csv");
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/octet-stream"
    );
    assert_eq!(
        response.headers().get(CONTENT_DISPOSITION).unwrap(),
        r#"attachment; filename="folder\\report\"final\".csv""#
    );
}
