use monad_mev_core::{
    Address, Error, EventEnvelope, EventKind, ReplayClock, ReplayReport, Result, StreamItem, B256,
};
use serde::{Deserialize, Serialize};

use crate::ChainEvent;

/// Replay filters applied to normalized chain events.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReplayFilter {
    /// Inclusive first descriptor sequence number.
    pub from_seqno: Option<u64>,
    /// Inclusive last descriptor sequence number.
    pub to_seqno: Option<u64>,
    /// Inclusive first proposed block number.
    pub from_block: Option<u64>,
    /// Inclusive last proposed block number.
    pub to_block: Option<u64>,
    /// Allowed event kinds. Empty means all.
    pub event_kinds: Vec<EventKind>,
    /// Emitter address for log events.
    pub address: Option<Address>,
    /// Topic0 for log events.
    pub topic0: Option<B256>,
    /// Transaction index.
    pub txn_idx: Option<u64>,
}

/// Replay runner configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReplayConfig {
    /// Clock behavior.
    pub clock: ReplayClock,
    /// Event filters.
    pub filter: ReplayFilter,
    /// Whether to collect JSONL event output.
    pub collect_events_jsonl: bool,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            clock: ReplayClock::AsFastAsPossible,
            filter: ReplayFilter::default(),
            collect_events_jsonl: false,
        }
    }
}

/// Replay output.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReplayRun {
    /// Aggregated report.
    pub report: ReplayReport,
    /// JSONL normalized events if requested.
    pub events_jsonl: String,
}

impl ReplayRun {
    /// Serializes the report to stable JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if report serialization fails.
    pub fn json_report(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.report)
            .map_err(|err| Error::Message(format!("failed to serialize replay report: {err}")))
    }

    /// Returns human summary output.
    #[must_use]
    pub fn human_summary(&self) -> String {
        self.report.human_summary()
    }
}

/// Deterministic replay runner for normalized chain-event streams.
#[derive(Clone, Debug)]
pub struct ReplayRunner {
    config: ReplayConfig,
}

impl ReplayRunner {
    /// Creates a replay runner.
    #[must_use]
    pub const fn new(config: ReplayConfig) -> Self {
        Self { config }
    }

    /// Runs replay over an in-memory stream.
    ///
    /// # Errors
    ///
    /// Returns an error if JSONL event serialization fails.
    pub fn run(
        &self,
        items: impl IntoIterator<Item = StreamItem<ChainEvent>>,
    ) -> Result<ReplayRun> {
        let mut run = ReplayRun::default();

        for item in items {
            match item {
                StreamItem::Event(envelope) => {
                    run.report.record_event();
                    if !event_matches_filter(&envelope, &self.config.filter) {
                        continue;
                    }

                    record_chain_event(&mut run.report, &envelope);
                    if self.config.collect_events_jsonl {
                        let line = serde_json::to_string(&envelope).map_err(|err| {
                            Error::Message(format!("failed to serialize replay event: {err}"))
                        })?;
                        run.events_jsonl.push_str(&line);
                        run.events_jsonl.push('\n');
                    }
                }
                StreamItem::Gap(_) => run.report.record_gap(),
                StreamItem::PayloadExpired(_) => run.report.record_payload_expired(),
                StreamItem::SchemaMismatch(_) => run.report.record_schema_mismatch(),
                StreamItem::SourceEnded => break,
            }
        }

        Ok(run)
    }
}

/// Returns true when an event passes a replay filter.
#[must_use]
pub fn event_matches_filter(envelope: &EventEnvelope<ChainEvent>, filter: &ReplayFilter) -> bool {
    if filter
        .from_seqno
        .is_some_and(|from_seqno| envelope.seqno() < from_seqno)
    {
        return false;
    }
    if filter
        .to_seqno
        .is_some_and(|to_seqno| envelope.seqno() > to_seqno)
    {
        return false;
    }
    if let Some(from_block) = filter.from_block {
        match envelope.block_number() {
            Some(block_number) if block_number >= from_block => {}
            _ => return false,
        }
    }
    if let Some(to_block) = filter.to_block {
        match envelope.block_number() {
            Some(block_number) if block_number <= to_block => {}
            _ => return false,
        }
    }
    if !filter.event_kinds.is_empty() && !filter.event_kinds.contains(&envelope.meta.event_kind) {
        return false;
    }
    if filter
        .txn_idx
        .is_some_and(|txn_idx| envelope.txn_idx() != Some(txn_idx))
    {
        return false;
    }
    if let Some(address) = filter.address {
        let ChainEvent::Log(log) = &envelope.payload else {
            return false;
        };
        if log.address != Some(address) {
            return false;
        }
    }
    if let Some(topic0) = filter.topic0 {
        let ChainEvent::Log(log) = &envelope.payload else {
            return false;
        };
        if log.topic0() != Some(topic0) {
            return false;
        }
    }

    true
}

