use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct ProfileForm {
    #[is_string]
    display_name: String,
}

fn main() {
    let form = ProfileForm {
        display_name: "Alice".into(),
    };

    let _ = form.validate();
}
