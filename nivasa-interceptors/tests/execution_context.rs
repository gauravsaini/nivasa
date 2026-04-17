use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use nivasa_common::{HttpException, RequestContext};
use nivasa_interceptors::{CallHandler, ExecutionContext, Interceptor, InterceptorFuture};

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    fn raw_waker() -> RawWaker {
        fn clone(_: *const ()) -> RawWaker {
            raw_waker()
        }

        fn wake(_: *const ()) {}

        fn wake_by_ref(_: *const ()) {}

        fn drop(_: *const ()) {}

        RawWaker::new(
            std::ptr::null(),
            &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
        )
    }

    let waker = unsafe { Waker::from_raw(raw_waker()) };
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(&waker);

    loop {
        match Future::poll(Pin::as_mut(&mut future), &mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[derive(Debug, PartialEq)]
struct AuditRequest {
    id: &'static str,
}

#[derive(Debug)]
struct MetadataPrefixInterceptor;

impl Interceptor for MetadataPrefixInterceptor {
    type Response = String;

    fn intercept(
        &self,
        context: &ExecutionContext,
        next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        let class_name = context.class_name().unwrap_or("unknown").to_string();
        let role = context
            .request_context()
            .and_then(|request_context| request_context.handler_metadata("role"))
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .to_string();

        Box::pin(async move {
            let value = next.handle().await?;
            Ok(format!("{class_name}:{role}:{value}"))
        })
    }
}

#[test]
fn execution_context_propagates_request_context_metadata() {
    let mut request_context = RequestContext::new();
    request_context.insert_request_data(AuditRequest { id: "req-42" });
    request_context.set_handler_metadata("role", "admin");
    request_context.set_class_metadata("version", "v1");
    request_context.set_custom_data("request_id", "trace-123");

    let context = ExecutionContext::new()
        .with_request("GET", "/audit")
        .with_handler_name("show_audit")
        .with_class_name("AuditController")
        .with_request_context(request_context);

    assert_eq!(context.request_method(), Some("GET"));
    assert_eq!(context.request_path(), Some("/audit"));
    assert_eq!(context.handler_name(), Some("show_audit"));
    assert_eq!(context.class_name(), Some("AuditController"));
    assert_eq!(
        context.request_data::<AuditRequest>(),
        Some(&AuditRequest { id: "req-42" })
    );

    let shared = context
        .request_context()
        .expect("request context must exist");
    assert_eq!(
        shared
            .handler_metadata("role")
            .and_then(|value| value.as_str()),
        Some("admin")
    );
    assert_eq!(
        shared
            .class_metadata("version")
            .and_then(|value| value.as_str()),
        Some("v1")
    );
    assert_eq!(
        shared
            .custom_data("request_id")
            .and_then(|value| value.as_str()),
        Some("trace-123")
    );
}

#[test]
fn interceptor_and_call_handler_chain_preserves_metadata_and_runs_once() {
    let interceptor = MetadataPrefixInterceptor;
    let calls = Arc::new(AtomicUsize::new(0));
    let recorded = Arc::clone(&calls);

    let mut request_context = RequestContext::new();
    request_context.set_handler_metadata("role", "admin");

    let context = ExecutionContext::new()
        .with_class_name("AuditController")
        .with_request_context(request_context);

    let next = CallHandler::new(move || {
        let recorded = Arc::clone(&recorded);
        async move {
            recorded.fetch_add(1, Ordering::SeqCst);
            Ok::<_, HttpException>("ok".to_string())
        }
    });

    let result = block_on(interceptor.intercept(&context, next)).expect("interceptor should pass");

    assert_eq!(result, "AuditController:admin:ok");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
