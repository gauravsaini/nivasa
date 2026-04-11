//! Upload contract types for future multipart support.
//!
//! This module models uploaded files, multipart limits, and upload-layer
//! interceptors without wiring multipart decoding into the main request
//! pipeline. Parsing still stays at the upload helper layer so the SCXML-driven
//! HTTP flow keeps control of request execution.
//!
//! # Examples
//!
//! ```rust
//! use nivasa_http::upload::{FileInterceptor, MultipartLimits, UploadedFile};
//!
//! let file = UploadedFile::new("avatar.png", Some("image/png".to_string()), vec![1, 2, 3]);
//! assert_eq!(file.filename(), "avatar.png");
//! assert_eq!(file.len(), 3);
//!
//! let limits = MultipartLimits::new()
//!     .allowed_fields(["avatar"])
//!     .allowed_mime_types(["image/png"])
//!     .whole_stream(1024)
//!     .per_field(256);
//!
//! let _interceptor = FileInterceptor::new("avatar").with_limits(limits);
//! ```
//!
//! ```rust
//! use nivasa_http::upload::{FileInterceptor, FilesInterceptor};
//!
//! fn multipart_body(parts: &[(&str, &str, Option<&str>, &[u8])]) -> (String, Vec<u8>) {
//!     let boundary = "X-BOUNDARY";
//!     let mut body = Vec::new();
//!
//!     for (field_name, filename, content_type, bytes) in parts {
//!         body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
//!         body.extend_from_slice(
//!             format!(
//!                 "Content-Disposition: form-data; name=\"{field_name}\"; filename=\"{filename}\"\r\n"
//!             )
//!             .as_bytes(),
//!         );
//!         if let Some(content_type) = content_type {
//!             body.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
//!         }
//!         body.extend_from_slice(b"\r\n");
//!         body.extend_from_slice(bytes);
//!         body.extend_from_slice(b"\r\n");
//!     }
//!
//!     body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
//!     (format!("multipart/form-data; boundary={boundary}"), body)
//! }
//!
//! let (content_type, body) =
//!     multipart_body(&[("avatar", "avatar.png", Some("image/png"), b"png-data")]);
//!
//! let file = FileInterceptor::new("avatar")
//!     .extract_from_bytes(&content_type, &body)
//!     .expect("single file should parse");
//! assert_eq!(file.filename(), "avatar.png");
//!
//! let (content_type, body) = multipart_body(&[
//!     ("attachments", "one.txt", Some("text/plain"), b"first"),
//!     ("attachments", "two.txt", Some("text/plain"), b"second"),
//! ]);
//!
//! let files = FilesInterceptor::new("attachments")
//!     .extract_from_bytes(&content_type, &body)
//!     .expect("multiple files should parse");
//! assert_eq!(files.len(), 2);
//! ```

use std::collections::BTreeMap;
use std::fmt;

/// Buffered file payload extracted from a multipart request.
///
/// ```rust
/// use nivasa_http::upload::UploadedFile;
///
/// let file = UploadedFile::new("avatar.png", Some("image/png".to_string()), vec![1, 2, 3]);
/// assert_eq!(file.filename(), "avatar.png");
/// assert_eq!(file.content_type(), Some("image/png"));
/// assert_eq!(file.len(), 3);
/// assert_eq!(file.into_parts(), ("avatar.png".to_string(), Some("image/png".to_string()), vec![1, 2, 3]));
/// ```
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
///
/// ```rust
/// use nivasa_http::upload::MultipartLimits;
///
/// let limits = MultipartLimits::new()
///     .allowed_fields(["avatar", "attachments"])
///     .allowed_mime_types(["image/png", "image/jpeg"])
///     .whole_stream(1024)
///     .per_field(256)
///     .field_limit("avatar", 128);
///
/// assert_eq!(limits.allowed_fields_list(), &["avatar".to_string(), "attachments".to_string()]);
/// assert!(limits.allows_mime_type(Some("image/png")));
/// assert!(!limits.allows_mime_type(Some("text/plain")));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MultipartLimits {
    whole_stream: Option<u64>,
    per_field: Option<u64>,
    field_limits: BTreeMap<String, u64>,
    allowed_fields: Vec<String>,
    allowed_mime_types: Vec<String>,
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

    /// Restrict multipart parsing to a known set of MIME types.
    pub fn allowed_mime_types<I, S>(mut self, mime_types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_mime_types = mime_types.into_iter().map(Into::into).collect();
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

    /// Borrow the allowed MIME type list.
    pub fn allowed_mime_types_list(&self) -> &[String] {
        &self.allowed_mime_types
    }

    /// Whether the provided MIME type is allowed by this configuration.
    pub fn allows_mime_type(&self, mime_type: Option<&str>) -> bool {
        if self.allowed_mime_types.is_empty() {
            return true;
        }

        mime_type
            .map(|mime_type| self.allowed_mime_types.iter().any(|item| item == mime_type))
            .unwrap_or(false)
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

/// Errors raised while extracting uploaded files from a multipart payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadInterceptError {
    MissingMultipartBoundary,
    InvalidMultipart(String),
    UnknownField {
        name: String,
    },
    MissingFile {
        field: String,
    },
    TooManyFiles {
        field: String,
        count: usize,
    },
    FieldTooLarge {
        field: String,
        limit: u64,
        actual: usize,
    },
    StreamTooLarge {
        limit: u64,
        actual: usize,
    },
    DisallowedMimeType {
        mime_type: Option<String>,
    },
}

impl fmt::Display for UploadInterceptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingMultipartBoundary => {
                write!(f, "missing multipart boundary in content type")
            }
            Self::InvalidMultipart(error) => write!(f, "invalid multipart payload: {error}"),
            Self::UnknownField { name } => write!(f, "multipart field `{name}` is not allowed"),
            Self::MissingFile { field } => write!(f, "missing uploaded file for field `{field}`"),
            Self::TooManyFiles { field, count } => {
                write!(f, "expected one file for field `{field}`, found {count}")
            }
            Self::FieldTooLarge {
                field,
                limit,
                actual,
            } => write!(
                f,
                "multipart field `{field}` exceeded size limit {limit} bytes with {actual} bytes"
            ),
            Self::StreamTooLarge { limit, actual } => write!(
                f,
                "multipart payload exceeded size limit {limit} bytes with {actual} bytes"
            ),
            Self::DisallowedMimeType { mime_type } => match mime_type {
                Some(mime_type) => write!(f, "multipart MIME type `{mime_type}` is not allowed"),
                None => write!(f, "multipart file is missing an allowed MIME type"),
            },
        }
    }
}

