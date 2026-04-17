use nivasa_validation::{
    is_not_empty, is_url, matches_regex, ValidationContext, ValidationError, ValidationErrors,
    ValidationGroup,
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
