//! # nivasa-routing
//!
//! Nivasa framework routing primitives.

/// Metadata describing a controller and the route prefix it owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerMetadata {
    path: String,
    version: Option<String>,
}

impl ControllerMetadata {
    /// Create controller metadata with a normalized route prefix.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: normalize_path(path.into()),
            version: None,
        }
    }

    /// Attach an explicit version to this controller.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        let version = version.into();
        self.version = if version.trim().is_empty() {
            None
        } else {
            Some(version)
        };
        self
    }

    /// The normalized route prefix for this controller.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The optional version segment for this controller.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}

/// Trait implemented by controller types.
///
/// The first controller macro pass can target this trait directly, and the
/// metadata is intentionally small so routing can remain framework-agnostic.
pub trait Controller: Send + Sync + 'static {
    fn metadata(&self) -> ControllerMetadata;
}

fn normalize_path(path: String) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "/".to_string();
    }

    let prefixed = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    };

    if prefixed.len() > 1 {
        prefixed.trim_end_matches('/').to_string()
    } else {
        prefixed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UsersController;

    impl Controller for UsersController {
        fn metadata(&self) -> ControllerMetadata {
            ControllerMetadata::new("users").with_version("v1")
        }
    }

    #[test]
    fn metadata_normalizes_route_prefixes() {
        let metadata = ControllerMetadata::new("users/");

        assert_eq!(metadata.path(), "/users");
        assert_eq!(metadata.version(), None);
    }

    #[test]
    fn metadata_supports_optional_version() {
        let metadata = ControllerMetadata::new("/users").with_version("v1");

        assert_eq!(metadata.path(), "/users");
        assert_eq!(metadata.version(), Some("v1"));
    }

    #[test]
    fn blank_path_collapses_to_root() {
        let metadata = ControllerMetadata::new("   ");

        assert_eq!(metadata.path(), "/");
    }

    #[test]
    fn controller_trait_returns_metadata() {
        let controller = UsersController;

        assert_eq!(
            controller.metadata(),
            ControllerMetadata::new("/users").with_version("v1")
        );
    }
}
