use bytes::Bytes;
use http::{Request, StatusCode, header::AUTHORIZATION};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use hyper_util::rt::TokioExecutor;
use nivasa_guards::{AuthGuard, ExecutionContext as GuardExecutionContext, Guard, GuardFuture};
use nivasa_http::{Body, NivasaRequest, NivasaResponse, NivasaServer};
use nivasa_routing::RouteMethod;
use serde::Deserialize;
use serde_json::json;
use std::{error::Error, net::TcpListener as StdTcpListener, time::Duration};
use tokio::{
    sync::oneshot,
    time::{sleep, timeout},
};

const SESSION_TOKEN: &str = "Bearer header.payload.signature";

#[derive(Debug, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Clone, Copy)]
struct AuthFlowGuard;

impl Guard for AuthFlowGuard {
    fn can_activate<'a>(&'a self, context: &'a GuardExecutionContext) -> GuardFuture<'a> {
        Box::pin(async move {
            let request = context
                .request::<NivasaRequest>()
                .expect("guard context must carry the request");

            if request.path() == "/auth/login" {
                return Ok(true);
            }

            let authorization = request
                .header("authorization")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("");

            Ok(AuthGuard::new().can_activate(context).await? && authorization == SESSION_TOKEN)
        })
    }
}

fn free_port() -> u16 {
    StdTcpListener::bind("127.0.0.1:0")
        .expect("must bind an ephemeral port")
        .local_addr()
        .expect("must inspect ephemeral addr")
        .port()
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
async fn auth_flow_login_issues_token_and_protected_route_accepts_it() -> Result<(), Box<dyn Error>>
{
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = NivasaServer::builder()
        .use_global_guard(AuthFlowGuard)
        .route(RouteMethod::Post, "/auth/login", |request| {
            let credentials = request
                .extract::<nivasa_http::Json<LoginRequest>>()
                .expect("login body must deserialize")
                .into_inner();

            if credentials.username == "ada" && credentials.password == "secret" {
                NivasaResponse::new(
                    StatusCode::OK,
                    Body::json(json!({
                        "accessToken": SESSION_TOKEN,
                        "tokenType": "Bearer",
                    })),
                )
            } else {
                NivasaResponse::new(
                    StatusCode::UNAUTHORIZED,
                    Body::json(json!({
                        "error": "invalid credentials",
                    })),
                )
            }
        })
        .expect("login route must register")
        .route(RouteMethod::Get, "/auth/profile", |_| {
            NivasaResponse::new(
                StatusCode::OK,
                Body::json(json!({
                    "profile": "protected",
                    "subject": "ada",
                })),
            )
        })
        .expect("protected route must register")
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        server
            .listen("127.0.0.1", port)
            .await
            .expect("server must stop cleanly");
    });

    wait_for_server(port).await;

    let client: Client<HttpConnector, Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();

    let login_request = Request::post(format!("http://127.0.0.1:{port}/auth/login"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from_static(
            br#"{"username":"ada","password":"secret"}"#,
        )))?;
    let login_response = client.request(login_request).await?;
    assert_eq!(login_response.status(), StatusCode::OK);

    let login_body = login_response.into_body().collect().await?.to_bytes();
    let login_value: serde_json::Value = serde_json::from_slice(&login_body)?;
    let token = login_value["accessToken"]
        .as_str()
        .expect("login response must include a bearer token");
    assert_eq!(token, SESSION_TOKEN);

    let protected_request = Request::get(format!("http://127.0.0.1:{port}/auth/profile"))
        .header(AUTHORIZATION, token)
        .body(Full::new(Bytes::new()))?;
    let protected_response = client.request(protected_request).await?;
    assert_eq!(protected_response.status(), StatusCode::OK);

    let protected_body = protected_response.into_body().collect().await?.to_bytes();
    let protected_value: serde_json::Value = serde_json::from_slice(&protected_body)?;
    assert_eq!(protected_value["profile"], "protected");
    assert_eq!(protected_value["subject"], "ada");

    let denied_request = Request::get(format!("http://127.0.0.1:{port}/auth/profile"))
        .body(Full::new(Bytes::new()))?;
    let denied_response = client.request(denied_request).await?;
    assert_eq!(denied_response.status(), StatusCode::FORBIDDEN);

    drop(client);
    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}
