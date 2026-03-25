pub mod lifecycle;

use async_trait::async_trait;
use std::any::TypeId;
use crate::di::DependencyContainer;

/// Metadata for a Nivasa module.
#[derive(Default, Clone)]
pub struct ModuleMetadata {
    pub imports: Vec<TypeId>,
    pub providers: Vec<TypeId>,
    pub controllers: Vec<TypeId>,
    pub exports: Vec<TypeId>,
}

/// The core trait for all Nivasa modules.
#[async_trait]
pub trait Module: Send + Sync + 'static {
    fn metadata(&self) -> ModuleMetadata;
    async fn configure(&self, container: &DependencyContainer) -> Result<(), crate::di::error::DiError>;
}

/// Lifecycle hook traits
#[async_trait]
pub trait OnModuleInit: Send + Sync { async fn on_module_init(&self); }
#[async_trait]
pub trait OnModuleDestroy: Send + Sync { async fn on_module_destroy(&self); }
#[async_trait]
pub trait OnApplicationBootstrap: Send + Sync { async fn on_application_bootstrap(&self); }
#[async_trait]
pub trait OnApplicationShutdown: Send + Sync { async fn on_application_shutdown(&self); }
