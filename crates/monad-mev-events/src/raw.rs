use std::fmt::{Display, Formatter};

use monad_mev_core::{
    CommitState, Error, EventEnvelope, EventKind, EventMeta, EventSourceKind, FlowTags, Result,
    B256,
};
use serde::{Deserialize, Serialize};

use crate::SnapshotDescriptor;

/// Pinned Monad execution-event type IDs from `exec_event_ctypes.h`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecEventType {
    /// `MONAD_EXEC_NONE`.
    None,
    /// `MONAD_EXEC_RECORD_ERROR`.
    RecordError,
    /// `MONAD_EXEC_BLOCK_START`.
    BlockStart,
    /// `MONAD_EXEC_BLOCK_REJECT`.
    BlockReject,
    /// `MONAD_EXEC_BLOCK_PERF_EVM_ENTER`.
    BlockPerfEvmEnter,
    /// `MONAD_EXEC_BLOCK_PERF_EVM_EXIT`.
    BlockPerfEvmExit,
    /// `MONAD_EXEC_BLOCK_END`.
    BlockEnd,
    /// `MONAD_EXEC_BLOCK_QC`.
    BlockQc,
    /// `MONAD_EXEC_BLOCK_FINALIZED`.
    BlockFinalized,
    /// `MONAD_EXEC_BLOCK_VERIFIED`.
    BlockVerified,
    /// `MONAD_EXEC_TXN_HEADER_START`.
    TxnHeaderStart,
    /// `MONAD_EXEC_TXN_ACCESS_LIST_ENTRY`.
    TxnAccessListEntry,
    /// `MONAD_EXEC_TXN_AUTH_LIST_ENTRY`.
    TxnAuthListEntry,
    /// `MONAD_EXEC_TXN_HEADER_END`.
    TxnHeaderEnd,
    /// `MONAD_EXEC_TXN_REJECT`.
    TxnReject,
    /// `MONAD_EXEC_TXN_PERF_EVM_ENTER`.
    TxnPerfEvmEnter,
    /// `MONAD_EXEC_TXN_PERF_EVM_EXIT`.
    TxnPerfEvmExit,
    /// `MONAD_EXEC_TXN_EVM_OUTPUT`.
    TxnEvmOutput,
    /// `MONAD_EXEC_TXN_LOG`.
    TxnLog,
    /// `MONAD_EXEC_TXN_CALL_FRAME`.
    TxnCallFrame,
    /// `MONAD_EXEC_TXN_END`.
    TxnEnd,
    /// `MONAD_EXEC_ACCOUNT_ACCESS_LIST_HEADER`.
    AccountAccessListHeader,
    /// `MONAD_EXEC_ACCOUNT_ACCESS`.
    AccountAccess,
    /// `MONAD_EXEC_STORAGE_ACCESS`.
    StorageAccess,
    /// `MONAD_EXEC_EVM_ERROR`.
    EvmError,
}

impl ExecEventType {
    /// Converts a raw event type ID to a pinned event type.
    #[must_use]
    pub const fn from_u16(value: u16) -> Option<Self> {
        Some(match value {
            0 => Self::None,
            1 => Self::RecordError,
            2 => Self::BlockStart,
            3 => Self::BlockReject,
            4 => Self::BlockPerfEvmEnter,
            5 => Self::BlockPerfEvmExit,
            6 => Self::BlockEnd,
            7 => Self::BlockQc,
            8 => Self::BlockFinalized,
            9 => Self::BlockVerified,
            10 => Self::TxnHeaderStart,
            11 => Self::TxnAccessListEntry,
            12 => Self::TxnAuthListEntry,
            13 => Self::TxnHeaderEnd,
            14 => Self::TxnReject,
            15 => Self::TxnPerfEvmEnter,
            16 => Self::TxnPerfEvmExit,
            17 => Self::TxnEvmOutput,
            18 => Self::TxnLog,
            19 => Self::TxnCallFrame,
            20 => Self::TxnEnd,
            21 => Self::AccountAccessListHeader,
            22 => Self::AccountAccess,
            23 => Self::StorageAccess,
            24 => Self::EvmError,
            _ => return None,
        })
    }

