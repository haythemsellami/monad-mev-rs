use std::fmt::Display;
use std::path::{Path, PathBuf};

use monad_mev_core::{Error, EventSourceKind, Result, SchemaMismatch, StreamItem, B256};
use serde::{Deserialize, Serialize};

use crate::SdkMetadata;

/// Expected content type for Monad execution event rings.
pub const EXPECTED_EXEC_CONTENT_TYPE: &str = "exec";

/// Metadata discovered from a snapshot, live ring, or fixture source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceInfo {
    /// High-level source kind.
    pub kind: EventSourceKind,
    /// Human-readable source label.
    pub label: String,
    /// Filesystem path, when the source is path-backed.
    pub path: Option<PathBuf>,
    /// Event-ring content type, when known.
    pub content_type: Option<String>,
    /// Event-ring schema hash, when known.
    pub schema_hash: Option<B256>,
    /// SDK metadata expected by this framework build.
    pub sdk: SdkMetadata,
}

impl SourceInfo {
    /// Creates source metadata with the pinned SDK metadata attached.
    #[must_use]
    pub fn new(kind: EventSourceKind, label: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
            path: None,
            content_type: None,
            schema_hash: None,
            sdk: crate::sdk_metadata(),
        }
    }

    /// Creates path-backed source metadata.
    #[must_use]
    pub fn path(kind: EventSourceKind, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let label = path.display().to_string();

        Self {
            kind,
            label,
            path: Some(path),
            content_type: None,
            schema_hash: None,
            sdk: crate::sdk_metadata(),
        }
    }

    /// Attaches content type metadata.
    #[must_use]
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Attaches schema hash metadata.
    #[must_use]
    pub const fn with_schema_hash(mut self, schema_hash: B256) -> Self {
        self.schema_hash = Some(schema_hash);
        self
    }

    /// Validates this source's content type.
    ///
    /// # Errors
    ///
    /// Returns an error when the source content type is missing or does not match `expected`.
    pub fn validate_content_type(&self, expected: &str) -> Result<ContentTypeValidation> {
        validate_content_type(self, expected)
    }

    /// Validates this source's schema hash according to `policy`.
    ///
    /// # Errors
    ///
    /// Returns an error for mismatches when `policy` is [`SchemaPolicy::RequireMatch`].
    pub fn validate_schema(
        &self,
        expected: B256,
        policy: SchemaPolicy,
    ) -> Result<SchemaValidation> {
        validate_schema(self, expected, policy)
    }
}

/// Minimal common behavior for execution-event sources.
pub trait ExecEventSource {
    /// Returns discovered source metadata.
    fn source_info(&self) -> &SourceInfo;

    /// Validates source content type and schema.
    ///
    /// # Errors
    ///
    /// Returns an error when content-type validation fails, or when schema validation fails under
    /// [`SchemaPolicy::RequireMatch`].
    fn validate_source(
        &self,
        expected_schema_hash: B256,
        schema_policy: SchemaPolicy,
    ) -> Result<SchemaValidation> {
        self.source_info()
            .validate_content_type(EXPECTED_EXEC_CONTENT_TYPE)?;
        self.source_info()
            .validate_schema(expected_schema_hash, schema_policy)
    }
}

/// Source content-type validation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentTypeValidation {
    /// Expected content type.
    pub expected: String,
    /// Observed content type.
    pub observed: String,
}

/// Schema validation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaPolicy {
    /// Missing or mismatched schema hash is fatal.
    RequireMatch,
    /// Missing or mismatched schema hash is returned as a warning.
    Warn,
    /// Do not compare schema hashes. Intended only for tests and debugging.
    SkipCheck,
}

/// Schema validation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaValidation {
    /// Observed schema hash matched expected hash.
    Match,
    /// Schema mismatch was observed but allowed by policy.
    Warning(SchemaMismatch),
    /// Schema comparison was skipped.
    Skipped,
}

/// Validates source content type.
///
/// # Errors
///
/// Returns an error when the content type is missing or mismatched.
pub fn validate_content_type(source: &SourceInfo, expected: &str) -> Result<ContentTypeValidation> {
    let observed = source.content_type.as_deref().ok_or_else(|| {
        Error::Message(format!(
            "content type missing on {} source {:?}; expected {expected}",
            source.kind, source.label
        ))
    })?;

    if observed != expected {
        return Err(Error::Message(format!(
            "content type mismatch on {} source {:?}: expected {expected}, observed {observed}",
            source.kind, source.label
        )));
    }

    Ok(ContentTypeValidation {
        expected: expected.to_owned(),
        observed: observed.to_owned(),
    })
}

/// Validates source schema hash according to `policy`.
///
/// # Errors
///
/// Returns an error for mismatches when `policy` is [`SchemaPolicy::RequireMatch`].
pub fn validate_schema(
    source: &SourceInfo,
    expected: B256,
    policy: SchemaPolicy,
) -> Result<SchemaValidation> {
    if matches!(policy, SchemaPolicy::SkipCheck) {
        return Ok(SchemaValidation::Skipped);
    }

    if source.schema_hash == Some(expected) {
        return Ok(SchemaValidation::Match);
    }

    let mismatch = schema_mismatch(source, expected);

    match policy {
        SchemaPolicy::RequireMatch => Err(mismatch.into()),
        SchemaPolicy::Warn => Ok(SchemaValidation::Warning(mismatch)),
        SchemaPolicy::SkipCheck => Ok(SchemaValidation::Skipped),
    }
}

