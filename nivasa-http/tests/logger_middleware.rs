use http::{Method, StatusCode};
use nivasa_http::{
    Body, LoggerMiddleware, NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse,
};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

struct BufferWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl Write for BufferWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut buffer = self.buffer.lock().expect("buffer lock");
        buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn logger_middleware_emits_method_path_status_and_duration_without_altering_response() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_target(false)
        .compact()
        .with_writer({
            let buffer = Arc::clone(&buffer);
            move || BufferWriter {
                buffer: Arc::clone(&buffer),
            }
        })
        .finish();

    let _guard = tracing::subscriber::set_default(subscriber);

    let middleware = LoggerMiddleware::new();
    let next = NextMiddleware::new(|request: NivasaRequest| async move {
        assert_eq!(request.path(), "/logger");
        NivasaResponse::new(StatusCode::CREATED, Body::text("ok"))
    });

    let response = middleware
        .use_(
            NivasaRequest::new(Method::POST, "/logger", Body::empty()),
            next,
        )
        .await;

    let logs =
        String::from_utf8(buffer.lock().expect("buffer lock").clone()).expect("logs must be utf-8");

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.body(), &Body::text("ok"));
    assert!(logs.contains("method=POST"));
    assert!(logs.contains("path=/logger"));
    assert!(logs.contains("status=201"));
    assert!(logs.contains("duration="));
}

#[tokio::test]
async fn logger_middleware_respects_stricter_log_filter() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_target(false)
        .with_max_level(tracing::Level::WARN)
        .compact()
        .with_writer({
            let buffer = Arc::clone(&buffer);
            move || BufferWriter {
                buffer: Arc::clone(&buffer),
            }
        })
        .finish();

    let _guard = tracing::subscriber::set_default(subscriber);

    let middleware = LoggerMiddleware::new();
    let next = NextMiddleware::new(|request: NivasaRequest| async move {
        assert_eq!(request.path(), "/logger");
        NivasaResponse::new(StatusCode::CREATED, Body::text("ok"))
    });

    let response = middleware
        .use_(
            NivasaRequest::new(Method::POST, "/logger", Body::empty()),
            next,
        )
        .await;

    let logs =
        String::from_utf8(buffer.lock().expect("buffer lock").clone()).expect("logs must be utf-8");

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.body(), &Body::text("ok"));
    assert!(logs.is_empty(), "expected no info output, got: {logs:?}");
}