impl std::error::Error for UploadInterceptError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedMultipartFile {
    field_name: String,
    file: UploadedFile,
}

/// Upload-layer helper for extracting a single file from a multipart payload.
///
/// ```rust
/// use nivasa_http::upload::FileInterceptor;
///
/// fn multipart_body(parts: &[(&str, &str, Option<&str>, &[u8])]) -> (String, Vec<u8>) {
///     let boundary = "X-BOUNDARY";
///     let mut body = Vec::new();
///
///     for (field_name, filename, content_type, bytes) in parts {
///         body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
///         body.extend_from_slice(
///             format!(
///                 "Content-Disposition: form-data; name=\"{field_name}\"; filename=\"{filename}\"\r\n"
///             )
///             .as_bytes(),
///         );
///         if let Some(content_type) = content_type {
///             body.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
///         }
///         body.extend_from_slice(b"\r\n");
///         body.extend_from_slice(bytes);
///         body.extend_from_slice(b"\r\n");
///     }
///
///     body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
///     (format!("multipart/form-data; boundary={boundary}"), body)
/// }
///
/// let (content_type, body) =
///     multipart_body(&[("avatar", "avatar.png", Some("image/png"), b"png-data")]);
///
/// let file = FileInterceptor::new("avatar")
///     .extract_from_bytes(&content_type, &body)
///     .expect("single file should parse");
///
/// assert_eq!(file.filename(), "avatar.png");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInterceptor {
    field_name: String,
    limits: MultipartLimits,
}

impl FileInterceptor {
    /// Create a new single-file interceptor for a field name.
    pub fn new(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            limits: MultipartLimits::new(),
        }
    }

    /// Apply multipart limits to this interceptor.
    pub fn with_limits(mut self, limits: MultipartLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Extract a single file from a multipart payload body.
    pub fn extract_from_bytes(
        &self,
        content_type: &str,
        body: &[u8],
    ) -> Result<UploadedFile, UploadInterceptError> {
        let files = parse_uploaded_files(content_type, body, &self.limits)?
            .into_iter()
            .filter(|part| part.field_name == self.field_name)
            .map(|part| part.file)
            .collect::<Vec<_>>();

        match files.len() {
            0 => Err(UploadInterceptError::MissingFile {
                field: self.field_name.clone(),
            }),
            1 => Ok(files.into_iter().next().expect("single file must exist")),
            count => Err(UploadInterceptError::TooManyFiles {
                field: self.field_name.clone(),
                count,
            }),
        }
    }
}

