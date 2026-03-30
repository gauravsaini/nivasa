//! # nivasa-validation
//!
//! Nivasa framework — validation.
//!
//! This crate provides the validation core used by later DTO and pipe
//! integrations. It stays crate-local for now: no macros, no HTTP wiring.

use serde::Serialize;
use std::{collections::BTreeMap, error::Error as StdError, fmt};

/// Trait for types that can validate their own invariants.
pub trait Validate {
    /// Validate the current value.
    fn validate(&self) -> Result<(), ValidationErrors>;
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
}
