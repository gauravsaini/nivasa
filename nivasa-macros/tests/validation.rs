use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct SignupForm {
    #[is_email]
    email: String,
    #[min_length(6)]
    password: String,
}

#[derive(Dto)]
struct ProfileForm {
    #[is_string]
    display_name: String,
}

#[derive(Dto)]
struct ContactDetails {
    #[is_email]
    email: String,
    #[min_length(6)]
    password: String,
}

#[derive(Dto)]
struct AccountForm {
    #[validate_nested]
    contact: ContactDetails,
}

#[derive(Dto)]
struct FeatureFlags {
    #[is_boolean]
    enabled: bool,
}

#[derive(Dto)]
struct UsageStats {
    #[is_number]
    retry_count: u32,
    #[is_number]
    average_latency_ms: f64,
}

#[derive(Dto)]
struct BioForm {
    #[max_length(12)]
    bio: String,
}

#[test]
fn dto_validation_accepts_valid_input() {
    let form = SignupForm {
        email: "alice@example.com".into(),
        password: "secret1".into(),
    };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_collects_multiple_field_errors() {
    let form = SignupForm {
        email: "invalid-email".into(),
        password: "123".into(),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 2);
    assert_eq!(errors.errors()[0].field, "email");
    assert_eq!(
        errors.errors()[0].constraints.get("is_email"),
        Some(&"must be a valid email".to_string())
    );
    assert_eq!(errors.errors()[1].field, "password");
    assert_eq!(
        errors.errors()[1].constraints.get("min_length"),
        Some(&"must be at least 6 characters".to_string())
    );
}

#[test]
fn dto_validation_accepts_string_fields() {
    let form = ProfileForm {
        display_name: "Alice".into(),
    };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_collects_nested_field_errors_with_prefixes() {
    let form = AccountForm {
        contact: ContactDetails {
            email: "not-an-email".into(),
            password: "123".into(),
        },
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 2);
    assert_eq!(errors.errors()[0].field, "contact.email");
    assert_eq!(
        errors.errors()[0].constraints.get("is_email"),
        Some(&"must be a valid email".to_string())
    );
    assert_eq!(errors.errors()[1].field, "contact.password");
    assert_eq!(
        errors.errors()[1].constraints.get("min_length"),
        Some(&"must be at least 6 characters".to_string())
    );
}

#[test]
fn dto_validation_accepts_nested_valid_input() {
    let form = AccountForm {
        contact: ContactDetails {
            email: "alice@example.com".into(),
            password: "secret1".into(),
        },
    };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_accepts_boolean_fields() {
    let form = FeatureFlags { enabled: true };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_accepts_numeric_fields() {
    let form = UsageStats {
        retry_count: 3,
        average_latency_ms: 12.5,
    };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_applies_max_length_rules() {
    let form = BioForm {
        bio: "short bio".into(),
    };

    assert!(form.validate().is_ok());

    let form = BioForm {
        bio: "this bio is too long".into(),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "bio");
    assert_eq!(
        errors.errors()[0].constraints.get("max_length"),
        Some(&"must be at most 12 characters".to_string())
    );
}
