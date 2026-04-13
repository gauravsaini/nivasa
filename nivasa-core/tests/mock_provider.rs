use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct MockProvider<Args, Output> {
    state: Arc<Mutex<MockState<Args, Output>>>,
}

struct MockState<Args, Output> {
    calls: Vec<Args>,
    responses: VecDeque<Output>,
}

impl<Args, Output> MockProvider<Args, Output>
where
    Args: Clone + PartialEq + Debug,
{
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockState {
                calls: Vec::new(),
                responses: VecDeque::new(),
            })),
        }
    }

    fn with_response(response: Output) -> Self {
        let mock = Self::new();
        mock.enqueue_response(response);
        mock
    }

    fn enqueue_response(&self, response: Output) {
        self.state
            .lock()
            .expect("mock provider lock poisoned")
            .responses
            .push_back(response);
    }

    fn call(&self, args: Args) -> Output {
        let mut state = self.state.lock().expect("mock provider lock poisoned");
        state.calls.push(args);
        state
            .responses
            .pop_front()
            .expect("mock provider has no queued response")
    }

    fn call_count(&self) -> usize {
        self.state
            .lock()
            .expect("mock provider lock poisoned")
            .calls
            .len()
    }

    fn calls(&self) -> Vec<Args> {
        self.state
            .lock()
            .expect("mock provider lock poisoned")
            .calls
            .clone()
    }

    fn assert_call_count(&self, expected: usize) {
        assert_eq!(self.call_count(), expected);
    }

    fn assert_called_with(&self, expected: &[Args]) {
        let calls = self.calls();
        assert_eq!(calls, expected);
    }
}

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

