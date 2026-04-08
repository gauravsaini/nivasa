use nivasa_macros::Dto;

fn first(value: &String) -> bool {
    !value.is_empty()
}

fn second(value: &String) -> bool {
    !value.is_empty()
}

#[derive(Dto)]
struct MultipleCustomValidators {
    #[custom_validate(first, second)]
    email: String,
}

fn main() {}
