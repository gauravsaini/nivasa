use nivasa_core::MockProvider;

#[tokio::test]
async fn mock_provider_records_calls_and_returns_values() {
    let mock = MockProvider::new();
    mock.enqueue_response(String::from("first"));
    mock.enqueue_response(String::from("second"));

    let first = mock.call(("get", "/users"));
    let second = mock.call(("post", "/users"));

    assert_eq!(first, "first");
    assert_eq!(second, "second");
    mock.assert_call_count(2);
    mock.assert_called_with(&[("get", "/users"), ("post", "/users")]);
}

#[tokio::test]
async fn mock_provider_supports_single_response_helper() {
    let mock = MockProvider::with_response(42usize);

    let value = mock.call("answer");

    assert_eq!(value, 42);
    mock.assert_call_count(1);
    mock.assert_called_with(&["answer"]);
}
