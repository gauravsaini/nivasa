use nivasa_macros::Dto;

#[derive(Dto)]
struct InvalidArrayMinSizeArgsForm {
    #[array_min_size("two")]
    tags: Vec<String>,
}

fn main() {}
