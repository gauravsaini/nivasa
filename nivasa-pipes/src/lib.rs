//! # nivasa-pipes
//!
//! Nivasa framework — pipes.

use nivasa_common::HttpException;
use serde_json::Value;
use std::any::TypeId;
use std::marker::PhantomData;
use std::num::ParseFloatError;
use std::num::ParseIntError;
use std::str::ParseBoolError;
use uuid::Uuid;

/// Metadata passed into a pipe for the current argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentMetadata {
    pub param_name: Option<String>,
    pub metatype: Option<TypeId>,
    pub data_type: Option<String>,
    pub index: usize,
}

impl ArgumentMetadata {
    /// Create a metadata record for the argument at the given index.
    pub fn new(index: usize) -> Self {
        Self {
            param_name: None,
            metatype: None,
            data_type: None,
            index,
        }
    }

    /// Set the argument name.
    pub fn with_param_name(mut self, param_name: impl Into<String>) -> Self {
        self.param_name = Some(param_name.into());
        self
    }

    /// Set the argument metatype.
    pub fn with_metatype(mut self, metatype: TypeId) -> Self {
        self.metatype = Some(metatype);
        self
    }

    /// Set the argument data type label.
    pub fn with_data_type(mut self, data_type: impl Into<String>) -> Self {
        self.data_type = Some(data_type.into());
        self
    }
}

/// A value transformer that can validate or coerce a handler argument.
pub trait Pipe: Send + Sync + 'static {
    fn transform(&self, value: Value, metadata: ArgumentMetadata) -> Result<Value, HttpException>;
}

/// Supported integer targets for [`ParseIntPipe`].
pub trait ParseIntTarget: Send + Sync + 'static {
    fn parse(input: &str) -> Result<Self, ParseIntError>
    where
        Self: Sized;

    fn into_value(value: Self) -> Value
    where
        Self: Sized;
}

impl ParseIntTarget for i32 {
    fn parse(input: &str) -> Result<Self, ParseIntError> {
        input.parse::<i32>()
    }

    fn into_value(value: Self) -> Value {
        Value::from(value)
    }
}

impl ParseIntTarget for i64 {
    fn parse(input: &str) -> Result<Self, ParseIntError> {
        input.parse::<i64>()
    }

    fn into_value(value: Self) -> Value {
        Value::from(value)
    }
}

/// Supported floating-point targets for [`ParseFloatPipe`].
pub trait ParseFloatTarget: Send + Sync + 'static {
    fn parse(input: &str) -> Result<Self, ParseFloatError>
    where
        Self: Sized;

    fn into_value(value: Self) -> Value
    where
        Self: Sized;
}

impl ParseFloatTarget for f32 {
    fn parse(input: &str) -> Result<Self, ParseFloatError> {
        input.parse::<f32>()
    }

    fn into_value(value: Self) -> Value {
        Value::from(value)
    }
}

impl ParseFloatTarget for f64 {
    fn parse(input: &str) -> Result<Self, ParseFloatError> {
        input.parse::<f64>()
    }

    fn into_value(value: Self) -> Value {
        Value::from(value)
    }
}

/// Parse a JSON string into a boolean value.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParseBoolPipe;

impl ParseBoolPipe {
    /// Create a new boolean parser.
    pub const fn new() -> Self {
        Self
    }
}

impl Pipe for ParseBoolPipe {
    fn transform(&self, value: Value, _metadata: ArgumentMetadata) -> Result<Value, HttpException> {
        let input = value
            .as_str()
            .ok_or_else(|| HttpException::bad_request("ParseBoolPipe expects a string value"))?;

        let parsed = input.parse::<bool>().map_err(|_error: ParseBoolError| {
            HttpException::bad_request(format!(
                "ParseBoolPipe could not parse `{input}` as a boolean"
            ))
        })?;

        Ok(Value::from(parsed))
    }
}

/// Parse a JSON string into an integer value.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParseIntPipe<T = i64> {
    _marker: PhantomData<T>,
}

impl<T> ParseIntPipe<T> {
    /// Create a new integer parser.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T> Pipe for ParseIntPipe<T>
where
    T: ParseIntTarget,
{
    fn transform(&self, value: Value, _metadata: ArgumentMetadata) -> Result<Value, HttpException> {
        let input = value
            .as_str()
            .ok_or_else(|| HttpException::bad_request("ParseIntPipe expects a string value"))?;

        let parsed = T::parse(input).map_err(|_| {
            HttpException::bad_request(format!(
                "ParseIntPipe could not parse `{input}` as an integer"
            ))
        })?;

        Ok(T::into_value(parsed))
    }
}

/// Parse a JSON string into a floating-point value.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParseFloatPipe<T = f64> {
    _marker: PhantomData<T>,
}

