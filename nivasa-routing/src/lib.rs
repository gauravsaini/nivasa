//! # nivasa-routing
//!
//! Nivasa framework routing primitives.

use std::cmp::Ordering;

/// A normalized route pattern.
///
/// Static routes remain the common case, and the parser also accepts named
/// parameters plus trailing wildcard segments for broader matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutePattern {
    Static(Vec<String>),
    Pattern(Vec<RoutePatternSegment>),
}

/// A single parsed route segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutePatternSegment {
    Literal(String),
    Parameter { name: String },
    Wildcard { name: Option<String> },
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
        let segments = parse_segments(&normalized)?;

        if let Some(segment) = segments
            .iter()
            .find(|segment| !matches!(segment, RoutePatternSegment::Literal(_)))
        {
            return Err(unsupported_segment(&normalized, &segment_label(segment)));
        }

        Ok(RoutePattern::Static(
            segments
                .into_iter()
                .map(|segment| match segment {
                    RoutePatternSegment::Literal(value) => value,
                    _ => unreachable!(),
                })
                .collect(),
        ))
    }

    /// Parse a route pattern from a path string.
    pub fn parse(path: impl Into<String>) -> Result<Self, RouteRegistryError> {
        let normalized = normalize_path(path.into());
        let segments = parse_segments(&normalized)?;

        if segments
            .iter()
            .all(|segment| matches!(segment, RoutePatternSegment::Literal(_)))
        {
            Ok(RoutePattern::Static(
                segments
                    .into_iter()
                    .map(|segment| match segment {
                        RoutePatternSegment::Literal(value) => value,
                        _ => unreachable!(),
                    })
                    .collect(),
            ))
        } else {
            Ok(RoutePattern::Pattern(segments))
        }
    }

    /// The canonical route path for this pattern.
    pub fn path(&self) -> String {
        match self {
            RoutePattern::Static(segments) => join_segments(segments),
            RoutePattern::Pattern(segments) => {
                let segments = segments
                    .iter()
                    .map(|segment| match segment {
                        RoutePatternSegment::Literal(value) => value.clone(),
                        RoutePatternSegment::Parameter { name } => format!(":{}", name),
                        RoutePatternSegment::Wildcard { name } => name
                            .as_ref()
                            .map(|value| format!("*{}", value))
                            .unwrap_or_else(|| "*".to_string()),
                    })
                    .collect::<Vec<_>>();

                join_segments(&segments)
            }
        }
    }

    /// Check whether this pattern matches the provided path.
    pub fn matches(&self, path: &str) -> bool {
        let candidate = normalize_path(path.to_string());
        let candidate_segments = split_path_segments(&candidate).collect::<Vec<_>>();

        match self {
            RoutePattern::Static(expected) => expected == &candidate_segments,
            RoutePattern::Pattern(expected) => matches_pattern(expected, &candidate_segments),
        }
    }

    fn specificity(&self) -> Vec<u8> {
        match self {
            RoutePattern::Static(segments) => {
                vec![RouteSegmentKind::Literal.rank(); segments.len()]
            }
            RoutePattern::Pattern(segments) => segments
                .iter()
                .map(|segment| match segment {
                    RoutePatternSegment::Literal(_) => RouteSegmentKind::Literal.rank(),
                    RoutePatternSegment::Parameter { .. } => RouteSegmentKind::Parameter.rank(),
                    RoutePatternSegment::Wildcard { .. } => RouteSegmentKind::Wildcard.rank(),
                })
                .collect(),
        }
    }

    fn cmp_specificity(&self, other: &Self) -> Ordering {
        let self_specificity = self.specificity();
        let other_specificity = other.specificity();

        for (lhs, rhs) in self_specificity.iter().zip(other_specificity.iter()) {
            let ordering = rhs.cmp(lhs);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        self_specificity.len().cmp(&other_specificity.len())
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

    pub fn register(&mut self, pattern: RoutePattern, value: T) -> Result<(), RouteRegistryError> {
        let path = pattern.path();
        if self.routes.iter().any(|entry| entry.pattern.path() == path) {
            return Err(RouteRegistryError::DuplicateRoute { path });
        }

        self.routes.push(RouteEntry { pattern, value });
        self.routes
            .sort_by(|left, right| left.pattern.cmp_specificity(&right.pattern));
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

    pub fn register_pattern(
        &mut self,
        path: impl Into<String>,
        value: T,
    ) -> Result<(), RouteRegistryError> {
        let pattern = RoutePattern::parse(path)?;
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
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
}

fn join_segments(segments: &[String]) -> String {
    if segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", segments.join("/"))
    }
}

fn parse_segments(path: &str) -> Result<Vec<RoutePatternSegment>, RouteRegistryError> {
    let segments = split_path_segments(path).collect::<Vec<_>>();
    let mut parsed = Vec::with_capacity(segments.len());
    let mut saw_wildcard = false;

    for (index, segment) in segments.iter().enumerate() {
        if saw_wildcard {
            return Err(unsupported_segment(path, segment));
        }

        if segment.ends_with('?') {
            return Err(unsupported_segment(path, segment));
        }

        if let Some(name) = segment.strip_prefix(':') {
            if name.is_empty() {
                return Err(unsupported_segment(path, segment));
            }

            parsed.push(RoutePatternSegment::Parameter {
                name: name.to_string(),
            });
            continue;
        }

        if let Some(name) = segment.strip_prefix('*') {
            if index != segments.len() - 1 {
                return Err(unsupported_segment(path, segment));
            }

            saw_wildcard = true;
            parsed.push(RoutePatternSegment::Wildcard {
                name: if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                },
            });
            continue;
        }

        parsed.push(RoutePatternSegment::Literal(segment.clone()));
    }

    Ok(parsed)
}

fn matches_pattern(pattern: &[RoutePatternSegment], candidate: &[String]) -> bool {
    let mut candidate_index = 0;

    for (index, segment) in pattern.iter().enumerate() {
        match segment {
            RoutePatternSegment::Literal(expected) => {
                let Some(actual) = candidate.get(candidate_index) else {
                    return false;
                };

                if actual != expected {
                    return false;
                }

                candidate_index += 1;
            }
            RoutePatternSegment::Parameter { .. } => {
                if candidate.get(candidate_index).is_none() {
                    return false;
                }

                candidate_index += 1;
            }
            RoutePatternSegment::Wildcard { .. } => {
                return index == pattern.len() - 1;
            }
        }
    }

    candidate_index == candidate.len()
}

fn unsupported_segment(path: &str, segment: &str) -> RouteRegistryError {
    RouteRegistryError::UnsupportedPatternSegment {
        path: path.to_string(),
        segment: segment.to_string(),
    }
}

fn segment_label(segment: &RoutePatternSegment) -> String {
    match segment {
        RoutePatternSegment::Literal(value) => value.clone(),
        RoutePatternSegment::Parameter { name } => format!(":{}", name),
        RoutePatternSegment::Wildcard { name } => name
            .as_ref()
            .map(|value| format!("*{}", value))
            .unwrap_or_else(|| "*".to_string()),
    }
}

#[derive(Clone, Copy)]
enum RouteSegmentKind {
    Literal,
    Parameter,
    Wildcard,
}

impl RouteSegmentKind {
    fn rank(self) -> u8 {
        match self {
            RouteSegmentKind::Literal => 3,
            RouteSegmentKind::Parameter => 2,
            RouteSegmentKind::Wildcard => 1,
        }
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
    fn route_registry_keeps_static_path_constructor_strict() {
        let err = RoutePattern::static_path("/users/:id").unwrap_err();

        assert_eq!(
            err,
            RouteRegistryError::UnsupportedPatternSegment {
                path: "/users/:id".to_string(),
                segment: ":id".to_string()
            }
        );
    }

    #[test]
    fn route_pattern_parses_named_parameters() {
        let pattern = RoutePattern::parse("/users/:id").unwrap();

        assert_eq!(pattern.path(), "/users/:id");
        assert!(pattern.matches("/users/42"));
        assert!(!pattern.matches("/users"));
        assert!(!pattern.matches("/users/42/posts"));
    }

    #[test]
    fn route_pattern_parses_wildcards() {
        let pattern = RoutePattern::parse("/files/*path").unwrap();

        assert_eq!(pattern.path(), "/files/*path");
        assert!(pattern.matches("/files"));
        assert!(pattern.matches("/files/a/b/c"));
        assert!(!pattern.matches("/other"));
    }

    #[test]
    fn route_registry_prefers_more_specific_patterns() {
        let mut registry = RouteRegistry::new();

        registry
            .register_pattern("/files/*path", "wildcard")
            .unwrap();
        registry
            .register_pattern("/files/:name", "parameter")
            .unwrap();
        registry
            .register_static("/files/archive", "static")
            .unwrap();

        assert_eq!(registry.resolve("/files/archive"), Some(&"static"));
        assert_eq!(registry.resolve("/files/readme"), Some(&"parameter"));
        assert_eq!(registry.resolve("/files/docs/guide"), Some(&"wildcard"));
    }
}
