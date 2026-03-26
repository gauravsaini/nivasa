use super::ModuleMetadata;
use std::any::TypeId;

/// Dynamic module metadata plus additional provider registrations.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DynamicModule {
    pub metadata: ModuleMetadata,
    pub providers: Vec<TypeId>,
}

impl DynamicModule {
    pub fn new(metadata: ModuleMetadata) -> Self {
        Self {
            metadata,
            providers: Vec::new(),
        }
    }

    pub fn with_providers(mut self, providers: Vec<TypeId>) -> Self {
        self.providers = providers;
        self
    }

    pub fn with_global(mut self, is_global: bool) -> Self {
        self.metadata = self.metadata.with_global(is_global);
        self
    }
}

/// Factory trait for modules that expose runtime-configurable metadata.
pub trait ConfigurableModule {
    type Options;

    fn for_root(options: Self::Options) -> DynamicModule;

    fn for_feature(options: Self::Options) -> DynamicModule;
}
