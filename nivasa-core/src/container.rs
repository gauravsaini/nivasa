//! Dependency injection container.
//!
//! Placeholder — full implementation in Phase 1.

/// The dependency injection container.
pub struct DependencyContainer;

impl DependencyContainer {
    /// Create a new empty container.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DependencyContainer {
    fn default() -> Self {
        Self::new()
    }
}
