use nivasa_macros::{Dto, PartialDto};

fn uses_example_domain(value: &String) -> bool {
    value.ends_with("@example.com")
}

#[derive(Dto)]
struct CreateUserForm {
    #[custom_validate(uses_example_domain)]
    email: String,
}

#[derive(PartialDto)]
struct UpdateUserForm {
    #[custom_validate(uses_example_domain)]
    email: Option<String>,
}

fn main() {
    let _ = CreateUserForm {
        email: "alice@example.com".into(),
    };
    let _ = UpdateUserForm {
        email: Some("alice@example.com".into()),
    };
}
