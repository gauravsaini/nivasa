use http::{Method, StatusCode};
use nivasa_http::{Body, NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse};
use std::{
    convert::Infallible,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tower::service_fn;

#[tokio::test]
async fn tower_service_middleware_wraps_a_tower_service() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&calls);

    let service = service_fn(move |request: NivasaRequest| {
        let seen = Arc::clone(&seen);
        async move {
            seen.fetch_add(1, Ordering::SeqCst);
            assert_eq!(request.method(), &Method::POST);
            assert_eq!(request.path(), "/tower");
            Ok::<_, Infallible>(NivasaResponse::new(
                StatusCode::ACCEPTED,
                request.body().clone(),
            ))
        }
    });

    let middleware = nivasa_http::TowerServiceMiddleware::new(service);
    let next = NextMiddleware::new(|_| async move {
        panic!("tower service middleware should not delegate to next");
    });

    let response = middleware
        .use_(
            NivasaRequest::new(Method::POST, "/tower", Body::text("payload")),
            next,
        )
        .await;

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.body(), &Body::text("payload"));
}
