//! Integration boundary for Monad Execution Events SDK crates.
//!
//! This crate owns the boundary to the pinned upstream `monad-event-ring` and
//! `monad-exec-events` SDK crates.

mod abi;
mod commit_state;
mod defi;
mod fixtures;
mod flow;
mod health;
#[cfg(feature = "live")]
mod live;
mod normalize;
mod raw;
mod replay;
#[cfg(all(feature = "sdk", target_os = "linux"))]
mod sdk_live;
mod snapshot;
mod source;
mod stream;

pub use abi::{
    AbiDecoder, AbiEventDefinition, AbiEventInput, AbiValue, DecodedAbiEvent, DecodedAbiField,
};
pub use commit_state::{CommitStateIssue, CommitStateTracker, TrackedBlockState};
pub use defi::{
    decode_basic_defi_log, event_topic, DeFiEvent, DexSwap, DexSwapKind, DexSync, Erc20Approval,
    Erc20Transfer, UnknownLog, ERC20_APPROVAL_SIGNATURE, ERC20_TRANSFER_SIGNATURE,
    UNISWAP_V2_SWAP_SIGNATURE, UNISWAP_V2_SYNC_SIGNATURE, UNISWAP_V3_SWAP_SIGNATURE,
};
pub use fixtures::{
    fixture_path, fixture_root, fixture_stream_items, load_fixture, load_golden,
    load_workspace_fixture, FixtureDocument, FixtureReport,
};
pub use flow::{
    TransactionBundle, TransactionFlowKey, TransactionFlowSummary, TransactionFlowUpdate,
    TxnFlowTracker,
};
pub use health::{
    action_for_gap_policy, GapObserver, GapPolicyOverride, SequenceTracker, StreamHealthAction,
    StreamHealthTracker, StreamHealthWarningSummary,
};
#[cfg(feature = "live")]
pub use live::{
    bounded_live_channel, default_event_ring_dir, host_supports_live_event_ring,
    live_availability_reason, live_gap_policy_action, normalize_live_stream_item,
    observe_fake_stream, parse_duration_millis, resolve_event_ring_path, LiveConfig,
    LiveEventRingSource, LiveExecutionEventStream, LiveGapAction, LiveMetrics,
    DEFAULT_LIVE_RING_NAME,
};
pub use normalize::{
    fixture_log_payload, normalize_raw_event, normalize_stream_item, AccountAccessEvent,
    BlockEvent, CallFrameEvent, ChainEvent, CommitStateEvent, LogEvent, StorageAccessEvent,
    TransactionEvent, TxnOutputEvent, UnknownChainEvent,
};
pub use raw::{
    fixture_block_tag_payload, fixture_block_verified_payload, fixture_raw_envelope,
    fixture_snapshot_descriptor, fixture_txn_header_start_payload, flow_tags_from_content_ext,
    raw_event_from_snapshot, ExecEventType, RawBlockStart, RawBlockStateEvent, RawBlockTag,
    RawBlockVerified, RawExecDescriptor, RawExecEvent, RawKnownEvent, RawTxnHeaderStart,
    RawUnknownExecEvent,
};
pub use replay::{event_matches_filter, ReplayConfig, ReplayFilter, ReplayRun, ReplayRunner};
pub use snapshot::{
    SnapshotDescriptor, SnapshotOpenOptions, SnapshotReader, SnapshotSource, SnapshotSummary,
};
pub use source::{
    map_sdk_error, schema_mismatch_stream_item, validate_readable_path, ContentTypeValidation,
    ExecEventSource, SchemaPolicy, SchemaValidation, SourceInfo, EXPECTED_EXEC_CONTENT_TYPE,
};
pub use stream::{
    collect_execution_stream, compare_stream_parity, execution_stream_report, ExecutionEventPoller,
    ExecutionEventStream, StreamParityComparison, StreamParityReport, VecExecutionEventStream,
};

/// Git repository that provides the pinned Monad Execution Events Rust SDK.
pub const SDK_REPOSITORY: &str = "https://github.com/category-labs/monad";

/// Git tag used for the pinned Monad Execution Events Rust SDK.
pub const SDK_TAG: &str = "release/exec-events-sdk-v1.1";

/// Commit resolved by the upstream baseline [`SDK_TAG`].
pub const SDK_TAG_COMMIT: &str = "b7c13e1565f40556cb717090eae245e34bb5c6e7";

/// Exact upstream revision used by active Cargo dependencies.
pub const SDK_REVISION: &str = "4f2289307196a1b70dfa1fb5282600a07ca40767";

/// Static metadata for the pinned Monad Execution Events Rust SDK.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct SdkMetadata {
    /// Upstream Git repository URL.
    pub repository: &'static str,
    /// Upstream Git tag.
    pub tag: &'static str,
    /// Commit resolved by the upstream baseline tag.
    pub tag_commit: &'static str,
    /// Exact revision compiled by the active SDK feature.
    pub revision: &'static str,
}

/// Returns the pinned SDK metadata used by this crate.
#[must_use]
pub const fn sdk_metadata() -> SdkMetadata {
    SdkMetadata {
        repository: SDK_REPOSITORY,
        tag: SDK_TAG,
        tag_commit: SDK_TAG_COMMIT,
        revision: SDK_REVISION,
    }
}

/// Cargo dependency snippet for the pinned SDK crates.
///
/// This mirrors the active target-specific declarations in `Cargo.toml`.
pub const SDK_DEPENDENCY_SNIPPET: &str = r#"[dependencies]
monad-event-ring = { git = "https://github.com/category-labs/monad", rev = "4f2289307196a1b70dfa1fb5282600a07ca40767", optional = true }
monad-exec-events = { git = "https://github.com/category-labs/monad", rev = "4f2289307196a1b70dfa1fb5282600a07ca40767", optional = true }

[features]
default = []
sdk = ["dep:monad-event-ring", "dep:monad-exec-events"]
"#;

/// Explains the active SDK integration.
#[must_use]
#[cfg(feature = "sdk")]
pub const fn sdk_feature_hint() -> &'static str {
    "the `sdk` feature enables the exact pinned Monad SDK revision on Linux"
}

/// Explains how to enable the upstream SDK-backed APIs in this crate.
#[must_use]
#[cfg(not(feature = "sdk"))]
pub const fn sdk_feature_hint() -> &'static str {
    "enable the `sdk` feature on Linux to compile the pinned Monad SDK integration"
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
        assert_eq!(
            metadata.tag_commit,
            "b7c13e1565f40556cb717090eae245e34bb5c6e7"
        );
        assert_eq!(
            metadata.revision,
            "4f2289307196a1b70dfa1fb5282600a07ca40767"
        );
    }

    #[test]
    fn sdk_feature_is_documented() {
        assert!(sdk_feature_hint().contains("sdk"));
    }

    #[test]
    fn dependency_snippet_pins_execution_repo_v1_1() {
        assert!(SDK_DEPENDENCY_SNIPPET.contains("https://github.com/category-labs/monad"));
        assert!(SDK_DEPENDENCY_SNIPPET.contains(SDK_REVISION));
        assert!(SDK_DEPENDENCY_SNIPPET.contains("monad-event-ring"));
        assert!(SDK_DEPENDENCY_SNIPPET.contains("monad-exec-events"));
    }
}