    /// Raw numeric event type ID.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        match self {
            Self::None => 0,
            Self::RecordError => 1,
            Self::BlockStart => 2,
            Self::BlockReject => 3,
            Self::BlockPerfEvmEnter => 4,
            Self::BlockPerfEvmExit => 5,
            Self::BlockEnd => 6,
            Self::BlockQc => 7,
            Self::BlockFinalized => 8,
            Self::BlockVerified => 9,
            Self::TxnHeaderStart => 10,
            Self::TxnAccessListEntry => 11,
            Self::TxnAuthListEntry => 12,
            Self::TxnHeaderEnd => 13,
            Self::TxnReject => 14,
            Self::TxnPerfEvmEnter => 15,
            Self::TxnPerfEvmExit => 16,
            Self::TxnEvmOutput => 17,
            Self::TxnLog => 18,
            Self::TxnCallFrame => 19,
            Self::TxnEnd => 20,
            Self::AccountAccessListHeader => 21,
            Self::AccountAccess => 22,
            Self::StorageAccess => 23,
            Self::EvmError => 24,
        }
    }

    /// Framework-level event kind for this execution-event type.
    #[must_use]
    pub const fn event_kind(self) -> EventKind {
        match self {
            Self::None => EventKind::None,
            Self::RecordError => EventKind::RecordError,
            Self::BlockStart => EventKind::BlockStart,
            Self::BlockReject => EventKind::BlockReject,
            Self::BlockPerfEvmEnter => EventKind::BlockPerfEvmEnter,
            Self::BlockPerfEvmExit => EventKind::BlockPerfEvmExit,
            Self::BlockEnd => EventKind::BlockEnd,
            Self::BlockQc => EventKind::BlockQc,
            Self::BlockFinalized => EventKind::BlockFinalized,
            Self::BlockVerified => EventKind::BlockVerified,
            Self::TxnHeaderStart => EventKind::TxnHeaderStart,
            Self::TxnAccessListEntry => EventKind::TxnAccessListEntry,
            Self::TxnAuthListEntry => EventKind::TxnAuthListEntry,
            Self::TxnHeaderEnd => EventKind::TxnHeaderEnd,
            Self::TxnReject => EventKind::TxnReject,
            Self::TxnPerfEvmEnter => EventKind::TxnPerfEvmEnter,
            Self::TxnPerfEvmExit => EventKind::TxnPerfEvmExit,
            Self::TxnEvmOutput => EventKind::TxnEvmOutput,
            Self::TxnLog => EventKind::TxnLog,
            Self::TxnCallFrame => EventKind::TxnCallFrame,
            Self::TxnEnd => EventKind::TxnEnd,
            Self::AccountAccessListHeader => EventKind::AccountAccessListHeader,
            Self::AccountAccess => EventKind::AccountAccess,
            Self::StorageAccess => EventKind::StorageAccess,
            Self::EvmError => EventKind::EvmError,
        }
    }

    /// Stable label for CLI inspection and JSON output.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::RecordError => "record_error",
            Self::BlockStart => "block_start",
            Self::BlockReject => "block_reject",
            Self::BlockPerfEvmEnter => "block_perf_evm_enter",
            Self::BlockPerfEvmExit => "block_perf_evm_exit",
            Self::BlockEnd => "block_end",
            Self::BlockQc => "block_qc",
            Self::BlockFinalized => "block_finalized",
            Self::BlockVerified => "block_verified",
            Self::TxnHeaderStart => "txn_header_start",
            Self::TxnAccessListEntry => "txn_access_list_entry",
            Self::TxnAuthListEntry => "txn_auth_list_entry",
            Self::TxnHeaderEnd => "txn_header_end",
            Self::TxnReject => "txn_reject",
            Self::TxnPerfEvmEnter => "txn_perf_evm_enter",
            Self::TxnPerfEvmExit => "txn_perf_evm_exit",
            Self::TxnEvmOutput => "txn_evm_output",
            Self::TxnLog => "txn_log",
            Self::TxnCallFrame => "txn_call_frame",
            Self::TxnEnd => "txn_end",
            Self::AccountAccessListHeader => "account_access_list_header",
            Self::AccountAccess => "account_access",
            Self::StorageAccess => "storage_access",
            Self::EvmError => "evm_error",
        }
    }
}

