use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use nivasa_common::RequestContext;
use nivasa_guards::{ExecutionContext, ThrottlerGuard, ThrottlerStorage};

#[derive(Debug, Default)]
struct RecordingStorage {
    calls: Mutex<Vec<ThrottleCall>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThrottleCall {
    key: String,
    limit: u32,
    ttl: Duration,
}

impl RecordingStorage {
    fn calls(&self) -> Vec<ThrottleCall> {
        self.calls.lock().expect("test storage lock").clone()
    }
}

impl ThrottlerStorage for RecordingStorage {
    fn allow(&self, key: &str, limit: u32, ttl: Duration) -> bool {
        self.calls
            .lock()
            .expect("test storage lock")
            .push(ThrottleCall {
                key: key.to_string(),
                limit,
                ttl,
            });
        true
    }
}

#[test]
fn throttler_guard_builds_full_context_key_with_default_window() {
    let storage = Arc::new(RecordingStorage::default());
    let guard = ThrottlerGuard::new(7, Duration::from_secs(30)).with_storage(storage.clone());

    let mut request_context = RequestContext::new();
    request_context.set_custom_data("module_name", "users");
    request_context.set_custom_data("user_id", "42");
    request_context.set_custom_data("request_method", "PATCH");
    request_context.set_custom_data("request_path", "/profile");

    let context = ExecutionContext::new(()).with_request_context(request_context);

    assert!(guard.allows_request(&context));
    assert_eq!(
        storage.calls(),
        vec![ThrottleCall {
            key: "users:42:PATCH:/profile".to_string(),
            limit: 7,
            ttl: Duration::from_secs(30),
        }]
    );
}
