use async_trait::async_trait;
use http::{Method, StatusCode};
use nivasa_http::{Body, NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

struct PassThroughMiddleware;

#[async_trait]
impl NivasaMiddleware for PassThroughMiddleware {
    async fn use_(&self, req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        next.run(req).await
    }
}

struct HeaderInjectingMiddleware;

#[async_trait]
impl NivasaMiddleware for HeaderInjectingMiddleware {
    async fn use_(&self, mut req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        req.body_mut().clone_from(&Body::text("middleware"));
        next.run(req).await
    }
}

#[tokio::test]
async fn next_middleware_runs_the_terminal_handler() {
    let seen = Arc::new(AtomicBool::new(false));
    let flag = seen.clone();
    let next = NextMiddleware::new(move |request: NivasaRequest| {
        let flag = flag.clone();
        async move {
            flag.store(true, Ordering::SeqCst);
            assert_eq!(request.path(), "/middleware");
            NivasaResponse::text("ok")
        }
    });

    let response = next
        .run(NivasaRequest::new(
            Method::GET,
            "/middleware",
            Body::empty(),
        ))
        .await;

    assert!(seen.load(Ordering::SeqCst));
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body(), &Body::text("ok"));
}

#[tokio::test]
async fn middleware_can_delegate_to_the_next_handler() {
    let middleware = PassThroughMiddleware;
    let next = NextMiddleware::new(|request: NivasaRequest| async move {
        assert_eq!(request.path(), "/delegate");
        NivasaResponse::text("delegated")
    });

    let response = middleware
        .use_(
            NivasaRequest::new(Method::GET, "/delegate", Body::empty()),
            next,
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body(), &Body::text("delegated"));
}

#[tokio::test]
async fn middleware_can_mutate_the_request_before_forwarding() {
    let middleware = HeaderInjectingMiddleware;
    let next = NextMiddleware::new(|request: NivasaRequest| async move {
        assert_eq!(request.body(), &Body::text("middleware"));
        NivasaResponse::new(StatusCode::CREATED, request.body().clone())
    });

    let response = middleware
        .use_(
            NivasaRequest::new(Method::POST, "/middleware", Body::empty()),
            next,
        )
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.body(), &Body::text("middleware"));
}
