use nivasa_http::NivasaResponse;
use nivasa_interceptors::{CallHandler, ExecutionContext, Interceptor, LoggingInterceptor};
use nivasa_common::HttpException;
use std::sync::{Arc, Mutex};

fn capture_logs() -> (Arc<Mutex<Vec<String>>>, impl Fn(String) + Send + Sync + 'static) {
    let logs = Arc::new(Mutex::new(Vec::new()));
    let sink_logs = Arc::clone(&logs);

    let sink = move |entry| {
        sink_logs.lock().unwrap().push(entry);
    };

    (logs, sink)
}

#[tokio::test]
async fn logging_interceptor_records_request_metadata_and_status() {
    let (logs, sink) = capture_logs();
    let interceptor = LoggingInterceptor::new(sink, |response: &NivasaResponse| {
        response.status().as_u16().to_string()
    });
    let context = ExecutionContext::new()
        .with_request("GET", "/logging")
        .with_handler_name("list_users")
        .with_class_name("UsersController");
    let next = CallHandler::new(|| async { Ok::<_, HttpException>(NivasaResponse::text("ok")) });

    let response = interceptor.intercept(&context, next).await.unwrap();

    assert_eq!(response.status(), http::StatusCode::OK);
    let entry = logs.lock().unwrap().pop().expect("log entry must exist");
    assert!(entry.contains("method=GET"));
    assert!(entry.contains("path=/logging"));
    assert!(entry.contains("handler=list_users"));
    assert!(entry.contains("class=UsersController"));
    assert!(entry.contains("status=200"));
    assert!(entry.contains("duration_ns="));
}

#[tokio::test]
async fn logging_interceptor_records_failure_status_codes() {
    let (logs, sink) = capture_logs();
    let interceptor = LoggingInterceptor::new(sink, |response: &NivasaResponse| {
        response.status().as_u16().to_string()
    });
    let context = ExecutionContext::new()
        .with_request("POST", "/logging")
        .with_handler_name("create_user")
        .with_class_name("UsersController");
    let next = CallHandler::new(|| async {
        Err::<NivasaResponse, _>(HttpException::bad_request("boom"))
    });

    let error = interceptor.intercept(&context, next).await.unwrap_err();

    assert_eq!(error.status_code, 400);
    let entry = logs.lock().unwrap().pop().expect("log entry must exist");
    assert!(entry.contains("method=POST"));
    assert!(entry.contains("path=/logging"));
    assert!(entry.contains("handler=create_user"));
    assert!(entry.contains("class=UsersController"));
    assert!(entry.contains("status=400"));
    assert!(entry.contains("duration_ns="));
}
