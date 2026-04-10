use thiserror::Error;

/// DI container failure.
#[derive(Debug, Error)]
pub enum DiError {
    /// No provider exists for the requested type.
    #[error("Provider not found for type: {0}")]
    ProviderNotFound(&'static str),

    /// Resolution formed a dependency cycle.
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    /// Provider build failed after resolution started.
    #[error("Failed to construct provider {0}: {1}")]
    ConstructionFailed(&'static str, String),

    /// Provider scope was not valid for the request.
    #[error("Invalid scope requested for provider {0}")]
    InvalidScope(&'static str),

    /// Registration failed while inserting a provider.
    #[error("Registration error: {0}")]
    Registration(String),
}
