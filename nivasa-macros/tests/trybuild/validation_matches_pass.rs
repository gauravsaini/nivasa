use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct SlugForm {
    #[matches("^[a-z0-9-]+$")]
    slug: String,
}

fn main() {
    let form = SlugForm {
        slug: "valid-slug".into(),
    };

    let _ = form.validate();
}
