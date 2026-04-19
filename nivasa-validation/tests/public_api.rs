use nivasa_validation::{
    is_not_empty, is_url, matches_regex, Validate, ValidationContext, ValidationError,
    ValidationErrors, ValidationGroup,
};

#[test]
fn validation_context_deduplicates_groups_and_serializes_cleanly() {
    let mut context = ValidationContext::new()
        .with_groups(vec!["update", "create", "update"])
        .with_group("delete");

    assert!(context.has_group("create"));
    assert!(context.has_group("update"));
    assert!(context.has_group("delete"));
    assert!(!context.has_group("archive"));

    assert!(context.insert_group("archive"));
    assert!(!context.insert_group("archive"));

    let active_groups: Vec<_> = context
        .active_groups()
        .map(ValidationGroup::as_str)
        .collect();
    assert_eq!(active_groups, vec!["archive", "create", "delete", "update"]);

    let json = serde_json::to_value(&context).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "active_groups": ["archive", "create", "delete", "update"]
        })
    );

    let active_groups = context.into_active_groups();
    assert_eq!(active_groups.len(), 4);
    assert!(active_groups.contains("create"));
    assert!(active_groups.contains("archive"));
}

#[test]
fn validation_errors_cover_empty_single_and_multi_error_shapes() {
    let empty = ValidationErrors::new();
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert_eq!(empty.errors(), &[]);
    assert_eq!(empty.to_string(), "validation failed with no errors");

    let single = ValidationErrors::from_error(
        ValidationError::new("email").with_constraint("is_email", "must contain an @ symbol"),
    );
    assert!(!single.is_empty());
    assert_eq!(single.len(), 1);
    assert_eq!(single.errors()[0].field, "email");
    assert_eq!(single.to_string(), "validation failed for `email`");

    let mut multi = ValidationErrors::new();
    multi.push(ValidationError::new("username"));
    multi.extend(vec![ValidationError::new("password")]);

    assert_eq!(multi.len(), 2);
    assert_eq!(multi.to_string(), "validation failed with 2 errors");

    let multi_json = serde_json::to_value(&multi).unwrap();
    assert_eq!(
        multi_json,
        serde_json::json!({
            "errors": [
                { "field": "username", "constraints": {} },
                { "field": "password", "constraints": {} }
            ]
        })
    );

    let owned_errors = multi.into_errors();
    assert_eq!(owned_errors.len(), 2);
    assert_eq!(owned_errors[0].field, "username");
    assert_eq!(owned_errors[1].field, "password");
}

#[test]
fn validation_helpers_cover_predicates_and_group_serialization() {
    assert!(is_url("https://example.com/docs"));
    assert!(!is_url("relative/path"));
    assert!(!is_url("https://"));

    assert!(matches_regex(
        "alice@example.com",
        r"^[^@\s]+@[^@\s]+\.[^@\s]+$"
    ));
    assert!(!matches_regex("alice@example.com", r"^[0-9]+$"));
    assert!(!matches_regex("alice@example.com", r"(["));

    assert!(is_not_empty("hello"));
    assert!(is_not_empty(&String::from("hello")));
    assert!(is_not_empty(&["a", "b"][..]));
    assert!(!is_not_empty(""));

    let group = ValidationGroup::from("create");
    assert_eq!(group.as_str(), "create");
    assert_eq!(
        serde_json::to_value(&group).unwrap(),
        serde_json::json!("create")
    );
}

#[test]
fn validation_types_round_trip_through_serde() {
    let context = ValidationContext::new()
        .with_group("create")
        .with_group("update");
    let context_json = serde_json::to_string(&context).unwrap();
    let context_round_trip: ValidationContext = serde_json::from_str(&context_json).unwrap();
    assert!(context_round_trip.has_group("create"));
    assert!(context_round_trip.has_group("update"));

    let error = ValidationError::new("email")
        .with_constraint("is_email", "must contain an @ symbol")
        .with_constraint("min_length", "must be at least 3 characters");
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        serde_json::json!({
            "field": "email",
            "constraints": {
                "is_email": "must contain an @ symbol",
                "min_length": "must be at least 3 characters"
            }
        })
    );
    assert_eq!(error.field, "email");
    assert_eq!(
        error.constraints.get("is_email"),
        Some(&"must contain an @ symbol".to_string())
    );
    let overwritten = ValidationError::new("email")
        .with_constraint("is_email", "must contain an @ symbol")
        .with_constraint("is_email", "overwritten");
    assert_eq!(
        overwritten.constraints.get("is_email"),
        Some(&"overwritten".to_string())
    );

    let mut errors = ValidationErrors::new();
    errors.push(error.clone());
    assert_eq!(
        serde_json::to_value(&errors).unwrap(),
        serde_json::json!({
            "errors": [
                {
                    "field": "email",
                    "constraints": {
                        "is_email": "must contain an @ symbol",
                        "min_length": "must be at least 3 characters"
                    }
                }
            ]
        })
    );
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0], error);
}

#[test]
fn validation_trait_default_validate_with_delegates_to_validate() {
    #[derive(Debug)]
    struct DefaultOnlyForm {
        name: String,
    }

    impl Validate for DefaultOnlyForm {
        fn validate(&self) -> Result<(), ValidationErrors> {
            if self.name.trim().is_empty() {
                Err(ValidationErrors::from_error(
                    ValidationError::new("name").with_constraint("required", "must not be empty"),
                ))
            } else {
                Ok(())
            }
        }
    }

    let form = DefaultOnlyForm {
        name: "alice".into(),
    };

    assert!(form.validate_with(&ValidationContext::new()).is_ok());

    let invalid = DefaultOnlyForm {
        name: String::new(),
    };

    let errors = invalid
        .validate_with(&ValidationContext::new().with_group("create"))
        .unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "name");
}
