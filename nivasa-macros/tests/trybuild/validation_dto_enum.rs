use nivasa_macros::Dto;

#[derive(Dto)]
enum InvalidEnumDto {
    Email {
        #[is_email]
        value: String,
    },
}

fn main() {}
