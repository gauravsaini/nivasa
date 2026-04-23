use nivasa_macros::{
    body, connected_socket, custom_param, file, files, headers, ip, message_body, param, req, res,
    session,
};

struct CustomExtractor;

#[body("payload")]
fn body_marker() -> &'static str {
    "body"
}

#[message_body("payload")]
fn message_body_marker() -> &'static str {
    "message_body"
}

#[connected_socket]
fn connected_socket_marker() -> &'static str {
    "connected_socket"
}

#[param("id")]
fn param_marker() -> &'static str {
    "param"
}

#[headers]
fn headers_marker() -> &'static str {
    "headers"
}

#[req]
fn req_marker() -> &'static str {
    "req"
}

#[res]
fn res_marker() -> &'static str {
    "res"
}

#[custom_param(CustomExtractor)]
fn custom_param_marker() -> &'static str {
    "custom_param"
}

#[ip]
fn ip_marker() -> &'static str {
    "ip"
}

#[session]
fn session_marker() -> &'static str {
    "session"
}

#[file]
fn file_marker() -> &'static str {
    "file"
}

#[files]
fn files_marker() -> &'static str {
    "files"
}

#[test]
fn noop_exported_parameter_attrs_return_items_unchanged() {
    let _ = CustomExtractor;

    assert_eq!(body_marker(), "body");
    assert_eq!(message_body_marker(), "message_body");
    assert_eq!(connected_socket_marker(), "connected_socket");
    assert_eq!(param_marker(), "param");
    assert_eq!(headers_marker(), "headers");
    assert_eq!(req_marker(), "req");
    assert_eq!(res_marker(), "res");
    assert_eq!(custom_param_marker(), "custom_param");
    assert_eq!(ip_marker(), "ip");
    assert_eq!(session_marker(), "session");
    assert_eq!(file_marker(), "file");
    assert_eq!(files_marker(), "files");
}
