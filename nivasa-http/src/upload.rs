//! Upload contract types for future multipart support.
//!
//! This module intentionally models uploaded files without performing any
//! request parsing. Multipart decoding still belongs to the SCXML-driven HTTP
//! pipeline once the multipart dependency and interceptor wiring are landed.

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
}

#[cfg(test)]
mod tests {
    use super::UploadedFile;

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
}
