use super::lifecycle::{NivasaModuleEvent, NivasaModuleState, NivasaModuleStatechart};
use super::{Module, OnModuleDestroy, OnModuleInit};
use crate::di::error::DiError;
use crate::di::DependencyContainer;
use nivasa_statechart::StatechartEngine;
use std::panic::{catch_unwind, AssertUnwindSafe};
use thiserror::Error;

#[derive(Debug, Error)]
/// Errors from module lifecycle runtime.
pub enum ModuleLifecycleError {
    /// Statechart rejected state transition.
    #[error("invalid module lifecycle transition from {state:?} using {event:?}: {details}")]
    InvalidTransition {
        state: NivasaModuleState,
        event: NivasaModuleEvent,
        details: String,
    },
    /// Dependency container failure.
    #[error(transparent)]
    DependencyInjection(#[from] DiError),
}

/// Runtime owner for one module instance and its SCXML-backed lifecycle engine.
///
/// The statechart remains the source of truth: every lifecycle change goes
/// through `send_event`, and this type only packages the valid sequence into a
/// module-friendly API.
pub struct ModuleRuntime<M> {
    module: M,
    container: DependencyContainer,
    engine: StatechartEngine<NivasaModuleStatechart>,
}

impl<M> ModuleRuntime<M> {
    /// Build runtime with fresh dependency container.
    pub fn new(module: M) -> Self {
        Self::with_container(module, DependencyContainer::new())
    }

    /// Build runtime with caller-owned dependency container.
    pub fn with_container(module: M, container: DependencyContainer) -> Self {
        Self {
            module,
            container,
            engine: StatechartEngine::new(NivasaModuleState::Unloaded),
        }
    }

    /// Underlying module instance.
    pub fn module(&self) -> &M {
        &self.module
    }

    /// Dependency container owned by runtime.
    pub fn container(&self) -> &DependencyContainer {
        &self.container
    }

    /// Current SCXML state.
    pub fn state(&self) -> NivasaModuleState {
        self.engine.current_state()
    }

    /// Events valid in current state.
    pub fn valid_events(&self) -> Vec<NivasaModuleEvent> {
        self.engine.valid_events()
    }

    /// True when engine in final state.
    pub fn is_terminal(&self) -> bool {
        self.engine.is_in_final_state()
    }

    /// Send raw lifecycle event into SCXML engine.
    pub fn send_event(
        &mut self,
        event: NivasaModuleEvent,
    ) -> Result<NivasaModuleState, ModuleLifecycleError> {
        let current_state = self.engine.current_state();
        let event_for_err = event.clone();

        catch_unwind(AssertUnwindSafe(|| self.engine.send_event(event)))
            .map_err(|panic_payload| ModuleLifecycleError::InvalidTransition {
                state: current_state,
                event: event_for_err,
                details: panic_message(panic_payload),
            })?
            .map_err(|err| {
                let details = err.to_string();
                ModuleLifecycleError::InvalidTransition {
                    state: err.current_state,
                    event: err.event,
                    details,
                }
            })
    }
}

impl<M: Module> ModuleRuntime<M> {
    /// Enter loading state.
    pub fn start_loading(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ModuleLoad)
    }

    /// Mark imports resolved.
    pub fn imports_resolved(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ImportsResolved)
    }

    /// Mark missing import error.
    pub fn import_missing(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ErrorImportMissing)
    }

    /// Run module configure, then mark providers registered.
    pub async fn register_providers(
        &mut self,
    ) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.module.configure(&self.container).await?;
        self.send_event(NivasaModuleEvent::ProvidersRegistered)
    }

    /// Initialize dependency container, then map outcome to lifecycle state.
    pub async fn resolve_dependencies(
        &mut self,
    ) -> Result<NivasaModuleState, ModuleLifecycleError> {
        match self.container.initialize().await {
            Ok(()) => self.send_event(NivasaModuleEvent::DependenciesResolved),
            Err(err) => {
                let event = match &err {
                    DiError::CircularDependency(_) => NivasaModuleEvent::ErrorDiCircular,
                    DiError::ProviderNotFound(_) => NivasaModuleEvent::ErrorDiMissingProvider,
                    _ => return Err(err.into()),
                };

                let _ = self.send_event(event)?;
                Err(err.into())
            }
        }
    }

    /// Full load sequence.
    pub async fn load(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.start_loading()?;
        self.imports_resolved()?;
        self.register_providers().await?;
        self.resolve_dependencies().await
    }

    /// Abort loading sequence.
    pub fn abort_load(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ModuleAbort)
    }

    /// Enter init state.
    pub fn initialize(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ModuleInit)
    }

    /// Enter active state.
    pub fn activate(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ModuleActivate)
    }

    /// Begin destroy sequence.
    pub fn begin_destroy(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ModuleDestroy)
    }

    /// Finish destroy sequence.
    pub fn complete_destroy(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::DestroyComplete)
    }

    /// Init, then activate.
    pub async fn activate_after_init(
        &mut self,
    ) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.initialize()?;
        self.activate()
    }
}

impl<M: Module + OnModuleInit> ModuleRuntime<M> {
    /// Init, then run `on_module_init`.
    pub async fn initialize_with_hooks(
        &mut self,
    ) -> Result<NivasaModuleState, ModuleLifecycleError> {
        let state = self.initialize()?;
        self.module.on_module_init().await;
        Ok(state)
    }
}

impl<M: Module + OnModuleDestroy> ModuleRuntime<M> {
    /// Begin destroy, run `on_module_destroy`, then finish destroy.
    pub async fn destroy_with_hooks(
        &mut self,
    ) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.begin_destroy()?;
        self.module.on_module_destroy().await;
        self.complete_destroy()
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }

    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }

    "statechart engine panicked without a string payload".to_string()
}
