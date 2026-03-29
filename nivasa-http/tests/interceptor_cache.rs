use http::StatusCode;
use nivasa_common::HttpException;
use nivasa_http::NivasaResponse;
use nivasa_interceptors::{CacheInterceptor, CallHandler, ExecutionContext, Interceptor};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[tokio::test]
async fn cache_interceptor_reuses_the_first_successful_response() {
    let interceptor = CacheInterceptor::<NivasaResponse>::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let context = ExecutionContext::new()
        .with_request("GET", "/cache")
        .with_handler_name("list_users")
        .with_class_name("UsersController");

    let first = {
        let calls = Arc::clone(&calls);
        let next = CallHandler::new(move || {
            let calls = Arc::clone(&calls);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<_, HttpException>(NivasaResponse::text("cached"))
            }
        });

        interceptor.intercept(&context, next).await.unwrap()
    };

    let second = {
        let calls = Arc::clone(&calls);
        let next = CallHandler::new(move || {
            let calls = Arc::clone(&calls);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<_, HttpException>(NivasaResponse::text("fresh"))
            }
        });

        interceptor.intercept(&context, next).await.unwrap()
    };

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(first.body().as_bytes(), b"cached");
    assert_eq!(second.body().as_bytes(), b"cached");
}
