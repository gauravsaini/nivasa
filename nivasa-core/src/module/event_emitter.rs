use super::{Module, ModuleMetadata};
use crate::di::error::DiError;
use crate::di::provider::Injectable;
use crate::di::{DependencyContainer, ProviderScope};
use async_trait::async_trait;
use std::any::TypeId;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

type EventFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
type EventHandler = Arc<dyn Fn(String) -> EventFuture + Send + Sync + 'static>;

#[derive(Clone)]
struct EventListener {
    pattern: String,
    handler: EventHandler,
}

/// Injectable event bus for app-local event dispatch.
///
/// Handlers are registered by event pattern and invoked in registration order.
/// The matching rules are intentionally small:
/// - `*` matches every event
/// - `user.*` matches `user.created`, `user.updated`, etc.
/// - otherwise the event name must match exactly
#[derive(Clone, Default)]
pub struct EventEmitter {
    listeners: Arc<Mutex<Vec<EventListener>>>,
}

impl fmt::Debug for EventEmitter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventEmitter")
            .field("listener_count", &self.listener_count())
            .finish()
    }
}

impl EventEmitter {
    /// Create an empty emitter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an async handler for one event pattern.
    pub fn on<F, Fut>(&self, pattern: impl Into<String>, handler: F)
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let handler = Arc::new(handler);
        let listener = EventListener {
            pattern: pattern.into(),
            handler: Arc::new(move |payload| Box::pin((handler.as_ref())(payload))),
        };

        self.listeners
            .lock()
            .expect("event emitter lock poisoned")
            .push(listener);
    }

    /// Emit one event payload to every matching handler.
    pub async fn emit(&self, event: impl Into<String>, payload: impl Into<String>) -> usize {
        let event = event.into();
        let payload = payload.into();
        let listeners = self.listeners_for(&event);
        let mut delivered = 0;

        for listener in listeners {
            (listener.handler)(payload.clone()).await;
            delivered += 1;
        }

        delivered
    }

    /// Return the number of registered handlers.
    pub fn listener_count(&self) -> usize {
        self.listeners
            .lock()
            .expect("event emitter lock poisoned")
            .len()
    }

    fn listeners_for(&self, event: &str) -> Vec<EventListener> {
        self.listeners
            .lock()
            .expect("event emitter lock poisoned")
            .iter()
            .filter(|listener| matches_event(event, &listener.pattern))
            .cloned()
            .collect()
    }
}

fn matches_event(event: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix(".*") {
        if prefix.is_empty() {
            return true;
        }

        return event
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('.'));
    }

    event == pattern
}

#[async_trait]
impl Injectable for EventEmitter {
    async fn build(_container: &DependencyContainer) -> Result<Self, DiError> {
        Ok(Self::new())
    }

    fn dependencies() -> Vec<TypeId> {
        Vec::new()
    }
}

/// Event-emitter module that registers the injectable event bus.
#[derive(Debug, Default, Clone, Copy)]
pub struct EventEmitterModule;

impl EventEmitterModule {
    /// Create a new module shell.
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Module for EventEmitterModule {
    fn metadata(&self) -> ModuleMetadata {
        ModuleMetadata::new()
            .with_providers(vec![TypeId::of::<EventEmitter>()])
            .with_exports(vec![TypeId::of::<EventEmitter>()])
            .with_global(true)
    }

    async fn configure(&self, container: &DependencyContainer) -> Result<(), DiError> {
        container
            .register_injectable::<EventEmitter>(ProviderScope::Singleton, vec![])
            .await;
        Ok(())
    }
}
