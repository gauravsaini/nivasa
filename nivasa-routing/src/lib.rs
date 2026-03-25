//! # nivasa-routing
//!
//! Nivasa framework routing primitives.

use std::cmp::Ordering;

/// A normalized route pattern.
///
/// Static routes remain the common case, and the parser also accepts named
/// parameters, trailing optional parameters, and trailing wildcard segments for
/// broader matching.
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
    OptionalParameter { name: String },
    Wildcard { name: Option<String> },
}

/// A route entry stored in the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteEntry<T> {
    pub pattern: RoutePattern,
    pub value: T,
}

/// An HTTP-like method used by the dispatch registry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RouteMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
    All,
    Other(String),
}

/// A method-aware route entry stored in the dispatch registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDispatchEntry<T> {
    pub method: RouteMethod,
    pub pattern: RoutePattern,
    pub version: Option<String>,
    pub value: T,
}

/// Captured values extracted from a matching route path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoutePathCaptures {
    parameters: Vec<(String, String)>,
    wildcard: Option<(Option<String>, String)>,
}

/// A matched dispatch entry plus its captured path values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDispatchMatch<'a, T> {
    pub entry: &'a RouteDispatchEntry<T>,
    pub captures: RoutePathCaptures,
}

/// Errors raised when registering or parsing routes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteRegistryError {
    DuplicateRoute { path: String },
    UnsupportedPatternSegment { path: String, segment: String },
}

/// Errors raised when registering or parsing method-aware routes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteDispatchError {
    DuplicateRoute { method: String, path: String },
    UnsupportedPatternSegment { path: String, segment: String },
}

impl std::fmt::Display for RouteRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteRegistryError::DuplicateRoute { path } => {
                write!(f, "duplicate route `{path}`")
            }
            RouteRegistryError::UnsupportedPatternSegment { path, segment } => {
                write!(f, "unsupported route segment `{segment}` in `{path}`")
            }
        }
    }
}

impl std::error::Error for RouteRegistryError {}

impl std::fmt::Display for RouteDispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteDispatchError::DuplicateRoute { method, path } => {
                write!(f, "duplicate route `{method} {path}`")
            }
            RouteDispatchError::UnsupportedPatternSegment { path, segment } => {
                write!(f, "unsupported route segment `{segment}` in `{path}`")
            }
        }
    }
}

impl std::error::Error for RouteDispatchError {}

