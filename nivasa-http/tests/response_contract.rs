use http::header::{CONTENT_DISPOSITION, CONTENT_TYPE, LOCATION};
use http::StatusCode;
use nivasa_common::HttpException;
use nivasa_http::{Download, Html, IntoResponse, Json, Redirect, Text};

enum BodyExpectation<'a> {
    Bytes(&'a [u8]),
    Json(serde_json::Value),
    Empty,
}

struct Case<'a> {
    name: &'a str,
    response: nivasa_http::NivasaResponse,
    status: StatusCode,
    content_type: Option<&'a str>,
    body: BodyExpectation<'a>,
    location: Option<&'a str>,
    disposition: Option<&'a str>,
}

#[test]
fn response_wrapper_contract_matrix_matches_public_wire_shape() {
    let cases = [
        Case {
            name: "text",
            response: Text("plain text").into_response(),
            status: StatusCode::OK,
            content_type: Some("text/plain; charset=utf-8"),
            body: BodyExpectation::Bytes(b"plain text"),
            location: None,
            disposition: None,
        },
        Case {
            name: "html",
            response: Html("<strong>ready</strong>").into_response(),
            status: StatusCode::OK,
            content_type: Some("text/html; charset=utf-8"),
            body: BodyExpectation::Bytes(b"<strong>ready</strong>"),
            location: None,
            disposition: None,
        },
        Case {
            name: "json",
            response: Json(serde_json::json!({
                "ok": true,
                "count": 2
            }))
            .into_response(),
            status: StatusCode::OK,
            content_type: Some("application/json"),
            body: BodyExpectation::Json(serde_json::json!({
                "ok": true,
                "count": 2
            })),
            location: None,
            disposition: None,
        },
        Case {
            name: "redirect",
            response: Redirect::temporary("/users").into_response(),
            status: StatusCode::FOUND,
            content_type: None,
            body: BodyExpectation::Empty,
            location: Some("/users"),
            disposition: None,
        },
        Case {
            name: "download",
            response: Download::attachment("report.csv", b"id,name\n1,Ada\n".to_vec())
                .into_response(),
            status: StatusCode::OK,
            content_type: Some("application/octet-stream"),
            body: BodyExpectation::Bytes(b"id,name\n1,Ada\n"),
            location: None,
            disposition: Some("attachment; filename=\"report.csv\""),
        },
    ];

    for case in cases {
        assert_eq!(case.response.status(), case.status, "case {}", case.name);

        match case.content_type {
            Some(content_type) => assert_eq!(
                case.response.headers().get(CONTENT_TYPE).unwrap(),
                content_type,
                "case {}",
                case.name
            ),
            None => assert!(
                case.response.headers().get(CONTENT_TYPE).is_none(),
                "case {}",
                case.name
            ),
        }

        match case.body {
            BodyExpectation::Bytes(expected) => {
                assert_eq!(case.response.body().as_bytes(), expected, "case {}", case.name);
            }
            BodyExpectation::Json(expected) => {
                let actual: serde_json::Value =
                    serde_json::from_slice(&case.response.body().as_bytes()).unwrap();
                assert_eq!(actual, expected, "case {}", case.name);
            }
            BodyExpectation::Empty => {
                assert!(case.response.body().is_empty(), "case {}", case.name);
            }
        }

        match case.location {
            Some(location) => assert_eq!(
                case.response.headers().get(LOCATION).unwrap(),
                location,
                "case {}",
                case.name
            ),
            None => assert!(
                case.response.headers().get(LOCATION).is_none(),
                "case {}",
                case.name
            ),
        }

        match case.disposition {
            Some(disposition) => assert_eq!(
                case.response.headers().get(CONTENT_DISPOSITION).unwrap(),
                disposition,
                "case {}",
                case.name
            ),
            None => assert!(
                case.response.headers().get(CONTENT_DISPOSITION).is_none(),
                "case {}",
                case.name
            ),
        }
    }
}

#[test]
fn result_http_exception_propagates_success_and_json_error_contracts() {
    let ok = Result::<Text<&str>, HttpException>::Ok(Text("wrapped")).into_response();
    assert_eq!(ok.status(), StatusCode::OK);
    assert_eq!(ok.body().as_bytes(), b"wrapped");
    assert_eq!(
        ok.headers().get(CONTENT_TYPE).unwrap(),
        "text/plain; charset=utf-8"
    );

    let err = Result::<Text<&str>, HttpException>::Err(
        HttpException::unprocessable_entity("Validation failed").with_details(
            serde_json::json!({
                "fields": {
                    "email": "must be a valid email"
                }
            }),
        ),
    )
    .into_response();

    assert_eq!(err.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err.headers().get(CONTENT_TYPE).unwrap(), "application/json");

    let body: serde_json::Value = serde_json::from_slice(&err.body().as_bytes()).unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "statusCode": 422,
            "message": "Validation failed",
            "error": "Unprocessable Entity",
            "details": {
                "fields": {
                    "email": "must be a valid email"
                }
            }
        })
    );
}