impl Display for ExecEventType {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

/// Raw descriptor metadata preserved with every raw execution event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawExecDescriptor {
    /// Descriptor sequence number.
    pub seqno: u64,
    /// Raw event type ID from the descriptor.
    pub event_type: u16,
    /// Framework-level event kind.
    pub event_kind: EventKind,
    /// Payload byte length from the descriptor.
    pub payload_size: u32,
    /// Source-provided record timestamp in epoch nanoseconds.
    pub record_epoch_nanos: u64,
    /// Unwrapped payload buffer offset.
    pub payload_buf_offset: u64,
    /// Raw descriptor flow tags.
    pub flow: FlowTags,
    /// Source schema hash, when known.
    pub schema_hash: Option<B256>,
}

impl RawExecDescriptor {
    /// Builds descriptor metadata from a snapshot descriptor and source metadata.
    #[must_use]
    pub fn from_snapshot_descriptor(
        descriptor: &SnapshotDescriptor,
        schema_hash: Option<B256>,
    ) -> Self {
        let event_kind = ExecEventType::from_u16(descriptor.event_type).map_or(
            EventKind::Unknown(descriptor.event_type),
            ExecEventType::event_kind,
        );

        Self {
            seqno: descriptor.seqno,
            event_type: descriptor.event_type,
            event_kind,
            payload_size: descriptor.payload_size,
            record_epoch_nanos: descriptor.record_epoch_nanos,
            payload_buf_offset: descriptor.payload_buf_offset,
            flow: flow_tags_from_content_ext(descriptor.content_ext),
            schema_hash,
        }
    }
}

/// Block identifier and proposed number parsed from consensus event payloads.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawBlockTag {
    /// Monad consensus block ID.
    pub block_id: B256,
    /// Proposed block number.
    pub proposed_block_number: u64,
}

/// Parsed subset of block-start metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawBlockStart {
    /// Descriptor metadata.
    pub descriptor: RawExecDescriptor,
    /// Block tag if the payload was large enough to contain one.
    pub block: Option<RawBlockTag>,
    /// Full owned payload bytes.
    pub payload: Vec<u8>,
}

/// Parsed subset of block consensus metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawBlockStateEvent {
    /// Descriptor metadata.
    pub descriptor: RawExecDescriptor,
    /// Block tag if the payload was large enough to contain one.
    pub block: Option<RawBlockTag>,
    /// Full owned payload bytes.
    pub payload: Vec<u8>,
}

/// Parsed subset of block verification metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawBlockVerified {
    /// Descriptor metadata.
    pub descriptor: RawExecDescriptor,
    /// Verified block number if the payload was large enough.
    pub block_number: Option<u64>,
    /// Full owned payload bytes.
    pub payload: Vec<u8>,
}

/// Parsed subset of transaction-header metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawTxnHeaderStart {
    /// Descriptor metadata.
    pub descriptor: RawExecDescriptor,
    /// Transaction hash if the payload was large enough.
    pub txn_hash: Option<B256>,
    /// Full owned payload bytes.
    pub payload: Vec<u8>,
}

