use bytes::Bytes;
use http::{Request, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use nivasa_common::HttpException;
use nivasa_filters::{
    ArgumentsHost, ExceptionFilter, ExceptionFilterFuture, ExceptionFilterMetadata,
};
use nivasa_http::{NivasaResponse, NivasaServer};
use nivasa_macros::Dto;
use nivasa_pipes::ValidationPipe;
use nivasa_routing::RouteMethod;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::net::TcpListener as StdTcpListener;
use std::time::Duration;
use tokio::{
    sync::oneshot,
    time::{sleep, timeout},
};

#[derive(Debug, Serialize, Deserialize, Dto)]
struct SignupDto {
    #[is_email]
    email: String,
    #[min_length(6)]
    password: String,
}

struct DetailedHttpExceptionFilter;

impl ExceptionFilter<HttpException, NivasaResponse> for DetailedHttpExceptionFilter {
    fn catch<'a>(
        &'a self,
        exception: HttpException,
        _host: ArgumentsHost,
    ) -> ExceptionFilterFuture<'a, NivasaResponse> {
        Box::pin(async move {
            NivasaResponse::new(
                StatusCode::from_u16(exception.status_code)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                serde_json::json!({
                    "statusCode": exception.status_code,
                    "message": exception.message,
                    "error": exception.error,
                    "details": exception.details,
                }),
            )
        })
    }
}

impl ExceptionFilterMetadata for DetailedHttpExceptionFilter {
    fn exception_type(&self) -> Option<&'static str> {
        Some(std::any::type_name::<HttpException>())
    }
}

fn free_port() -> u16 {
    let listener = match StdTcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(err) => panic!("must bind an ephemeral port: {err}"),
    };
    match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(err) => panic!("must read ephemeral port: {err}"),
    }
}

async fn wait_for_server(port: u16) {
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return;
        }

        sleep(Duration::from_millis(20)).await;
    }

    panic!("server did not become ready");
}

#[tokio::test]
async fn validation_pipe_rejects_invalid_dto_with_field_level_details() -> Result<(), Box<dyn Error>>
{
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .use_global_filter(DetailedHttpExceptionFilter)
        .use_global_pipe(ValidationPipe::<SignupDto>::new())
        .route(RouteMethod::Post, "/validate", |_| {
            NivasaResponse::text("ok")
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        if let Err(err) = server.listen("127.0.0.1", port).await {
            panic!("server must stop cleanly: {err}");
        }
    });

    wait_for_server(port).await;

    let client: Client<HttpConnector, Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let request = Request::post(format!("http://127.0.0.1:{port}/validate"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from_static(
            br#"{"email":"not-an-email","password":"123"}"#,
        )))?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .map(|value| value.as_bytes()),
        Some(b"application/json".as_slice())
    );

    let body = response.into_body().collect().await?.to_bytes();
    let body: Value = serde_json::from_slice(&body)?;

    assert_eq!(body["statusCode"], 400);
    assert_eq!(body["error"], "Bad Request");
    assert_eq!(body["message"], "Validation failed");

    let Some(errors) = body["details"]["errors"].as_array() else {
        panic!("validation details must contain an errors array");
    };

    let Some(email_error) = errors.iter().find(|error| error["field"] == "email") else {
        panic!("email validation error must exist");
    };
    assert_eq!(
        email_error["constraints"]["is_email"],
        "must be a valid email"
    );

    let Some(password_error) = errors.iter().find(|error| error["field"] == "password") else {
        panic!("password validation error must exist");
    };
    assert_eq!(
        password_error["constraints"]["min_length"],
        "must be at least 6 characters"
    );

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn validation_pipe_rejects_malformed_json_before_handler_runs() -> Result<(), Box<dyn Error>>
{
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .use_global_filter(DetailedHttpExceptionFilter)
        .use_global_pipe(ValidationPipe::<SignupDto>::new())
        .route(RouteMethod::Post, "/validate", |_| {
            NivasaResponse::text("ok")
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        if let Err(err) = server.listen("127.0.0.1", port).await {
            panic!("server must stop cleanly: {err}");
        }
    });

    wait_for_server(port).await;

    let client: Client<HttpConnector, Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let request = Request::post(format!("http://127.0.0.1:{port}/validate"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from_static(br#"{"email":"broken""#)))?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await?.to_bytes();
    let body: Value = serde_json::from_slice(&body)?;

    assert_eq!(body["statusCode"], 400);
    assert_eq!(body["error"], "Bad Request");
    assert!(body["message"]
        .as_str()
        .unwrap()
        .starts_with("global pipe could not parse request body as JSON:"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}

#[tokio::test]
async fn validation_pipe_rejects_non_utf8_request_body_before_handler_runs(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .use_global_filter(DetailedHttpExceptionFilter)
        .use_global_pipe(ValidationPipe::<SignupDto>::new())
        .route(RouteMethod::Post, "/validate", |_| {
            NivasaResponse::text("ok")
        })?
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        if let Err(err) = server.listen("127.0.0.1", port).await {
            panic!("server must stop cleanly: {err}");
        }
    });

    wait_for_server(port).await;

    let client: Client<HttpConnector, Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let request = Request::post(format!("http://127.0.0.1:{port}/validate"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from_static(&[0xff, 0xfe, 0xfd])))?;

    let response = client.request(request).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await?.to_bytes();
    let body: Value = serde_json::from_slice(&body)?;

    assert_eq!(body["statusCode"], 400);
    assert_eq!(body["error"], "Bad Request");
    assert!(body["message"]
        .as_str()
        .unwrap()
        .starts_with("global pipe requires a UTF-8 request body:"));

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}
