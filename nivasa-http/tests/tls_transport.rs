#![cfg(feature = "tls")]

use nivasa_http::{Body, NivasaResponse, NivasaServer};
use nivasa_routing::RouteMethod;
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer, ServerName},
    ClientConfig, RootCertStore, ServerConfig,
};
use std::{
    error::Error, io::Cursor, net::TcpListener as StdTcpListener, sync::Arc, time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::oneshot,
    time::{sleep, timeout},
};
use tokio_rustls::TlsConnector;

const CERT_PEM: &str = r#"-----BEGIN CERTIFICATE-----
MIIDJjCCAg6gAwIBAgIUT2VJQ6DXLJ5lXk8c4ZIXnd8FNiowDQYJKoZIhvcNAQEL
BQAwFDESMBAGA1UEAwwJbG9jYWxob3N0MB4XDTI2MDMyNTA5MzAwN1oXDTI2MDMy
NjA5MzAwN1owFDESMBAGA1UEAwwJbG9jYWxob3N0MIIBIjANBgkqhkiG9w0BAQEF
AAOCAQ8AMIIBCgKCAQEAzba3AKXgXNxuAX3LvAmL4Uiv7tuDjEElOFuZAxVGJEaB
gHirjlcKby1gOjmD4vY9OofIByFWogYBE8fBgfZAFuBpFOTjK17DpLl1X+zE7cx+
qLmdrW7WNixDR1a6/1APdK0sUoud3dmIrCCzvHQb7Jd/ieRxv7A4pSfusoqv03DA
nJLVGHwGUai39COlbinRsj+XI0hpsrSly6p/CF86ijNEglm3OLx+7AVz09YumYqT
fTC6UsPkgDBbcSdhR7X8nUVz4fj/+JbCX2PXHnd3r0QxMISQcPe7W11mkVK8Bi1E
SPkdZvhq0MsB7G7I9+ujl/L2yahbRuoVf+aXhUeQ2QIDAQABo3AwbjAdBgNVHQ4E
FgQUlSOhfa/0XZHNqGJNnaNOlzC97lYwHwYDVR0jBBgwFoAUlSOhfa/0XZHNqGJN
naNOlzC97lYwCQYDVR0TBAIwADAUBgNVHREEDTALgglsb2NhbGhvc3QwCwYDVR0P
BAQDAgWgMA0GCSqGSIb3DQEBCwUAA4IBAQBfkuzbtcWwd/CT9UdQb85RbNbCSbxl
FtI6qGdPiV2bZCXl5wWGM7bSjHHBLvScPo6kUE2BEN0H9fgWsrOnuEijhWQeBmKY
+7Z153Q5rK8JgSU6afztikpkKedZxIg2V6vmS+rqHJzi1CijPvCLjJSbj3pk064l
qdtaTF3IwoYVzJexMXENs3jP7bM7cnDRgGpet/2UFiCghLp8qh0rZIVAf9LbJi6M
iXNwBXhtVNX8nsx4p9jRJw3tsTjR+yDSjmp6KDiUCqFAKEOCOz18bsNHqS/l8SAM
Nc+jwQXQ4lbLmSmrFJRraTCwmx+XuO5eb07uO6vOKTonrT7+S3g9upBa
-----END CERTIFICATE-----"#;

