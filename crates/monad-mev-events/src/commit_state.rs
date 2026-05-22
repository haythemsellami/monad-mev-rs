use std::collections::BTreeMap;

use monad_mev_core::{BlockRef, CommitState, EventEnvelope, B256};
use serde::{Deserialize, Serialize};

use crate::RawExecEvent;

/// In-memory state for one block observed in the execution-event stream.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrackedBlockState {
    /// Stable block reference.
    pub block: BlockRef,
    /// Current inferred commit state.
    pub state: CommitState,
    /// Last descriptor sequence number that changed or confirmed this state.
    pub last_transition_seqno: u64,
}

/// Non-fatal issue detected while tracking block state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitStateIssue {
    /// A transition attempted to move a block to a lower state.
    InvalidRegression {
        /// Block ID whose state would regress.
        block_id: B256,
        /// Existing state.
        current: CommitState,
        /// Rejected target state.
        attempted: CommitState,
        /// Descriptor sequence number where the regression was observed.
        seqno: u64,
    },
    /// A consensus event did not contain enough payload metadata to identify a block.
    UnresolvedBlock {
        /// Descriptor sequence number where the unresolved event was observed.
        seqno: u64,
        /// State implied by the event.
        attempted: CommitState,
    },
}

/// Tracks Monad block lifecycle state from raw execution events.
#[derive(Clone, Debug, Default)]
pub struct CommitStateTracker {
    by_id: BTreeMap<B256, TrackedBlockState>,
    by_start_seqno: BTreeMap<u64, B256>,
    by_block_number: BTreeMap<u64, B256>,
    verified_numbers: BTreeMap<u64, u64>,
    issues: Vec<CommitStateIssue>,
    retention_blocks: usize,
}

