//! # nivasa-pipes
//!
//! Nivasa framework — pipes.

use nivasa_common::HttpException;
use serde_json::Value;
use std::any::TypeId;

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
}
