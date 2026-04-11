use thiserror::Error;

/// DI container failure.
///
/// ```rust
/// use nivasa_core::di::DiError;
///
/// let err = DiError::ProviderNotFound("ExampleService");
/// assert_eq!(err.to_string(), "Provider not found for type: ExampleService");
/// ```
#[derive(Debug, Error)]
pub enum DiError {
    /// No provider exists for the requested type.
    ///
    /// ```rust
    /// use nivasa_core::di::DiError;
    ///
    /// let err = DiError::ProviderNotFound("ExampleService");
    /// assert_eq!(err.to_string(), "Provider not found for type: ExampleService");
    /// ```
    #[error("Provider not found for type: {0}")]
    ProviderNotFound(&'static str),

    /// Resolution formed a dependency cycle.
    ///
    /// ```rust
    /// use nivasa_core::di::DiError;
    ///
    /// let err = DiError::CircularDependency("A -> B -> A".into());
    /// assert!(err.to_string().contains("Circular dependency detected"));
    /// ```
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    /// Provider build failed after resolution started.
    ///
    /// ```rust
    /// use nivasa_core::di::DiError;
    ///
    /// let err = DiError::ConstructionFailed("ExampleService", "boom".into());
    /// assert!(err.to_string().contains("Failed to construct provider ExampleService"));
    /// ```
    #[error("Failed to construct provider {0}: {1}")]
    ConstructionFailed(&'static str, String),

    /// Provider scope was not valid for the request.
    ///
    /// ```rust
    /// use nivasa_core::di::DiError;
    ///
    /// let err = DiError::InvalidScope("ExampleService");
    /// assert_eq!(err.to_string(), "Invalid scope requested for provider ExampleService");
    /// ```
    #[error("Invalid scope requested for provider {0}")]
    InvalidScope(&'static str),

    /// Registration failed while inserting a provider.
    ///
    /// ```rust
    /// use nivasa_core::di::DiError;
    ///
    /// let err = DiError::Registration("duplicate binding".into());
    /// assert_eq!(err.to_string(), "Registration error: duplicate binding");
    /// ```
    #[error("Registration error: {0}")]
    Registration(String),
}
