use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct ContentForm {
    #[is_not_empty]
    title: String,
}

fn main() {
    let form = ContentForm {
        title: "hello".into(),
    };

    let _ = form.validate();
}