/// Result of a method-aware dispatch lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteDispatchOutcome<'a, T> {
    Matched(&'a RouteDispatchEntry<T>),
    MethodNotAllowed {
        path: String,
        allowed_methods: Vec<String>,
    },
    NotFound,
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

    /// The controller route prefix with an optional URI version prefix.
    ///
    /// Versions are normalized to a `v{version}` segment so `1` becomes `v1`.
    pub fn versioned_path(&self) -> String {
        match self.version.as_deref() {
            Some(version) => {
                merge_route_paths(format!("/{}", version_segment(version)), self.path.clone())
            }
            None => self.path.clone(),
        }
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
                        RoutePatternSegment::OptionalParameter { name } => format!(":{}?", name),
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
        self.captures(path).is_some()
    }

    /// Capture values from a matching route path.
    pub fn captures(&self, path: &str) -> Option<RoutePathCaptures> {
        let candidate = normalize_path(path.to_string());
        let candidate_segments = split_path_segments(&candidate).collect::<Vec<_>>();

        match self {
            RoutePattern::Static(expected) => {
                if expected == &candidate_segments {
                    Some(RoutePathCaptures::default())
                } else {
                    None
                }
            }
            RoutePattern::Pattern(expected) => capture_pattern(expected, &candidate_segments),
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
                    RoutePatternSegment::OptionalParameter { .. } => {
                        RouteSegmentKind::OptionalParameter.rank()
                    }
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

impl RouteMethod {
    /// Parse a method name into a route method.
    pub fn parse(method: impl Into<String>) -> Self {
        match method.into().trim().to_ascii_uppercase().as_str() {
            "GET" => RouteMethod::Get,
            "POST" => RouteMethod::Post,
            "PUT" => RouteMethod::Put,
            "DELETE" => RouteMethod::Delete,
            "PATCH" => RouteMethod::Patch,
            "HEAD" => RouteMethod::Head,
            "OPTIONS" => RouteMethod::Options,
            "ALL" => RouteMethod::All,
            other => RouteMethod::Other(other.to_string()),
        }
    }

    /// The canonical method string.
    pub fn as_str(&self) -> &str {
        match self {
            RouteMethod::Get => "GET",
            RouteMethod::Post => "POST",
            RouteMethod::Put => "PUT",
            RouteMethod::Delete => "DELETE",
            RouteMethod::Patch => "PATCH",
            RouteMethod::Head => "HEAD",
            RouteMethod::Options => "OPTIONS",
            RouteMethod::All => "ALL",
            RouteMethod::Other(method) => method.as_str(),
        }
    }

    fn matches(&self, method: &str) -> bool {
        matches!(self, RouteMethod::All) || self.as_str() == method
    }
}

impl From<&str> for RouteMethod {
    fn from(value: &str) -> Self {
        RouteMethod::parse(value)
    }
}

impl From<String> for RouteMethod {
    fn from(value: String) -> Self {
        RouteMethod::parse(value)
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

/// Registry of method-aware routes keyed by normalized path pattern.
#[derive(Debug, Clone)]
pub struct RouteDispatchRegistry<T> {
    routes: Vec<RouteDispatchEntry<T>>,
}

impl<T> Default for RouteDispatchRegistry<T> {
    fn default() -> Self {
        Self { routes: Vec::new() }
    }
}

impl<T> RouteDispatchRegistry<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &RouteDispatchEntry<T>> {
        self.routes.iter()
    }

    pub fn register(
        &mut self,
        method: impl Into<RouteMethod>,
        pattern: RoutePattern,
        value: T,
    ) -> Result<(), RouteDispatchError> {
        self.register_versioned(method, pattern, None, value)
    }

    pub fn register_versioned(
        &mut self,
        method: impl Into<RouteMethod>,
        pattern: RoutePattern,
        version: Option<String>,
        value: T,
    ) -> Result<(), RouteDispatchError> {
        let method = method.into();
        let path = pattern.path();
        let version = version.and_then(|value| normalize_version_token(&value));

        if self
            .routes
            .iter()
            .any(|entry| {
                entry.method == method
                    && entry.pattern.path() == path
                    && entry.version == version
            })
        {
            return Err(RouteDispatchError::DuplicateRoute {
                method: method.as_str().to_string(),
                path,
            });
        }

        self.routes.push(RouteDispatchEntry {
            method,
            pattern,
            version,
            value,
        });
        self.routes
            .sort_by(|left, right| left.pattern.cmp_specificity(&right.pattern));
        Ok(())
    }

    pub fn register_static(
        &mut self,
        method: impl Into<RouteMethod>,
        path: impl Into<String>,
        value: T,
    ) -> Result<(), RouteDispatchError> {
        let pattern = RoutePattern::static_path(path)
            .map_err(|err| match err {
                RouteRegistryError::UnsupportedPatternSegment { path, segment } => {
                    RouteDispatchError::UnsupportedPatternSegment { path, segment }
                }
                RouteRegistryError::DuplicateRoute { path } => {
                    RouteDispatchError::DuplicateRoute {
                        method: RouteMethod::parse("GET").as_str().to_string(),
                        path,
                    }
                }
            })?;
        self.register(method, pattern, value)
    }

    pub fn register_pattern(
        &mut self,
        method: impl Into<RouteMethod>,
        path: impl Into<String>,
        value: T,
    ) -> Result<(), RouteDispatchError> {
        let path = path.into();
        let pattern = RoutePattern::parse(path.clone()).map_err(|err| match err {
            RouteRegistryError::UnsupportedPatternSegment { path, segment } => {
                RouteDispatchError::UnsupportedPatternSegment { path, segment }
            }
            RouteRegistryError::DuplicateRoute { path } => RouteDispatchError::DuplicateRoute {
                method: RouteMethod::parse("GET").as_str().to_string(),
                path,
            },
        })?;
        self.register(method, pattern, value)
    }

    pub fn register_controller_route(
        &mut self,
        method: impl Into<RouteMethod>,
        controller_prefix: impl Into<String>,
        path: impl Into<String>,
        value: T,
    ) -> Result<(), RouteDispatchError> {
        let merged = merge_route_paths(controller_prefix.into(), path.into());
        self.register_pattern(method, merged, value)
    }

    pub fn register_versioned_controller_route(
        &mut self,
        method: impl Into<RouteMethod>,
        metadata: &ControllerMetadata,
        path: impl Into<String>,
        value: T,
    ) -> Result<(), RouteDispatchError> {
        let merged = merge_route_paths(metadata.versioned_path(), path.into());
        self.register_pattern(method, merged, value)
    }

    pub fn register_header_versioned_route(
        &mut self,
        method: impl Into<RouteMethod>,
        version: impl Into<String>,
        path: impl Into<String>,
        value: T,
    ) -> Result<(), RouteDispatchError> {
        let path = path.into();
        let pattern = RoutePattern::parse(path.clone()).map_err(|err| match err {
            RouteRegistryError::UnsupportedPatternSegment { path, segment } => {
                RouteDispatchError::UnsupportedPatternSegment { path, segment }
            }
            RouteRegistryError::DuplicateRoute { path } => RouteDispatchError::DuplicateRoute {
                method: RouteMethod::parse("GET").as_str().to_string(),
                path,
            },
        })?;
        self.register_versioned(
            method,
            pattern,
            normalize_version_token(&version.into()),
            value,
        )
    }

    pub fn register_media_type_versioned_route(
        &mut self,
        method: impl Into<RouteMethod>,
        version: impl Into<String>,
        path: impl Into<String>,
        value: T,
    ) -> Result<(), RouteDispatchError> {
        let path = path.into();
        let pattern = RoutePattern::parse(path.clone()).map_err(|err| match err {
            RouteRegistryError::UnsupportedPatternSegment { path, segment } => {
                RouteDispatchError::UnsupportedPatternSegment { path, segment }
            }
            RouteRegistryError::DuplicateRoute { path } => RouteDispatchError::DuplicateRoute {
                method: RouteMethod::parse("GET").as_str().to_string(),
                path,
            },
        })?;
        self.register_versioned(
            method,
            pattern,
            normalize_version_token(&version.into()),
            value,
        )
    }

    pub fn dispatch(&self, method: impl AsRef<str>, path: &str) -> RouteDispatchOutcome<'_, T> {
        let method = method.as_ref().trim().to_ascii_uppercase();
        let normalized_path = normalize_path(path.to_string());
        let mut allowed_methods = Vec::new();

        for entry in &self.routes {
            if entry.version.is_some() {
                continue;
            }

            if !entry.pattern.matches(&normalized_path) {
                continue;
            }

            if entry.method.matches(&method) {
                return RouteDispatchOutcome::Matched(entry);
            }

            allowed_methods.push(entry.method.as_str().to_string());
        }

        if allowed_methods.is_empty() {
            RouteDispatchOutcome::NotFound
        } else {
            allowed_methods.sort();
            allowed_methods.dedup();
            RouteDispatchOutcome::MethodNotAllowed {
                path: normalized_path,
                allowed_methods,
            }
        }
    }

    pub fn dispatch_versioned(
        &self,
        method: impl AsRef<str>,
        path: &str,
        version: Option<&str>,
    ) -> RouteDispatchOutcome<'_, T> {
        let method = method.as_ref().trim().to_ascii_uppercase();
        let normalized_path = normalize_path(path.to_string());
        let version = version.and_then(normalize_version_token);
        if let Some(version) = version.as_deref() {
            let mut allowed_methods = Vec::new();
            let mut saw_exact_version = false;

            for entry in &self.routes {
                if entry.version.as_deref() != Some(version) || !entry.pattern.matches(&normalized_path) {
                    continue;
                }

                saw_exact_version = true;
                if entry.method.matches(&method) {
                    return RouteDispatchOutcome::Matched(entry);
                }

                allowed_methods.push(entry.method.as_str().to_string());
            }

            if saw_exact_version {
                allowed_methods.sort();
                allowed_methods.dedup();
                return RouteDispatchOutcome::MethodNotAllowed {
                    path: normalized_path,
                    allowed_methods,
                };
            }
        }

        let mut allowed_methods = Vec::new();
        let mut saw_unversioned = false;

        for entry in &self.routes {
            if entry.version.is_some() || !entry.pattern.matches(&normalized_path) {
                continue;
            }

            saw_unversioned = true;
            if entry.method.matches(&method) {
                return RouteDispatchOutcome::Matched(entry);
            }

            allowed_methods.push(entry.method.as_str().to_string());
        }

        if saw_unversioned {
            allowed_methods.sort();
            allowed_methods.dedup();
            return RouteDispatchOutcome::MethodNotAllowed {
                path: normalized_path,
                allowed_methods,
            };
        }

        RouteDispatchOutcome::NotFound
    }

    pub fn dispatch_header_versioned(
        &self,
        method: impl AsRef<str>,
        path: &str,
        header_value: Option<&str>,
    ) -> RouteDispatchOutcome<'_, T> {
        let version = header_value.and_then(parse_api_version_header);
        self.dispatch_versioned(method, path, version.as_deref())
    }

    pub fn dispatch_media_type_versioned(
        &self,
        method: impl AsRef<str>,
        path: &str,
        accept_value: Option<&str>,
    ) -> RouteDispatchOutcome<'_, T> {
        let version = accept_value.and_then(parse_api_version_accept);
        self.dispatch_versioned(method, path, version.as_deref())
    }

    pub fn resolve(&self, method: impl AsRef<str>, path: &str) -> Option<&T> {
        match self.dispatch(method, path) {
            RouteDispatchOutcome::Matched(entry) => Some(&entry.value),
            _ => None,
        }
    }

    pub fn resolve_versioned(
        &self,
        method: impl AsRef<str>,
        path: &str,
        version: Option<&str>,
    ) -> Option<&T> {
        match self.dispatch_versioned(method, path, version) {
            RouteDispatchOutcome::Matched(entry) => Some(&entry.value),
            _ => None,
        }
    }

    pub fn resolve_header_versioned(
        &self,
        method: impl AsRef<str>,
        path: &str,
        header_value: Option<&str>,
    ) -> Option<&T> {
        let version = header_value.and_then(parse_api_version_header);
        self.resolve_versioned(method, path, version.as_deref())
    }

    pub fn resolve_media_type_versioned(
        &self,
        method: impl AsRef<str>,
        path: &str,
        accept_value: Option<&str>,
    ) -> Option<&T> {
        let version = accept_value.and_then(parse_api_version_accept);
        self.resolve_versioned(method, path, version.as_deref())
    }

    pub fn resolve_entry(
        &self,
        method: impl AsRef<str>,
        path: &str,
    ) -> Option<&RouteDispatchEntry<T>> {
        match self.dispatch(method, path) {
            RouteDispatchOutcome::Matched(entry) => Some(entry),
            _ => None,
        }
    }

    pub fn contains(&self, method: impl AsRef<str>, path: &str) -> bool {
        self.resolve(method, path).is_some()
    }

    /// Resolve a route and return its captured path values.
    pub fn resolve_match(
        &self,
        method: impl AsRef<str>,
        path: &str,
    ) -> Option<RouteDispatchMatch<'_, T>> {
        let method = method.as_ref().trim().to_ascii_uppercase();
        let normalized_path = normalize_path(path.to_string());

        for entry in &self.routes {
            let Some(captures) = entry.pattern.captures(&normalized_path) else {
                continue;
            };

            if entry.method.matches(&method) {
                return Some(RouteDispatchMatch { entry, captures });
            }
        }

        None
    }

    pub fn resolve_match_versioned(
        &self,
        method: impl AsRef<str>,
        path: &str,
        version: Option<&str>,
    ) -> Option<RouteDispatchMatch<'_, T>> {
        let method = method.as_ref().trim().to_ascii_uppercase();
        let normalized_path = normalize_path(path.to_string());
        let version = version.and_then(normalize_version_token);

        if let Some(version) = version.as_deref() {
            for entry in &self.routes {
                if entry.version.as_deref() != Some(version)
                    || !entry.pattern.matches(&normalized_path)
                {
                    continue;
                }

                let Some(captures) = entry.pattern.captures(&normalized_path) else {
                    continue;
                };

                if entry.method.matches(&method) {
                    return Some(RouteDispatchMatch { entry, captures });
                }
            }
        }

        for entry in &self.routes {
            if entry.version.is_some() || !entry.pattern.matches(&normalized_path) {
                continue;
            }

            let Some(captures) = entry.pattern.captures(&normalized_path) else {
                continue;
            };

            if entry.method.matches(&method) {
                return Some(RouteDispatchMatch { entry, captures });
            }
        }

        None
    }

    pub fn resolve_header_match(
        &self,
        method: impl AsRef<str>,
        path: &str,
        header_value: Option<&str>,
    ) -> Option<RouteDispatchMatch<'_, T>> {
        let version = header_value.and_then(parse_api_version_header);
        self.resolve_match_versioned(method, path, version.as_deref())
    }

    pub fn resolve_media_type_match(
        &self,
        method: impl AsRef<str>,
        path: &str,
        accept_value: Option<&str>,
    ) -> Option<RouteDispatchMatch<'_, T>> {
        let version = accept_value.and_then(parse_api_version_accept);
        self.resolve_match_versioned(method, path, version.as_deref())
    }
}

