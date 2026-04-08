use nivasa_macros::Dto;
use nivasa_pipes::ParseEnumTarget;
use serde_json::Value;
use nivasa_validation::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessLevel {
    Admin,
    Reader,
}

impl ParseEnumTarget for AccessLevel {
    fn parse(input: &str) -> Result<Self, String> {
        match input {
            "Admin" => Ok(Self::Admin),
            "Reader" => Ok(Self::Reader),
            other => Err(format!("unknown access level `{other}`")),
        }
    }

    fn into_value(value: Self) -> Value {
        match value {
            Self::Admin => Value::from("Admin"),
            Self::Reader => Value::from("Reader"),
        }
    }
}

#[derive(Dto)]
struct AccessForm {
    #[is_enum(AccessLevel)]
    access_level: String,
}

fn main() {
    let form = AccessForm {
        access_level: "Admin".into(),
    };

    let _ = form.validate();
}
