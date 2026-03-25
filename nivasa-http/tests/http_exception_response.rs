use nivasa_common::HttpException;
use nivasa_http::IntoResponse;

#[test]
fn result_success_maps_through_existing_response_wrappers() {
    let response = Result::<&str, HttpException>::Ok("ready").into_response();

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(response.body().as_bytes(), b"ready");
    assert_eq!(
        response.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "text/plain; charset=utf-8"
    );
}

#[test]
fn result_error_maps_http_exception_to_json_response() {
    let response = Result::<&str, HttpException>::Err(
        HttpException::unprocessable_entity("Validation failed").with_details(serde_json::json!({
            "fields": {
                "email": "must be a valid email",
            }
        })),
    )
    .into_response();

    assert_eq!(response.status(), http::StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "application/json"
    );

    let json: serde_json::Value = serde_json::from_slice(&response.body().as_bytes()).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "statusCode": 422,
            "message": "Validation failed",
            "error": "Unprocessable Entity",
            "details": {
                "fields": {
                    "email": "must be a valid email",
                }
            }
        })
    );
}
