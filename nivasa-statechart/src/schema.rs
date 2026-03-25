//! W3C SCXML XSD validation support.
//!
//! This module validates `.scxml` files against the vendored W3C SCXML schema
//! tree before semantic validation and code generation run.

use libxml::error::{StructuredError, XmlErrorLevel};
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};
use std::fmt;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Root of the vendored W3C SCXML schema tree.
pub fn scxml_schema_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scxml_schema/w3c")
}

/// Path to the W3C SCXML schema driver file.
pub fn scxml_schema_file() -> PathBuf {
    scxml_schema_root().join("scxml.xsd")
}

/// A single XSD diagnostic emitted by libxml2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDiagnostic {
    pub level: String,
    pub message: String,
    pub filename: Option<String>,
    pub line: Option<i32>,
    pub column: Option<i32>,
    pub domain: i32,
    pub code: i32,
}

impl fmt::Display for SchemaDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(filename) = &self.filename {
            write!(f, "{filename}")?;
            if self.line.is_some() || self.column.is_some() {
                write!(f, ":")?;
            }
        }

        if let Some(line) = self.line {
            write!(f, "{line}")?;
        }

        if let Some(column) = self.column {
            write!(f, ":{column}")?;
        }

        if self.filename.is_some() || self.line.is_some() || self.column.is_some() {
            write!(f, ": ")?;
        }

        write!(
            f,
            "{} (level={}, domain={}, code={})",
            self.message, self.level, self.domain, self.code
        )
    }
}

/// A collection of diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDiagnostics(pub Vec<SchemaDiagnostic>);

impl fmt::Display for SchemaDiagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.0.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{diagnostic}")?;
        }
        Ok(())
    }
}

/// Errors produced by schema validation.
#[derive(Debug, Error)]
pub enum SchemaValidationError {
    #[error("failed to load W3C SCXML XSD schema from {schema_path}: {diagnostics}")]
    SchemaLoad {
        schema_path: PathBuf,
        diagnostics: SchemaDiagnostics,
    },
    #[error("SCXML file {path} is not valid against the W3C SCXML XSD schema: {diagnostics}")]
    Invalid {
        path: PathBuf,
        diagnostics: SchemaDiagnostics,
    },
}

/// Validate an SCXML file against the vendored W3C SCXML XSD tree.
pub fn validate_scxml_schema(path: impl AsRef<Path>) -> Result<(), SchemaValidationError> {
    let path = path.as_ref();
    let schema_path = scxml_schema_file();
    let schema_path_string = schema_path.to_string_lossy().into_owned();

    let mut parser = SchemaParserContext::from_file(&schema_path_string);
    let mut validator = SchemaValidationContext::from_parser(&mut parser).map_err(|errors| {
        SchemaValidationError::SchemaLoad {
            schema_path: schema_path.clone(),
            diagnostics: SchemaDiagnostics(
                errors.into_iter().map(schema_diagnostic_from).collect(),
            ),
        }
    })?;

    let path_string = path.to_string_lossy().into_owned();
    validator
        .validate_file(&path_string)
        .map_err(|errors| SchemaValidationError::Invalid {
            path: path.to_path_buf(),
            diagnostics: SchemaDiagnostics(
                errors.into_iter().map(schema_diagnostic_from).collect(),
            ),
        })
}

fn schema_diagnostic_from(error: StructuredError) -> SchemaDiagnostic {
    let level = match error.level {
        XmlErrorLevel::None => "none",
        XmlErrorLevel::Warning => "warning",
        XmlErrorLevel::Error => "error",
        XmlErrorLevel::Fatal => "fatal",
    }
    .to_string();

    SchemaDiagnostic {
        level,
        message: error
            .message
            .unwrap_or_else(|| "SCXML XSD validation error".to_string()),
        filename: error.filename,
        line: error.line,
        column: error.col,
        domain: error.domain,
        code: error.code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_scxml_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("nivasa-{name}-{stamp}.scxml"))
    }

    #[test]
    fn valid_scxml_passes_schema_validation() {
        let path = temp_scxml_path("valid");
        fs::write(
            &path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" version="1.0" initial="a">
  <state id="a">
    <transition event="go" target="b"/>
  </state>
  <final id="b"/>
</scxml>"#,
        )
        .unwrap();

        let result = validate_scxml_schema(&path);
        fs::remove_file(&path).ok();

        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn invalid_scxml_reports_xsd_failure() {
        let path = temp_scxml_path("invalid");
        fs::write(
            &path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" version="1.0" initial="a">
  <state id="a">
    <bogus/>
  </state>
</scxml>"#,
        )
        .unwrap();

        let result = validate_scxml_schema(&path);
        fs::remove_file(&path).ok();

        assert!(matches!(result, Err(SchemaValidationError::Invalid { .. })));
    }
}
