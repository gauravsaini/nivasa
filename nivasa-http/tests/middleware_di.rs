use async_trait::async_trait;
use http::{Method, StatusCode};
use nivasa_core::di::DependencyContainer;
use nivasa_http::{Body, NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse};
use nivasa_macros::injectable;
use std::sync::Arc;

struct PrefixConfig {
    prefix: &'static str,
}

#[injectable]
struct PrefixMiddleware {
    #[inject]
    config: Arc<PrefixConfig>,
}

#[async_trait]
impl NivasaMiddleware for PrefixMiddleware {
    async fn use_(&self, mut req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        let body = String::from_utf8(req.body().as_bytes()).expect("request body must be UTF-8");
        req.body_mut()
            .clone_from(&Body::text(format!("{}{}", self.config.prefix, body)));

        next.run(req).await
    }
}

#[tokio::test]
async fn injectable_middleware_struct_can_be_resolved_and_used() {
    let container = DependencyContainer::new();
    container
        .register_value(PrefixConfig { prefix: "mw: " })
        .await;
    PrefixMiddleware::__nivasa_register(&container).await;

    let middleware = container
        .resolve::<PrefixMiddleware>()
        .await
        .expect("injectable middleware must resolve from the container");

    let next = NextMiddleware::new(|request: NivasaRequest| async move {
        assert_eq!(request.body(), &Body::text("mw: original"));
        NivasaResponse::new(StatusCode::OK, request.body().clone())
    });

    let response = middleware
        .use_(
            NivasaRequest::new(Method::POST, "/middleware", Body::text("original")),
            next,
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.body(), &Body::text("mw: original"));
}