impl<T> ParseFloatPipe<T> {
    /// Create a new float parser.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T> Pipe for ParseFloatPipe<T>
where
    T: ParseFloatTarget,
{
    fn transform(&self, value: Value, _metadata: ArgumentMetadata) -> Result<Value, HttpException> {
        let input = value
            .as_str()
            .ok_or_else(|| HttpException::bad_request("ParseFloatPipe expects a string value"))?;

        let parsed = T::parse(input).map_err(|_| {
            HttpException::bad_request(format!(
                "ParseFloatPipe could not parse `{input}` as a float"
            ))
        })?;

        Ok(T::into_value(parsed))
    }
}

/// Trim leading and trailing whitespace from a JSON string value.
#[derive(Debug, Clone, Copy, Default)]
pub struct TrimPipe;

impl TrimPipe {
    /// Create a new string trimmer.
    pub const fn new() -> Self {
        Self
    }
}

impl Pipe for TrimPipe {
    fn transform(&self, value: Value, _metadata: ArgumentMetadata) -> Result<Value, HttpException> {
        let input = value
            .as_str()
            .ok_or_else(|| HttpException::bad_request("TrimPipe expects a string value"))?;

        Ok(Value::from(input.trim().to_string()))
    }
}

/// Parse a JSON string into a UUID value.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParseUuidPipe;

impl ParseUuidPipe {
    /// Create a new UUID parser.
    pub const fn new() -> Self {
        Self
    }
}

impl Pipe for ParseUuidPipe {
    fn transform(&self, value: Value, _metadata: ArgumentMetadata) -> Result<Value, HttpException> {
        let input = value
            .as_str()
            .ok_or_else(|| HttpException::bad_request("ParseUuidPipe expects a string value"))?;

        let parsed = Uuid::parse_str(input).map_err(|_| {
            HttpException::bad_request(format!(
                "ParseUuidPipe could not parse `{input}` as a UUID"
            ))
        })?;

        Ok(Value::from(parsed.to_string()))
    }
}

/// Supported enum targets for [`ParseEnumPipe`].
pub trait ParseEnumTarget: Send + Sync + 'static {
    fn parse(input: &str) -> Result<Self, String>
    where
        Self: Sized;

    fn into_value(value: Self) -> Value
    where
        Self: Sized;
}

/// Parse a JSON string into an enum-like value.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParseEnumPipe<T> {
    _marker: PhantomData<T>,
}

