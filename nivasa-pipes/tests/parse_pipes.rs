use nivasa_common::HttpException;
use nivasa_pipes::{
    ArgumentMetadata, ParseBoolPipe, ParseEnumPipe, ParseEnumTarget, ParseFloatPipe, ParseIntPipe,
    ParseUuidPipe, Pipe, PipeChain,
};
use nivasa_validation::Validate;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessLevel {
    Admin,
    Reader,
}

impl ParseEnumTarget for AccessLevel {
    fn parse(input: &str) -> Result<Self, String> {
        match input.to_ascii_lowercase().as_str() {
            "admin" => Ok(Self::Admin),
            "reader" => Ok(Self::Reader),
            other => Err(format!("unknown access level `{other}`")),
        }
    }

    fn into_value(value: Self) -> Value {
        match value {
            Self::Admin => Value::from("admin"),
            Self::Reader => Value::from("reader"),
        }
    }
}

struct FailingPipe {
    calls: Arc<AtomicUsize>,
}

impl FailingPipe {
    fn new(calls: Arc<AtomicUsize>) -> Self {
        Self { calls }
    }
}

impl Pipe for FailingPipe {
    fn transform(
        &self,
        _value: Value,
        _metadata: ArgumentMetadata,
    ) -> Result<Value, HttpException> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(HttpException::bad_request("first pipe failed"))
    }
}

struct CountingPipe {
    calls: Arc<AtomicUsize>,
}

impl CountingPipe {
    fn new(calls: Arc<AtomicUsize>) -> Self {
        Self { calls }
    }
}

impl Pipe for CountingPipe {
    fn transform(&self, value: Value, _metadata: ArgumentMetadata) -> Result<Value, HttpException> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(value)
    }
}

#[test]
fn parse_pipes_handle_the_common_scalar_shapes() {
    let metadata = ArgumentMetadata::new(0);

    assert_eq!(
        ParseBoolPipe::new()
            .transform(json!("true"), metadata.clone())
            .unwrap(),
        json!(true)
    );
    assert_eq!(
        ParseIntPipe::<i64>::new()
            .transform(json!("42"), metadata.clone())
            .unwrap(),
        json!(42)
    );
    assert_eq!(
        ParseFloatPipe::<f64>::new()
            .transform(json!("3.5"), metadata.clone())
            .unwrap(),
        json!(3.5)
    );
    assert_eq!(
        ParseUuidPipe::new()
            .transform(json!("550e8400-e29b-41d4-a716-446655440000"), metadata)
            .unwrap(),
        json!("550e8400-e29b-41d4-a716-446655440000")
    );
}

#[test]
fn parse_enum_pipe_turns_strings_into_enum_values() {
    let pipe = ParseEnumPipe::<AccessLevel>::new();

    assert_eq!(
        pipe.transform(json!("ADMIN"), ArgumentMetadata::new(1))
            .unwrap(),
        json!("admin")
    );
}

#[test]
fn parse_pipes_reject_bad_input_with_clear_errors() {
    let bool_error = ParseBoolPipe::new()
        .transform(json!("not-bool"), ArgumentMetadata::new(2))
        .unwrap_err();
    assert_eq!(
        bool_error.message,
        "ParseBoolPipe could not parse `not-bool` as a boolean"
    );

    let int_error = ParseIntPipe::<i64>::new()
        .transform(json!("abc"), ArgumentMetadata::new(3))
        .unwrap_err();
    assert_eq!(
        int_error.message,
        "ParseIntPipe could not parse `abc` as an integer"
    );

    let float_error = ParseFloatPipe::<f64>::new()
        .transform(json!("nope"), ArgumentMetadata::new(4))
        .unwrap_err();
    assert_eq!(
        float_error.message,
        "ParseFloatPipe could not parse `nope` as a float"
    );

    let uuid_error = ParseUuidPipe::new()
        .transform(json!("not-a-uuid"), ArgumentMetadata::new(5))
        .unwrap_err();
    assert_eq!(
        uuid_error.message,
        "ParseUuidPipe could not parse `not-a-uuid` as a UUID"
    );
}

#[test]
fn parse_pipes_reject_non_string_input_with_clear_errors() {
    let bool_error = ParseBoolPipe::new()
        .transform(json!(true), ArgumentMetadata::new(6))
        .unwrap_err();
    assert_eq!(bool_error.message, "ParseBoolPipe expects a string value");

    let int_error = ParseIntPipe::<i64>::new()
        .transform(json!(42), ArgumentMetadata::new(7))
        .unwrap_err();
    assert_eq!(int_error.message, "ParseIntPipe expects a string value");

    let float_error = ParseFloatPipe::<f64>::new()
        .transform(json!(3.5), ArgumentMetadata::new(8))
        .unwrap_err();
    assert_eq!(float_error.message, "ParseFloatPipe expects a string value");

    let uuid_error = ParseUuidPipe::new()
        .transform(
            json!({"id": "550e8400-e29b-41d4-a716-446655440000"}),
            ArgumentMetadata::new(9),
        )
        .unwrap_err();
    assert_eq!(uuid_error.message, "ParseUuidPipe expects a string value");

    let enum_error = ParseEnumPipe::<AccessLevel>::new()
        .transform(json!(null), ArgumentMetadata::new(10))
        .unwrap_err();
    assert_eq!(enum_error.message, "ParseEnumPipe expects a string value");
}

#[test]
fn pipe_chain_short_circuits_when_the_first_pipe_fails() {
    let first_calls = Arc::new(AtomicUsize::new(0));
    let second_calls = Arc::new(AtomicUsize::new(0));
    let chain = PipeChain::new(
        FailingPipe::new(Arc::clone(&first_calls)),
        CountingPipe::new(Arc::clone(&second_calls)),
    );

    let error = chain
        .transform(json!("anything"), ArgumentMetadata::new(6))
        .unwrap_err();

    assert_eq!(error.status_code, 400);
    assert_eq!(error.message, "first pipe failed");
    assert_eq!(first_calls.load(Ordering::SeqCst), 1);
    assert_eq!(second_calls.load(Ordering::SeqCst), 0);
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct BrokenSerializeDto {
    name: String,
}

impl Validate for BrokenSerializeDto {
    fn validate(&self) -> Result<(), nivasa_validation::ValidationErrors> {
        Ok(())
    }
}

impl Serialize for BrokenSerializeDto {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(serde::ser::Error::custom("serialize boom"))
    }
}

#[test]
fn validation_pipe_reports_serialize_errors_after_successful_validation() {
    let pipe = nivasa_pipes::ValidationPipe::<BrokenSerializeDto>::new();

    let error = pipe
        .transform(
            json!({
                "name": "Ada"
            }),
            ArgumentMetadata::new(11).with_data_type("body"),
        )
        .unwrap_err();

    assert_eq!(error.status_code, 400);
    assert!(
        error
            .message
            .starts_with("ValidationPipe could not serialize validated DTO:"),
        "unexpected error message: {}",
        error.message
    );
}