/// Builds a schema mismatch value for a source.
#[must_use]
pub fn schema_mismatch(source: &SourceInfo, expected: B256) -> SchemaMismatch {
    SchemaMismatch {
        expected,
        observed: source.schema_hash,
        source: source.kind.clone(),
    }
}

/// Builds a stream item carrying a schema mismatch for this source.
#[must_use]
pub fn schema_mismatch_stream_item<T>(source: &SourceInfo, expected: B256) -> StreamItem<T> {
    StreamItem::SchemaMismatch(schema_mismatch(source, expected))
}

/// Validates that a source path exists and is a file.
///
/// # Errors
///
/// Returns an error when the path cannot be read or is not a file.
pub fn validate_readable_path(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let metadata = std::fs::metadata(path).map_err(|err| {
        Error::Message(format!(
            "source path is not readable {}: {err}",
            path.display()
        ))
    })?;

    if !metadata.is_file() {
        return Err(Error::Message(format!(
            "source path is not a file: {}",
            path.display()
        )));
    }

    Ok(())
}

/// Converts an upstream SDK error into a framework error.
#[must_use]
pub fn map_sdk_error(error: impl Display) -> Error {
    Error::Message(format!("Monad Execution Events SDK error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct FakeSource {
        info: SourceInfo,
    }

    impl ExecEventSource for FakeSource {
        fn source_info(&self) -> &SourceInfo {
            &self.info
        }
    }

    fn hash(byte: u8) -> B256 {
        B256::from([byte; 32])
    }

    fn source(schema_hash: B256) -> SourceInfo {
        SourceInfo::new(EventSourceKind::Snapshot, "fixture")
            .with_content_type(EXPECTED_EXEC_CONTENT_TYPE)
            .with_schema_hash(schema_hash)
    }

    #[test]
    fn matching_schema_hash_passes() {
        let info = source(hash(1));

        let result = info
            .validate_schema(hash(1), SchemaPolicy::RequireMatch)
            .expect("matching schema should pass");

        assert_eq!(result, SchemaValidation::Match);
    }

    #[test]
    fn mismatched_schema_hash_errors_when_required() {
        let info = source(hash(2));

        let error = info
            .validate_schema(hash(1), SchemaPolicy::RequireMatch)
            .expect_err("mismatch should fail");

        assert!(error.to_string().contains("schema mismatch"));
        assert!(error.to_string().contains(&hash(1).to_string()));
        assert!(error.to_string().contains(&hash(2).to_string()));
    }

    #[test]
    fn mismatched_schema_hash_can_warn() {
        let info = source(hash(2));

        let result = info
            .validate_schema(hash(1), SchemaPolicy::Warn)
            .expect("warn policy should not fail");

        assert_eq!(
            result,
            SchemaValidation::Warning(SchemaMismatch {
                expected: hash(1),
                observed: Some(hash(2)),
                source: EventSourceKind::Snapshot,
            })
        );
    }

    #[test]
    fn schema_check_can_be_skipped() {
        let info = SourceInfo::new(EventSourceKind::Fixture, "fixture");

        let result = info
            .validate_schema(hash(1), SchemaPolicy::SkipCheck)
            .expect("skip policy should pass");

        assert_eq!(result, SchemaValidation::Skipped);
    }

    #[test]
    fn schema_mismatch_can_be_stream_item() {
        let info = source(hash(2));

        let item = schema_mismatch_stream_item::<()>(&info, hash(1));

        assert_eq!(
            item,
            StreamItem::SchemaMismatch(SchemaMismatch {
                expected: hash(1),
                observed: Some(hash(2)),
                source: EventSourceKind::Snapshot,
            })
        );
    }

    #[test]
    fn wrong_content_type_errors() {
        let info = SourceInfo::new(EventSourceKind::Snapshot, "fixture")
            .with_content_type("not-exec")
            .with_schema_hash(hash(1));

        let error = info
            .validate_content_type(EXPECTED_EXEC_CONTENT_TYPE)
            .expect_err("wrong content type should fail");

        assert!(error.to_string().contains("content type mismatch"));
        assert!(error.to_string().contains("expected exec"));
        assert!(error.to_string().contains("observed not-exec"));
    }

    #[test]
    fn missing_content_type_errors() {
        let info = SourceInfo::new(EventSourceKind::Snapshot, "fixture");

        let error = info
            .validate_content_type(EXPECTED_EXEC_CONTENT_TYPE)
            .expect_err("missing content type should fail");

        assert!(error.to_string().contains("content type missing"));
        assert!(error.to_string().contains("expected exec"));
    }

    #[test]
    fn missing_source_path_errors() {
        let missing =
            std::env::temp_dir().join(format!("monad-mev-rs-missing-{}", std::process::id()));

        let error = validate_readable_path(&missing).expect_err("missing path should fail");

        assert!(error.to_string().contains("source path is not readable"));
        assert!(error.to_string().contains(&missing.display().to_string()));
    }

    #[test]
    fn fake_source_validates_content_and_schema() {
        let source = FakeSource {
            info: self::source(hash(1)),
        };

        let result = source
            .validate_source(hash(1), SchemaPolicy::RequireMatch)
            .expect("source should validate");

        assert_eq!(result, SchemaValidation::Match);
    }

    #[test]
    fn sdk_errors_map_to_framework_error() {
        let error = map_sdk_error("bad descriptor");

        assert!(error
            .to_string()
            .contains("Monad Execution Events SDK error"));
        assert!(error.to_string().contains("bad descriptor"));
    }
}