impl CommitStateTracker {
    /// Creates a tracker with default replay-friendly retention.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            by_id: BTreeMap::new(),
            by_start_seqno: BTreeMap::new(),
            by_block_number: BTreeMap::new(),
            verified_numbers: BTreeMap::new(),
            issues: Vec::new(),
            retention_blocks: 4096,
        }
    }

    /// Creates a tracker with a bounded block retention window.
    #[must_use]
    pub const fn with_retention(retention_blocks: usize) -> Self {
        Self {
            by_id: BTreeMap::new(),
            by_start_seqno: BTreeMap::new(),
            by_block_number: BTreeMap::new(),
            verified_numbers: BTreeMap::new(),
            issues: Vec::new(),
            retention_blocks,
        }
    }

    /// Returns tracked state for a block ID.
    #[must_use]
    pub fn block_by_id(&self, block_id: B256) -> Option<&TrackedBlockState> {
        self.by_id.get(&block_id)
    }

    /// Returns tracked state for a block-start sequence number.
    #[must_use]
    pub fn block_by_start_seqno(&self, block_start_seqno: u64) -> Option<&TrackedBlockState> {
        self.by_start_seqno
            .get(&block_start_seqno)
            .and_then(|block_id| self.by_id.get(block_id))
    }

    /// Returns detected non-fatal issues.
    #[must_use]
    pub fn issues(&self) -> &[CommitStateIssue] {
        &self.issues
    }

    /// Observes one raw event and returns an envelope with block commit metadata attached.
    pub fn observe(
        &mut self,
        mut envelope: EventEnvelope<RawExecEvent>,
    ) -> EventEnvelope<RawExecEvent> {
        let seqno = envelope.seqno();
        let flow_block_seqno = envelope.meta.flow.block_seqno;

        match &envelope.payload {
            RawExecEvent::BlockStart(event) => {
                if let Some(block) = event.block.as_ref() {
                    let block_ref = BlockRef {
                        block_id: block.block_id,
                        proposed_block_number: block.proposed_block_number,
                        block_start_seqno: seqno,
                    };
                    self.upsert_block(&block_ref, CommitState::Proposed, seqno);
                    envelope.meta.block = Some(block_ref);
                    envelope.meta.commit_state = CommitState::Proposed;
                } else {
                    envelope.meta.commit_state = CommitState::Unknown;
                }
            }
            RawExecEvent::BlockQc(event) => {
                let block_tuple = event.block.as_ref().map(|block| {
                    (
                        block.block_id,
                        block.proposed_block_number,
                        flow_block_seqno.unwrap_or(seqno),
                    )
                });
                self.observe_block_state(
                    &mut envelope.meta.block,
                    block_tuple,
                    CommitState::Voted,
                    seqno,
                );
                envelope.meta.commit_state = envelope
                    .meta
                    .block
                    .as_ref()
                    .and_then(|block| self.by_id.get(&block.block_id))
                    .map_or(CommitState::Unknown, |state| state.state);
            }
            RawExecEvent::BlockFinalized(event) => {
                let block_tuple = event.block.as_ref().map(|block| {
                    (
                        block.block_id,
                        block.proposed_block_number,
                        flow_block_seqno.unwrap_or(seqno),
                    )
                });
                self.observe_block_state(
                    &mut envelope.meta.block,
                    block_tuple,
                    CommitState::Finalized,
                    seqno,
                );
                envelope.meta.commit_state = envelope
                    .meta
                    .block
                    .as_ref()
                    .and_then(|block| self.by_id.get(&block.block_id))
                    .map_or(CommitState::Unknown, |state| state.state);
            }
            RawExecEvent::BlockVerified(event) => {
                if let Some(block_number) = event.block_number {
                    self.mark_verified(block_number, seqno);
                    if let Some(block_id) = self.by_block_number.get(&block_number) {
                        if let Some(state) = self.by_id.get(block_id) {
                            envelope.meta.block = Some(state.block.clone());
                            envelope.meta.commit_state = state.state;
                        }
                    } else {
                        envelope.meta.commit_state = CommitState::Verified;
                    }
                }
            }
            RawExecEvent::BlockReject(_) => {
                if let Some(block) = self.resolve_flow_block(envelope.meta.flow.block_seqno) {
                    let block_ref = block.block.clone();
                    self.upsert_block(&block_ref, CommitState::Abandoned, seqno);
                    envelope.meta.block = Some(block_ref);
                    envelope.meta.commit_state = CommitState::Abandoned;
                }
            }
            _ => {
                if let Some(block) = self.resolve_flow_block(envelope.meta.flow.block_seqno) {
                    envelope.meta.block = Some(block.block.clone());
                    envelope.meta.commit_state = block.state;
                }
            }
        }

        envelope
    }

    fn observe_block_state(
        &mut self,
        envelope_block: &mut Option<BlockRef>,
        block_tuple: Option<(B256, u64, u64)>,
        target: CommitState,
        seqno: u64,
    ) {
        let Some((block_id, proposed_block_number, block_start_seqno)) = block_tuple else {
            self.issues.push(CommitStateIssue::UnresolvedBlock {
                seqno,
                attempted: target,
            });
            return;
        };

        let block_ref = self.by_id.get(&block_id).map_or_else(
            || BlockRef {
                block_id,
                proposed_block_number,
                block_start_seqno,
            },
            |tracked| tracked.block.clone(),
        );

        self.upsert_block(&block_ref, target, seqno);
        *envelope_block = Some(block_ref);
    }

    fn upsert_block(&mut self, block: &BlockRef, target: CommitState, seqno: u64) {
        let mut accepted = true;

        self.by_id
            .entry(block.block_id)
            .and_modify(|tracked| {
                if is_commit_regression(tracked.state, target) {
                    self.issues.push(CommitStateIssue::InvalidRegression {
                        block_id: block.block_id,
                        current: tracked.state,
                        attempted: target,
                        seqno,
                    });
                    accepted = false;
                } else {
                    tracked.state = stronger_state(tracked.state, target);
                    tracked.last_transition_seqno = seqno;
                    tracked.block = block.clone();
                }
            })
            .or_insert_with(|| TrackedBlockState {
                block: block.clone(),
                state: target,
                last_transition_seqno: seqno,
            });

        if accepted {
            self.by_start_seqno
                .insert(block.block_start_seqno, block.block_id);
            self.by_block_number
                .insert(block.proposed_block_number, block.block_id);
            self.evict_old_blocks();
        }
    }

    fn mark_verified(&mut self, block_number: u64, seqno: u64) {
        self.verified_numbers.insert(block_number, seqno);

        if let Some(block_id) = self.by_block_number.get(&block_number).copied() {
            if let Some(block) = self
                .by_id
                .get(&block_id)
                .map(|tracked| tracked.block.clone())
            {
                self.upsert_block(&block, CommitState::Verified, seqno);
            }
        }
    }

    fn resolve_flow_block(&self, block_start_seqno: Option<u64>) -> Option<&TrackedBlockState> {
        block_start_seqno.and_then(|seqno| self.block_by_start_seqno(seqno))
    }

    fn evict_old_blocks(&mut self) {
        if self.retention_blocks == 0 || self.by_id.len() <= self.retention_blocks {
            return;
        }

        while self.by_id.len() > self.retention_blocks {
            let Some((old_seqno, old_block_id)) = self
                .by_start_seqno
                .first_key_value()
                .map(|(seqno, block_id)| (*seqno, *block_id))
            else {
                break;
            };

            self.by_start_seqno.remove(&old_seqno);
            if let Some(old_block) = self.by_id.remove(&old_block_id) {
                self.by_block_number
                    .remove(&old_block.block.proposed_block_number);
            }
        }
    }
}

