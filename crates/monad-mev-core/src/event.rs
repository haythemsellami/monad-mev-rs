use std::fmt::{Display, Formatter};

pub use alloy_primitives::{keccak256, Address, B256, U256};
use serde::{Deserialize, Serialize};

/// Normalized item that passed through the framework pipeline.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope<T> {
    /// Event payload.
    pub payload: T,
    /// Metadata attached to the event payload.
    pub meta: EventMeta,
}

impl<T> EventEnvelope<T> {
    /// Creates a new event envelope.
    #[must_use]
    pub const fn new(payload: T, meta: EventMeta) -> Self {
        Self { payload, meta }
    }

    /// Maps the payload while keeping metadata unchanged.
    #[must_use]
    pub fn map_payload<U>(self, mapper: impl FnOnce(T) -> U) -> EventEnvelope<U> {
        EventEnvelope {
            payload: mapper(self.payload),
            meta: self.meta,
        }
    }

    /// Descriptor sequence number.
    #[must_use]
    pub const fn seqno(&self) -> u64 {
        self.meta.seqno
    }

    /// Proposed block number if known.
    #[must_use]
    pub fn block_number(&self) -> Option<u64> {
        self.meta
            .block
            .as_ref()
            .map(|block| block.proposed_block_number)
    }

    /// Transaction index if known.
    #[must_use]
    pub fn txn_idx(&self) -> Option<u64> {
        self.meta.txn.as_ref().map(|txn| txn.txn_idx)
    }
}

/// Metadata attached to every normalized event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventMeta {
    /// Event descriptor sequence number.
    pub seqno: u64,
    /// Source-provided record timestamp in epoch nanoseconds.
    pub record_epoch_nanos: u64,
    /// Framework event kind.
    pub event_kind: EventKind,
    /// Event source kind.
    pub source: EventSourceKind,
    /// Block context, when known.
    pub block: Option<BlockRef>,
    /// Transaction context, when known.
    pub txn: Option<TxnRef>,
    /// Raw flow tags extracted from descriptor metadata.
    pub flow: FlowTags,
    /// Current commit state inferred by the framework.
    pub commit_state: CommitState,
    /// Compiled or source schema hash, when known.
    pub schema_hash: Option<B256>,
}

/// Block context for an event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BlockRef {
    /// Monad block ID.
    pub block_id: B256,
    /// Proposed block number.
    pub proposed_block_number: u64,
    /// Sequence number of the block start event.
    pub block_start_seqno: u64,
}

/// Transaction context for an event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TxnRef {
    /// Transaction index within the associated block.
    pub txn_idx: u64,
    /// Transaction hash, when known.
    pub txn_hash: Option<B256>,
}

/// Raw flow tags extracted from descriptor metadata.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FlowTags {
    /// Sequence number of the block start event associated with this flow.
    pub block_seqno: Option<u64>,
    /// Transaction identifier from descriptor flow metadata.
    pub txn_id: Option<u64>,
    /// Account access-list index from descriptor flow metadata.
    pub account_index: Option<u64>,
}

/// Block commit state inferred from Monad consensus/execution events.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitState {
    /// The source cannot infer commit state yet.
    #[default]
    Unknown,
    /// Block has been proposed and may still be abandoned.
    Proposed,
    /// Block has received a quorum certificate.
    Voted,
    /// Block is finalized.
    Finalized,
    /// Block is verified/canonical absent hard fork.
    Verified,
    /// Block was rejected or abandoned.
    Abandoned,
}

impl CommitState {
    /// Returns true when this state satisfies a strategy's minimum state requirement.
    #[must_use]
    pub const fn satisfies_min(self, min: Self) -> bool {
        match (self.rank(), min.rank()) {
            (_, Some(0)) => true,
            (Some(current), Some(required)) => current >= required,
            (None, None) => matches!(self, Self::Abandoned) && matches!(min, Self::Abandoned),
            _ => false,
        }
    }

    const fn rank(self) -> Option<u8> {
        match self {
            Self::Unknown => Some(0),
            Self::Proposed => Some(1),
            Self::Voted => Some(2),
            Self::Finalized => Some(3),
            Self::Verified => Some(4),
            Self::Abandoned => None,
        }
    }
}

/// High-level source category for an event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSourceKind {
    /// Historical snapshot replay.
    Snapshot,
    /// Live shared-memory event ring.
    Live,
    /// Normalized test fixture.
    Fixture,
    /// Synthetic event source created by tests or examples.
    Synthetic,
    /// RPC source.
    Rpc,
    /// WebSocket source.
    WebSocket,
    /// Timer event source.
    Timer,
    /// Unknown or not-yet-classified source.
    Unknown,
}

