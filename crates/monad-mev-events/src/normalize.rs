use monad_mev_core::{
    Address, BlockRef, CommitState, Error, EventEnvelope, EventKind, PayloadExpired, Result,
    SchemaMismatch, StreamItem, TxnRef, B256,
};
use serde::{Deserialize, Serialize};

use crate::RawExecEvent;

/// Framework fixture log payload header length.
const FIXTURE_LOG_MIN_LEN: usize = 25;

/// EVM-oriented event layer built from raw Monad execution events.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChainEvent {
    /// Block boundary or block execution event.
    Block(BlockEvent),
    /// Transaction boundary event.
    Transaction(TransactionEvent),
    /// EVM log event.
    Log(LogEvent),
    /// EVM call-frame event.
    CallFrame(CallFrameEvent),
    /// Account access event.
    AccountAccess(AccountAccessEvent),
    /// Storage access event.
    StorageAccess(StorageAccessEvent),
    /// Transaction output event.
    TxnOutput(TxnOutputEvent),
    /// Commit-state event.
    CommitState(CommitStateEvent),
    /// Raw event that does not yet have a normalized representation.
    UnknownRaw(UnknownChainEvent),
}

/// Normalized block event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BlockEvent {
    /// Framework event kind.
    pub kind: EventKind,
    /// Block context, when known.
    pub block: Option<BlockRef>,
    /// Raw context preserved for advanced users.
    pub raw: RawExecEvent,
}

/// Normalized transaction event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TransactionEvent {
    /// Framework event kind.
    pub kind: EventKind,
    /// Transaction context, when known.
    pub txn: Option<TxnRef>,
    /// Raw context preserved for advanced users.
    pub raw: RawExecEvent,
}

/// Normalized EVM log event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    /// Emitting contract address.
    pub address: Option<Address>,
    /// Log topics in order.
    pub topics: Vec<B256>,
    /// Log data bytes.
    pub data: Vec<u8>,
    /// True when the raw payload could not be parsed into the fixture log format.
    pub malformed: bool,
    /// Raw context preserved for advanced users.
    pub raw: RawExecEvent,
}

impl LogEvent {
    /// Returns topic0 when present.
    #[must_use]
    pub fn topic0(&self) -> Option<B256> {
        self.topics.first().copied()
    }
}

/// Normalized call-frame event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallFrameEvent {
    /// Raw context preserved for advanced users.
    pub raw: RawExecEvent,
}

/// Normalized account access event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountAccessEvent {
    /// Raw context preserved for advanced users.
    pub raw: RawExecEvent,
}

/// Normalized storage access event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StorageAccessEvent {
    /// Raw context preserved for advanced users.
    pub raw: RawExecEvent,
}

/// Normalized transaction output event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TxnOutputEvent {
    /// Raw context preserved for advanced users.
    pub raw: RawExecEvent,
}

/// Normalized commit-state event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommitStateEvent {
    /// Commit state represented by this event.
    pub state: CommitState,
    /// Block context, when known.
    pub block: Option<BlockRef>,
    /// Raw context preserved for advanced users.
    pub raw: RawExecEvent,
}

/// Fallback normalized event for malformed or not-yet-normalized raw events.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnknownChainEvent {
    /// Framework event kind.
    pub kind: EventKind,
    /// Raw context preserved for advanced users.
    pub raw: RawExecEvent,
}

/// Converts one raw execution-event envelope into a chain-event envelope.
#[must_use]
pub fn normalize_raw_event(envelope: EventEnvelope<RawExecEvent>) -> EventEnvelope<ChainEvent> {
    let meta = envelope.meta.clone();
    let chain_event = match meta.event_kind {
        EventKind::BlockStart | EventKind::BlockEnd | EventKind::BlockReject => {
            ChainEvent::Block(BlockEvent {
                kind: meta.event_kind.clone(),
                block: meta.block.clone(),
                raw: envelope.payload,
            })
        }
        EventKind::BlockQc | EventKind::BlockFinalized | EventKind::BlockVerified => {
            ChainEvent::CommitState(CommitStateEvent {
                state: meta.commit_state,
                block: meta.block.clone(),
                raw: envelope.payload,
            })
        }
        EventKind::TxnHeaderStart | EventKind::TxnHeaderEnd | EventKind::TxnEnd => {
            ChainEvent::Transaction(TransactionEvent {
                kind: meta.event_kind.clone(),
                txn: meta.txn.clone(),
                raw: envelope.payload,
            })
        }
        EventKind::TxnLog => ChainEvent::Log(parse_log_event(envelope.payload)),
        EventKind::TxnCallFrame => ChainEvent::CallFrame(CallFrameEvent {
            raw: envelope.payload,
        }),
        EventKind::TxnEvmOutput => ChainEvent::TxnOutput(TxnOutputEvent {
            raw: envelope.payload,
        }),
        EventKind::AccountAccess => ChainEvent::AccountAccess(AccountAccessEvent {
            raw: envelope.payload,
        }),
        EventKind::StorageAccess => ChainEvent::StorageAccess(StorageAccessEvent {
            raw: envelope.payload,
        }),
        _ => ChainEvent::UnknownRaw(UnknownChainEvent {
            kind: meta.event_kind.clone(),
            raw: envelope.payload,
        }),
    };

    EventEnvelope::new(chain_event, meta)
}