/// Upload-layer helper for extracting multiple files from a multipart payload.
///
/// ```rust
/// use nivasa_http::upload::FilesInterceptor;
///
/// fn multipart_body(parts: &[(&str, &str, Option<&str>, &[u8])]) -> (String, Vec<u8>) {
///     let boundary = "X-BOUNDARY";
///     let mut body = Vec::new();
///
///     for (field_name, filename, content_type, bytes) in parts {
///         body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
///         body.extend_from_slice(
///             format!(
///                 "Content-Disposition: form-data; name=\"{field_name}\"; filename=\"{filename}\"\r\n"
///             )
///             .as_bytes(),
///         );
///         if let Some(content_type) = content_type {
///             body.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
///         }
///         body.extend_from_slice(b"\r\n");
///         body.extend_from_slice(bytes);
///         body.extend_from_slice(b"\r\n");
///     }
///
///     body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
///     (format!("multipart/form-data; boundary={boundary}"), body)
/// }
///
/// let (content_type, body) = multipart_body(&[
///     ("attachments", "one.txt", Some("text/plain"), b"first"),
///     ("attachments", "two.txt", Some("text/plain"), b"second"),
/// ]);
///
/// let files = FilesInterceptor::new("attachments")
///     .extract_from_bytes(&content_type, &body)
///     .expect("multiple files should parse");
///
/// assert_eq!(files.len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesInterceptor {
    field_name: String,
    limits: MultipartLimits,
}

impl FilesInterceptor {
    /// Create a new multi-file interceptor for a field name.
    pub fn new(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            limits: MultipartLimits::new(),
        }
    }

    /// Apply multipart limits to this interceptor.
    pub fn with_limits(mut self, limits: MultipartLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Extract all files for a field from a multipart payload body.
    pub fn extract_from_bytes(
        &self,
        content_type: &str,
        body: &[u8],
    ) -> Result<Vec<UploadedFile>, UploadInterceptError> {
        let files = parse_uploaded_files(content_type, body, &self.limits)?
            .into_iter()
            .filter(|part| part.field_name == self.field_name)
            .map(|part| part.file)
            .collect::<Vec<_>>();

        if files.is_empty() {
            return Err(UploadInterceptError::MissingFile {
                field: self.field_name.clone(),
            });
        }

        Ok(files)
    }
}

fn parse_uploaded_files(
    content_type: &str,
    body: &[u8],
    limits: &MultipartLimits,
) -> Result<Vec<ParsedMultipartFile>, UploadInterceptError> {
    let boundary = multer::parse_boundary(content_type)
        .map_err(|_| UploadInterceptError::MissingMultipartBoundary)?;
    let whole_size = body.len();
    if let Some(limit) = limits.whole_stream_limit() {
        if whole_size > limit as usize {
            return Err(UploadInterceptError::StreamTooLarge {
                limit,
                actual: whole_size,
            });
        }
    }

    let boundary_marker = format!("--{boundary}");
    let mut files = Vec::new();

    for part in multipart_sections(body, boundary_marker.as_bytes())? {
        let (headers, content) = split_once_bytes(part, b"\r\n\r\n").ok_or_else(|| {
            UploadInterceptError::InvalidMultipart("missing header separator".to_string())
        })?;
        let mut field_name = None;
        let mut filename = None;
        let mut content_type = None;

        let headers = std::str::from_utf8(headers).map_err(|_| {
            UploadInterceptError::InvalidMultipart(
                "multipart headers must be valid UTF-8".to_string(),
            )
        })?;

        for header in headers.split("\r\n") {
            let (name, value) = header.split_once(':').ok_or_else(|| {
                UploadInterceptError::InvalidMultipart(format!("invalid header line `{header}`"))
            })?;
            let name = name.trim();
            let value = value.trim();

            if name.eq_ignore_ascii_case("content-disposition") {
                for segment in value.split(';').skip(1) {
                    let segment = segment.trim();
                    if let Some(value) = segment.strip_prefix("name=") {
                        field_name = Some(value.trim_matches('"').to_string());
                    } else if let Some(value) = segment.strip_prefix("filename=") {
                        filename = Some(value.trim_matches('"').to_string());
                    }
                }
            } else if name.eq_ignore_ascii_case("content-type") {
                content_type = Some(value.to_string());
            }
        }

        let Some(field_name) = field_name else {
            return Err(UploadInterceptError::InvalidMultipart(
                "multipart field is missing a name".to_string(),
            ));
        };
        let Some(filename) = filename else {
            continue;
        };

        if !limits.allowed_fields_list().is_empty()
            && !limits
                .allowed_fields_list()
                .iter()
                .any(|field| field == &field_name)
        {
            return Err(UploadInterceptError::UnknownField { name: field_name });
        }

        if !limits.allows_mime_type(content_type.as_deref()) {
            return Err(UploadInterceptError::DisallowedMimeType {
                mime_type: content_type,
            });
        }

        let bytes = content.to_vec();
        let actual = bytes.len();
        let per_field_limit = limits
            .field_limit_for(&field_name)
            .or_else(|| limits.per_field_limit());

        if let Some(limit) = per_field_limit {
            if actual > limit as usize {
                return Err(UploadInterceptError::FieldTooLarge {
                    field: field_name,
                    limit,
                    actual,
                });
            }
        }

        files.push(ParsedMultipartFile {
            field_name,
            file: UploadedFile::new(filename, content_type, bytes),
        });
    }

    Ok(files)
}