fn is_commit_regression(current: CommitState, target: CommitState) -> bool {
    commit_rank(target) < commit_rank(current)
}

fn stronger_state(current: CommitState, target: CommitState) -> CommitState {
    if commit_rank(target) >= commit_rank(current) {
        target
    } else {
        current
    }
}

const fn commit_rank(state: CommitState) -> u8 {
    match state {
        CommitState::Unknown => 0,
        CommitState::Proposed => 1,
        CommitState::Voted => 2,
        CommitState::Finalized => 3,
        CommitState::Verified => 4,
        CommitState::Abandoned => 5,
    }
}

#[cfg(test)]
mod tests {
    use monad_mev_core::{EventMeta, FlowTags};

    use super::*;
    use crate::{
        fixture_block_tag_payload, fixture_block_verified_payload, fixture_raw_envelope,
        ExecEventType,
    };

    fn block_event(
        seqno: u64,
        event_type: ExecEventType,
        block_id: B256,
        block_number: u64,
        flow: [u64; 4],
    ) -> EventEnvelope<RawExecEvent> {
        fixture_raw_envelope(
            seqno,
            event_type,
            flow,
            fixture_block_tag_payload(block_id, block_number),
        )
        .expect("fixture event should build")
    }

    fn verified(seqno: u64, block_number: u64) -> EventEnvelope<RawExecEvent> {
        fixture_raw_envelope(
            seqno,
            ExecEventType::BlockVerified,
            [0; 4],
            fixture_block_verified_payload(block_number),
        )
        .expect("fixture event should build")
    }

    #[test]
    fn commit_state_tracks_proposed_to_verified() {
        let block_id = B256::from([7_u8; 32]);
        let mut tracker = CommitStateTracker::new();

        let start = tracker.observe(block_event(
            10,
            ExecEventType::BlockStart,
            block_id,
            99,
            [0; 4],
        ));
        assert_eq!(start.meta.commit_state, CommitState::Proposed);

        let qc = tracker.observe(block_event(
            11,
            ExecEventType::BlockQc,
            block_id,
            99,
            [10, 0, 0, 0],
        ));
        assert_eq!(qc.meta.commit_state, CommitState::Voted);

        let finalized = tracker.observe(block_event(
            12,
            ExecEventType::BlockFinalized,
            block_id,
            99,
            [10, 0, 0, 0],
        ));
        assert_eq!(finalized.meta.commit_state, CommitState::Finalized);

        let verified = tracker.observe(verified(13, 99));
        assert_eq!(verified.meta.commit_state, CommitState::Verified);
        assert_eq!(
            tracker.block_by_id(block_id).map(|tracked| tracked.state),
            Some(CommitState::Verified)
        );
    }