/// Converts one raw stream item into a normalized chain stream item.
#[must_use]
pub fn normalize_stream_item(item: StreamItem<RawExecEvent>) -> StreamItem<ChainEvent> {
    match item {
        StreamItem::Event(envelope) => StreamItem::Event(normalize_raw_event(envelope)),
        StreamItem::Gap(gap) => StreamItem::Gap(gap),
        StreamItem::PayloadExpired(expired) => StreamItem::PayloadExpired(PayloadExpired {
            seqno: expired.seqno,
            event_kind: expired.event_kind,
            source: expired.source,
        }),
        StreamItem::SchemaMismatch(mismatch) => StreamItem::SchemaMismatch(SchemaMismatch {
            expected: mismatch.expected,
            observed: mismatch.observed,
            source: mismatch.source,
        }),
        StreamItem::SourceEnded => StreamItem::SourceEnded,
    }
}

fn parse_log_event(raw: RawExecEvent) -> LogEvent {
    let payload = raw.payload();
    let Some(address) = read_address(payload, 0) else {
        return LogEvent {
            address: None,
            topics: Vec::new(),
            data: payload.to_vec(),
            malformed: true,
            raw,
        };
    };

    let Some(topic_count) = payload.get(20).copied().map(usize::from) else {
        return malformed_log(raw, Some(address), Vec::new(), Vec::new());
    };
    let topics_offset = 21_usize;
    let data_len_offset = topics_offset.saturating_add(topic_count.saturating_mul(32));

    let mut topics = Vec::with_capacity(topic_count);
    for topic_index in 0..topic_count {
        let offset = topics_offset + topic_index * 32;
        let Some(topic) = read_b256(payload, offset) else {
            return malformed_log(raw, Some(address), topics, Vec::new());
        };
        topics.push(topic);
    }

    let Some(data_len) =
        read_u32(payload, data_len_offset).and_then(|value| usize::try_from(value).ok())
    else {
        return malformed_log(raw, Some(address), topics, Vec::new());
    };
    let data_offset = data_len_offset + 4;
    let Some(data_end) = data_offset.checked_add(data_len) else {
        return malformed_log(raw, Some(address), topics, Vec::new());
    };
    let Some(data) = payload.get(data_offset..data_end) else {
        let remaining = payload.get(data_offset..).unwrap_or_default().to_vec();
        return malformed_log(raw, Some(address), topics, remaining);
    };

    LogEvent {
        address: Some(address),
        topics,
        data: data.to_vec(),
        malformed: false,
        raw,
    }
}

fn malformed_log(
    raw: RawExecEvent,
    address: Option<Address>,
    topics: Vec<B256>,
    data: Vec<u8>,
) -> LogEvent {
    LogEvent {
        address,
        topics,
        data,
        malformed: true,
        raw,
    }
}

fn read_address(payload: &[u8], offset: usize) -> Option<Address> {
    let bytes: [u8; 20] = payload
        .get(offset..offset.checked_add(20)?)?
        .try_into()
        .ok()?;
    Some(Address::from(bytes))
}

fn read_b256(payload: &[u8], offset: usize) -> Option<B256> {
    let bytes: [u8; 32] = payload
        .get(offset..offset.checked_add(32)?)?
        .try_into()
        .ok()?;
    Some(B256::from(bytes))
}

fn read_u32(payload: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = payload
        .get(offset..offset.checked_add(4)?)?
        .try_into()
        .ok()?;
    Some(u32::from_le_bytes(bytes))
}

