use nivasa_macros::Dto;

#[derive(Dto)]
struct InvalidArrayMaxSizeArgsForm {
    #[array_max_size]
    tags: Vec<String>,
}

fn main() {}
