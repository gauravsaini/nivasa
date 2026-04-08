use nivasa_macros::Dto;

#[derive(Dto)]
struct InvalidMinForm {
    #[min]
    retry_count: u32,
}

fn main() {}
