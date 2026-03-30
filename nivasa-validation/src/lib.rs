//! # nivasa-validation
//!
//! Nivasa framework — validation.
//!
//! This crate provides the validation core used by later DTO and pipe
//! integrations. It stays crate-local for now: no macros, no HTTP wiring.

use serde::Serialize;
use serde::Deserialize;
use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet},
    error::Error as StdError,
    fmt,
};

/// Trait for types that can validate their own invariants.
pub trait Validate {
    /// Validate the current value.
    fn validate(&self) -> Result<(), ValidationErrors>;

    /// Validate the current value with validation context.
    ///
    /// The context currently carries active validation groups such as
    /// `create` or `update`. Implementations can override this to branch on
    /// group-specific rules while keeping `validate()` as the no-context
    /// convenience entry point.
    fn validate_with(&self, context: &ValidationContext) -> Result<(), ValidationErrors> {
        let _ = context;
        self.validate()
    }
}

/// Return whether the supplied string looks like a valid absolute URL.
///
/// The helper intentionally stays small and reusable so later macro wiring can
/// attach the appropriate field-level validation error without changing the
/// core contract.
pub fn is_url(value: &str) -> bool {
    value
        .parse::<http::Uri>()
        .map(|uri| uri.scheme_str().is_some() && uri.authority().is_some())
        .unwrap_or(false)
}

/// Named validation group active for a validation pass.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ValidationGroup(String);

impl ValidationGroup {
    /// Create a new validation group name.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Borrow the group name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ValidationGroup {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ValidationGroup {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for ValidationGroup {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for ValidationGroup {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

/// Context passed into group-aware validation.
///
/// This core-only slice currently tracks the set of active validation groups.
/// Later macro and HTTP integrations can thread this context through derived
/// validators without changing the core contract.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ValidationContext {
    active_groups: BTreeSet<ValidationGroup>,
}

impl ValidationContext {
    /// Create an empty validation context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a validation context with a single active group.
    pub fn with_group(mut self, group: impl Into<ValidationGroup>) -> Self {
        self.insert_group(group);
        self
    }

    /// Create a validation context from a collection of active groups.
    pub fn with_groups<I, G>(mut self, groups: I) -> Self
    where
        I: IntoIterator<Item = G>,
        G: Into<ValidationGroup>,
    {
        self.extend_groups(groups);
        self
    }

    /// Add an active validation group to the context.
    pub fn insert_group(&mut self, group: impl Into<ValidationGroup>) -> bool {
        self.active_groups.insert(group.into())
    }

    /// Add many active validation groups to the context.
    pub fn extend_groups<I, G>(&mut self, groups: I)
    where
        I: IntoIterator<Item = G>,
        G: Into<ValidationGroup>,
    {
        self.active_groups
            .extend(groups.into_iter().map(Into::into));
    }

    /// Return whether a validation group is active.
    pub fn has_group(&self, group: impl AsRef<str>) -> bool {
        self.active_groups.contains(group.as_ref())
    }

    /// Iterate over the active validation groups.
    pub fn active_groups(&self) -> impl Iterator<Item = &ValidationGroup> {
        self.active_groups.iter()
    }

    /// Consume the context and return the active validation groups.
    pub fn into_active_groups(self) -> BTreeSet<ValidationGroup> {
        self.active_groups
    }
}

/// A single field-level validation error with structured constraint messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationError {
    /// The field or property name that failed validation.
    pub field: String,
    /// Constraint messages keyed by rule name.
    pub constraints: BTreeMap<String, String>,
}

impl ValidationError {
    /// Create a new validation error for the given field.
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            constraints: BTreeMap::new(),
        }
    }

    /// Attach a rule-specific message.
    pub fn with_constraint(mut self, rule: impl Into<String>, message: impl Into<String>) -> Self {
        self.constraints.insert(rule.into(), message.into());
        self
    }
}

/// Aggregate of one or more validation errors.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ValidationErrors {
    errors: Vec<ValidationError>,
}

impl ValidationErrors {
    /// Create an empty validation error collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a collection from a single error.
    pub fn from_error(error: ValidationError) -> Self {
        Self {
            errors: vec![error],
        }
    }

    /// Add an error to the collection.
    pub fn push(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// Extend the collection with more errors.
    pub fn extend<I>(&mut self, errors: I)
    where
        I: IntoIterator<Item = ValidationError>,
    {
        self.errors.extend(errors);
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Number of collected validation errors.
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// Borrow the collected errors.
    pub fn errors(&self) -> &[ValidationError] {
        &self.errors
    }

    /// Consume the collection and return the inner errors.
    pub fn into_errors(self) -> Vec<ValidationError> {
        self.errors
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.errors.as_slice() {
            [] => write!(f, "validation failed with no errors"),
            [first] => write!(f, "validation failed for `{}`", first.field),
            errors => write!(f, "validation failed with {} errors", errors.len()),
        }
    }
}

impl StdError for ValidationErrors {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct SignupForm {
        username: String,
        email: String,
    }

    impl Validate for SignupForm {
        fn validate(&self) -> Result<(), ValidationErrors> {
            let mut errors = ValidationErrors::new();

            if self.username.trim().len() < 3 {
                errors.push(
                    ValidationError::new("username")
                        .with_constraint("min_length", "must be at least 3 characters"),
                );
            }

            if !self.email.contains('@') {
                errors.push(
                    ValidationError::new("email")
                        .with_constraint("is_email", "must contain an @ symbol"),
                );
            }

            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors)
            }
        }
    }

