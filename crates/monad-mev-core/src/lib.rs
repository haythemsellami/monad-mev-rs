//! Core framework types for `monad-mev-rs`.
//!
//! This crate owns framework-level types and traits.
//!
//! During WP-03 it only exposes diagnostics metadata. Framework-owned event,
//! replay, strategy, and executor types are implemented in later work packages.

mod error;
mod event;
mod health;
mod replay;

pub use error::{Error, Result};
pub use event::{
    Address, BlockRef, CommitState, EventEnvelope, EventKind, EventMeta, EventSourceKind, FlowTags,
    PayloadMode, RunMode, TxnRef, B256,
};
pub use health::{GapEvent, GapPolicy, PayloadExpired, SchemaMismatch, StreamItem};
pub use replay::{ReplayClock, ReplayReport};

/// Crate version, exposed for diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn commit_state_satisfies_minimum_state() {
        assert!(CommitState::Verified.satisfies_min(CommitState::Finalized));
        assert!(CommitState::Finalized.satisfies_min(CommitState::Voted));
        assert!(CommitState::Proposed.satisfies_min(CommitState::Proposed));
        assert!(CommitState::Unknown.satisfies_min(CommitState::Unknown));
        assert!(CommitState::Abandoned.satisfies_min(CommitState::Abandoned));

        assert!(!CommitState::Proposed.satisfies_min(CommitState::Finalized));
        assert!(!CommitState::Abandoned.satisfies_min(CommitState::Proposed));
        assert!(!CommitState::Finalized.satisfies_min(CommitState::Abandoned));
    }

    #[test]
    fn public_metadata_serializes_with_hex_hashes() {
        let hash = B256::from([1_u8; 32]);
        let envelope = EventEnvelope::new(
            "payload",
            EventMeta {
                seqno: 42,
                record_epoch_nanos: 1000,
                event_kind: EventKind::TxnLog,
                source: EventSourceKind::Snapshot,
                block: Some(BlockRef {
                    block_id: hash,
                    proposed_block_number: 7,
                    block_start_seqno: 40,
                }),
                txn: Some(TxnRef {
                    txn_idx: 3,
                    txn_hash: Some(hash),
                }),
                flow: FlowTags {
                    block_seqno: Some(40),
                    txn_id: Some(4),
                    account_index: None,
                },
                commit_state: CommitState::Finalized,
                schema_hash: Some(hash),
            },
        );

        let json = serde_json::to_value(envelope).expect("envelope should serialize");

        assert_eq!(json["meta"]["seqno"], 42);
        assert_eq!(json["meta"]["event_kind"], "txn_log");
        assert_eq!(json["meta"]["source"], "snapshot");
        assert_eq!(json["meta"]["commit_state"], "finalized");
        assert_eq!(
            json["meta"]["schema_hash"],
            "0x0101010101010101010101010101010101010101010101010101010101010101"
        );
    }

    #[test]
    fn health_events_have_human_readable_display() {
        let gap = GapEvent::new(10, 13, EventSourceKind::Live);
        let expired = PayloadExpired {
            seqno: 20,
            event_kind: Some(EventKind::TxnLog),
            source: EventSourceKind::Snapshot,
        };
        let mismatch = SchemaMismatch {
            expected: B256::from([1_u8; 32]),
            observed: Some(B256::from([2_u8; 32])),
            source: EventSourceKind::Snapshot,
        };

        assert!(gap.to_string().contains("missed 3"));
        assert!(expired.to_string().contains("payload expired"));
        assert!(mismatch.to_string().contains("schema mismatch"));
    }

    #[test]
    fn gap_policy_defaults_match_run_modes() {
        assert_eq!(
            GapPolicy::default_for_run_mode(RunMode::SnapshotReplay),
            GapPolicy::FailFast
        );
        assert_eq!(
            GapPolicy::default_for_run_mode(RunMode::CliInspect),
            GapPolicy::LogAndContinue
        );
        assert_eq!(
            GapPolicy::default_for_run_mode(RunMode::LiveObserve),
            GapPolicy::RiskOffThenFail
        );
        assert_eq!(
            GapPolicy::default_for_run_mode(RunMode::UnitTest),
            GapPolicy::FailFast
        );
    }

    #[test]
    fn replay_report_serializes_stable_counter_names() {
        let mut report = ReplayReport::for_source("snapshot.zst");
        report.record_event();
        report.record_decoded_event();
        report.record_gap();

        let json = serde_json::to_value(report).expect("report should serialize");

        assert_eq!(json["source_path"], "snapshot.zst");
        assert_eq!(json["events_seen"], 1);
        assert_eq!(json["events_decoded"], 1);
        assert_eq!(json["gaps"], 1);
    }
}
