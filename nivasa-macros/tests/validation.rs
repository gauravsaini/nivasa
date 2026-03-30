use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct SignupForm {
    #[is_email]
    email: String,
    #[min_length(6)]
    password: String,
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
