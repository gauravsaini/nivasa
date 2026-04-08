use nivasa_macros::Dto;

#[derive(Dto)]
struct SessionForm {
    #[is_uuid]
    session_id: u32,
}

fn main() {}