    #[test]
    fn validate_trait_accepts_valid_input() {
        let form = SignupForm {
            username: "alice".into(),
            email: "alice@example.com".into(),
        };

        assert!(form.validate().is_ok());
    }

    #[test]
    fn validate_trait_collects_multiple_field_errors() {
        let form = SignupForm {
            username: "al".into(),
            email: "invalid-email".into(),
        };

        let errors = form.validate().unwrap_err();
        assert_eq!(errors.len(), 2);
        assert_eq!(errors.errors()[0].field, "username");
        assert_eq!(
            errors.errors()[0].constraints.get("min_length"),
            Some(&"must be at least 3 characters".to_string())
        );
        assert_eq!(errors.errors()[1].field, "email");
        assert_eq!(
            errors.errors()[1].constraints.get("is_email"),
            Some(&"must contain an @ symbol".to_string())
        );
    }

    #[test]
    fn validation_error_serializes_as_structured_json() {
        let error = ValidationError::new("email")
            .with_constraint("is_email", "must contain an @ symbol")
            .with_constraint("min_length", "must be at least 3 characters");

        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "field": "email",
                "constraints": {
                    "is_email": "must contain an @ symbol",
                    "min_length": "must be at least 3 characters"
                }
            })
        );
    }

    #[test]
    fn validation_errors_serializes_as_collection_of_structured_errors() {
        let mut errors = ValidationErrors::new();
        errors.push(
            ValidationError::new("username")
                .with_constraint("min_length", "must be at least 3 characters"),
        );
        errors.push(
            ValidationError::new("email")
                .with_constraint("is_email", "must contain an @ symbol"),
        );

        let json = serde_json::to_value(&errors).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "errors": [
                    {
                        "field": "username",
                        "constraints": {
                            "min_length": "must be at least 3 characters"
                        }
                    },
                    {
                        "field": "email",
                        "constraints": {
                            "is_email": "must contain an @ symbol"
                        }
                    }
                ]
            })
        );
    }

    #[test]
    fn validation_context_tracks_active_groups() {
        let context = ValidationContext::new()
            .with_group("create")
            .with_group("update")
            .with_group("create");

        let active_groups: Vec<_> = context
            .active_groups()
            .map(ValidationGroup::as_str)
            .collect();

        assert_eq!(active_groups, vec!["create", "update"]);
        assert!(context.has_group("create"));
        assert!(context.has_group("update"));
        assert!(!context.has_group("delete"));
    }

    #[test]
    fn validation_context_serializes_active_groups() {
        let context = ValidationContext::new()
            .with_group("create")
            .with_group("update");

        let json = serde_json::to_value(&context).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "active_groups": ["create", "update"]
            })
        );
    }

    #[test]
    fn is_url_accepts_absolute_urls_with_scheme_and_authority() {
        assert!(is_url("https://example.com/path?query=1"));
        assert!(is_url("http://localhost:8080"));
    }

    #[test]
    fn is_url_rejects_relative_or_incomplete_values() {
        assert!(!is_url("/relative/path"));
        assert!(!is_url("https://"));
        assert!(!is_url("not a url"));
    }

    #[test]
    fn validation_trait_can_consume_group_context() {
        #[derive(Debug)]
        struct GroupAwareForm {
            username: String,
            email: Option<String>,
        }

        impl GroupAwareForm {
            fn validate_inner(
                &self,
                context: &ValidationContext,
            ) -> Result<(), ValidationErrors> {
                let mut errors = ValidationErrors::new();

                if self.username.trim().len() < 3 {
                    errors.push(
                        ValidationError::new("username")
                            .with_constraint("min_length", "must be at least 3 characters"),
                    );
                }

                if context.has_group("create")
                    && self.email.as_deref().unwrap_or("").trim().is_empty()
                {
                    errors.push(
                        ValidationError::new("email")
                            .with_constraint("required", "must be provided for create"),
                    );
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
        }

        impl Validate for GroupAwareForm {
            fn validate(&self) -> Result<(), ValidationErrors> {
                self.validate_inner(&ValidationContext::new())
            }

            fn validate_with(&self, context: &ValidationContext) -> Result<(), ValidationErrors> {
                self.validate_inner(context)
            }
        }

        let form = GroupAwareForm {
            username: "alice".into(),
            email: None,
        };

        assert!(form.validate().is_ok());

        let create_errors = form
            .validate_with(&ValidationContext::new().with_group("create"))
            .unwrap_err();
        assert_eq!(create_errors.len(), 1);
        assert_eq!(create_errors.errors()[0].field, "email");
        assert_eq!(
            create_errors.errors()[0].constraints.get("required"),
            Some(&"must be provided for create".to_string())
        );
    }
}