fn record_chain_event(report: &mut ReplayReport, envelope: &EventEnvelope<ChainEvent>) {
    report.record_decoded_event();

    match &envelope.payload {
        ChainEvent::Block(_) => report.record_block(),
        ChainEvent::Transaction(_) => report.record_transaction(),
        ChainEvent::Log(_) => report.record_log(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use monad_mev_core::{BlockRef, CommitState, EventMeta, EventSourceKind, FlowTags, TxnRef};

    use super::*;
    use crate::{
        event_topic, fixture_log_payload, fixture_raw_envelope, normalize_raw_event, ChainEvent,
        ExecEventType,
    };

    fn word(value: u8) -> B256 {
        B256::from([value; 32])
    }

    fn log_envelope(seqno: u64, address: Address, topic0: B256) -> EventEnvelope<ChainEvent> {
        let payload = fixture_log_payload(address, &[topic0], &[]).expect("fixture log");
        let raw = fixture_raw_envelope(seqno, ExecEventType::TxnLog, [1, 2, 0, 0], payload)
            .expect("fixture raw");
        let mut envelope = normalize_raw_event(raw);
        envelope.meta.block = Some(BlockRef {
            block_id: word(9),
            proposed_block_number: 100,
            block_start_seqno: 1,
        });
        envelope.meta.txn = Some(TxnRef {
            txn_idx: 2,
            txn_hash: None,
        });
        envelope
    }

    fn block_envelope(seqno: u64, block_number: u64) -> EventEnvelope<ChainEvent> {
        EventEnvelope::new(
            ChainEvent::Block(crate::BlockEvent {
                kind: EventKind::BlockStart,
                block: None,
                raw: fixture_raw_envelope(seqno, ExecEventType::BlockStart, [0; 4], Vec::new())
                    .expect("fixture raw")
                    .payload,
            }),
            EventMeta {
                seqno,
                record_epoch_nanos: seqno,
                event_kind: EventKind::BlockStart,
                source: EventSourceKind::Fixture,
                block: Some(BlockRef {
                    block_id: word(8),
                    proposed_block_number: block_number,
                    block_start_seqno: seqno,
                }),
                txn: None,
                flow: FlowTags::default(),
                commit_state: CommitState::Proposed,
                schema_hash: None,
            },
        )
    }

    #[test]
    fn replay_filters_each_field() {
        let address = Address::from([1_u8; 20]);
        let topic0 = event_topic("A(uint256)");
        let event = log_envelope(5, address, topic0);

        assert!(event_matches_filter(
            &event,
            &ReplayFilter {
                from_seqno: Some(5),
                to_seqno: Some(5),
                from_block: Some(100),
                to_block: Some(100),
                event_kinds: vec![EventKind::TxnLog],
                address: Some(address),
                topic0: Some(topic0),
                txn_idx: Some(2),
            }
        ));
        assert!(!event_matches_filter(
            &event,
            &ReplayFilter {
                from_seqno: Some(6),
                ..ReplayFilter::default()
            }
        ));
        assert!(!event_matches_filter(
            &event,
            &ReplayFilter {
                address: Some(Address::from([2_u8; 20])),
                ..ReplayFilter::default()
            }
        ));
    }

    #[test]
    fn replay_combined_filters_exclude_non_matching_events() {
        let event = log_envelope(5, Address::from([1_u8; 20]), event_topic("A(uint256)"));
        let filter = ReplayFilter {
            from_seqno: Some(1),
            to_seqno: Some(10),
            event_kinds: vec![EventKind::BlockStart],
            ..ReplayFilter::default()
        };

        assert!(!event_matches_filter(&event, &filter));
    }

    #[test]
    fn replay_report_is_deterministic_across_runs() {
        let items = vec![
            StreamItem::Event(block_envelope(1, 100)),
            StreamItem::Event(log_envelope(
                2,
                Address::from([1_u8; 20]),
                event_topic("A(uint256)"),
            )),
            StreamItem::SourceEnded,
        ];
        let runner = ReplayRunner::new(ReplayConfig::default());
        let first = runner.run(items.clone()).expect("first replay");
        let second = runner.run(items).expect("second replay");

        assert_eq!(first.report, second.report);
    }

    #[test]
    fn replay_report_counts_events_gaps_expiration_logs_and_jsonl() {
        let runner = ReplayRunner::new(ReplayConfig {
            collect_events_jsonl: true,
            ..ReplayConfig::default()
        });
        let run = runner
            .run([
                StreamItem::Event(log_envelope(
                    1,
                    Address::from([1_u8; 20]),
                    event_topic("A(uint256)"),
                )),
                StreamItem::Gap(monad_mev_core::GapEvent::new(
                    2,
                    4,
                    EventSourceKind::Fixture,
                )),
                StreamItem::PayloadExpired(monad_mev_core::PayloadExpired {
                    seqno: 5,
                    event_kind: Some(EventKind::TxnLog),
                    source: EventSourceKind::Fixture,
                }),
                StreamItem::SourceEnded,
            ])
            .expect("replay should run");

        assert_eq!(run.report.events_seen, 1);
        assert_eq!(run.report.events_decoded, 1);
        assert_eq!(run.report.logs_seen, 1);
        assert_eq!(run.report.gaps, 1);
        assert_eq!(run.report.payload_expired, 1);
        assert_eq!(run.events_jsonl.lines().count(), 1);
        assert!(run
            .json_report()
            .expect("json report")
            .contains("events_seen"));
        assert!(run.human_summary().contains("logs=1"));
    }
}
