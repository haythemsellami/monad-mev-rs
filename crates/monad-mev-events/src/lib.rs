//! Integration boundary for Monad Execution Events SDK crates.
//!
//! This crate owns the V1 boundary where upstream `monad-event-ring` and
//! `monad-exec-events` dependencies will be activated.

mod raw;
mod snapshot;
mod source;

pub use raw::{
    fixture_block_tag_payload, fixture_block_verified_payload, fixture_raw_envelope,
    fixture_snapshot_descriptor, fixture_txn_header_start_payload, flow_tags_from_content_ext,
    raw_event_from_snapshot, ExecEventType, RawBlockStart, RawBlockStateEvent, RawBlockTag,
    RawBlockVerified, RawExecDescriptor, RawExecEvent, RawKnownEvent, RawTxnHeaderStart,
    RawUnknownExecEvent,
};
pub use snapshot::{
    SnapshotDescriptor, SnapshotOpenOptions, SnapshotReader, SnapshotSource, SnapshotSummary,
};
pub use source::{
    map_sdk_error, schema_mismatch_stream_item, validate_readable_path, ContentTypeValidation,
    ExecEventSource, SchemaPolicy, SchemaValidation, SourceInfo, EXPECTED_EXEC_CONTENT_TYPE,
};

/// Git repository that provides the pinned Monad Execution Events Rust SDK.
pub const SDK_REPOSITORY: &str = "https://github.com/category-labs/monad";

/// Git tag used for the pinned Monad Execution Events Rust SDK.
pub const SDK_TAG: &str = "release/exec-events-sdk-v1.1";

/// Commit currently resolved by [`SDK_TAG`].
pub const SDK_COMMIT: &str = "b7c13e1565f40556cb717090eae245e34bb5c6e7";

/// Static metadata for the pinned Monad Execution Events Rust SDK.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct SdkMetadata {
    /// Upstream Git repository URL.
    pub repository: &'static str,
    /// Upstream Git tag.
    pub tag: &'static str,
    /// Commit resolved by the upstream Git tag when this version was pinned.
    pub commit: &'static str,
}

/// Returns the pinned SDK metadata used by this crate.
#[must_use]
pub const fn sdk_metadata() -> SdkMetadata {
    SdkMetadata {
        repository: SDK_REPOSITORY,
        tag: SDK_TAG,
        commit: SDK_COMMIT,
    }
}

/// Cargo dependency snippet for the pinned SDK crates.
///
/// This is intentionally not active in `Cargo.toml` yet because Cargo fetches
/// the large upstream repository and submodules even when optional git
/// dependencies are not enabled. The active dependency is deferred until the
/// ingestion work packages decide the right fetch/vendor strategy.
pub const SDK_DEPENDENCY_SNIPPET: &str = r#"[dependencies]
monad-event-ring = { git = "https://github.com/category-labs/monad", tag = "release/exec-events-sdk-v1.1", package = "monad-event-ring", optional = true }
monad-exec-events = { git = "https://github.com/category-labs/monad", tag = "release/exec-events-sdk-v1.1", package = "monad-exec-events", optional = true }

[features]
default = []
sdk = ["dep:monad-event-ring", "dep:monad-exec-events"]
"#;

/// Placeholder for SDK-backed APIs that will be enabled once the dependency
/// fetch strategy is finalized.
#[must_use]
#[cfg(feature = "sdk")]
pub const fn sdk_feature_hint() -> &'static str {
    "the `sdk` feature is reserved for the pinned Monad SDK integration; active git dependencies are deferred until SDK fetch/vendor strategy is finalized"
}

/// Explains how to enable the upstream SDK-backed APIs in this crate.
#[must_use]
#[cfg(not(feature = "sdk"))]
pub const fn sdk_feature_hint() -> &'static str {
    "the `sdk` feature is reserved for the pinned Monad SDK integration"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_metadata_is_pinned_to_execution_repo_v1_1() {
        let metadata = sdk_metadata();

        assert_eq!(
            metadata.repository,
            "https://github.com/category-labs/monad"
        );
        assert_eq!(metadata.tag, "release/exec-events-sdk-v1.1");
        assert_eq!(metadata.commit, "b7c13e1565f40556cb717090eae245e34bb5c6e7");
    }

    #[test]
    fn sdk_feature_is_documented() {
        assert!(sdk_feature_hint().contains("sdk"));
    }

    #[test]
    fn dependency_snippet_pins_execution_repo_v1_1() {
        assert!(SDK_DEPENDENCY_SNIPPET.contains("https://github.com/category-labs/monad"));
        assert!(SDK_DEPENDENCY_SNIPPET.contains("release/exec-events-sdk-v1.1"));
        assert!(SDK_DEPENDENCY_SNIPPET.contains("monad-event-ring"));
        assert!(SDK_DEPENDENCY_SNIPPET.contains("monad-exec-events"));
    }
}