impl<T> ParseEnumPipe<T> {
    /// Create a new enum parser.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T> Pipe for ParseEnumPipe<T>
where
    T: ParseEnumTarget,
{
    fn transform(&self, value: Value, _metadata: ArgumentMetadata) -> Result<Value, HttpException> {
        let input = value
            .as_str()
            .ok_or_else(|| HttpException::bad_request("ParseEnumPipe expects a string value"))?;

        let parsed = T::parse(input).map_err(|reason| {
            HttpException::bad_request(format!(
                "ParseEnumPipe could not parse `{input}` as an enum variant: {reason}"
            ))
        })?;

        Ok(T::into_value(parsed))
    }
}

/// Compose two pipes and run them left to right.
///
/// This is a reusable sequencing primitive for future `#[pipe(...)]` support.
pub struct PipeChain<A, B> {
    first: A,
    second: B,
}

impl<A, B> PipeChain<A, B> {
    /// Create a pipe chain that runs `first` and then `second`.
    pub const fn new(first: A, second: B) -> Self {
        Self { first, second }
    }
}

impl<A, B> Pipe for PipeChain<A, B>
where
    A: Pipe,
    B: Pipe,
{
    fn transform(&self, value: Value, metadata: ArgumentMetadata) -> Result<Value, HttpException> {
        let value = self.first.transform(value, metadata.clone())?;
        self.second.transform(value, metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    struct EchoPipe;

    impl Pipe for EchoPipe {
        fn transform(
            &self,
            value: Value,
            metadata: ArgumentMetadata,
        ) -> Result<Value, HttpException> {
            assert_eq!(metadata.param_name.as_deref(), Some("user_id"));
            assert_eq!(metadata.data_type.as_deref(), Some("param"));
            assert_eq!(metadata.index, 1);
            assert!(metadata.metatype.is_some());
            Ok(value)
        }
    }

    struct RecordingPipe {
        calls: Arc<Mutex<Vec<ArgumentMetadata>>>,
    }

    impl RecordingPipe {
        fn new(calls: Arc<Mutex<Vec<ArgumentMetadata>>>) -> Self {
            Self { calls }
        }
    }

    impl Pipe for RecordingPipe {
        fn transform(
            &self,
            value: Value,
            metadata: ArgumentMetadata,
        ) -> Result<Value, HttpException> {
            self.calls.lock().unwrap().push(metadata);
            Ok(value)
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
        fn transform(
            &self,
            value: Value,
            _metadata: ArgumentMetadata,
        ) -> Result<Value, HttpException> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(value)
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

    #[test]
    fn metadata_builder_populates_the_expected_fields() {
        let metadata = ArgumentMetadata::new(3)
            .with_param_name("user_id")
            .with_metatype(TypeId::of::<u64>())
            .with_data_type("param");

        assert_eq!(metadata.param_name.as_deref(), Some("user_id"));
        assert_eq!(metadata.metatype, Some(TypeId::of::<u64>()));
        assert_eq!(metadata.data_type.as_deref(), Some("param"));
        assert_eq!(metadata.index, 3);
    }

    #[test]
    fn pipe_trait_accepts_a_value_and_metadata_bundle() {
        fn assert_pipe<T: Pipe>() {}
        assert_pipe::<EchoPipe>();

        let pipe = EchoPipe;
        let metadata = ArgumentMetadata::new(1)
            .with_param_name("user_id")
            .with_metatype(TypeId::of::<u64>())
            .with_data_type("param");

        let value = json!({ "id": 42 });
        let transformed = pipe.transform(value.clone(), metadata).unwrap();

        assert_eq!(transformed, value);
    }

    #[test]
    fn parse_int_pipe_transforms_integer_strings_for_i32_and_i64() {
        let metadata = ArgumentMetadata::new(0);

        let i32_pipe = ParseIntPipe::<i32>::new();
        let i64_pipe = ParseIntPipe::<i64>::new();

        assert_eq!(
            i32_pipe.transform(json!("42"), metadata.clone()).unwrap(),
            json!(42)
        );
        assert_eq!(
            i64_pipe.transform(json!("-17"), metadata).unwrap(),
            json!(-17)
        );
    }

    #[test]
    fn parse_int_pipe_rejects_non_integer_input() {
        let pipe = ParseIntPipe::<i64>::new();

        let error = pipe
            .transform(json!("abc"), ArgumentMetadata::new(2))
            .unwrap_err();

        assert_eq!(error.status_code, 400);
        assert_eq!(
            error.message,
            "ParseIntPipe could not parse `abc` as an integer"
        );
    }

    #[test]
    fn parse_float_pipe_transforms_float_strings_for_f32_and_f64() {
        let metadata = ArgumentMetadata::new(0);

        let f32_pipe = ParseFloatPipe::<f32>::new();
        let f64_pipe = ParseFloatPipe::<f64>::new();

        assert_eq!(
            f32_pipe.transform(json!("3.5"), metadata.clone()).unwrap(),
            json!(3.5f32)
        );
        assert_eq!(
            f64_pipe.transform(json!("-0.125"), metadata).unwrap(),
            json!(-0.125f64)
        );
    }

    #[test]
    fn parse_float_pipe_rejects_non_float_input() {
        let pipe = ParseFloatPipe::<f64>::new();

        let error = pipe
            .transform(json!("not-a-float"), ArgumentMetadata::new(4))
            .unwrap_err();

        assert_eq!(error.status_code, 400);
        assert_eq!(
            error.message,
            "ParseFloatPipe could not parse `not-a-float` as a float"
        );
    }

    #[test]
    fn parse_bool_pipe_transforms_boolean_strings() {
        let pipe = ParseBoolPipe::new();
        let metadata = ArgumentMetadata::new(0);

        assert_eq!(
            pipe.transform(json!("true"), metadata.clone()).unwrap(),
            json!(true)
        );
        assert_eq!(
            pipe.transform(json!("false"), metadata).unwrap(),
            json!(false)
        );
    }

    #[test]
    fn parse_bool_pipe_rejects_non_boolean_input() {
        let pipe = ParseBoolPipe::new();

        let error = pipe
            .transform(json!("definitely-not-bool"), ArgumentMetadata::new(5))
            .unwrap_err();

        assert_eq!(error.status_code, 400);
        assert_eq!(
            error.message,
            "ParseBoolPipe could not parse `definitely-not-bool` as a boolean"
        );
    }

    #[test]
    fn trim_pipe_trims_outer_whitespace_and_preserves_inner_spacing() {
        let pipe = TrimPipe::new();
        let metadata = ArgumentMetadata::new(6);

        assert_eq!(
            pipe.transform(json!("  hello   world  "), metadata).unwrap(),
            json!("hello   world")
        );
    }

    #[test]
    fn trim_pipe_rejects_non_string_input() {
        let pipe = TrimPipe::new();

        let error = pipe
            .transform(json!(true), ArgumentMetadata::new(7))
            .unwrap_err();

        assert_eq!(error.status_code, 400);
        assert_eq!(error.message, "TrimPipe expects a string value");
    }

    #[test]
    fn parse_uuid_pipe_transforms_uuid_strings() {
        let pipe = ParseUuidPipe::new();

        assert_eq!(
            pipe.transform(
                json!("550E8400-E29B-41D4-A716-446655440000"),
                ArgumentMetadata::new(8),
            )
            .unwrap(),
            json!("550e8400-e29b-41d4-a716-446655440000")
        );
    }

    #[test]
    fn parse_uuid_pipe_rejects_invalid_uuid_text_and_non_strings() {
        let pipe = ParseUuidPipe::new();

        let invalid_uuid_error = pipe
            .transform(json!("not-a-uuid"), ArgumentMetadata::new(9))
            .unwrap_err();
        assert_eq!(invalid_uuid_error.status_code, 400);
        assert_eq!(
            invalid_uuid_error.message,
            "ParseUuidPipe could not parse `not-a-uuid` as a UUID"
        );

        let non_string_error = pipe
            .transform(json!(123), ArgumentMetadata::new(10))
            .unwrap_err();
        assert_eq!(non_string_error.status_code, 400);
        assert_eq!(
            non_string_error.message,
            "ParseUuidPipe expects a string value"
        );
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum AccessLevel {
        Admin,
        Reader,
    }

    impl ParseEnumTarget for AccessLevel {
        fn parse(input: &str) -> Result<Self, String> {
            match input.to_ascii_lowercase().as_str() {
                "admin" | "administrator" => Ok(Self::Admin),
                "reader" | "read" => Ok(Self::Reader),
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

    #[test]
    fn parse_enum_pipe_transforms_string_values_into_enum_variants() {
        let pipe = ParseEnumPipe::<AccessLevel>::new();

        assert_eq!(
            pipe.transform(json!("ADMINISTRATOR"), ArgumentMetadata::new(11))
                .unwrap(),
            json!("admin")
        );
    }

    #[test]
    fn parse_enum_pipe_rejects_invalid_and_non_string_input() {
        let pipe = ParseEnumPipe::<AccessLevel>::new();

        let invalid_enum_error = pipe
            .transform(json!("guest"), ArgumentMetadata::new(12))
            .unwrap_err();
        assert_eq!(invalid_enum_error.status_code, 400);
        assert_eq!(
            invalid_enum_error.message,
            "ParseEnumPipe could not parse `guest` as an enum variant: unknown access level `guest`"
        );

        let non_string_error = pipe
            .transform(json!(false), ArgumentMetadata::new(13))
            .unwrap_err();
        assert_eq!(non_string_error.status_code, 400);
        assert_eq!(
            non_string_error.message,
            "ParseEnumPipe expects a string value"
        );
    }

    #[test]
    fn pipe_chain_runs_pipes_left_to_right() {
        let chain = PipeChain::new(TrimPipe::new(), ParseBoolPipe::new());

        assert_eq!(
            chain.transform(json!("  true  "), ArgumentMetadata::new(8)).unwrap(),
            json!(true)
        );
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
            .transform(json!("anything"), ArgumentMetadata::new(9))
            .unwrap_err();

        assert_eq!(error.status_code, 400);
        assert_eq!(error.message, "first pipe failed");
        assert_eq!(first_calls.load(Ordering::SeqCst), 1);
        assert_eq!(second_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn pipe_chain_preserves_metadata_for_both_stages() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let chain = PipeChain::new(
            RecordingPipe::new(Arc::clone(&seen)),
            RecordingPipe::new(Arc::clone(&seen)),
        );
        let metadata = ArgumentMetadata::new(10)
            .with_param_name("user_id")
            .with_metatype(TypeId::of::<u64>())
            .with_data_type("param");

        let output = chain.transform(json!("  spaced  "), metadata.clone()).unwrap();

        assert_eq!(output, json!("  spaced  "));

        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], metadata);
        assert_eq!(seen[1], metadata);
    }
}
