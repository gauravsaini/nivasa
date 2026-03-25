use super::lifecycle::{NivasaModuleEvent, NivasaModuleState, NivasaModuleStatechart};
use super::{Module, OnModuleDestroy, OnModuleInit};
use crate::di::error::DiError;
use crate::di::DependencyContainer;
use nivasa_statechart::StatechartEngine;
use std::panic::{catch_unwind, AssertUnwindSafe};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModuleLifecycleError {
    #[error("invalid module lifecycle transition from {state:?} using {event:?}: {details}")]
    InvalidTransition {
        state: NivasaModuleState,
        event: NivasaModuleEvent,
        details: String,
    },
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
    pub fn new(module: M) -> Self {
        Self::with_container(module, DependencyContainer::new())
    }

    pub fn with_container(module: M, container: DependencyContainer) -> Self {
        Self {
            module,
            container,
            engine: StatechartEngine::new(NivasaModuleState::Unloaded),
        }
    }

    pub fn module(&self) -> &M {
        &self.module
    }

    pub fn container(&self) -> &DependencyContainer {
        &self.container
    }

    pub fn state(&self) -> NivasaModuleState {
        self.engine.current_state()
    }

    pub fn valid_events(&self) -> Vec<NivasaModuleEvent> {
        self.engine.valid_events()
    }

    pub fn is_terminal(&self) -> bool {
        self.engine.is_in_final_state()
    }

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
    pub fn start_loading(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ModuleLoad)
    }

    pub fn imports_resolved(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ImportsResolved)
    }

    pub fn import_missing(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ErrorImportMissing)
    }

    pub async fn register_providers(
        &mut self,
    ) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.module.configure(&self.container).await?;
        self.send_event(NivasaModuleEvent::ProvidersRegistered)
    }

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

    pub async fn load(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.start_loading()?;
        self.imports_resolved()?;
        self.register_providers().await?;
        self.resolve_dependencies().await
    }

    pub fn abort_load(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ModuleAbort)
    }

    pub fn initialize(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ModuleInit)
    }

    pub fn activate(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ModuleActivate)
    }

    pub fn begin_destroy(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::ModuleDestroy)
    }

    pub fn complete_destroy(&mut self) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.send_event(NivasaModuleEvent::DestroyComplete)
    }

    pub async fn activate_after_init(
        &mut self,
    ) -> Result<NivasaModuleState, ModuleLifecycleError> {
        self.initialize()?;
        self.activate()
    }
}

impl<M: Module + OnModuleInit> ModuleRuntime<M> {
    pub async fn initialize_with_hooks(
        &mut self,
    ) -> Result<NivasaModuleState, ModuleLifecycleError> {
        let state = self.initialize()?;
        self.module.on_module_init().await;
        Ok(state)
    }
}

impl<M: Module + OnModuleDestroy> ModuleRuntime<M> {
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
