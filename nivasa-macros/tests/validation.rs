use nivasa_macros::{Dto, PartialDto};
use nivasa_pipes::ParseEnumTarget;
use nivasa_validation::{Validate, ValidationContext};

#[derive(Dto)]
struct SignupForm {
    #[is_email]
    email: String,
    #[min_length(6)]
    password: String,
}

#[derive(Dto)]
#[allow(dead_code)]
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
struct ContactListForm {
    #[validate_nested]
    contacts: Vec<ContactDetails>,
}

#[derive(Dto)]
#[allow(dead_code)]
struct FeatureFlags {
    #[is_boolean]
    enabled: bool,
}

#[derive(Dto)]
#[allow(dead_code)]
struct UsageStats {
    #[is_number]
    retry_count: u32,
    #[is_number]
    average_latency_ms: f64,
}

#[derive(Dto)]
#[allow(dead_code)]
struct IntMetrics {
    #[is_int]
    retry_count: i32,
}

#[derive(Dto)]
struct NumericBoundsForm {
    #[min(2)]
    #[max(5)]
    retry_count: u32,
    #[min(-10.5)]
    #[max(20.25)]
    average_latency_ms: f64,
}

#[derive(Dto)]
struct OptionalNumericBoundsForm {
    #[is_optional]
    #[min(2)]
    #[max(5)]
    retry_count: Option<u32>,
}

#[derive(Dto)]
struct BioForm {
    #[max_length(12)]
    bio: String,
}

#[derive(Dto)]
struct SessionForm {
    #[is_uuid]
    session_id: String,
}

#[derive(Dto)]
struct WebhookForm {
    #[is_url]
    callback_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessLevel {
    Admin,
    Reader,
}

impl ParseEnumTarget for AccessLevel {
    fn parse(input: &str) -> Result<Self, String> {
        match input {
            "Admin" => Ok(Self::Admin),
            "Reader" => Ok(Self::Reader),
            other => Err(format!("unknown access level `{other}`")),
        }
    }

