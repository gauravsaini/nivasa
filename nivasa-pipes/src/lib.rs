//! # nivasa-pipes
//!
//! Nivasa framework — pipes.

use nivasa_common::HttpException;
use serde_json::Value;
use std::any::TypeId;
use std::marker::PhantomData;
use std::num::ParseIntError;

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
        let input = value.as_str().ok_or_else(|| {
            HttpException::bad_request("ParseIntPipe expects a string value")
        })?;

        let parsed = T::parse(input).map_err(|_| {
            HttpException::bad_request(format!("ParseIntPipe could not parse `{input}` as an integer"))
        })?;

        Ok(T::into_value(parsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

        let error = pipe.transform(json!("abc"), ArgumentMetadata::new(2)).unwrap_err();

        assert_eq!(error.status_code, 400);
        assert_eq!(
            error.message,
            "ParseIntPipe could not parse `abc` as an integer"
        );
    }
}