const KEY_PEM: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDNtrcApeBc3G4B
fcu8CYvhSK/u24OMQSU4W5kDFUYkRoGAeKuOVwpvLWA6OYPi9j06h8gHIVaiBgET
x8GB9kAW4GkU5OMrXsOkuXVf7MTtzH6ouZ2tbtY2LENHVrr/UA90rSxSi53d2Yis
ILO8dBvsl3+J5HG/sDilJ+6yiq/TcMCcktUYfAZRqLf0I6VuKdGyP5cjSGmytKXL
qn8IXzqKM0SCWbc4vH7sBXPT1i6ZipN9MLpSw+SAMFtxJ2FHtfydRXPh+P/4lsJf
Y9ced3evRDEwhJBw97tbXWaRUrwGLURI+R1m+GrQywHsbsj366OX8vbJqFtG6hV/
5peFR5DZAgMBAAECggEABO2f6dTPfXU6PT01HiXdzN5q+PsXmXEh5TBlx8bHP0BU
j840Q/NBxXyUWqJWm1+Ag4OA7zf+y8ZlJhydJIUqL2VRNyYtnnQe4ty4WzCaEV1a
pz89Iwxg9uH6ez39bSqCMeG7lyWpORYIgDrCiF0BInRvSOko/XZJQR5y1jwPofsK
RIRqPOUxnf6MaVEplTrW/WeaI7hoX3F+Q42+uqpBp/j9q94PXCUvP2+waATxkg62
6eoUKsiFwVP4z2dFE5m+C1mJ1NMcGP9FrqBIlE5QQUwIKYzLzDl3c5bEls7tbwI0
80vEfP9Z5GQ1uM0zvmOqNg5gEZVMsF7+y8m7QJq3BQKBgQDyzeN+ytGwBzN2HjJj
KIn+CmtQqEcehY/FjRR2gPvGZ9w3aEci80MCiC1neQ+m5OBz9NrYIhNMl4oE6TTm
4Akie38yUTKLTIMNc8EHsutTYKUlTNhMELO0/KPpZTwOT+LSLOa4QezQtgebUtaF
tLg0w9mZlPwlJkzUG+e8tu61DQKBgQDY5MoZ7oDD0J1iJFeXJFE1PkEWzvxKAJAf
UESCkl0+/Z8F+7ceaIYEgKY0VWcnh3w4dWHD0dxCuNznw/6QRm0AbyU+xnQUA4TD
JaOkEw1ThGPMyHEctdMyXE7nxXw+u5/JJ5J/5BZKrtz7QspBWy854c5KdBB8IWZF
fnys6tZv/QKBgQCag5+kjpmGde6v2mOiaqf2PNcySwSHTePCihddmpOfHXUs5XVv
rnMUZ2jNkmL9iGW2JTonlPfHJCC3I1mBG0103jaB5N5Pe29i3ikXJytOshAmfpKf
RXm3UZdV7hCb4warTdu9omZ8I3sPw1W1XN5k5cXSUNdtJMR7rw54L8oU+QKBgDpz
vjuq2Szshd2zKZ/j+7a7plL9SWSwLiciPLRruZGGTFsScVFSnfuMqD4mXfx7OPEG
QWjCn/ejVnVnjq1XLX0WdxUp6pKSOC9Xb3iCYe8GGNdRlZpFLju+QM2ZnVhSvEc7
0PGCiLdr1MYqQ9PFPT7+KdhK2z54ydUkt3jqeQwJAoGBAI3lomiLGwC+yp15x8Y2
tj8BCWE4qEcrBbhfXQ5F5fWZJvHEQEUwpnS3CTn97VabrDIeh9NG0R6N+CLgDZna
jrOkHQ5zKxgYc9B6n4Q9ow016saE0TNDhuEgpcV2lgku0QqJCZk6Av1VsIOYvf+1
vWsf8FpXmVZOhBXVxXZTApXc
-----END PRIVATE KEY-----"#;

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
    let mut reader = Cursor::new(pem.as_bytes());
    let mut certs = Vec::new();

    while let Some(item) =
        rustls_pemfile::read_one(&mut reader).expect("certificate PEM must parse")
    {
        if let rustls_pemfile::Item::X509Certificate(cert) = item {
            certs.push(cert);
        }
    }

    certs
}

fn load_key(pem: &str) -> PrivateKeyDer<'static> {
    let mut reader = Cursor::new(pem.as_bytes());

    while let Some(item) =
        rustls_pemfile::read_one(&mut reader).expect("private key PEM must parse")
    {
        match item {
            rustls_pemfile::Item::Pkcs8Key(key) => return key.into(),
            rustls_pemfile::Item::Pkcs1Key(key) => return key.into(),
            rustls_pemfile::Item::Sec1Key(key) => return key.into(),
            _ => {}
        }
    }

    panic!("private key PEM must contain a key");
}

#[tokio::test]
async fn tls_transport_path_preserves_request_pipeline_flow() -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let provider = rustls::crypto::ring::default_provider();
    let certs = load_certs(CERT_PEM);
    let mut roots = RootCertStore::empty();
    for cert in &certs {
        roots.add(cert.clone())?;
    }

    let server_config = ServerConfig::builder_with_provider(provider.clone().into())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(certs, load_key(KEY_PEM))?;
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