fn multipart_sections<'a>(
    body: &'a [u8],
    boundary_marker: &[u8],
) -> Result<Vec<&'a [u8]>, UploadInterceptError> {
    let mut rest = body;
    let mut sections = Vec::new();

    while let Some(index) = find_bytes(rest, boundary_marker) {
        rest = &rest[index + boundary_marker.len()..];

        if rest.starts_with(b"--") {
            break;
        }

        if let Some(stripped) = rest.strip_prefix(b"\r\n") {
            rest = stripped;
        }

        let Some(next_boundary) = find_bytes(rest, boundary_marker) else {
            return Err(UploadInterceptError::InvalidMultipart(
                "multipart payload is missing a closing boundary".to_string(),
            ));
        };

        let mut part = &rest[..next_boundary];
        if let Some(stripped) = part.strip_suffix(b"\r\n") {
            part = stripped;
        }
        sections.push(part);
        rest = &rest[next_boundary..];
    }

    Ok(sections)
}

fn split_once_bytes<'a>(bytes: &'a [u8], delimiter: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
    let index = find_bytes(bytes, delimiter)?;
    Some((&bytes[..index], &bytes[index + delimiter.len()..]))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::{
        FileInterceptor, FilesInterceptor, MultipartLimits, UploadInterceptError, UploadedFile,
    };

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
    fn multipart_limits_track_allowed_mime_types() {
        let limits = MultipartLimits::new().allowed_mime_types(["image/png", "image/jpeg"]);

        assert_eq!(
            limits.allowed_mime_types_list(),
            &["image/png".to_string(), "image/jpeg".to_string()]
        );
        assert!(limits.allows_mime_type(Some("image/png")));
        assert!(!limits.allows_mime_type(Some("text/plain")));
        assert!(!limits.allows_mime_type(None));
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

    fn multipart_body(parts: &[(&str, &str, Option<&str>, &[u8])]) -> (String, Vec<u8>) {
        let boundary = "X-BOUNDARY";
        let mut body = Vec::new();

        for (field_name, filename, content_type, bytes) in parts {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{field_name}\"; filename=\"{filename}\"\r\n"
                )
                .as_bytes(),
            );
            if let Some(content_type) = content_type {
                body.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
            }
            body.extend_from_slice(b"\r\n");
            body.extend_from_slice(bytes);
            body.extend_from_slice(b"\r\n");
        }

        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        (format!("multipart/form-data; boundary={boundary}"), body)
    }

    #[test]
    fn file_interceptor_extracts_a_single_uploaded_file() {
        let (content_type, body) =
            multipart_body(&[("avatar", "avatar.png", Some("image/png"), b"png-data")]);

        let file = FileInterceptor::new("avatar")
            .extract_from_bytes(&content_type, &body)
            .expect("single file should parse");

        assert_eq!(file.filename(), "avatar.png");
        assert_eq!(file.content_type(), Some("image/png"));
        assert_eq!(file.bytes(), b"png-data");
    }

    #[test]
    fn files_interceptor_extracts_multiple_files_for_one_field() {
        let (content_type, body) = multipart_body(&[
            ("attachments", "one.txt", Some("text/plain"), b"first"),
            ("attachments", "two.txt", Some("text/plain"), b"second"),
        ]);

        let files = FilesInterceptor::new("attachments")
            .extract_from_bytes(&content_type, &body)
            .expect("multiple files should parse");

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].filename(), "one.txt");
        assert_eq!(files[1].filename(), "two.txt");
    }

    #[test]
    fn file_interceptor_rejects_disallowed_mime_types() {
        let (content_type, body) =
            multipart_body(&[("avatar", "avatar.txt", Some("text/plain"), b"text-data")]);

        let error = FileInterceptor::new("avatar")
            .with_limits(MultipartLimits::new().allowed_mime_types(["image/png"]))
            .extract_from_bytes(&content_type, &body)
            .expect_err("unexpected mime type should be rejected");

        assert_eq!(
            error,
            UploadInterceptError::DisallowedMimeType {
                mime_type: Some("text/plain".to_string()),
            }
        );
    }

    #[test]
    fn files_interceptor_enforces_per_field_limits() {
        let (content_type, body) =
            multipart_body(&[("attachments", "large.bin", None, b"1234567890")]);

        let error = FilesInterceptor::new("attachments")
            .with_limits(MultipartLimits::new().per_field(4))
            .extract_from_bytes(&content_type, &body)
            .expect_err("oversized field should be rejected");

        assert_eq!(
            error,
            UploadInterceptError::FieldTooLarge {
                field: "attachments".to_string(),
                limit: 4,
                actual: 10,
            }
        );
    }
}
