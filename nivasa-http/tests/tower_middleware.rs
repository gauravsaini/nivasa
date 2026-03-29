use async_trait::async_trait;
use http::{Method, StatusCode};
use nivasa_http::{
    Body, NextMiddleware, NivasaMiddleware, NivasaMiddlewareLayer, NivasaRequest, NivasaResponse,
};
use std::{
    convert::Infallible,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tower::{service_fn, Layer, Service};

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

struct PrefixMiddleware;

#[async_trait]
impl NivasaMiddleware for PrefixMiddleware {
    async fn use_(&self, mut req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        let body = String::from_utf8(req.body().as_bytes()).expect("request body must be UTF-8");
        req.body_mut()
            .clone_from(&Body::text(format!("layer: {body}")));

        next.run(req).await
    }
}

#[tokio::test]
async fn nivasa_middleware_layer_wraps_a_tower_service() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&calls);

    let service = service_fn(move |request: NivasaRequest| {
        let seen = Arc::clone(&seen);
        async move {
            seen.fetch_add(1, Ordering::SeqCst);
            assert_eq!(request.method(), &Method::PUT);
            assert_eq!(request.path(), "/layer");
            assert_eq!(request.body(), &Body::text("layer: payload"));
            Ok::<_, Infallible>(NivasaResponse::new(StatusCode::OK, request.body().clone()))
        }
    });

    let layer = NivasaMiddlewareLayer::new(PrefixMiddleware);
    let mut layered_service = layer.layer(service);

    let response = layered_service
        .call(NivasaRequest::new(
            Method::PUT,
            "/layer",
            Body::text("payload"),
        ))
        .await
        .expect("tower service should succeed");

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body(), &Body::text("layer: payload"));
}
