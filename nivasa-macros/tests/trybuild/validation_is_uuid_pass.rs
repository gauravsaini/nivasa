use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct SessionForm {
    #[is_uuid]
    session_id: String,
}

fn main() {
    let form = SessionForm {
        session_id: "550e8400-e29b-41d4-a716-446655440000".into(),
    };

    let _ = form.validate();
}