/// Known event whose payload is intentionally still opaque in V1 raw mode.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawKnownEvent {
    /// Descriptor metadata.
    pub descriptor: RawExecDescriptor,
    /// Stable event type.
    pub event_type: ExecEventType,
    /// Full owned payload bytes.
    pub payload: Vec<u8>,
}

/// Unknown event type preserved without lossy decoding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawUnknownExecEvent {
    /// Descriptor metadata.
    pub descriptor: RawExecDescriptor,
    /// Full owned payload bytes.
    pub payload: Vec<u8>,
}

/// Framework-owned raw execution event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RawExecEvent {
    /// No-op or empty event.
    None(RawKnownEvent),
    /// Monad execution record error.
    RecordError(RawKnownEvent),
    /// Block start event.
    BlockStart(RawBlockStart),
    /// Block reject event.
    BlockReject(RawKnownEvent),
    /// Block EVM performance enter marker.
    BlockPerfEvmEnter(RawKnownEvent),
    /// Block EVM performance exit marker.
    BlockPerfEvmExit(RawKnownEvent),
    /// Block end event.
    BlockEnd(RawKnownEvent),
    /// Block quorum-certificate event.
    BlockQc(RawBlockStateEvent),
    /// Block finalized event.
    BlockFinalized(RawBlockStateEvent),
    /// Block verified event.
    BlockVerified(RawBlockVerified),
    /// Transaction header start event.
    TxnHeaderStart(RawTxnHeaderStart),
    /// Transaction access-list entry event.
    TxnAccessListEntry(RawKnownEvent),
    /// Transaction authorization-list entry event.
    TxnAuthListEntry(RawKnownEvent),
    /// Transaction header end marker.
    TxnHeaderEnd(RawKnownEvent),
    /// Transaction reject event.
    TxnReject(RawKnownEvent),
    /// Transaction EVM performance enter marker.
    TxnPerfEvmEnter(RawKnownEvent),
    /// Transaction EVM performance exit marker.
    TxnPerfEvmExit(RawKnownEvent),
    /// Transaction EVM output event.
    TxnEvmOutput(RawKnownEvent),
    /// Transaction log event.
    TxnLog(RawKnownEvent),
    /// Transaction call-frame event.
    TxnCallFrame(RawKnownEvent),
    /// Transaction end marker.
    TxnEnd(RawKnownEvent),
    /// Account access-list header event.
    AccountAccessListHeader(RawKnownEvent),
    /// Account access event.
    AccountAccess(RawKnownEvent),
    /// Storage access event.
    StorageAccess(RawKnownEvent),
    /// EVM error event.
    EvmError(RawKnownEvent),
    /// Source-specific unknown event type.
    Unknown(RawUnknownExecEvent),
}

impl RawExecEvent {
    /// Converts one owned snapshot descriptor into a raw execution event.
    #[must_use]
    pub fn from_snapshot_descriptor(
        descriptor: SnapshotDescriptor,
        schema_hash: Option<B256>,
    ) -> Self {
        let raw_descriptor = RawExecDescriptor::from_snapshot_descriptor(&descriptor, schema_hash);
        let payload = descriptor.payload;

        match ExecEventType::from_u16(raw_descriptor.event_type) {
            Some(ExecEventType::BlockStart) => Self::BlockStart(RawBlockStart {
                block: parse_block_tag(&payload),
                descriptor: raw_descriptor,
                payload,
            }),
            Some(ExecEventType::BlockQc) => Self::BlockQc(RawBlockStateEvent {
                block: parse_block_tag(&payload),
                descriptor: raw_descriptor,
                payload,
            }),
            Some(ExecEventType::BlockFinalized) => Self::BlockFinalized(RawBlockStateEvent {
                block: parse_block_tag(&payload),
                descriptor: raw_descriptor,
                payload,
            }),
            Some(ExecEventType::BlockVerified) => Self::BlockVerified(RawBlockVerified {
                block_number: read_u64(&payload, 0),
                descriptor: raw_descriptor,
                payload,
            }),
            Some(ExecEventType::TxnHeaderStart) => Self::TxnHeaderStart(RawTxnHeaderStart {
                txn_hash: read_b256(&payload, 0),
                descriptor: raw_descriptor,
                payload,
            }),
            Some(event_type) => known_raw_event(raw_descriptor, event_type, payload),
            None => Self::Unknown(RawUnknownExecEvent {
                descriptor: raw_descriptor,
                payload,
            }),
        }
    }

