use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiError {
    #[error("Provider not found for type: {0}")]
    ProviderNotFound(&'static str),

    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    #[error("Failed to construct provider {0}: {1}")]
    ConstructionFailed(&'static str, String),

    #[error("Invalid scope requested for provider {0}")]
    InvalidScope(&'static str),

    #[error("Registration error: {0}")]
    Registration(String),
}