    #[test]
    fn finalized_without_prior_start_creates_resolvable_state() {
        let block_id = B256::from([8_u8; 32]);
        let mut tracker = CommitStateTracker::new();

        let finalized = tracker.observe(block_event(
            50,
            ExecEventType::BlockFinalized,
            block_id,
            101,
            [0; 4],
        ));

        assert_eq!(finalized.meta.commit_state, CommitState::Finalized);
        assert_eq!(
            finalized.meta.block,
            Some(BlockRef {
                block_id,
                proposed_block_number: 101,
                block_start_seqno: 50,
            })
        );
    }

    #[test]
    fn duplicate_transition_is_idempotent() {
        let block_id = B256::from([9_u8; 32]);
        let mut tracker = CommitStateTracker::new();

        tracker.observe(block_event(
            1,
            ExecEventType::BlockStart,
            block_id,
            1,
            [0; 4],
        ));
        tracker.observe(block_event(
            2,
            ExecEventType::BlockStart,
            block_id,
            1,
            [0; 4],
        ));

        assert!(tracker.issues().is_empty());
        assert_eq!(
            tracker.block_by_id(block_id).map(|tracked| tracked.state),
            Some(CommitState::Proposed)
        );
    }

    #[test]
    fn invalid_regression_is_reported_without_corrupting_state() {
        let block_id = B256::from([10_u8; 32]);
        let mut tracker = CommitStateTracker::new();

        tracker.observe(block_event(
            1,
            ExecEventType::BlockFinalized,
            block_id,
            1,
            [0; 4],
        ));
        tracker.observe(block_event(2, ExecEventType::BlockQc, block_id, 1, [0; 4]));

        assert_eq!(tracker.issues().len(), 1);
        assert_eq!(
            tracker.block_by_id(block_id).map(|tracked| tracked.state),
            Some(CommitState::Finalized)
        );
    }

    #[test]
    fn flow_scoped_event_receives_block_state() {
        let block_id = B256::from([11_u8; 32]);
        let mut tracker = CommitStateTracker::new();
        tracker.observe(block_event(
            20,
            ExecEventType::BlockStart,
            block_id,
            77,
            [0; 4],
        ));

        let tx_event = fixture_raw_envelope(21, ExecEventType::TxnLog, [20, 3, 0, 0], Vec::new())
            .expect("fixture event should build");
        let tx_event = tracker.observe(tx_event);

        assert_eq!(tx_event.meta.commit_state, CommitState::Proposed);
        assert_eq!(
            tx_event.meta.block.map(|block| block.block_id),
            Some(block_id)
        );
    }

    #[test]
    fn unresolved_commit_event_records_issue() {
        let mut tracker = CommitStateTracker::new();
        let raw = RawExecEvent::BlockFinalized(crate::RawBlockStateEvent {
            descriptor: crate::RawExecDescriptor {
                seqno: 1,
                event_type: ExecEventType::BlockFinalized.as_u16(),
                event_kind: ExecEventType::BlockFinalized.event_kind(),
                payload_size: 0,
                record_epoch_nanos: 1,
                payload_buf_offset: 0,
                flow: FlowTags::default(),
                schema_hash: None,
            },
            block: None,
            payload: Vec::new(),
        });
        let envelope = EventEnvelope::new(
            raw,
            EventMeta {
                seqno: 1,
                record_epoch_nanos: 1,
                event_kind: ExecEventType::BlockFinalized.event_kind(),
                source: monad_mev_core::EventSourceKind::Fixture,
                block: None,
                txn: None,
                flow: FlowTags::default(),
                commit_state: CommitState::Unknown,
                schema_hash: None,
            },
        );

        let observed = tracker.observe(envelope);

        assert_eq!(observed.meta.commit_state, CommitState::Unknown);
        assert!(matches!(
            tracker.issues(),
            [CommitStateIssue::UnresolvedBlock { .. }]
        ));
    }
}