    /// Returns the descriptor metadata carried by this event.
    #[must_use]
    pub const fn descriptor(&self) -> &RawExecDescriptor {
        match self {
            Self::None(event)
            | Self::RecordError(event)
            | Self::BlockReject(event)
            | Self::BlockPerfEvmEnter(event)
            | Self::BlockPerfEvmExit(event)
            | Self::BlockEnd(event)
            | Self::TxnAccessListEntry(event)
            | Self::TxnAuthListEntry(event)
            | Self::TxnHeaderEnd(event)
            | Self::TxnReject(event)
            | Self::TxnPerfEvmEnter(event)
            | Self::TxnPerfEvmExit(event)
            | Self::TxnEvmOutput(event)
            | Self::TxnLog(event)
            | Self::TxnCallFrame(event)
            | Self::TxnEnd(event)
            | Self::AccountAccessListHeader(event)
            | Self::AccountAccess(event)
            | Self::StorageAccess(event)
            | Self::EvmError(event) => &event.descriptor,
            Self::BlockStart(event) => &event.descriptor,
            Self::BlockQc(event) | Self::BlockFinalized(event) => &event.descriptor,
            Self::BlockVerified(event) => &event.descriptor,
            Self::TxnHeaderStart(event) => &event.descriptor,
            Self::Unknown(event) => &event.descriptor,
        }
    }

    /// Returns the full owned payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        match self {
            Self::None(event)
            | Self::RecordError(event)
            | Self::BlockReject(event)
            | Self::BlockPerfEvmEnter(event)
            | Self::BlockPerfEvmExit(event)
            | Self::BlockEnd(event)
            | Self::TxnAccessListEntry(event)
            | Self::TxnAuthListEntry(event)
            | Self::TxnHeaderEnd(event)
            | Self::TxnReject(event)
            | Self::TxnPerfEvmEnter(event)
            | Self::TxnPerfEvmExit(event)
            | Self::TxnEvmOutput(event)
            | Self::TxnLog(event)
            | Self::TxnCallFrame(event)
            | Self::TxnEnd(event)
            | Self::AccountAccessListHeader(event)
            | Self::AccountAccess(event)
            | Self::StorageAccess(event)
            | Self::EvmError(event) => &event.payload,
            Self::BlockStart(event) => &event.payload,
            Self::BlockQc(event) | Self::BlockFinalized(event) => &event.payload,
            Self::BlockVerified(event) => &event.payload,
            Self::TxnHeaderStart(event) => &event.payload,
            Self::Unknown(event) => &event.payload,
        }
    }

    /// Returns parsed block tag metadata when this event carries it.
    #[must_use]
    pub const fn block_tag(&self) -> Option<&RawBlockTag> {
        match self {
            Self::BlockStart(event) => event.block.as_ref(),
            Self::BlockQc(event) | Self::BlockFinalized(event) => event.block.as_ref(),
            _ => None,
        }
    }

    /// Returns parsed verified block number when this is a block-verified event.
    #[must_use]
    pub const fn verified_block_number(&self) -> Option<u64> {
        match self {
            Self::BlockVerified(event) => event.block_number,
            _ => None,
        }
    }

    /// Returns parsed transaction hash when this event carries it.
    #[must_use]
    pub const fn txn_hash(&self) -> Option<B256> {
        match self {
            Self::TxnHeaderStart(event) => event.txn_hash,
            _ => None,
        }
    }

    /// Stable label suitable for inspect output.
    #[must_use]
    pub fn inspect_label(&self) -> String {
        match ExecEventType::from_u16(self.descriptor().event_type) {
            Some(event_type) => event_type.label().to_owned(),
            None => format!("unknown({})", self.descriptor().event_type),
        }
    }
}

