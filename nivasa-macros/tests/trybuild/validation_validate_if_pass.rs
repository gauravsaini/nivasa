use nivasa_macros::{Dto, PartialDto};
use nivasa_validation::Validate;

#[derive(Dto)]
struct ConditionalDto {
    mode: String,
    #[validate_if(mode, "create")]
    #[is_email]
    email: String,
}

#[derive(Dto)]
struct ConditionalOptionalDto {
    mode: String,
    #[is_optional]
    #[validate_if(mode, "create")]
    #[is_email]
    email: Option<String>,
}

#[derive(PartialDto)]
struct ConditionalPartialDto {
    mode: Option<String>,
    #[validate_if(mode, "create")]
    #[is_email]
    email: Option<String>,
}

fn main() {
    let form = ConditionalDto {
        mode: "create".into(),
        email: "alice@example.com".into(),
    };
    assert!(form.validate().is_ok());

    let optional_form = ConditionalOptionalDto {
        mode: "update".into(),
        email: Some("not-an-email".into()),
    };
    assert!(optional_form.validate().is_ok());

    let partial_form = ConditionalPartialDto {
        mode: Some("create".into()),
        email: Some("alice@example.com".into()),
    };
    assert!(partial_form.validate().is_ok());
}