/// Builds a fixture log payload consumed by [`normalize_raw_event`].
///
/// # Errors
///
/// Returns an error when the topic count or data length cannot fit the fixture format.
pub fn fixture_log_payload(address: Address, topics: &[B256], data: &[u8]) -> Result<Vec<u8>> {
    let topic_count = u8::try_from(topics.len()).map_err(|_| {
        Error::Message(format!(
            "fixture log topic count {} exceeds u8",
            topics.len()
        ))
    })?;
    let data_len = u32::try_from(data.len()).map_err(|_| {
        Error::Message(format!(
            "fixture log data length {} exceeds u32",
            data.len()
        ))
    })?;

    let mut payload = Vec::with_capacity(FIXTURE_LOG_MIN_LEN + topics.len() * 32 + data.len());
    payload.extend_from_slice(address.as_slice());
    payload.push(topic_count);
    for topic in topics {
        payload.extend_from_slice(topic.as_slice());
    }
    payload.extend_from_slice(&data_len.to_le_bytes());
    payload.extend_from_slice(data);
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use monad_mev_core::{CommitState, EventMeta, EventSourceKind, FlowTags};

    use super::*;
    use crate::{
        fixture_block_tag_payload, fixture_raw_envelope, ExecEventType, RawUnknownExecEvent,
    };

    fn raw_envelope(event_type: ExecEventType, payload: Vec<u8>) -> EventEnvelope<RawExecEvent> {
        fixture_raw_envelope(1, event_type, [1, 2, 0, 0], payload)
            .expect("fixture event should build")
    }

    #[test]
    fn normalize_raw_log_decodes_fixture_log_payload() {
        let address = Address::from([0x11_u8; 20]);
        let topic0 = B256::from([0x22_u8; 32]);
        let payload =
            fixture_log_payload(address, &[topic0], &[1, 2, 3]).expect("fixture log should build");
        let normalized = normalize_raw_event(raw_envelope(ExecEventType::TxnLog, payload));

        let ChainEvent::Log(log) = normalized.payload else {
            panic!("expected log event");
        };

        assert_eq!(log.address, Some(address));
        assert_eq!(log.topic0(), Some(topic0));
        assert_eq!(log.data, vec![1, 2, 3]);
        assert!(!log.malformed);
    }

    #[test]
    fn normalize_block_boundary_preserves_block_context() {
        let block_id = B256::from([3_u8; 32]);
        let raw = raw_envelope(
            ExecEventType::BlockStart,
            fixture_block_tag_payload(block_id, 10),
        );
        let mut raw = raw;
        raw.meta.block = Some(BlockRef {
            block_id,
            proposed_block_number: 10,
            block_start_seqno: 1,
        });
        raw.meta.commit_state = CommitState::Proposed;
        let normalized = normalize_raw_event(raw);

        let ChainEvent::Block(block) = normalized.payload else {
            panic!("expected block event");
        };

        assert_eq!(block.block.map(|block| block.block_id), Some(block_id));
        assert_eq!(normalized.meta.commit_state, CommitState::Proposed);
    }

    #[test]
    fn normalize_commit_event_preserves_commit_state() {
        let mut raw = raw_envelope(ExecEventType::BlockFinalized, Vec::new());
        raw.meta.commit_state = CommitState::Finalized;
        let normalized = normalize_raw_event(raw);

        let ChainEvent::CommitState(commit) = normalized.payload else {
            panic!("expected commit event");
        };

        assert_eq!(commit.state, CommitState::Finalized);
    }

    #[test]
    fn normalize_malformed_log_does_not_panic() {
        let normalized = normalize_raw_event(raw_envelope(ExecEventType::TxnLog, vec![1, 2, 3]));

        let ChainEvent::Log(log) = normalized.payload else {
            panic!("expected log event");
        };

        assert!(log.malformed);
        assert_eq!(log.data, vec![1, 2, 3]);
    }

    #[test]
    fn normalize_unknown_raw_event_does_not_panic() {
        let raw = RawExecEvent::Unknown(RawUnknownExecEvent {
            descriptor: crate::RawExecDescriptor {
                seqno: 1,
                event_type: 999,
                event_kind: EventKind::Unknown(999),
                payload_size: 0,
                record_epoch_nanos: 1,
                payload_buf_offset: 0,
                flow: FlowTags::default(),
                schema_hash: None,
            },
            payload: Vec::new(),
        });
        let envelope = EventEnvelope::new(
            raw,
            EventMeta {
                seqno: 1,
                record_epoch_nanos: 1,
                event_kind: EventKind::Unknown(999),
                source: EventSourceKind::Fixture,
                block: None,
                txn: None,
                flow: FlowTags::default(),
                commit_state: CommitState::Unknown,
                schema_hash: None,
            },
        );
        let normalized = normalize_raw_event(envelope);

        assert!(matches!(normalized.payload, ChainEvent::UnknownRaw(_)));
    }
}