fn known_raw_event(
    descriptor: RawExecDescriptor,
    event_type: ExecEventType,
    payload: Vec<u8>,
) -> RawExecEvent {
    let event = RawKnownEvent {
        descriptor,
        event_type,
        payload,
    };

    match event_type {
        ExecEventType::None => RawExecEvent::None(event),
        ExecEventType::RecordError => RawExecEvent::RecordError(event),
        ExecEventType::BlockReject => RawExecEvent::BlockReject(event),
        ExecEventType::BlockPerfEvmEnter => RawExecEvent::BlockPerfEvmEnter(event),
        ExecEventType::BlockPerfEvmExit => RawExecEvent::BlockPerfEvmExit(event),
        ExecEventType::BlockEnd => RawExecEvent::BlockEnd(event),
        ExecEventType::TxnAccessListEntry => RawExecEvent::TxnAccessListEntry(event),
        ExecEventType::TxnAuthListEntry => RawExecEvent::TxnAuthListEntry(event),
        ExecEventType::TxnHeaderEnd => RawExecEvent::TxnHeaderEnd(event),
        ExecEventType::TxnReject => RawExecEvent::TxnReject(event),
        ExecEventType::TxnPerfEvmEnter => RawExecEvent::TxnPerfEvmEnter(event),
        ExecEventType::TxnPerfEvmExit => RawExecEvent::TxnPerfEvmExit(event),
        ExecEventType::TxnEvmOutput => RawExecEvent::TxnEvmOutput(event),
        ExecEventType::TxnLog => RawExecEvent::TxnLog(event),
        ExecEventType::TxnCallFrame => RawExecEvent::TxnCallFrame(event),
        ExecEventType::TxnEnd => RawExecEvent::TxnEnd(event),
        ExecEventType::AccountAccessListHeader => RawExecEvent::AccountAccessListHeader(event),
        ExecEventType::AccountAccess => RawExecEvent::AccountAccess(event),
        ExecEventType::StorageAccess => RawExecEvent::StorageAccess(event),
        ExecEventType::EvmError => RawExecEvent::EvmError(event),
        ExecEventType::BlockStart
        | ExecEventType::BlockQc
        | ExecEventType::BlockFinalized
        | ExecEventType::BlockVerified
        | ExecEventType::TxnHeaderStart => {
            unreachable!("metadata-rich event types are handled before known_raw_event")
        }
    }
}

/// Converts a snapshot descriptor envelope into a raw execution-event envelope.
#[must_use]
pub fn raw_event_from_snapshot(
    envelope: EventEnvelope<SnapshotDescriptor>,
) -> EventEnvelope<RawExecEvent> {
    let schema_hash = envelope.meta.schema_hash;
    let raw_event = RawExecEvent::from_snapshot_descriptor(envelope.payload, schema_hash);
    let descriptor = raw_event.descriptor();
    let meta = EventMeta {
        seqno: descriptor.seqno,
        record_epoch_nanos: descriptor.record_epoch_nanos,
        event_kind: descriptor.event_kind.clone(),
        source: envelope.meta.source,
        block: envelope.meta.block,
        txn: envelope.meta.txn,
        flow: descriptor.flow,
        commit_state: envelope.meta.commit_state,
        schema_hash,
    };

    EventEnvelope::new(raw_event, meta)
}

/// Converts a descriptor flow-extension array into framework flow tags.
#[must_use]
pub const fn flow_tags_from_content_ext(content_ext: [u64; 4]) -> FlowTags {
    FlowTags {
        block_seqno: nonzero_u64(content_ext[0]),
        txn_id: nonzero_u64(content_ext[1]),
        account_index: nonzero_u64(content_ext[2]),
    }
}

