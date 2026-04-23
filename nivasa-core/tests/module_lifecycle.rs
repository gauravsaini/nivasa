use async_trait::async_trait;
use nivasa_core::di::DependencyContainer;
use nivasa_core::lifecycle as app_lifecycle;
use nivasa_core::module::lifecycle::NivasaModuleState;
use nivasa_core::module::{
    Module, ModuleLifecycleError, ModuleMetadata, ModuleRuntime, OnApplicationShutdown,
    OnModuleDestroy, OnModuleInit,
};
use nivasa_macros::injectable;
use std::sync::{Arc, Mutex};

#[injectable]
struct LifecycleService;

#[injectable]
struct MissingDependencyService {
    #[allow(dead_code)]
    missing: Arc<NeverRegisteredService>,
}

struct NeverRegisteredService;

#[derive(Clone)]
struct HookedModule {
    hooks: Arc<Mutex<Vec<&'static str>>>,
}

struct BrokenModule;

#[derive(Clone)]
struct ShutdownMirror {
    events: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl Module for HookedModule {
    fn metadata(&self) -> ModuleMetadata {
        ModuleMetadata::new().with_providers(vec![std::any::TypeId::of::<LifecycleService>()])
    }

    async fn configure(
        &self,
        container: &DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        container
            .register_injectable::<LifecycleService>(
                LifecycleService::__NIVASA_INJECTABLE_SCOPE,
                <LifecycleService as nivasa_core::di::provider::Injectable>::dependencies(),
            )
            .await;
        Ok(())
    }
}

#[async_trait]
impl OnModuleInit for HookedModule {
    async fn on_module_init(&self) {
        self.hooks.lock().unwrap().push("init");
    }
}

#[async_trait]
impl OnModuleDestroy for HookedModule {
    async fn on_module_destroy(&self) {
        self.hooks.lock().unwrap().push("destroy");
    }
}

#[async_trait]
impl OnApplicationShutdown for ShutdownMirror {
    async fn on_application_shutdown(&self) {
        self.events.lock().unwrap().push("module.shutdown");
    }
}

#[async_trait]
impl Module for BrokenModule {
    fn metadata(&self) -> ModuleMetadata {
        ModuleMetadata::new()
            .with_providers(vec![std::any::TypeId::of::<MissingDependencyService>()])
    }

    async fn configure(
        &self,
        container: &DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        let _ = &self;
        container
            .register_injectable::<MissingDependencyService>(
                MissingDependencyService::__NIVASA_INJECTABLE_SCOPE,
                <MissingDependencyService as nivasa_core::di::provider::Injectable>::dependencies(),
            )
            .await;
        Ok(())
    }
}

#[tokio::test]
async fn module_runtime_happy_path_tracks_scxml_lifecycle() {
    let hooks = Arc::new(Mutex::new(Vec::new()));
    let module = HookedModule {
        hooks: hooks.clone(),
    };
    let mut runtime = ModuleRuntime::new(module);

    assert_eq!(runtime.state(), NivasaModuleState::Unloaded);

    let load_state = runtime.load().await.unwrap();
    assert_eq!(load_state, NivasaModuleState::Loaded);
    assert_eq!(runtime.state(), NivasaModuleState::Loaded);

    let initialized = runtime.initialize_with_hooks().await.unwrap();
    assert_eq!(initialized, NivasaModuleState::Initialized);

    let active = runtime.activate().unwrap();
    assert_eq!(active, NivasaModuleState::Active);
    assert!(runtime
        .container()
        .resolve::<LifecycleService>()
        .await
        .is_ok());

    let destroyed = runtime.destroy_with_hooks().await.unwrap();
    assert_eq!(destroyed, NivasaModuleState::Destroyed);
    assert!(runtime.is_terminal());
    assert_eq!(&*hooks.lock().unwrap(), &["init", "destroy"]);
}

#[tokio::test]
async fn invalid_transition_rejection_keeps_statechart_in_control() {
    let module = HookedModule {
        hooks: Arc::new(Mutex::new(Vec::new())),
    };
    let mut runtime = ModuleRuntime::new(module);

    let err = runtime.imports_resolved().unwrap_err();
    match err {
        ModuleLifecycleError::InvalidTransition { state, .. } => {
            assert_eq!(state, NivasaModuleState::Unloaded);
        }
        other => panic!("unexpected error: {other:?}"),
    }

    assert_eq!(runtime.state(), NivasaModuleState::Unloaded);
}

#[tokio::test]
async fn dependency_failures_transition_to_load_failed() {
    let mut runtime = ModuleRuntime::new(BrokenModule);

    let err = runtime.load().await.unwrap_err();
    assert!(matches!(
        err,
        ModuleLifecycleError::DependencyInjection(
            nivasa_core::di::error::DiError::ProviderNotFound(_)
        )
    ));
    assert_eq!(runtime.state(), NivasaModuleState::LoadFailed);

    let failed = runtime.abort_load().unwrap();
    assert_eq!(failed, NivasaModuleState::Failed);
    assert!(runtime.is_terminal());
}

#[tokio::test]
async fn application_shutdown_hook_shape_stays_consistent_across_public_paths() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let hooks = ShutdownMirror {
        events: Arc::clone(&events),
    };

    <ShutdownMirror as OnApplicationShutdown>::on_application_shutdown(&hooks).await;
    <ShutdownMirror as app_lifecycle::OnApplicationShutdown>::on_application_shutdown(&hooks).await;

    assert_eq!(
        &*events.lock().unwrap(),
        &["module.shutdown", "module.shutdown"]
    );
}
