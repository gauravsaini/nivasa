use std::net::TcpListener;
use std::thread;

#[test]
fn websocket_client_can_complete_handshake() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("must bind ephemeral port");
    let addr = listener
        .local_addr()
        .expect("must read ephemeral listener address");

    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("client must connect");
        let _socket = tokio_tungstenite::tungstenite::accept(stream)
            .expect("server websocket handshake should succeed");
    });

    let url = format!("ws://127.0.0.1:{}/ws", addr.port());
    let (_socket, response) = tokio_tungstenite::tungstenite::connect(url)
        .expect("client websocket handshake should succeed");

    assert_eq!(response.status(), 101);

    server.join().expect("server thread should finish cleanly");
}