    fn into_value(value: Self) -> serde_json::Value {
        match value {
            Self::Admin => serde_json::Value::from("Admin"),
            Self::Reader => serde_json::Value::from("Reader"),
        }
    }
}

#[derive(Dto)]
struct EnumForm {
    #[is_enum(AccessLevel)]
    access_level: String,
}

#[derive(Dto)]
struct NonEmptyForm {
    #[is_not_empty]
    title: String,
    #[is_not_empty]
    tags: Vec<String>,
}

#[derive(Dto)]
struct ArraySizeForm {
    #[array_min_size(2)]
    #[array_max_size(3)]
    tags: Vec<String>,
}

#[derive(Dto)]
struct OptionalArraySizeForm {
    #[is_optional]
    #[array_min_size(2)]
    #[array_max_size(3)]
    tags: Option<Vec<String>>,
}

#[derive(Dto)]
struct OptionalNonEmptyForm {
    #[is_optional]
    #[is_not_empty]
    title: Option<String>,
}

#[derive(Dto)]
struct SlugForm {
    #[matches("^[a-z0-9-]+$")]
    slug: String,
}

#[derive(Dto)]
struct OptionalContactForm {
    #[is_optional]
    #[is_email]
    email: Option<String>,
}

#[derive(Dto)]
struct ConditionalValidationForm {
    mode: String,
    #[validate_if(mode, "create")]
    #[is_email]
    email: String,
}

#[derive(Dto)]
struct OptionalConditionalValidationForm {
    mode: String,
    #[is_optional]
    #[validate_if(mode, "create")]
    #[is_email]
    email: Option<String>,
}

#[derive(Dto)]
struct GroupedChildForm {
    #[groups("create")]
    #[is_email]
    email: String,
}

#[derive(Dto)]
struct GroupedParentForm {
    #[groups("create")]
    #[is_email]
    create_email: String,
    #[groups("create", "update")]
    #[min_length(6)]
    password: String,
    #[is_email]
    always_email: String,
    #[validate_nested]
    child: GroupedChildForm,
}

#[allow(clippy::ptr_arg)]
fn uses_example_domain(value: &String) -> bool {
    value.ends_with("@example.com")
}

#[derive(Dto)]
struct CustomValidateForm {
    #[custom_validate(uses_example_domain)]
    email: String,
}

#[derive(Dto)]
struct OptionalCustomValidateForm {
    #[is_optional]
    #[custom_validate(uses_example_domain)]
    email: Option<String>,
}

#[derive(PartialDto)]
struct PartialContactForm {
    #[is_email]
    email: Option<String>,
    #[min_length(6)]
    password: Option<String>,
}

#[derive(PartialDto)]
struct PartialCustomValidateForm {
    #[custom_validate(uses_example_domain)]
    email: Option<String>,
}

#[derive(PartialDto)]
struct PartialConditionalValidationForm {
    mode: Option<String>,
    #[validate_if(mode, "create")]
    #[is_email]
    email: Option<String>,
}

#[derive(PartialDto)]
struct PartialAccountForm {
    #[validate_nested]
    contact: Option<ContactDetails>,
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
fn dto_validation_collects_vec_nested_field_errors_with_indices() {
    let form = ContactListForm {
        contacts: vec![
            ContactDetails {
                email: "not-an-email".into(),
                password: "123".into(),
            },
            ContactDetails {
                email: "bob@example.com".into(),
                password: "456".into(),
            },
        ],
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 3);
    assert_eq!(errors.errors()[0].field, "contacts[0].email");
    assert_eq!(errors.errors()[1].field, "contacts[0].password");
    assert_eq!(errors.errors()[2].field, "contacts[1].password");
}

#[test]
fn dto_validation_accepts_vec_nested_valid_input() {
    let form = ContactListForm {
        contacts: vec![
            ContactDetails {
                email: "alice@example.com".into(),
                password: "secret1".into(),
            },
            ContactDetails {
                email: "bob@example.com".into(),
                password: "secret2".into(),
            },
        ],
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
fn dto_validation_accepts_integer_fields() {
    let form = IntMetrics { retry_count: 3 };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_accepts_numeric_min_max_bounds() {
    let form = NumericBoundsForm {
        retry_count: 3,
        average_latency_ms: 12.5,
    };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_rejects_values_below_min() {
    let form = NumericBoundsForm {
        retry_count: 1,
        average_latency_ms: 12.5,
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "retry_count");
    assert_eq!(
        errors.errors()[0].constraints.get("min"),
        Some(&"must be at least 2".to_string())
    );
}

#[test]
fn dto_validation_rejects_values_above_max() {
    let form = NumericBoundsForm {
        retry_count: 3,
        average_latency_ms: 21.0,
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "average_latency_ms");
    assert_eq!(
        errors.errors()[0].constraints.get("max"),
        Some(&"must be at most 20.25".to_string())
    );
}

#[test]
fn dto_validation_skips_optional_numeric_bounds_when_absent() {
    let form = OptionalNumericBoundsForm { retry_count: None };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_applies_optional_numeric_bounds_when_present() {
    let form = OptionalNumericBoundsForm {
        retry_count: Some(1),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "retry_count");
    assert_eq!(
        errors.errors()[0].constraints.get("min"),
        Some(&"must be at least 2".to_string())
    );
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

#[test]
fn dto_validation_accepts_uuid_fields() {
    let form = SessionForm {
        session_id: "550e8400-e29b-41d4-a716-446655440000".into(),
    };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_rejects_invalid_uuid_fields() {
    let form = SessionForm {
        session_id: "not-a-uuid".into(),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "session_id");
    assert_eq!(
        errors.errors()[0].constraints.get("is_uuid"),
        Some(&"must be a valid UUID".to_string())
    );
}

#[test]
fn dto_validation_accepts_url_fields() {
    let form = WebhookForm {
        callback_url: "https://example.com/webhook".into(),
    };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_rejects_invalid_url_fields() {
    let form = WebhookForm {
        callback_url: "not a url".into(),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "callback_url");
    assert_eq!(
        errors.errors()[0].constraints.get("is_url"),
        Some(&"must be a valid URL".to_string())
    );
}

#[test]
fn dto_validation_accepts_enum_variants() {
    let form = EnumForm {
        access_level: "Reader".into(),
    };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_rejects_invalid_enum_variants() {
    let form = EnumForm {
        access_level: "Guest".into(),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "access_level");
    assert_eq!(
        errors.errors()[0].constraints.get("is_enum"),
        Some(&"must be a valid enum variant".to_string())
    );
}

#[test]
fn dto_validation_groups_skip_grouped_fields_for_plain_validate() {
    let form = GroupedParentForm {
        create_email: "not-an-email".into(),
        password: "123".into(),
        always_email: "still-not-an-email".into(),
        child: GroupedChildForm {
            email: "child-not-an-email".into(),
        },
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "always_email");
    assert_eq!(
        errors.errors()[0].constraints.get("is_email"),
        Some(&"must be a valid email".to_string())
    );
}

#[test]
fn dto_validation_groups_fire_for_matching_group() {
    let form = GroupedParentForm {
        create_email: "not-an-email".into(),
        password: "123".into(),
        always_email: "still-not-an-email".into(),
        child: GroupedChildForm {
            email: "child-not-an-email".into(),
        },
    };

    let errors = form
        .validate_with(&ValidationContext::new().with_group("create"))
        .unwrap_err();
    assert_eq!(errors.len(), 4);
    assert!(errors
        .errors()
        .iter()
        .any(|error| error.field == "create_email"));
    assert!(errors
        .errors()
        .iter()
        .any(|error| error.field == "password"));
    assert!(errors
        .errors()
        .iter()
        .any(|error| error.field == "always_email"));
    assert!(errors
        .errors()
        .iter()
        .any(|error| error.field == "child.email"));
}

#[test]
fn dto_validation_groups_skip_non_matching_group() {
    let form = GroupedParentForm {
        create_email: "not-an-email".into(),
        password: "123".into(),
        always_email: "still-not-an-email".into(),
        child: GroupedChildForm {
            email: "child-not-an-email".into(),
        },
    };

    let errors = form
        .validate_with(&ValidationContext::new().with_group("delete"))
        .unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "always_email");
}

#[test]
fn dto_validation_groups_allow_multi_group_rules() {
    let form = GroupedParentForm {
        create_email: "person@example.com".into(),
        password: "123".into(),
        always_email: "person@example.com".into(),
        child: GroupedChildForm {
            email: "child@example.com".into(),
        },
    };

    let errors = form
        .validate_with(&ValidationContext::new().with_group("update"))
        .unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "password");
    assert_eq!(
        errors.errors()[0].constraints.get("min_length"),
        Some(&"must be at least 6 characters".to_string())
    );
}

#[test]
fn dto_validation_accepts_non_empty_fields() {
    let form = NonEmptyForm {
        title: "hello".into(),
        tags: vec!["one".into()],
    };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_rejects_empty_fields() {
    let form = NonEmptyForm {
        title: String::new(),
        tags: Vec::new(),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 2);
    let title_error = errors
        .errors()
        .iter()
        .find(|error| error.field == "title")
        .expect("title error must exist");
    assert_eq!(
        title_error.constraints.get("is_not_empty"),
        Some(&"must not be empty".to_string())
    );

    let tags_error = errors
        .errors()
        .iter()
        .find(|error| error.field == "tags")
        .expect("tags error must exist");
    assert_eq!(
        tags_error.constraints.get("is_not_empty"),
        Some(&"must not be empty".to_string())
    );
}

#[test]
fn dto_validation_enforces_array_size_bounds() {
    let form = ArraySizeForm {
        tags: vec!["one".into(), "two".into()],
    };

    assert!(form.validate().is_ok());

    let form = ArraySizeForm {
        tags: vec!["one".into()],
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "tags");
    assert_eq!(
        errors.errors()[0].constraints.get("array_min_size"),
        Some(&"must contain at least 2 items".to_string())
    );

    let form = ArraySizeForm {
        tags: vec!["one".into(), "two".into(), "three".into(), "four".into()],
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "tags");
    assert_eq!(
        errors.errors()[0].constraints.get("array_max_size"),
        Some(&"must contain at most 3 items".to_string())
    );
}

#[test]
fn dto_validation_skips_optional_array_size_when_absent() {
    let form = OptionalArraySizeForm { tags: None };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_applies_optional_array_size_when_present() {
    let form = OptionalArraySizeForm {
        tags: Some(vec!["one".into()]),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "tags");
    assert_eq!(
        errors.errors()[0].constraints.get("array_min_size"),
        Some(&"must contain at least 2 items".to_string())
    );
}

#[test]
fn dto_validation_skips_optional_non_empty_fields_when_absent() {
    let form = OptionalNonEmptyForm { title: None };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_accepts_regex_matched_fields() {
    let form = SlugForm {
        slug: "valid-slug-123".into(),
    };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_rejects_regex_mismatches() {
    let form = SlugForm {
        slug: "Not A Slug".into(),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "slug");
    assert_eq!(
        errors.errors()[0].constraints.get("matches"),
        Some(&"must match the required pattern".to_string())
    );
}

#[test]
fn dto_validation_applies_validate_if_when_condition_matches() {
    let form = ConditionalValidationForm {
        mode: "create".into(),
        email: "not-an-email".into(),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "email");
    assert_eq!(
        errors.errors()[0].constraints.get("is_email"),
        Some(&"must be a valid email".to_string())
    );
}

#[test]
fn dto_validation_skips_validate_if_when_condition_does_not_match() {
    let form = ConditionalValidationForm {
        mode: "update".into(),
        email: "not-an-email".into(),
    };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_composes_validate_if_with_optional_fields() {
    let form = OptionalConditionalValidationForm {
        mode: "create".into(),
        email: None,
    };

    assert!(form.validate().is_ok());

    let form = OptionalConditionalValidationForm {
        mode: "create".into(),
        email: Some("not-an-email".into()),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "email");
    assert_eq!(
        errors.errors()[0].constraints.get("is_email"),
        Some(&"must be a valid email".to_string())
    );
}

#[test]
fn dto_validation_skips_optional_fields_when_absent() {
    let form = OptionalContactForm { email: None };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_validates_optional_fields_when_present() {
    let form = OptionalContactForm {
        email: Some("not-an-email".into()),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "email");
    assert_eq!(
        errors.errors()[0].constraints.get("is_email"),
        Some(&"must be a valid email".to_string())
    );
}

#[test]
fn dto_validation_accepts_custom_validators() {
    let form = CustomValidateForm {
        email: "alice@example.com".into(),
    };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_rejects_failed_custom_validators() {
    let form = CustomValidateForm {
        email: "alice@other.dev".into(),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "email");
    assert_eq!(
        errors.errors()[0].constraints.get("custom_validate"),
        Some(&"failed custom validation".to_string())
    );
}

#[test]
fn dto_validation_skips_optional_custom_validators_when_absent() {
    let form = OptionalCustomValidateForm { email: None };

    assert!(form.validate().is_ok());
}

#[test]
fn dto_validation_runs_optional_custom_validators_when_present() {
    let form = OptionalCustomValidateForm {
        email: Some("alice@other.dev".into()),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "email");
    assert_eq!(
        errors.errors()[0].constraints.get("custom_validate"),
        Some(&"failed custom validation".to_string())
    );
}

#[test]
fn partial_dto_validation_applies_validate_if_for_partial_fields() {
    let form = PartialConditionalValidationForm {
        mode: Some("create".into()),
        email: Some("not-an-email".into()),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "email");
    assert_eq!(
        errors.errors()[0].constraints.get("is_email"),
        Some(&"must be a valid email".to_string())
    );

    let form = PartialConditionalValidationForm {
        mode: Some("update".into()),
        email: Some("not-an-email".into()),
    };

    assert!(form.validate().is_ok());

    let form = PartialConditionalValidationForm {
        mode: None,
        email: Some("not-an-email".into()),
    };

    assert!(form.validate().is_ok());
}

#[test]
fn partial_dto_validation_accepts_absent_fields() {
    let form = PartialContactForm {
        email: None,
        password: None,
    };

    assert!(form.validate().is_ok());
}

#[test]
fn partial_dto_validation_accepts_present_valid_fields() {
    let form = PartialContactForm {
        email: Some("alice@example.com".into()),
        password: Some("secret1".into()),
    };

    assert!(form.validate().is_ok());
}

#[test]
fn partial_dto_validation_collects_present_invalid_fields() {
    let form = PartialContactForm {
        email: Some("not-an-email".into()),
        password: Some("123".into()),
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
fn partial_dto_validation_accepts_custom_validators_when_absent() {
    let form = PartialCustomValidateForm { email: None };

    assert!(form.validate().is_ok());
}

#[test]
fn partial_dto_validation_runs_custom_validators_when_present() {
    let form = PartialCustomValidateForm {
        email: Some("alice@other.dev".into()),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors.errors()[0].field, "email");
    assert_eq!(
        errors.errors()[0].constraints.get("custom_validate"),
        Some(&"failed custom validation".to_string())
    );
}

#[test]
fn partial_dto_validation_propagates_nested_errors() {
    let form = PartialAccountForm {
        contact: Some(ContactDetails {
            email: "not-an-email".into(),
            password: "123".into(),
        }),
    };

    let errors = form.validate().unwrap_err();
    assert_eq!(errors.len(), 2);
    assert_eq!(errors.errors()[0].field, "contact.email");
    assert_eq!(errors.errors()[1].field, "contact.password");
}