/// Builds fixture snapshot descriptors for tests and examples.
///
/// # Errors
///
/// Returns an error when the payload length cannot fit the snapshot descriptor field.
pub fn fixture_snapshot_descriptor(
    seqno: u64,
    event_type: u16,
    record_epoch_nanos: u64,
    content_ext: [u64; 4],
    payload: Vec<u8>,
) -> Result<SnapshotDescriptor> {
    let payload_size = u32::try_from(payload.len()).map_err(|_| {
        Error::Message(format!(
            "fixture payload length {} exceeds u32",
            payload.len()
        ))
    })?;

    Ok(SnapshotDescriptor {
        seqno,
        event_type,
        payload_size,
        record_epoch_nanos,
        payload_buf_offset: 0,
        content_ext,
        payload,
    })
}

fn parse_block_tag(payload: &[u8]) -> Option<RawBlockTag> {
    Some(RawBlockTag {
        block_id: read_b256(payload, 0)?,
        proposed_block_number: read_u64(payload, 32)?,
    })
}

fn read_b256(payload: &[u8], offset: usize) -> Option<B256> {
    let bytes: [u8; 32] = payload
        .get(offset..offset.checked_add(32)?)?
        .try_into()
        .ok()?;
    Some(B256::from(bytes))
}

fn read_u64(payload: &[u8], offset: usize) -> Option<u64> {
    let bytes: [u8; 8] = payload
        .get(offset..offset.checked_add(8)?)?
        .try_into()
        .ok()?;
    Some(u64::from_le_bytes(bytes))
}

const fn nonzero_u64(value: u64) -> Option<u64> {
    if value == 0 {
        None
    } else {
        Some(value)
    }
}

/// Builds a fixture block-tag payload.
#[must_use]
pub fn fixture_block_tag_payload(block_id: B256, proposed_block_number: u64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(40);
    payload.extend_from_slice(block_id.as_slice());
    payload.extend_from_slice(&proposed_block_number.to_le_bytes());
    payload
}

/// Builds a fixture block-verified payload.
#[must_use]
pub fn fixture_block_verified_payload(block_number: u64) -> Vec<u8> {
    block_number.to_le_bytes().to_vec()
}

/// Builds a fixture transaction-header-start payload.
#[must_use]
pub fn fixture_txn_header_start_payload(txn_hash: B256) -> Vec<u8> {
    txn_hash.as_slice().to_vec()
}