impl Display for EventSourceKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Snapshot => "snapshot",
            Self::Live => "live",
            Self::Fixture => "fixture",
            Self::Synthetic => "synthetic",
            Self::Rpc => "rpc",
            Self::WebSocket => "websocket",
            Self::Timer => "timer",
            Self::Unknown => "unknown",
        })
    }
}

/// Framework event kind.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// No-op or empty event type.
    None,
    /// Monad execution record error.
    RecordError,
    /// Block start event.
    BlockStart,
    /// Block reject event.
    BlockReject,
    /// Block EVM performance enter marker.
    BlockPerfEvmEnter,
    /// Block EVM performance exit marker.
    BlockPerfEvmExit,
    /// Block end event.
    BlockEnd,
    /// Block quorum-certificate event.
    BlockQc,
    /// Block finalized event.
    BlockFinalized,
    /// Block verified event.
    BlockVerified,
    /// Transaction header start event.
    TxnHeaderStart,
    /// Transaction access-list entry event.
    TxnAccessListEntry,
    /// Transaction authorization-list entry event.
    TxnAuthListEntry,
    /// Transaction header end marker.
    TxnHeaderEnd,
    /// Transaction reject event.
    TxnReject,
    /// Transaction EVM performance enter marker.
    TxnPerfEvmEnter,
    /// Transaction EVM performance exit marker.
    TxnPerfEvmExit,
    /// Transaction EVM output event.
    TxnEvmOutput,
    /// Transaction log event.
    TxnLog,
    /// Transaction call-frame event.
    TxnCallFrame,
    /// Transaction end event.
    TxnEnd,
    /// Account access-list header event.
    AccountAccessListHeader,
    /// Account access event.
    AccountAccess,
    /// Storage access event.
    StorageAccess,
    /// EVM error event.
    EvmError,
    /// Framework control event.
    Control,
    /// Timer event.
    Timer,
    /// Source-specific unknown event type.
    Unknown(u16),
}

impl Display for EventKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => formatter.write_str("none"),
            Self::RecordError => formatter.write_str("record_error"),
            Self::BlockStart => formatter.write_str("block_start"),
            Self::BlockReject => formatter.write_str("block_reject"),
            Self::BlockPerfEvmEnter => formatter.write_str("block_perf_evm_enter"),
            Self::BlockPerfEvmExit => formatter.write_str("block_perf_evm_exit"),
            Self::BlockEnd => formatter.write_str("block_end"),
            Self::BlockQc => formatter.write_str("block_qc"),
            Self::BlockFinalized => formatter.write_str("block_finalized"),
            Self::BlockVerified => formatter.write_str("block_verified"),
            Self::TxnHeaderStart => formatter.write_str("txn_header_start"),
            Self::TxnAccessListEntry => formatter.write_str("txn_access_list_entry"),
            Self::TxnAuthListEntry => formatter.write_str("txn_auth_list_entry"),
            Self::TxnHeaderEnd => formatter.write_str("txn_header_end"),
            Self::TxnReject => formatter.write_str("txn_reject"),
            Self::TxnPerfEvmEnter => formatter.write_str("txn_perf_evm_enter"),
            Self::TxnPerfEvmExit => formatter.write_str("txn_perf_evm_exit"),
            Self::TxnEvmOutput => formatter.write_str("txn_evm_output"),
            Self::TxnLog => formatter.write_str("txn_log"),
            Self::TxnCallFrame => formatter.write_str("txn_call_frame"),
            Self::TxnEnd => formatter.write_str("txn_end"),
            Self::AccountAccessListHeader => formatter.write_str("account_access_list_header"),
            Self::AccountAccess => formatter.write_str("account_access"),
            Self::StorageAccess => formatter.write_str("storage_access"),
            Self::EvmError => formatter.write_str("evm_error"),
            Self::Control => formatter.write_str("control"),
            Self::Timer => formatter.write_str("timer"),
            Self::Unknown(kind) => write!(formatter, "unknown({kind})"),
        }
    }
}

/// Execution mode for default policy selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Deterministic snapshot replay.
    SnapshotReplay,
    /// CLI inspection mode.
    CliInspect,
    /// Live observe-only mode.
    LiveObserve,
    /// Unit or fixture test mode.
    UnitTest,
}

/// Payload ownership mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadMode {
    /// Copy payloads into owned framework types.
    #[default]
    Owned,
    /// Allow advanced callers to filter zero-copy payload references.
    ZeroCopyFilter,
}
