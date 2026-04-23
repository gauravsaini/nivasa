use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    time::Duration,
};

use nivasa_common::RequestContext;
use nivasa_guards::{
    AuthGuard, ExecutionContext, Guard, RolesGuard, ThrottlerGuard, ThrottlerStorage,
};

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

#[derive(Debug, Default)]
struct CountingStorage {
    calls: AtomicUsize,
}

impl ThrottlerStorage for CountingStorage {
    fn allow(&self, _key: &str, _limit: u32, _ttl: Duration) -> bool {
        self.calls.fetch_add(1, Ordering::SeqCst);
        true
    }
}

#[test]
fn in_memory_throttler_storage_counts_and_rejects_at_limit() {
    let storage = nivasa_guards::InMemoryThrottlerStorage::new();

    assert!(storage.allow("users", 2, Duration::from_secs(1)));
    assert!(storage.allow("users", 2, Duration::from_secs(1)));
    assert!(!storage.allow("users", 2, Duration::from_secs(1)));
    assert_eq!(storage.snapshot().get("users"), Some(&2));
}

#[test]
fn in_memory_throttler_storage_resets_after_ttl_expires() {
    let storage = nivasa_guards::InMemoryThrottlerStorage::new();

    assert!(storage.allow("window", 1, Duration::from_millis(1)));
    std::thread::sleep(Duration::from_millis(10));
    assert!(storage.allow("window", 1, Duration::from_millis(1)));
    assert_eq!(storage.snapshot().get("window"), Some(&1));
}

#[test]
fn throttler_guard_uses_request_context_overrides_and_key_fallbacks() {
    let storage = Arc::new(RecordingStorage::default());
    let guard = ThrottlerGuard::new(10, Duration::from_secs(60)).with_storage(storage.clone());

    let mut request_context = RequestContext::new();
    request_context.set_custom_data("request_method", "POST");
    request_context.set_custom_data("request_path", "/things");
    request_context.set_custom_data("throttle_limit", 3_u64);
    request_context.set_custom_data("throttle_ttl_secs", 9_u64);

    let context = ExecutionContext::new(())
        .with_request_context(request_context)
        .with_custom_data("unused", "value");

    assert!(guard.allows_request(&context));

    let calls = storage.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0],
        ThrottleCall {
            key: "::POST:/things".to_string(),
            limit: 3,
            ttl: Duration::from_secs(9),
        }
    );
}

#[test]
fn throttler_guard_skips_storage_when_skip_flag_is_set() {
    let storage = Arc::new(CountingStorage::default());
    let guard = ThrottlerGuard::new(10, Duration::from_secs(60)).with_storage(storage.clone());

    let mut request_context = RequestContext::new();
    request_context.set_custom_data("throttle_skip", true);
    request_context.set_custom_data("request_method", "GET");
    request_context.set_custom_data("request_path", "/skip");

    let context = ExecutionContext::new(()).with_request_context(request_context);

    assert!(guard.allows_request(&context));
    assert_eq!(storage.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn auth_guard_falls_back_to_context_custom_data() {
    let guard = AuthGuard::new();

    let string_context = ExecutionContext::new(()).with_custom_data(
        "authorization",
        String::from("Bearer header.payload.signature"),
    );
    assert!(run_ready(guard.can_activate(&string_context)).unwrap());

    let static_context = ExecutionContext::new(())
        .with_custom_data("authorization", "Bearer header.payload.signature");
    assert!(run_ready(guard.can_activate(&static_context)).unwrap());
}

#[test]
fn roles_guard_allows_without_required_roles_and_uses_context_level_metadata() {
    let guard = RolesGuard::new();

    let empty_context = ExecutionContext::new(());
    assert!(run_ready(guard.can_activate(&empty_context)).unwrap());

    let context = ExecutionContext::new(())
        .with_class_metadata("roles", vec!["admin"])
        .with_custom_data("roles", vec!["admin"]);
    assert!(run_ready(guard.can_activate(&context)).unwrap());
}

fn run_ready<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let waker = noop_waker();
    let mut context = Context::from_waker(&waker);

    match Future::poll(Pin::as_mut(&mut future), &mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("test future unexpectedly pending"),
    }
}

fn noop_waker() -> Waker {
    unsafe { Waker::from_raw(noop_raw_waker()) }
}

fn noop_raw_waker() -> RawWaker {
    RawWaker::new(std::ptr::null(), &NOOP_RAW_WAKER_VTABLE)
}

unsafe fn noop_clone(_: *const ()) -> RawWaker {
    noop_raw_waker()
}

unsafe fn noop(_: *const ()) {}

static NOOP_RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);
