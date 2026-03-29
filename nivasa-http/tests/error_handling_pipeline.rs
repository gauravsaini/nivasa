use http::{Method, StatusCode};
use nivasa_common::{HttpException, RequestContext};
use nivasa_http::{Body, NivasaRequest, NivasaResponse, RequestPipeline};
use serde_json::json;

struct ErrorEnvelopeFilter;

struct ErrorEnvelopeHost {
    request_context: Option<RequestContext>,
}

impl ErrorEnvelopeHost {
    fn new() -> Self {
        Self {
            request_context: None,
        }
    }

    fn with_request_context(mut self, request_context: RequestContext) -> Self {
        self.request_context = Some(request_context);
        self
    }

    fn request_context(&self) -> Option<&RequestContext> {
        self.request_context.as_ref()
    }
}

impl ErrorEnvelopeFilter {
    async fn catch(
        &self,
        exception: HttpException,
        host: &ErrorEnvelopeHost,
    ) -> NivasaResponse {
        let request_id = host
            .request_context()
            .and_then(|context| context.custom_data("request_id"))
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");

        NivasaResponse::new(
            StatusCode::from_u16(exception.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            json!({
                "statusCode": exception.status_code,
                "message": exception.message,
                "error": exception.error,
                "details": exception.details,
                "requestId": request_id,
            }),
        )
    }
}

#[tokio::test]
async fn error_handling_pipeline_routes_exception_through_filter_into_response() {
    let request = NivasaRequest::new(Method::GET, "/errors", Body::empty());
    let mut pipeline = RequestPipeline::new(request);
    pipeline.fail_parse().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "ErrorHandling");

    let mut request_context = RequestContext::new();
    request_context.set_custom_data("request_id", json!("req-123"));
    let host = ErrorEnvelopeHost::new().with_request_context(request_context);
    let filter = ErrorEnvelopeFilter;

    let response = filter
        .catch(
            HttpException::unprocessable_entity("Validation failed").with_details(json!({
                "fields": {
                    "email": "must be a valid email"
                }
            })),
            &host,
        )
        .await;

    pipeline.handle_filter().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "SendingResponse");
    pipeline.complete_response().unwrap();
    assert_eq!(pipeline.snapshot().current_state, "Done");

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&response.body().as_bytes()).unwrap(),
        json!({
            "statusCode": 422,
            "message": "Validation failed",
            "error": "Unprocessable Entity",
            "details": {
                "fields": {
                    "email": "must be a valid email"
                }
            },
            "requestId": "req-123"
        })
    );
}
