use async_trait::async_trait;
use bytes::Bytes;
use http::{
    header::{ORIGIN, VARY},
    Method, Request, StatusCode,
};
use http_body_util::{BodyExt, Full};
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
use tower_http::cors::CorsLayer;

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

struct HttpRequestCompatService<S> {
    inner: Arc<tokio::sync::Mutex<S>>,
}

impl<S> HttpRequestCompatService<S> {
    fn new(inner: S) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(inner)),
        }
    }
}

impl<S> Service<Request<Full<Bytes>>> for HttpRequestCompatService<S>
where
    S: Service<NivasaRequest, Response = NivasaResponse, Error = Infallible> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = http::Response<Full<Bytes>>;
    type Error = Infallible;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<Full<Bytes>>) -> Self::Future {
        let inner = Arc::clone(&self.inner);

        Box::pin(async move {
            let (parts, body) = request.into_parts();
            let body = body
                .collect()
                .await
                .expect("request body must be readable")
                .to_bytes();
            let mut nivasa_request = NivasaRequest::new(
                parts.method,
                parts.uri.to_string(),
                Body::bytes(body.to_vec()),
            );

            for (name, value) in &parts.headers {
                nivasa_request.set_header(
                    name.as_str(),
                    value.to_str().expect("request header must be valid UTF-8"),
                );
            }

            let response = {
                let mut service = inner.lock().await;
                service.call(nivasa_request).await
            };

            let response = response
                .expect("middleware service must succeed")
                .into_inner();
            let mut builder = http::Response::builder().status(response.status());
            for (name, value) in response.headers() {
                builder = builder.header(name, value);
            }

            let response = builder
                .body(Full::new(Bytes::from(response.body().as_bytes())))
                .expect("response must be valid");

            Ok(response)
        })
    }
}

#[tokio::test]
async fn tower_http_cors_wraps_a_nivasa_middleware_service() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&calls);

    let service = service_fn(move |request: NivasaRequest| {
        let seen = Arc::clone(&seen);
        async move {
            seen.fetch_add(1, Ordering::SeqCst);
            assert_eq!(request.method(), &Method::GET);
            assert_eq!(request.path(), "/cors");
            assert_eq!(request.body(), &Body::text("layer: payload"));
            Ok::<_, Infallible>(NivasaResponse::text("cors ok"))
        }
    });

    let layered_service = NivasaMiddlewareLayer::new(PrefixMiddleware).layer(service);
    let compat_service = HttpRequestCompatService::new(layered_service);
    let mut cors_service = CorsLayer::permissive().layer(compat_service);

    let request = Request::builder()
        .method(Method::GET)
        .uri("http://example.com/cors")
        .header(ORIGIN, "https://example.com")
        .body(Full::new(Bytes::from_static(b"payload")))
        .expect("request must be valid");

    let response = cors_service
        .call(request)
        .await
        .expect("cors middleware should succeed");

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .expect("cors should add allow-origin"),
        "*"
    );
    let vary = response
        .headers()
        .get(VARY)
        .expect("cors should add vary")
        .to_str()
        .expect("vary must be valid header text");
    assert!(
        vary.contains("origin"),
        "vary header should include origin, got {vary}"
    );

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body should collect");
    let body = body.to_bytes();
    assert_eq!(body, Bytes::from_static(b"cors ok"));
}
