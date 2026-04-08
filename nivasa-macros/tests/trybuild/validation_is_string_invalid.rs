use nivasa_macros::Dto;

#[derive(Dto)]
struct ProfileForm {
    #[is_string]
    display_name: u32,
}

fn main() {}
