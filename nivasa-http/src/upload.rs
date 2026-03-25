//! Upload contract types for future multipart support.
//!
//! This module intentionally models uploaded files without performing any
//! request parsing. Multipart decoding still belongs to the SCXML-driven HTTP
//! pipeline once the multipart dependency and interceptor wiring are landed.

use std::collections::BTreeMap;

/// Buffered file payload extracted from a multipart request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedFile {
    filename: String,
    content_type: Option<String>,
    bytes: Vec<u8>,
}

impl UploadedFile {
    /// Create a new uploaded file value.
    pub fn new(
        filename: impl Into<String>,
        content_type: Option<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            filename: filename.into(),
            content_type,
            bytes: bytes.into(),
        }
    }

    /// Borrow the original filename.
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Borrow the declared content type, if the multipart part provided one.
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Borrow the buffered file bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return the buffered file length in bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the buffered file is empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Consume the value and return the buffered bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Consume the value and return the underlying upload parts.
    ///
    /// This keeps multipart parsing out of the request pipeline while still
    /// giving adapters a lightweight way to move the file metadata around.
    pub fn into_parts(self) -> (String, Option<String>, Vec<u8>) {
        (self.filename, self.content_type, self.bytes)
    }
}

/// Configurable multipart size limits that can be converted into `multer`
/// constraints later in the request pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MultipartLimits {
    whole_stream: Option<u64>,
    per_field: Option<u64>,
    field_limits: BTreeMap<String, u64>,
    allowed_fields: Vec<String>,
}

impl MultipartLimits {
    /// Create an empty limit set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum total payload size for the whole multipart stream.
    pub fn whole_stream(mut self, bytes: u64) -> Self {
        self.whole_stream = Some(bytes);
        self
    }

    /// Set the maximum size for any single multipart field.
    pub fn per_field(mut self, bytes: u64) -> Self {
        self.per_field = Some(bytes);
        self
    }

    /// Set a field-specific size limit.
    pub fn field_limit(mut self, field: impl Into<String>, bytes: u64) -> Self {
        self.field_limits.insert(field.into(), bytes);
        self
    }

    /// Restrict multipart parsing to a known set of field names.
    pub fn allowed_fields<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_fields = fields.into_iter().map(Into::into).collect();
        self
    }

    /// Borrow the whole-stream limit.
    pub fn whole_stream_limit(&self) -> Option<u64> {
        self.whole_stream
    }

    /// Borrow the default per-field limit.
    pub fn per_field_limit(&self) -> Option<u64> {
        self.per_field
    }

    /// Borrow a field-specific size limit.
    pub fn field_limit_for(&self, field: &str) -> Option<u64> {
        self.field_limits.get(field).copied()
    }

    /// Borrow the allowed field list.
    pub fn allowed_fields_list(&self) -> &[String] {
        &self.allowed_fields
    }

    /// Convert the configured limits into `multer` constraints.
    pub fn into_constraints(self) -> multer::Constraints {
        let mut constraints = multer::Constraints::new();

        if !self.allowed_fields.is_empty() {
            constraints = constraints.allowed_fields(self.allowed_fields);
        }

        if self.whole_stream.is_some() || self.per_field.is_some() || !self.field_limits.is_empty()
        {
            let mut size_limit = multer::SizeLimit::new();

            if let Some(bytes) = self.whole_stream {
                size_limit = size_limit.whole_stream(bytes);
            }

            if let Some(bytes) = self.per_field {
                size_limit = size_limit.per_field(bytes);
            }

            for (field, bytes) in self.field_limits {
                size_limit = size_limit.for_field(field, bytes);
            }

            constraints = constraints.size_limit(size_limit);
        }

        constraints
    }
}

#[cfg(test)]
mod tests {
    use super::{MultipartLimits, UploadedFile};

    #[test]
    fn uploaded_file_exposes_filename_content_type_and_bytes() {
        let file = UploadedFile::new(
            "avatar.png",
            Some("image/png".to_string()),
            vec![1, 2, 3, 4],
        );

        assert_eq!(file.filename(), "avatar.png");
        assert_eq!(file.content_type(), Some("image/png"));
        assert_eq!(file.bytes(), &[1, 2, 3, 4]);
        assert_eq!(file.len(), 4);
        assert!(!file.is_empty());
    }

    #[test]
    fn uploaded_file_round_trips_bytes() {
        let file = UploadedFile::new("notes.txt", None, b"hello".to_vec());

        assert_eq!(file.into_bytes(), b"hello".to_vec());
    }

    #[test]
    fn multipart_limits_track_size_configuration() {
        let limits = MultipartLimits::new()
            .whole_stream(1024)
            .per_field(256)
            .field_limit("avatar", 128)
            .allowed_fields(["avatar", "bio"]);

        assert_eq!(limits.whole_stream_limit(), Some(1024));
        assert_eq!(limits.per_field_limit(), Some(256));
        assert_eq!(limits.field_limit_for("avatar"), Some(128));
        assert_eq!(limits.field_limit_for("bio"), None);
        assert_eq!(
            limits.allowed_fields_list(),
            &["avatar".to_string(), "bio".to_string()]
        );
    }

    #[test]
    fn multipart_limits_convert_into_constraints_without_losing_limits() {
        let constraints = MultipartLimits::new()
            .whole_stream(2048)
            .per_field(512)
            .field_limit("avatar", 128)
            .allowed_fields(["avatar"])
            .into_constraints();

        let debug = format!("{constraints:?}");
        assert!(debug.contains("avatar"));
        assert!(debug.contains("2048"));
        assert!(debug.contains("512"));
    }
}
