#![cfg(feature = "tls")]

use nivasa_http::{Body, NivasaResponse, NivasaServer};
use nivasa_routing::RouteMethod;
use rustls::{
    pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer, ServerName},
    ClientConfig, RootCertStore, ServerConfig,
};
use std::{
    error::Error,
    fs,
    net::TcpListener as StdTcpListener,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::oneshot,
    time::{sleep, timeout},
};
use tokio_rustls::TlsConnector;

fn free_port() -> u16 {
    StdTcpListener::bind("127.0.0.1:0")
        .expect("must bind an ephemeral port")
        .local_addr()
        .expect("must inspect ephemeral addr")
        .port()
}

async fn wait_for_server(port: u16) {
    for _ in 0..50 {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return;
        }

        sleep(Duration::from_millis(20)).await;
    }

    panic!("server did not become ready");
}

fn load_certs(pem: &str) -> Vec<CertificateDer<'static>> {
    CertificateDer::pem_slice_iter(pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .expect("certificate PEM must parse")
}

fn load_key(pem: &str) -> PrivateKeyDer<'static> {
    PrivateKeyDer::from_pem_slice(pem.as_bytes()).expect("private key PEM must parse")
}

fn fixture_dir() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be monotonic enough for test fixtures")
        .as_nanos();
    std::env::temp_dir().join(format!("nivasa-tls-{unique}"))
}

fn generate_tls_material(
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), Box<dyn Error>> {
    let dir = fixture_dir();
    fs::create_dir_all(&dir)?;

    let config_path = dir.join("openssl.cnf");
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    fs::write(
        &config_path,
        r#"[req]
distinguished_name = dn
x509_extensions = v3_req
prompt = no
[dn]
CN = localhost
[v3_req]
subjectAltName = @alt_names
basicConstraints = critical,CA:FALSE
keyUsage = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth, clientAuth
[alt_names]
DNS.1 = localhost
IP.1 = 127.0.0.1
"#,
    )?;

    let status = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "3650",
            "-keyout",
            key_path.to_str().expect("key path must be utf-8"),
            "-out",
            cert_path.to_str().expect("cert path must be utf-8"),
            "-config",
            config_path.to_str().expect("config path must be utf-8"),
            "-extensions",
            "v3_req",
        ])
        .status()?;

    if !status.success() {
        return Err("openssl must generate TLS fixtures successfully".into());
    }

    let cert_pem = fs::read_to_string(&cert_path)?;
    let key_pem = fs::read_to_string(&key_path)?;
    Ok((load_certs(&cert_pem), load_key(&key_pem)))
}

#[tokio::test]
async fn tls_transport_path_preserves_request_pipeline_flow() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let provider = rustls::crypto::ring::default_provider();
    let (certs, key) = generate_tls_material()?;
    let mut roots = RootCertStore::empty();
    for cert in &certs {
        roots.add(cert.clone())?;
    }

    let server_config = ServerConfig::builder_with_provider(provider.clone().into())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    let client_config = ClientConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(client_config));

    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/users/:id", |request| {
            let user_id = request
                .path_param("id")
                .expect("pipeline must attach route captures");

            NivasaResponse::new(http::StatusCode::OK, Body::text(format!("tls-{user_id}")))
        })
        .expect("route must register")
        .tls_config(server_config)
        .shutdown_signal(shutdown_rx)
        .build();

    let server_task = tokio::spawn(async move {
        server
            .listen("127.0.0.1", port)
            .await
            .expect("server must stop cleanly");
    });

    wait_for_server(port).await;

    let tcp = TcpStream::connect(("127.0.0.1", port)).await?;
    let server_name = ServerName::try_from("localhost")?;
    let mut tls = connector.connect(server_name, tcp).await?;
    tls.write_all(b"GET /users/42 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await?;
    tls.flush().await?;

    let mut response = Vec::new();
    tls.read_to_end(&mut response).await?;
    let response = String::from_utf8(response)?;

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("tls-42"));

    let _ = shutdown_tx.send(());
    timeout(Duration::from_secs(2), server_task).await??;
    Ok(())
}
