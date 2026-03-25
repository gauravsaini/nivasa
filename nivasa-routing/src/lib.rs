//! # nivasa-routing
//!
//! Nivasa framework routing primitives.

/// A normalized route pattern.
///
/// This first pass only supports static path segments. The enum keeps the API
/// open for parameterized, wildcard, and optional segment variants later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutePattern {
    Static(Vec<String>),
}

/// A route entry stored in the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteEntry<T> {
    pub pattern: RoutePattern,
    pub value: T,
}

/// Errors raised when registering or parsing routes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteRegistryError {
    DuplicateRoute { path: String },
    UnsupportedPatternSegment { path: String, segment: String },
}

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

impl RoutePattern {
    /// Create a static route pattern from a path string.
    pub fn static_path(path: impl Into<String>) -> Result<Self, RouteRegistryError> {
        let normalized = normalize_path(path.into());
        let segments = split_path_segments(&normalized).collect::<Vec<_>>();

        if let Some(segment) = segments.iter().find(|segment| is_dynamic_segment(segment)) {
            return Err(RouteRegistryError::UnsupportedPatternSegment {
                path: normalized,
                segment: segment.clone(),
            });
        }

        Ok(RoutePattern::Static(segments))
    }

    /// The canonical route path for this pattern.
    pub fn path(&self) -> String {
        match self {
            RoutePattern::Static(segments) => join_segments(segments),
        }
    }

    /// Check whether this pattern matches the provided path.
    pub fn matches(&self, path: &str) -> bool {
        let candidate = normalize_path(path.to_string());
        let candidate_segments = split_path_segments(&candidate).collect::<Vec<_>>();

        match self {
            RoutePattern::Static(expected) => expected == &candidate_segments,
        }
    }

    fn specificity(&self) -> usize {
        match self {
            RoutePattern::Static(segments) => segments.len(),
        }
    }
}

/// Registry of routes keyed by normalized path pattern.
#[derive(Debug, Clone)]
pub struct RouteRegistry<T> {
    routes: Vec<RouteEntry<T>>,
}

impl<T> Default for RouteRegistry<T> {
    fn default() -> Self {
        Self { routes: Vec::new() }
    }
}

impl<T> RouteRegistry<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &RouteEntry<T>> {
        self.routes.iter()
    }

    pub fn register(
        &mut self,
        pattern: RoutePattern,
        value: T,
    ) -> Result<(), RouteRegistryError> {
        let path = pattern.path();
        if self.routes.iter().any(|entry| entry.pattern.path() == path) {
            return Err(RouteRegistryError::DuplicateRoute { path });
        }

        self.routes.push(RouteEntry { pattern, value });
        self.routes.sort_by_key(|entry| usize::MAX - entry.pattern.specificity());
        Ok(())
    }

    pub fn register_static(
        &mut self,
        path: impl Into<String>,
        value: T,
    ) -> Result<(), RouteRegistryError> {
        let pattern = RoutePattern::static_path(path)?;
        self.register(pattern, value)
    }

    pub fn resolve(&self, path: &str) -> Option<&T> {
        self.routes
            .iter()
            .find(|entry| entry.pattern.matches(path))
            .map(|entry| &entry.value)
    }

    pub fn resolve_entry(&self, path: &str) -> Option<&RouteEntry<T>> {
        self.routes.iter().find(|entry| entry.pattern.matches(path))
    }

    pub fn contains(&self, path: &str) -> bool {
        self.resolve(path).is_some()
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

fn split_path_segments(path: &str) -> impl Iterator<Item = String> + '_ {
    path.split('/').filter(|segment| !segment.is_empty()).map(str::to_string)
}

fn join_segments(segments: &[String]) -> String {
    if segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", segments.join("/"))
    }
}

fn is_dynamic_segment(segment: &str) -> bool {
    segment.starts_with(':') || segment.starts_with('*') || segment.ends_with('?')
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

    #[test]
    fn route_pattern_matches_static_paths() {
        let pattern = RoutePattern::static_path("/users").unwrap();

        assert!(pattern.matches("/users"));
        assert!(pattern.matches("users/"));
        assert!(!pattern.matches("/users/123"));
    }

    #[test]
    fn route_registry_resolves_static_routes() {
        let mut registry = RouteRegistry::new();

        registry.register_static("/users", "users").unwrap();
        registry.register_static("/health", "health").unwrap();

        assert_eq!(registry.resolve("/users"), Some(&"users"));
        assert_eq!(registry.resolve("health/"), Some(&"health"));
        assert_eq!(registry.resolve("/missing"), None);
    }

    #[test]
    fn route_registry_rejects_duplicates() {
        let mut registry = RouteRegistry::new();

        registry.register_static("/users", "first").unwrap();

        let err = registry.register_static("/users/", "second").unwrap_err();
        assert_eq!(
            err,
            RouteRegistryError::DuplicateRoute {
                path: "/users".to_string()
            }
        );
    }

    #[test]
    fn route_registry_rejects_dynamic_patterns_for_now() {
        let err = RoutePattern::static_path("/users/:id").unwrap_err();

        assert_eq!(
            err,
            RouteRegistryError::UnsupportedPatternSegment {
                path: "/users/:id".to_string(),
                segment: ":id".to_string()
            }
        );
    }
}
