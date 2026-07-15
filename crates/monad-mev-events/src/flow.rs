use std::collections::BTreeMap;

use monad_mev_core::{EventEnvelope, EventKind, TxnRef, B256};
use serde::{Deserialize, Serialize};

use crate::RawExecEvent;

/// Unique transaction flow key from descriptor flow tags.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct TransactionFlowKey {
    /// Sequence number of the block-start event associated with this transaction flow.
    pub block_start_seqno: u64,
    /// Transaction flow identifier from descriptor metadata.
    pub txn_id: u64,
}

impl TransactionFlowKey {
    /// Builds a key from event metadata flow tags.
    #[must_use]
    pub const fn from_envelope(envelope: &EventEnvelope<RawExecEvent>) -> Option<Self> {
        let Some(block_start_seqno) = envelope.meta.flow.block_seqno else {
            return None;
        };
        let Some(txn_id) = envelope.meta.flow.txn_id else {
            return None;
        };

        Some(Self {
            block_start_seqno,
            txn_id,
        })
    }

    /// Returns a transaction reference for this key and optional hash.
    #[must_use]
    pub const fn txn_ref(self, txn_hash: Option<B256>) -> TxnRef {
        TxnRef {
            // Flow extensions reserve zero for `None`, so transaction indexes
            // are encoded as index + 1 by fixture and live SDK adapters.
            txn_idx: self.txn_id.saturating_sub(1),
            txn_hash,
        }
    }
}

/// Aggregated transaction flow.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TransactionBundle {
    /// Transaction flow key.
    pub key: TransactionFlowKey,
    /// Transaction hash, if observed from the header-start event.
    pub txn_hash: Option<B256>,
    /// Whether the header-start event was observed.
    pub saw_header_start: bool,
    /// Whether the transaction-end event was observed.
    pub saw_txn_end: bool,
    /// Events in per-transaction stream order.
    pub events: Vec<EventEnvelope<RawExecEvent>>,
    /// First descriptor sequence number in the bundle.
    pub first_seqno: u64,
    /// Last descriptor sequence number in the bundle.
    pub last_seqno: u64,
}

impl TransactionBundle {
    /// Returns true when this flow has both a header start and transaction end.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.saw_header_start && self.saw_txn_end
    }
}

/// Result of observing one event through the transaction-flow tracker.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TransactionFlowUpdate {
    /// Event with transaction context attached when available.
    pub envelope: EventEnvelope<RawExecEvent>,
    /// Completed transaction bundle, emitted on `TXN_END`.
    pub completed: Option<TransactionBundle>,
    /// Bundles evicted to honor bounded memory.
    pub evicted: Vec<TransactionBundle>,
}

/// Summary counters for transaction-flow tracking.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TransactionFlowSummary {
    /// Active incomplete flows currently retained.
    pub active_flows: u64,
    /// Completed bundles emitted.
    pub completed_flows: u64,
    /// Flows that were observed without a header-start event.
    pub incomplete_flows: u64,
    /// Flows evicted before completion.
    pub evicted_flows: u64,
}

/// Tracks interleaved transaction events and emits completed bundles.
#[derive(Clone, Debug)]
pub struct TxnFlowTracker {
    active: BTreeMap<TransactionFlowKey, TransactionBundle>,
    completed_flows: u64,
    incomplete_flows: u64,
    evicted_flows: u64,
    max_active_flows: usize,
}

impl Default for TxnFlowTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TxnFlowTracker {
    /// Creates a tracker with replay-friendly active-flow retention.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: BTreeMap::new(),
            completed_flows: 0,
            incomplete_flows: 0,
            evicted_flows: 0,
            max_active_flows: 4096,
        }
    }

    /// Creates a tracker with a bounded active-flow count.
    #[must_use]
    pub const fn with_max_active_flows(max_active_flows: usize) -> Self {
        Self {
            active: BTreeMap::new(),
            completed_flows: 0,
            incomplete_flows: 0,
            evicted_flows: 0,
            max_active_flows,
        }
    }

    /// Returns the active bundle for a key.
    #[must_use]
    pub fn active_bundle(&self, key: TransactionFlowKey) -> Option<&TransactionBundle> {
        self.active.get(&key)
    }

    /// Returns current summary counters.
    #[must_use]
    pub fn summary(&self) -> TransactionFlowSummary {
        TransactionFlowSummary {
            active_flows: u64::try_from(self.active.len()).unwrap_or(u64::MAX),
            completed_flows: self.completed_flows,
            incomplete_flows: self.incomplete_flows,
            evicted_flows: self.evicted_flows,
        }
    }

    /// Observes one raw event and attaches transaction context when possible.
    pub fn observe(&mut self, mut envelope: EventEnvelope<RawExecEvent>) -> TransactionFlowUpdate {
        let Some(key) = TransactionFlowKey::from_envelope(&envelope) else {
            return TransactionFlowUpdate {
                envelope,
                completed: None,
                evicted: Vec::new(),
            };
        };

        let event_kind = envelope.meta.event_kind.clone();
        let txn_hash = envelope.payload.txn_hash();
        let is_header_start = matches!(event_kind, EventKind::TxnHeaderStart);
        let is_txn_end = matches!(event_kind, EventKind::TxnEnd);

        let mut was_new_orphan = false;
        let bundle = self.active.entry(key).or_insert_with(|| {
            was_new_orphan = !is_header_start;
            TransactionBundle {
                key,
                txn_hash,
                saw_header_start: is_header_start,
                saw_txn_end: false,
                events: Vec::new(),
                first_seqno: envelope.seqno(),
                last_seqno: envelope.seqno(),
            }
        });

        if was_new_orphan {
            self.incomplete_flows += 1;
        }
        if is_header_start {
            bundle.saw_header_start = true;
            bundle.txn_hash = txn_hash;
        } else if bundle.txn_hash.is_none() {
            bundle.txn_hash = txn_hash;
        }

        envelope.meta.txn = Some(key.txn_ref(bundle.txn_hash));
        bundle.last_seqno = envelope.seqno();
        bundle.events.push(envelope.clone());

        let completed = if is_txn_end {
            self.active.remove(&key).map(|mut completed| {
                completed.saw_txn_end = true;
                self.completed_flows += 1;
                completed
            })
        } else {
            None
        };

        let evicted = self.evict_overflow();

        TransactionFlowUpdate {
            envelope,
            completed,
            evicted,
        }
    }

    fn evict_overflow(&mut self) -> Vec<TransactionBundle> {
        if self.max_active_flows == 0 || self.active.len() <= self.max_active_flows {
            return Vec::new();
        }

        let mut evicted = Vec::new();
        while self.active.len() > self.max_active_flows {
            let Some(key) = self.active.keys().next().copied() else {
                break;
            };
            if let Some(bundle) = self.active.remove(&key) {
                self.evicted_flows += 1;
                evicted.push(bundle);
            }
        }
        evicted
    }
}

