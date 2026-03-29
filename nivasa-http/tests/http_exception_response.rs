use nivasa_common::HttpException;
use nivasa_filters::{ExceptionFilter, HttpArgumentsHost, HttpExceptionSummary};
use nivasa_http::{HttpExceptionFilter, IntoResponse};

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

#[test]
fn http_exception_summary_maps_to_the_standard_three_field_shape() -> Result<(), Box<dyn std::error::Error>> {
    let response = HttpExceptionSummary::from(&HttpException::unprocessable_entity(
        "Validation failed",
    ))
    .into_response();

    if response.status() != http::StatusCode::UNPROCESSABLE_ENTITY {
        return Err("unexpected summary response status".into());
    }
    if response.headers().get(http::header::CONTENT_TYPE).unwrap() != "application/json" {
        return Err("unexpected summary response content type".into());
    }

    let json: serde_json::Value = serde_json::from_slice(&response.body().as_bytes())?;
    if json["statusCode"] != serde_json::Value::from(422) {
        return Err("unexpected summary statusCode".into());
    }
    if json["message"] != "Validation failed" {
        return Err("unexpected summary message".into());
    }
    if json["error"] != "Unprocessable Entity" {
        return Err("unexpected summary error".into());
    }
    if json.get("details").is_some() {
        return Err("summary payload must not include details".into());
    }

    Ok(())
}

#[tokio::test]
async fn http_exception_filter_maps_any_http_exception_to_the_standard_shape() {
    let response = HttpExceptionFilter::new()
        .catch(
            HttpException::unprocessable_entity("Validation failed").with_details(
                serde_json::json!({
                    "fields": {
                        "email": "must be a valid email",
                    }
                }),
            ),
            HttpArgumentsHost::new(),
        )
        .await;

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
            "error": "Unprocessable Entity"
        })
    );
    assert!(json.get("details").is_none());
}
