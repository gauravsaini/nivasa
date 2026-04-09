use nivasa_macros::Dto;

#[derive(Dto)]
struct InvalidMaxForm {
    #[max("five")]
    retry_count: u32,
}

fn main() {}