impl<T> RouteDispatchEntry<T> {
    /// Capture values from this dispatch entry if the path matches.
    pub fn captures(&self, path: &str) -> Option<RoutePathCaptures> {
        self.pattern.captures(path)
    }

    /// The route version tag, if one was registered.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}

impl RoutePathCaptures {
    /// Create an empty capture set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the capture set is empty.
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty() && self.wildcard.is_none()
    }

    /// The number of captured values.
    pub fn len(&self) -> usize {
        self.parameters.len() + usize::from(self.wildcard.is_some())
    }

    /// Look up a named capture.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.parameters
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
            .or_else(|| match self.wildcard.as_ref() {
                Some((Some(key), value)) if key == name => Some(value.as_str()),
                Some((None, value)) if name == "*" => Some(value.as_str()),
                _ => None,
            })
    }

    /// Return the wildcard value, if present.
    pub fn wildcard(&self) -> Option<&str> {
        self.wildcard.as_ref().map(|(_, value)| value.as_str())
    }

    /// Return the wildcard name, if one was provided.
    pub fn wildcard_name(&self) -> Option<&str> {
        self.wildcard.as_ref().and_then(|(name, _)| name.as_deref())
    }

    /// Iterate over all captured values.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        let params = self
            .parameters
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()));
        let wildcard = self.wildcard.iter().map(|(name, value)| {
            let key = name.as_deref().unwrap_or("*");
            (key, value.as_str())
        });

        params.chain(wildcard)
    }

    fn push_parameter(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.parameters.push((name.into(), value.into()));
    }

    fn set_wildcard(&mut self, name: Option<String>, value: impl Into<String>) {
        self.wildcard = Some((name, value.into()));
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
            if index != segments.len() - 1 {
                return Err(unsupported_segment(path, segment));
            }

            let base = segment.trim_end_matches('?');
            let Some(name) = base.strip_prefix(':') else {
                return Err(unsupported_segment(path, segment));
            };

            if name.is_empty() {
                return Err(unsupported_segment(path, segment));
            }

            parsed.push(RoutePatternSegment::OptionalParameter {
                name: name.to_string(),
            });
            continue;
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

fn capture_pattern(
    pattern: &[RoutePatternSegment],
    candidate: &[String],
) -> Option<RoutePathCaptures> {
    let mut captures = RoutePathCaptures::new();

    if matches_pattern_from(pattern, candidate, 0, 0, &mut captures) {
        Some(captures)
    } else {
        None
    }
}

fn matches_pattern_from(
    pattern: &[RoutePatternSegment],
    candidate: &[String],
    pattern_index: usize,
    candidate_index: usize,
    captures: &mut RoutePathCaptures,
) -> bool {
    if pattern_index == pattern.len() {
        return candidate_index == candidate.len();
    }

    let Some(segment) = pattern.get(pattern_index) else {
        return candidate_index == candidate.len();
    };

    match segment {
        RoutePatternSegment::Literal(expected) => {
            let Some(actual) = candidate.get(candidate_index) else {
                return false;
            };

            if actual != expected {
                return false;
            }

            matches_pattern_from(
                pattern,
                candidate,
                pattern_index + 1,
                candidate_index + 1,
                captures,
            )
        }
        RoutePatternSegment::Parameter { name } => {
            if candidate.get(candidate_index).is_none() {
                return false;
            }

            let Some(value) = candidate.get(candidate_index) else {
                return false;
            };

            captures.push_parameter(name.clone(), value.clone());

            matches_pattern_from(
                pattern,
                candidate,
                pattern_index + 1,
                candidate_index + 1,
                captures,
            )
        }
        RoutePatternSegment::OptionalParameter { name } => {
            let mut skipped = captures.clone();
            if matches_pattern_from(
                pattern,
                candidate,
                pattern_index + 1,
                candidate_index,
                &mut skipped,
            ) {
                *captures = skipped;
                return true;
            }

            if candidate.get(candidate_index).is_none() {
                return false;
            }

            let Some(value) = candidate.get(candidate_index) else {
                return false;
            };

            captures.push_parameter(name.clone(), value.clone());

            matches_pattern_from(
                pattern,
                candidate,
                pattern_index + 1,
                candidate_index + 1,
                captures,
            )
        }
        RoutePatternSegment::Wildcard { name } => {
            if pattern_index != pattern.len() - 1 {
                return false;
            }

            let remainder = candidate[candidate_index..].join("/");
            captures.set_wildcard(name.clone(), remainder);
            true
        }
    }
}

fn unsupported_segment(path: &str, segment: &str) -> RouteRegistryError {
    RouteRegistryError::UnsupportedPatternSegment {
        path: path.to_string(),
        segment: segment.to_string(),
    }
}

fn merge_route_paths(prefix: String, path: String) -> String {
    let prefix = prefix.trim();
    let path = path.trim();

    let normalized_prefix = prefix.trim_end_matches('/');
    let normalized_path = path.trim_start_matches('/');

    match (normalized_prefix.is_empty(), normalized_path.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{}", normalized_path),
        (false, true) => normalized_prefix.to_string(),
        (false, false) => format!("{}/{}", normalized_prefix, normalized_path),
    }
}

fn version_segment(version: &str) -> String {
    let trimmed = version.trim().trim_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed.starts_with('v') || trimmed.starts_with('V') {
        trimmed.to_string()
    } else {
        format!("v{}", trimmed)
    }
}

pub fn parse_api_version_header(value: &str) -> Option<String> {
    normalize_version_token(value)
}

pub fn parse_api_version_accept(value: &str) -> Option<String> {
    value.split(',').find_map(|candidate| {
        let candidate = candidate.trim();
        let media_type = candidate.split(';').next()?.trim();
        let (_, subtype) = media_type.split_once('/')?;
        let (_, version) = subtype.rsplit_once(".v")?;
        let version = version
            .split(|ch| matches!(ch, '+' | ';' | '-' | '/'))
            .next()?;

        normalize_version_token(version)
    })
}

fn normalize_version_token(version: &str) -> Option<String> {
    let trimmed = version.trim().trim_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    let stripped = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);

    if stripped.is_empty() {
        None
    } else {
        Some(format!("v{}", stripped))
    }
}

