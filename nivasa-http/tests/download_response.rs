use http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use nivasa_http::{Download, IntoResponse, NivasaResponse};

#[test]
fn download_attachment_sets_headers_and_preserves_body_bytes() {
    let response = Download::attachment("report.csv", b"id,name\n1,Ada\n".to_vec()).into_response();

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(response.body().as_bytes(), b"id,name\n1,Ada\n");
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/octet-stream"
    );
    assert_eq!(
        response.headers().get(CONTENT_DISPOSITION).unwrap(),
        "attachment; filename=\"report.csv\""
    );
}

#[test]
fn response_builder_download_helper_uses_the_same_attachment_surface() {
    let response = NivasaResponse::download("archive.bin", vec![0, 1, 2, 3]);

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(response.body().as_bytes(), vec![0, 1, 2, 3]);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/octet-stream"
    );
    assert_eq!(
        response.headers().get(CONTENT_DISPOSITION).unwrap(),
        "attachment; filename=\"archive.bin\""
    );
}
