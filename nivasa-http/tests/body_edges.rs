use bytes::Bytes;
use nivasa_http::{Body, Html, Text};

#[test]
fn body_shared_bytes_match_owned_bytes_for_each_variant() {
    let cases = [
        ("empty", Body::empty(), Vec::new()),
        ("text", Body::text("hello"), b"hello".to_vec()),
        ("html", Body::html("<b>ok</b>"), b"<b>ok</b>".to_vec()),
        (
            "json",
            Body::json(serde_json::json!({ "ok": true })),
            br#"{"ok":true}"#.to_vec(),
        ),
        ("bytes", Body::bytes(vec![1, 2, 3]), vec![1, 2, 3]),
    ];

    for (name, body, expected) in cases {
        assert_eq!(body.as_bytes(), expected, "case {name}: as_bytes");
        assert_eq!(
            body.as_shared_bytes(),
            Bytes::from(expected.clone()),
            "case {name}: as_shared_bytes"
        );
        assert_eq!(
            body.clone().into_bytes(),
            expected,
            "case {name}: into_bytes"
        );
        assert_eq!(
            body.into_shared_bytes(),
            Bytes::from(expected),
            "case {name}: into_shared_bytes"
        );
    }
}

#[test]
fn body_conversions_cover_wrapper_slice_and_shared_bytes_edges() {
    assert!(Body::default().is_empty());

    assert_eq!(Text("plain text").into_inner(), "plain text");
    assert_eq!(
        Html("<strong>html</strong>").into_inner(),
        "<strong>html</strong>"
    );

    assert_eq!(Body::from(Text("plain text")).into_bytes(), b"plain text");
    assert_eq!(
        Body::from(Html("<strong>html</strong>")).into_bytes(),
        b"<strong>html</strong>"
    );
    assert_eq!(
        Body::from(String::from("owned string")).into_bytes(),
        b"owned string"
    );
    assert_eq!(Body::from(&b"slice bytes"[..]).into_bytes(), b"slice bytes");
    assert_eq!(
        Body::from(Bytes::from_static(b"shared bytes")).into_bytes(),
        b"shared bytes"
    );
}
