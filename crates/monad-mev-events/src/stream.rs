use monad_mev_core::{Error, Result, StreamItem};
use serde::{Deserialize, Serialize};

use crate::ChainEvent;

/// Shared stream contract for normalized Execution Events.
pub trait ExecutionEventStream {
    /// Returns the next stream item.
    ///
    /// # Errors
    ///
    /// Returns source-specific failures.
    fn next_item(&mut self) -> Result<StreamItem<ChainEvent>>;
}

/// In-memory execution event stream for fixtures, tests, and replay handoff.
#[derive(Clone, Debug)]
pub struct VecExecutionEventStream {
    items: Vec<StreamItem<ChainEvent>>,
    index: usize,
}

impl VecExecutionEventStream {
    /// Creates a vector-backed stream.
    #[must_use]
    pub fn new(items: Vec<StreamItem<ChainEvent>>) -> Self {
        Self { items, index: 0 }
    }
}

impl ExecutionEventStream for VecExecutionEventStream {
    fn next_item(&mut self) -> Result<StreamItem<ChainEvent>> {
        let Some(item) = self.items.get(self.index).cloned() else {
            return Ok(StreamItem::SourceEnded);
        };
        self.index += 1;
        Ok(item)
    }
}

/// Collects a stream into memory.
///
/// # Errors
///
/// Returns source errors.
pub fn collect_execution_stream(
    stream: &mut impl ExecutionEventStream,
) -> Result<Vec<StreamItem<ChainEvent>>> {
    let mut items = Vec::new();
    loop {
        let item = stream.next_item()?;
        if item.is_source_end() {
            items.push(StreamItem::SourceEnded);
            break;
        }
        items.push(item);
    }
    Ok(items)
}

/// Stable stream summary used for replay/live parity checks.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StreamParityReport {
    /// Source label.
    pub source: String,
    /// Total stream items, including terminal source end.
    pub items: u64,
    /// Event count.
    pub events: u64,
    /// Gap count.
    pub gaps: u64,
    /// Payload expiration count.
    pub payload_expired: u64,
    /// Schema mismatch count.
    pub schema_mismatches: u64,
    /// First event sequence number.
    pub first_seqno: Option<u64>,
    /// Last event sequence number.
    pub last_seqno: Option<u64>,
}

/// Builds a stream parity report.
#[must_use]
pub fn execution_stream_report(
    source: impl Into<String>,
    items: &[StreamItem<ChainEvent>],
) -> StreamParityReport {
    let mut report = StreamParityReport {
        source: source.into(),
        items: items.len() as u64,
        ..StreamParityReport::default()
    };

    for item in items {
        match item {
            StreamItem::Event(event) => {
                report.events += 1;
                report.first_seqno.get_or_insert(event.seqno());
                report.last_seqno = Some(event.seqno());
            }
            StreamItem::Gap(_) => report.gaps += 1,
            StreamItem::PayloadExpired(_) => report.payload_expired += 1,
            StreamItem::SchemaMismatch(_) => report.schema_mismatches += 1,
            StreamItem::SourceEnded => {}
        }
    }

    report
}

/// Result of comparing two stream reports.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StreamParityComparison {
    /// Left report.
    pub left: StreamParityReport,
    /// Right report.
    pub right: StreamParityReport,
    /// True when reports have parity, excluding source labels.
    pub matches: bool,
    /// Deterministic mismatch list.
    pub mismatches: Vec<String>,
}

/// Compares stream reports for replay/live parity.
///
/// # Errors
///
/// Returns an error when source labels are empty.
pub fn compare_stream_parity(
    left: StreamParityReport,
    right: StreamParityReport,
) -> Result<StreamParityComparison> {
    if left.source.is_empty() || right.source.is_empty() {
        return Err(Error::Message(
            "stream parity reports require source labels".to_owned(),
        ));
    }

    let mut mismatches = Vec::new();
    compare_field(&mut mismatches, "items", left.items, right.items);
    compare_field(&mut mismatches, "events", left.events, right.events);
    compare_field(&mut mismatches, "gaps", left.gaps, right.gaps);
    compare_field(
        &mut mismatches,
        "payload_expired",
        left.payload_expired,
        right.payload_expired,
    );
    compare_field(
        &mut mismatches,
        "schema_mismatches",
        left.schema_mismatches,
        right.schema_mismatches,
    );
    if left.first_seqno != right.first_seqno {
        mismatches.push("first_seqno mismatch".to_owned());
    }
    if left.last_seqno != right.last_seqno {
        mismatches.push("last_seqno mismatch".to_owned());
    }

    Ok(StreamParityComparison {
        left,
        right,
        matches: mismatches.is_empty(),
        mismatches,
    })
}

fn compare_field(mismatches: &mut Vec<String>, name: &str, left: u64, right: u64) {
    if left != right {
        mismatches.push(format!("{name} mismatch: {left} != {right}"));
    }
}

#[cfg(test)]
mod tests {
    use monad_mev_core::{
        CommitState, EventEnvelope, EventKind, EventMeta, EventSourceKind, FlowTags,
    };

    use super::*;
    use crate::{fixture_raw_envelope, normalize_raw_event, ChainEvent, ExecEventType};

    fn event(seqno: u64) -> StreamItem<ChainEvent> {
        let raw = fixture_raw_envelope(seqno, ExecEventType::BlockStart, [0; 4], Vec::new())
            .expect("fixture raw");
        StreamItem::Event(normalize_raw_event(raw))
    }

    #[test]
    fn vector_stream_collects_to_source_end() {
        let mut stream = VecExecutionEventStream::new(vec![event(1)]);

        let items = collect_execution_stream(&mut stream).expect("collect");

        assert_eq!(items.len(), 2);
        assert!(items[1].is_source_end());
    }

    #[test]
    fn stream_report_counts_items() {
        let items = vec![event(1), event(3), StreamItem::SourceEnded];

        let report = execution_stream_report("fixture", &items);

        assert_eq!(report.items, 3);
        assert_eq!(report.events, 2);
        assert_eq!(report.first_seqno, Some(1));
        assert_eq!(report.last_seqno, Some(3));
    }

    #[test]
    fn stream_parity_detects_mismatch() {
        let left = execution_stream_report("left", &[event(1), StreamItem::SourceEnded]);
        let right =
            execution_stream_report("right", &[event(1), event(2), StreamItem::SourceEnded]);

        let comparison = compare_stream_parity(left, right).expect("compare");

        assert!(!comparison.matches);
        assert!(comparison.mismatches[0].contains("items"));
    }

    #[test]
    fn report_handles_synthetic_event() {
        let event = EventEnvelope::new(
            ChainEvent::UnknownRaw(crate::UnknownChainEvent {
                kind: EventKind::Control,
                raw: fixture_raw_envelope(8, ExecEventType::RecordError, [0; 4], Vec::new())
                    .expect("fixture raw")
                    .payload,
            }),
            EventMeta {
                seqno: 8,
                record_epoch_nanos: 8,
                event_kind: EventKind::Control,
                source: EventSourceKind::Synthetic,
                block: None,
                txn: None,
                flow: FlowTags::default(),
                commit_state: CommitState::Unknown,
                schema_hash: None,
            },
        );

        let report = execution_stream_report("synthetic", &[StreamItem::Event(event)]);

        assert_eq!(report.events, 1);
        assert_eq!(report.first_seqno, Some(8));
    }
}
