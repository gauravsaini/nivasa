use nivasa_core::{DependencyContainer, EventEmitter, EventEmitterModule, Module};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

#[tokio::test]
async fn event_emitter_module_registers_injectable_service() {
    let container = DependencyContainer::new();
    EventEmitterModule.configure(&container).await.unwrap();

    let emitter = container.resolve::<EventEmitter>().await.unwrap();
    assert_eq!(emitter.listener_count(), 0);
}

#[tokio::test]
async fn event_emitter_dispatches_exact_wildcard_and_async_handlers() {
    let emitter = EventEmitter::new();
    let exact_one = Arc::new(AtomicUsize::new(0));
    let exact_two = Arc::new(AtomicUsize::new(0));
    let wildcard = Arc::new(AtomicUsize::new(0));
    let catch_all = Arc::new(AtomicUsize::new(0));
    let wildcard_payloads = Arc::new(Mutex::new(Vec::new()));
    let catch_all_payloads = Arc::new(Mutex::new(Vec::new()));

    {
        let exact_one = Arc::clone(&exact_one);
        emitter.on("user.created", move |payload| {
            let exact_one = Arc::clone(&exact_one);
            async move {
                assert_eq!(payload, "alice");
                exact_one.fetch_add(1, Ordering::SeqCst);
            }
        });
    }

    {
        let exact_two = Arc::clone(&exact_two);
        emitter.on("user.created", move |payload| {
            let exact_two = Arc::clone(&exact_two);
            async move {
                assert_eq!(payload, "alice");
                tokio::task::yield_now().await;
                exact_two.fetch_add(1, Ordering::SeqCst);
            }
        });
    }

    {
        let wildcard = Arc::clone(&wildcard);
        let wildcard_payloads = Arc::clone(&wildcard_payloads);
        emitter.on("user.*", move |payload| {
            let wildcard = Arc::clone(&wildcard);
            let wildcard_payloads = Arc::clone(&wildcard_payloads);
            async move {
                wildcard_payloads.lock().unwrap().push(payload);
                wildcard.fetch_add(1, Ordering::SeqCst);
            }
        });
    }

    {
        let catch_all = Arc::clone(&catch_all);
        let catch_all_payloads = Arc::clone(&catch_all_payloads);
        emitter.on("*", move |payload| {
            let catch_all = Arc::clone(&catch_all);
            let catch_all_payloads = Arc::clone(&catch_all_payloads);
            async move {
                catch_all_payloads.lock().unwrap().push(payload);
                catch_all.fetch_add(1, Ordering::SeqCst);
            }
        });
    }

    let delivered = emitter.emit("user.created", "alice").await;
    assert_eq!(delivered, 4);
    assert_eq!(exact_one.load(Ordering::SeqCst), 1);
    assert_eq!(exact_two.load(Ordering::SeqCst), 1);
    assert_eq!(wildcard.load(Ordering::SeqCst), 1);
    assert_eq!(catch_all.load(Ordering::SeqCst), 1);
    assert_eq!(
        wildcard_payloads.lock().unwrap().as_slice(),
        ["alice".to_string()]
    );
    assert_eq!(
        catch_all_payloads.lock().unwrap().as_slice(),
        ["alice".to_string()]
    );

    let delivered = emitter.emit("user.deleted", "bob").await;
    assert_eq!(delivered, 2);
    assert_eq!(exact_one.load(Ordering::SeqCst), 1);
    assert_eq!(exact_two.load(Ordering::SeqCst), 1);
    assert_eq!(wildcard.load(Ordering::SeqCst), 2);
    assert_eq!(catch_all.load(Ordering::SeqCst), 2);
    assert_eq!(
        wildcard_payloads.lock().unwrap().as_slice(),
        ["alice".to_string(), "bob".to_string()]
    );
    assert_eq!(
        catch_all_payloads.lock().unwrap().as_slice(),
        ["alice".to_string(), "bob".to_string()]
    );
}