/// Builds an event envelope from a fixture snapshot descriptor.
///
/// # Errors
///
/// Returns an error when fixture descriptor construction fails.
pub fn fixture_raw_envelope(
    seqno: u64,
    event_type: ExecEventType,
    flow: [u64; 4],
    payload: Vec<u8>,
) -> Result<EventEnvelope<RawExecEvent>> {
    let descriptor =
        fixture_snapshot_descriptor(seqno, event_type.as_u16(), seqno * 10, flow, payload)?;
    Ok(raw_event_from_snapshot(EventEnvelope::new(
        descriptor,
        EventMeta {
            seqno,
            record_epoch_nanos: seqno * 10,
            event_kind: EventKind::Unknown(event_type.as_u16()),
            source: EventSourceKind::Fixture,
            block: None,
            txn: None,
            flow: FlowTags::default(),
            commit_state: CommitState::Unknown,
            schema_hash: Some(B256::from([1_u8; 32])),
        },
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert(
        event_type: u16,
        payload: Vec<u8>,
        content_ext: [u64; 4],
    ) -> EventEnvelope<RawExecEvent> {
        let descriptor =
            fixture_snapshot_descriptor(7, event_type, 100, content_ext, payload).expect("fixture");
        raw_event_from_snapshot(EventEnvelope::new(
            descriptor,
            EventMeta {
                seqno: 7,
                record_epoch_nanos: 100,
                event_kind: EventKind::Unknown(event_type),
                source: EventSourceKind::Fixture,
                block: None,
                txn: None,
                flow: FlowTags::default(),
                commit_state: CommitState::Unknown,
                schema_hash: Some(B256::from([2_u8; 32])),
            },
        ))
    }

    #[test]
    fn raw_event_known_types_map_to_event_kinds() {
        for event_type in 0_u16..=24 {
            let raw_type = ExecEventType::from_u16(event_type).expect("known event type");
            let envelope = convert(event_type, Vec::new(), [0; 4]);

            assert_eq!(envelope.meta.event_kind, raw_type.event_kind());
            assert_eq!(envelope.payload.descriptor().event_type, event_type);
            assert_eq!(envelope.payload.inspect_label(), raw_type.label());
        }
    }

    #[test]
    fn raw_event_unknown_type_is_preserved() {
        let envelope = convert(777, vec![1, 2, 3], [0; 4]);

        let RawExecEvent::Unknown(event) = envelope.payload else {
            panic!("expected unknown event");
        };

        assert_eq!(envelope.meta.event_kind, EventKind::Unknown(777));
        assert_eq!(event.descriptor.event_type, 777);
        assert_eq!(event.payload, vec![1, 2, 3]);
    }

    #[test]
    fn raw_event_descriptor_metadata_is_carried() {
        let envelope = convert(ExecEventType::TxnLog.as_u16(), vec![4, 5], [11, 12, 13, 0]);

        assert_eq!(envelope.seqno(), 7);
        assert_eq!(envelope.meta.record_epoch_nanos, 100);
        assert_eq!(envelope.meta.flow.block_seqno, Some(11));
        assert_eq!(envelope.meta.flow.txn_id, Some(12));
        assert_eq!(envelope.meta.flow.account_index, Some(13));
        assert_eq!(envelope.meta.schema_hash, Some(B256::from([2_u8; 32])));
        assert_eq!(envelope.payload.payload(), &[4, 5]);
    }

    #[test]
    fn raw_event_parses_block_tag_prefix() {
        let block_id = B256::from([9_u8; 32]);
        let payload = fixture_block_tag_payload(block_id, 42);
        let envelope = convert(ExecEventType::BlockStart.as_u16(), payload, [0; 4]);

        let RawExecEvent::BlockStart(event) = envelope.payload else {
            panic!("expected block start event");
        };

        assert_eq!(
            event.block,
            Some(RawBlockTag {
                block_id,
                proposed_block_number: 42,
            })
        );
    }

    #[test]
    fn raw_event_parses_verified_block_number_boundary() {
        let payload = fixture_block_verified_payload(u64::MAX);
        let envelope = convert(ExecEventType::BlockVerified.as_u16(), payload, [0; 4]);

        let RawExecEvent::BlockVerified(event) = envelope.payload else {
            panic!("expected block verified event");
        };

        assert_eq!(event.block_number, Some(u64::MAX));
    }

    #[test]
    fn raw_event_short_timestamp_related_payload_does_not_panic() {
        let envelope = convert(ExecEventType::BlockVerified.as_u16(), vec![1, 2, 3], [0; 4]);

        let RawExecEvent::BlockVerified(event) = envelope.payload else {
            panic!("expected block verified event");
        };

        assert_eq!(event.block_number, None);
    }

    #[test]
    fn raw_event_parses_txn_hash_prefix() {
        let txn_hash = B256::from([3_u8; 32]);
        let payload = fixture_txn_header_start_payload(txn_hash);
        let envelope = convert(ExecEventType::TxnHeaderStart.as_u16(), payload, [0; 4]);

        let RawExecEvent::TxnHeaderStart(event) = envelope.payload else {
            panic!("expected txn header start event");
        };

        assert_eq!(event.txn_hash, Some(txn_hash));
    }
}