#[cfg(test)]
mod tests {
    use crate::{fixture_raw_envelope, fixture_txn_header_start_payload, ExecEventType};

    use super::*;

    fn event(seqno: u64, event_type: ExecEventType, txn_idx: u64) -> EventEnvelope<RawExecEvent> {
        let payload = if matches!(event_type, ExecEventType::TxnHeaderStart) {
            let byte = u8::try_from(txn_idx).expect("fixture txn_idx should fit u8");
            fixture_txn_header_start_payload(B256::from([byte; 32]))
        } else {
            Vec::new()
        };

        let txn_id = txn_idx
            .checked_add(1)
            .expect("fixture txn_idx should encode");

        fixture_raw_envelope(seqno, event_type, [1, txn_id, 0, 0], payload)
            .expect("fixture event should build")
    }

    #[test]
    fn transaction_grouping_preserves_interleaved_per_transaction_order() {
        let mut tracker = TxnFlowTracker::new();

        tracker.observe(event(1, ExecEventType::TxnHeaderStart, 10));
        tracker.observe(event(2, ExecEventType::TxnHeaderStart, 11));
        tracker.observe(event(3, ExecEventType::TxnLog, 10));
        tracker.observe(event(4, ExecEventType::TxnLog, 11));
        let first_done = tracker.observe(event(5, ExecEventType::TxnEnd, 10));
        let second_done = tracker.observe(event(6, ExecEventType::TxnEnd, 11));

        let first = first_done.completed.expect("first txn should complete");
        let second = second_done.completed.expect("second txn should complete");

        assert_eq!(
            first
                .events
                .iter()
                .map(EventEnvelope::seqno)
                .collect::<Vec<_>>(),
            vec![1, 3, 5]
        );
        assert_eq!(
            second
                .events
                .iter()
                .map(EventEnvelope::seqno)
                .collect::<Vec<_>>(),
            vec![2, 4, 6]
        );
        assert!(first.is_complete());
        assert!(second.is_complete());
    }

    #[test]
    fn transaction_context_is_attached_to_events() {
        let mut tracker = TxnFlowTracker::new();
        let update = tracker.observe(event(1, ExecEventType::TxnHeaderStart, 3));

        assert_eq!(
            update.envelope.meta.txn.as_ref().map(|txn| txn.txn_idx),
            Some(3)
        );
        assert_eq!(
            update.envelope.meta.txn.and_then(|txn| txn.txn_hash),
            Some(B256::from([3_u8; 32]))
        );
    }

    #[test]
    fn orphan_event_remains_usable_but_marked_incomplete() {
        let mut tracker = TxnFlowTracker::new();
        let update = tracker.observe(event(7, ExecEventType::TxnLog, 4));

        assert!(update.completed.is_none());
        assert_eq!(
            update.envelope.meta.txn.as_ref().map(|txn| txn.txn_idx),
            Some(4)
        );
        assert_eq!(tracker.summary().incomplete_flows, 1);
    }

    #[test]
    fn memory_cleanup_after_transaction_end_removes_active_flow() {
        let mut tracker = TxnFlowTracker::new();

        tracker.observe(event(1, ExecEventType::TxnHeaderStart, 1));
        let update = tracker.observe(event(2, ExecEventType::TxnEnd, 1));

        assert!(update.completed.is_some());
        assert_eq!(tracker.summary().active_flows, 0);
        assert_eq!(tracker.summary().completed_flows, 1);
    }

    #[test]
    fn bounded_memory_evicts_oldest_active_flow() {
        let mut tracker = TxnFlowTracker::with_max_active_flows(1);

        tracker.observe(event(1, ExecEventType::TxnHeaderStart, 1));
        let update = tracker.observe(event(2, ExecEventType::TxnHeaderStart, 2));

        assert_eq!(update.evicted.len(), 1);
        assert_eq!(update.evicted[0].key.txn_id, 2);
        assert_eq!(tracker.summary().active_flows, 1);
        assert_eq!(tracker.summary().evicted_flows, 1);
    }
}