fn segment_label(segment: &RoutePatternSegment) -> String {
    match segment {
        RoutePatternSegment::Literal(value) => value.clone(),
        RoutePatternSegment::Parameter { name } => format!(":{}", name),
        RoutePatternSegment::OptionalParameter { name } => format!(":{}?", name),
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
    OptionalParameter,
    Wildcard,
}

impl RouteSegmentKind {
    fn rank(self) -> u8 {
        match self {
            RouteSegmentKind::Literal => 3,
            RouteSegmentKind::Parameter => 2,
            RouteSegmentKind::OptionalParameter => 1,
            RouteSegmentKind::Wildcard => 0,
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
        let err = RoutePattern::static_path("/users/:id?").unwrap_err();

        assert_eq!(
            err,
            RouteRegistryError::UnsupportedPatternSegment {
                path: "/users/:id?".to_string(),
                segment: ":id?".to_string()
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

        let captures = pattern.captures("/users/42").unwrap();
        assert_eq!(captures.len(), 1);
        assert_eq!(captures.get("id"), Some("42"));
        assert!(captures.wildcard().is_none());
    }

    #[test]
    fn route_pattern_parses_optional_parameters() {
        let pattern = RoutePattern::parse("/users/:id?").unwrap();

        assert_eq!(pattern.path(), "/users/:id?");
        assert!(pattern.matches("/users"));
        assert!(pattern.matches("/users/42"));
        assert!(!pattern.matches("/users/42/posts"));
        assert!(!pattern.matches("/other"));

        let missing = pattern.captures("/users").unwrap();
        assert!(missing.is_empty());

        let present = pattern.captures("/users/42").unwrap();
        assert_eq!(present.len(), 1);
        assert_eq!(present.get("id"), Some("42"));
    }

    #[test]
    fn route_pattern_parses_wildcards() {
        let pattern = RoutePattern::parse("/files/*path").unwrap();

        assert_eq!(pattern.path(), "/files/*path");
        assert!(pattern.matches("/files"));
        assert!(pattern.matches("/files/a/b/c"));
        assert!(!pattern.matches("/other"));

        let empty = pattern.captures("/files").unwrap();
        assert_eq!(empty.len(), 1);
        assert_eq!(empty.get("path"), Some(""));
        assert_eq!(empty.wildcard(), Some(""));

        let nested = pattern.captures("/files/a/b/c").unwrap();
        assert_eq!(nested.get("path"), Some("a/b/c"));
        assert_eq!(nested.wildcard_name(), Some("path"));
    }

    #[test]
    fn route_registry_prefers_more_specific_patterns() {
        let mut registry = RouteRegistry::new();

        registry
            .register_pattern("/files/*path", "wildcard")
            .unwrap();
        registry.register_pattern("/files/:name?", "optional").unwrap();
        registry
            .register_pattern("/files/:name", "parameter")
            .unwrap();
        registry
            .register_static("/files/archive", "static")
            .unwrap();

        assert_eq!(registry.resolve("/files/archive"), Some(&"static"));
        assert_eq!(registry.resolve("/files/readme"), Some(&"parameter"));
        assert_eq!(registry.resolve("/files"), Some(&"optional"));
        assert_eq!(registry.resolve("/files/docs/guide"), Some(&"wildcard"));
    }

    #[test]
    fn route_dispatch_registry_resolves_methods_and_paths() {
        let mut registry = RouteDispatchRegistry::new();

        registry
            .register_static("GET", "/users", "list")
            .unwrap();

        match registry.dispatch("GET", "/users") {
            RouteDispatchOutcome::Matched(entry) => {
                assert_eq!(entry.method, RouteMethod::Get);
                assert_eq!(entry.pattern, RoutePattern::Static(vec!["users".to_string()]));
                assert_eq!(entry.value, "list");
            }
            other => panic!("unexpected dispatch result: {other:?}"),
        }

        assert_eq!(registry.resolve("GET", "/users"), Some(&"list"));
    }

    #[test]
    fn route_dispatch_registry_resolve_match_exposes_captures() {
        let metadata = ControllerMetadata::new("/users").with_version("1");
        let mut registry = RouteDispatchRegistry::new();

        registry
            .register_versioned_controller_route("GET", &metadata, "/:id", "show")
            .unwrap();

        let matched = registry.resolve_match("GET", "/v1/users/42").unwrap();

        assert_eq!(matched.entry.value, "show");
        assert_eq!(matched.entry.method, RouteMethod::Get);
        assert_eq!(matched.captures.get("id"), Some("42"));
        assert_eq!(matched.captures.len(), 1);
    }

    #[test]
    fn route_dispatch_registry_distinguishes_not_found_and_method_not_allowed() {
        let mut registry = RouteDispatchRegistry::new();

        registry
            .register_static("GET", "/users", "list")
            .unwrap();
        registry
            .register_pattern("POST", "/users/:id", "create")
            .unwrap();

        assert_eq!(
            registry.dispatch("GET", "/missing"),
            RouteDispatchOutcome::NotFound
        );

        assert_eq!(
            registry.dispatch("POST", "/users"),
            RouteDispatchOutcome::MethodNotAllowed {
                path: "/users".to_string(),
                allowed_methods: vec!["GET".to_string()],
            }
        );
    }

    #[test]
    fn route_dispatch_registry_prefers_more_specific_patterns() {
        let mut registry = RouteDispatchRegistry::new();

        registry
            .register_pattern("ALL", "/files/*path", "wildcard")
            .unwrap();
        registry
            .register_pattern("GET", "/files/:name?", "optional")
            .unwrap();
        registry
            .register_pattern("GET", "/files/:name", "parameter")
            .unwrap();
        registry
            .register_static("GET", "/files/archive", "static")
            .unwrap();

        assert_eq!(registry.resolve("GET", "/files/archive"), Some(&"static"));
        assert_eq!(registry.resolve("GET", "/files/readme"), Some(&"parameter"));
        assert_eq!(registry.resolve("GET", "/files"), Some(&"optional"));
        assert_eq!(registry.resolve("DELETE", "/files/docs/guide"), Some(&"wildcard"));
    }

    #[test]
    fn route_dispatch_registry_merges_controller_prefixes() {
        let mut registry = RouteDispatchRegistry::new();

        registry
            .register_controller_route("GET", "/users/", "list", "list")
            .unwrap();
        registry
            .register_controller_route("POST", "/users", "/create", "create")
            .unwrap();

        assert_eq!(registry.resolve("GET", "/users/list"), Some(&"list"));
        assert_eq!(registry.resolve("POST", "/users/create"), Some(&"create"));
    }

    #[test]
    fn metadata_versioned_path_normalizes_numeric_versions() {
        let metadata = ControllerMetadata::new("/users").with_version("1");

        assert_eq!(metadata.versioned_path(), "/v1/users");
    }

    #[test]
    fn route_dispatch_registry_registers_versioned_controller_routes() {
        let metadata = ControllerMetadata::new("/users").with_version("1");
        let mut registry = RouteDispatchRegistry::new();

        registry
            .register_versioned_controller_route("GET", &metadata, "/list", "v1-list")
            .unwrap();
        registry
            .register_versioned_controller_route("GET", &ControllerMetadata::new("/users").with_version("2"), "/list", "v2-list")
            .unwrap();

        assert_eq!(registry.resolve("GET", "/v1/users/list"), Some(&"v1-list"));
        assert_eq!(registry.resolve("GET", "/v2/users/list"), Some(&"v2-list"));
        assert_eq!(registry.resolve("GET", "/users/list"), None);
    }

    #[test]
    fn route_dispatch_registry_rejects_duplicate_versioned_routes() {
        let metadata = ControllerMetadata::new("/users").with_version("1");
        let mut registry = RouteDispatchRegistry::new();

        registry
            .register_versioned_controller_route("GET", &metadata, "/list", "first")
            .unwrap();

        let err = registry
            .register_versioned_controller_route("GET", &metadata, "list/", "second")
            .unwrap_err();

        assert_eq!(
            err,
            RouteDispatchError::DuplicateRoute {
                method: "GET".to_string(),
                path: "/v1/users/list".to_string()
            }
        );
    }

    #[test]
    fn api_version_parsers_handle_headers_and_accept_values() {
        assert_eq!(parse_api_version_header("1"), Some("v1".to_string()));
        assert_eq!(parse_api_version_header(" v2 "), Some("v2".to_string()));
        assert_eq!(
            parse_api_version_accept("application/vnd.app.v1+json"),
            Some("v1".to_string())
        );
        assert_eq!(
            parse_api_version_accept("application/vnd.app.v2+json; charset=utf-8"),
            Some("v2".to_string())
        );
        assert_eq!(parse_api_version_accept("application/json"), None);
    }

    #[test]
    fn route_dispatch_registry_supports_header_and_media_type_versioned_routes() {
        let mut registry = RouteDispatchRegistry::new();

        registry
            .register_header_versioned_route("GET", "1", "/users", "header-v1")
            .unwrap();
        registry
            .register_media_type_versioned_route("GET", "2", "/users", "media-v2")
            .unwrap();
        registry.register_static("GET", "/users", "default").unwrap();

        assert_eq!(
            registry.resolve_header_versioned("GET", "/users", Some("1")),
            Some(&"header-v1")
        );
        assert_eq!(
            registry.resolve_media_type_versioned(
                "GET",
                "/users",
                Some("application/vnd.app.v2+json")
            ),
            Some(&"media-v2")
        );
        assert_eq!(
            registry.resolve_header_versioned("GET", "/users", Some("3")),
            Some(&"default")
        );
    }

    #[test]
    fn route_dispatch_registry_prefers_exact_version_over_unversioned_fallback() {
        let mut registry = RouteDispatchRegistry::new();

        registry
            .register_header_versioned_route("POST", "1", "/users", "versioned")
            .unwrap();
        registry.register_static("GET", "/users", "default").unwrap();

        assert_eq!(
            registry.dispatch_header_versioned("GET", "/users", Some("1")),
            RouteDispatchOutcome::MethodNotAllowed {
                path: "/users".to_string(),
                allowed_methods: vec!["POST".to_string()],
            }
        );
    }

    #[test]
    fn route_dispatch_registry_rejects_duplicate_method_routes() {
        let mut registry = RouteDispatchRegistry::new();

        registry
            .register_static("GET", "/users", "first")
            .unwrap();

        let err = registry.register_static("GET", "/users/", "second").unwrap_err();
        assert_eq!(
            err,
            RouteDispatchError::DuplicateRoute {
                method: "GET".to_string(),
                path: "/users".to_string()
            }
        );
    }
}
