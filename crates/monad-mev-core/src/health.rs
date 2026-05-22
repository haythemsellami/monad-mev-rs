use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::{EventEnvelope, EventKind, EventSourceKind, RunMode, B256};

/// Item emitted by a framework source or decoder stream.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StreamItem<T> {
    /// Successfully decoded event.
    Event(EventEnvelope<T>),
    /// Descriptor sequence gap.
    Gap(GapEvent),
    /// Payload has already been overwritten.
    PayloadExpired(PayloadExpired),
    /// Source schema does not match compiled decoder schema.
    SchemaMismatch(SchemaMismatch),
    /// Source reached its end.
    SourceEnded,
}

impl<T> StreamItem<T> {
    /// Returns true when this item is `SourceEnded`.
    #[must_use]
    pub const fn is_source_end(&self) -> bool {
        matches!(self, Self::SourceEnded)
    }
}

/// Event-ring descriptor sequence gap.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GapEvent {
    /// Sequence number the reader expected next.
    pub expected_seqno: u64,
    /// Sequence number the reader actually observed.
    pub observed_seqno: u64,
    /// Number of missed descriptors.
    pub missed_count: u64,
    /// Source where the gap occurred.
    pub source: EventSourceKind,
}

impl GapEvent {
    /// Creates a new gap event and computes `missed_count`.
    #[must_use]
    pub const fn new(expected_seqno: u64, observed_seqno: u64, source: EventSourceKind) -> Self {
        Self {
            expected_seqno,
            observed_seqno,
            missed_count: observed_seqno.saturating_sub(expected_seqno),
            source,
        }
    }
}

impl Display for GapEvent {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "event stream gap on {}: expected seqno {}, observed seqno {}, missed {} event(s)",
            self.source, self.expected_seqno, self.observed_seqno, self.missed_count
        )
    }
}

/// Event payload expiration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PayloadExpired {
    /// Descriptor sequence number whose payload expired.
    pub seqno: u64,
    /// Event kind, if known.
    pub event_kind: Option<EventKind>,
    /// Source where expiration occurred.
    pub source: EventSourceKind,
}

impl Display for PayloadExpired {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.event_kind {
            Some(kind) => write!(
                formatter,
                "payload expired on {} for seqno {} ({kind})",
                self.source, self.seqno
            ),
            None => write!(
                formatter,
                "payload expired on {} for seqno {}",
                self.source, self.seqno
            ),
        }
    }
}

/// Source schema compatibility failure.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SchemaMismatch {
    /// Compiled schema hash expected by the decoder.
    pub expected: B256,
    /// Schema hash reported by the source, when available.
    pub observed: Option<B256>,
    /// Source where mismatch occurred.
    pub source: EventSourceKind,
}

impl Display for SchemaMismatch {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self.observed {
            Some(observed) => write!(
                formatter,
                "schema mismatch on {}: expected {}, observed {}",
                self.source, self.expected, observed
            ),
            None => write!(
                formatter,
                "schema mismatch on {}: expected {}, observed <missing>",
                self.source, self.expected
            ),
        }
    }
}

/// Policy used after detecting a stream gap.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapPolicy {
    /// Treat the gap as fatal.
    FailFast,
    /// Log the gap and continue processing.
    LogAndContinue,
    /// Enter risk-off behavior, then fail.
    RiskOffThenFail,
}

impl GapPolicy {
    /// Returns the default gap policy for a run mode.
    #[must_use]
    pub const fn default_for_run_mode(mode: RunMode) -> Self {
        match mode {
            RunMode::SnapshotReplay | RunMode::UnitTest => Self::FailFast,
            RunMode::CliInspect => Self::LogAndContinue,
            RunMode::LiveObserve => Self::RiskOffThenFail,
        }
    }
}
