use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

use monad_mev_core::{Error, EventSourceKind, GapEvent, GapPolicy, Result, StreamItem, B256};
use serde::{Deserialize, Serialize};

use crate::{
    normalize_raw_event, normalize_stream_item, CommitStateIssue, CommitStateTracker,
    ExecEventSource, ExecutionEventPoller, RawExecEvent, SchemaPolicy, SchemaValidation,
    SourceInfo, TransactionFlowSummary, TxnFlowTracker, EXPECTED_EXEC_CONTENT_TYPE,
};

/// Default shared-memory ring name used by Monad execution events.
pub const DEFAULT_LIVE_RING_NAME: &str = "monad-exec-events";

/// Live observe configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LiveConfig {
    /// Event ring name, resolved under the default shared-memory directory.
    pub ring_name: Option<String>,
    /// Explicit event ring path.
    pub ring_path: Option<PathBuf>,
    /// Maximum observe duration in milliseconds.
    pub duration_millis: Option<u64>,
    /// Poll interval in milliseconds.
    pub poll_interval_millis: u64,
    /// Bounded channel capacity between poller and consumer.
    pub channel_capacity: usize,
    /// Gap policy for live observation.
    pub gap_policy: GapPolicy,
    /// Schema validation policy.
    pub schema_policy: SchemaPolicy,
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            ring_name: Some(DEFAULT_LIVE_RING_NAME.to_owned()),
            ring_path: None,
            duration_millis: None,
            poll_interval_millis: 10,
            channel_capacity: 1024,
            gap_policy: GapPolicy::RiskOffThenFail,
            schema_policy: SchemaPolicy::Warn,
        }
    }
}

impl LiveConfig {
    /// Creates config for a named event ring.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            ring_name: Some(name.into()),
            ..Self::default()
        }
    }

    /// Creates config for an explicit event ring path.
    #[must_use]
    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self {
            ring_name: None,
            ring_path: Some(path.into()),
            ..Self::default()
        }
    }

    /// Sets observe duration from CLI syntax such as `10s`, `250ms`, or `1m`.
    ///
    /// # Errors
    ///
    /// Returns an error when the duration is not positive or uses an unsupported suffix.
    pub fn with_duration(mut self, value: &str) -> Result<Self> {
        self.duration_millis = Some(parse_duration_millis(value)?);
        Ok(self)
    }
}

/// Parses duration strings accepted by live observe CLI options.
///
/// # Errors
///
/// Returns an error for unsupported or non-positive durations.
pub fn parse_duration_millis(value: &str) -> Result<u64> {
    let (number, multiplier) = if let Some(number) = value.strip_suffix("ms") {
        (number, 1)
    } else if let Some(number) = value.strip_suffix('s') {
        (number, 1_000)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 60_000)
    } else {
        return Err(Error::Message(format!(
            "duration `{value}` must end with `ms`, `s`, or `m`"
        )));
    };
    let parsed = number
        .parse::<u64>()
        .map_err(|err| Error::Message(format!("invalid duration `{value}`: {err}")))?;
    if parsed == 0 {
        return Err(Error::Message(format!(
            "duration `{value}` must be positive"
        )));
    }
    parsed
        .checked_mul(multiplier)
        .ok_or_else(|| Error::Message(format!("duration `{value}` overflows milliseconds")))
}

/// Returns the default directory used for named live event rings.
#[must_use]
pub fn default_event_ring_dir() -> PathBuf {
    std::env::var_os("MONAD_MEV_EVENT_RING_DIR")
        .map_or_else(|| PathBuf::from("/dev/shm"), PathBuf::from)
}

/// Resolves an event ring path from config.
///
/// # Errors
///
/// Returns an error when neither explicit path nor ring name is configured.
pub fn resolve_event_ring_path(config: &LiveConfig) -> Result<PathBuf> {
    if let Some(path) = &config.ring_path {
        return Ok(path.clone());
    }

    let name = config
        .ring_name
        .as_deref()
        .ok_or_else(|| Error::Message("live config requires ring_name or ring_path".to_owned()))?;
    Ok(default_event_ring_dir().join(name))
}

/// Metadata and observe-only operations for a live execution event ring.
#[derive(Debug)]
pub struct LiveEventRingSource {
    config: LiveConfig,
    info: SourceInfo,
    #[cfg(all(feature = "sdk", target_os = "linux"))]
    reader: Option<crate::sdk_live::SdkLiveReader>,
}

impl LiveEventRingSource {
    /// Opens a live event ring source.
    ///
    /// # Errors
    ///
    /// Returns an error on unsupported platforms or when the ring path is not readable.
    #[cfg_attr(
        not(all(feature = "sdk", target_os = "linux")),
        allow(clippy::needless_pass_by_value)
    )]
    pub fn open(config: LiveConfig) -> Result<Self> {
        if !cfg!(target_os = "linux") {
            return Err(Error::Message(
                "live event ring observation requires Linux in v0.1".to_owned(),
            ));
        }

        let path = resolve_event_ring_path(&config)?;
        crate::validate_readable_path(&path)?;
        if config.channel_capacity == 0 {
            return Err(Error::Message(
                "live channel capacity must be greater than zero".to_owned(),
            ));
        }

        #[cfg(all(feature = "sdk", target_os = "linux"))]
        {
            let reader = crate::sdk_live::SdkLiveReader::open(
                path.clone(),
                config.poll_interval_millis,
                config.channel_capacity,
            )?;
            let mut source =
                Self::from_existing_path(config, path, Some(crate::sdk_live::schema_hash()));
            source.reader = Some(reader);
            Ok(source)
        }

        #[cfg(not(all(feature = "sdk", target_os = "linux")))]
        {
            let _ = path;
            Err(Error::Message(
                "live event-ring observation requires the `sdk` feature".to_owned(),
            ))
        }
    }

    /// Creates source metadata for tests or already-validated paths.
    #[must_use]
    pub fn from_existing_path(
        config: LiveConfig,
        path: impl Into<PathBuf>,
        schema_hash: Option<B256>,
    ) -> Self {
        let mut info = SourceInfo::path(EventSourceKind::Live, path)
            .with_content_type(EXPECTED_EXEC_CONTENT_TYPE);
        if let Some(schema_hash) = schema_hash {
            info = info.with_schema_hash(schema_hash);
        }
        Self {
            config,
            info,
            #[cfg(all(feature = "sdk", target_os = "linux"))]
            reader: None,
        }
    }

    /// Returns live config.
    #[must_use]
    pub const fn config(&self) -> &LiveConfig {
        &self.config
    }

    /// Polls one descriptor from the live source.
    ///
    /// # Errors
    ///
    /// Returns SDK reader failures or an error if this source was constructed
    /// for metadata inspection without opening a reader.
    pub fn poll_descriptor(&mut self) -> Result<Option<StreamItem<RawExecEvent>>> {
        #[cfg(all(feature = "sdk", target_os = "linux"))]
        {
            self.reader
                .as_ref()
                .ok_or_else(|| Error::Message("live SDK reader is not open".to_owned()))?
                .try_next()
        }

        #[cfg(not(all(feature = "sdk", target_os = "linux")))]
        Err(Error::Message(
            "live event-ring polling requires Linux and the `sdk` feature".to_owned(),
        ))
    }

    /// Validates live source metadata against an expected schema hash.
    ///
    /// # Errors
    ///
    /// Returns an error on content-type mismatch or required schema mismatch.
    pub fn validate_live_source(&self, expected_schema_hash: B256) -> Result<SchemaValidation> {
        self.validate_source(expected_schema_hash, self.config.schema_policy)
    }
}

impl ExecEventSource for LiveEventRingSource {
    fn source_info(&self) -> &SourceInfo {
        &self.info
    }
}

/// Normalized, non-blocking live stream with block and transaction context attached.
#[derive(Debug)]
pub struct LiveExecutionEventStream {
    source: LiveEventRingSource,
    commit_states: CommitStateTracker,
    transaction_flows: TxnFlowTracker,
}

impl LiveExecutionEventStream {
    /// Opens a live stream from an event-ring configuration.
    ///
    /// # Errors
    ///
    /// Returns source startup and SDK reader failures.
    pub fn open(config: LiveConfig) -> Result<Self> {
        LiveEventRingSource::open(config).map(Self::from_source)
    }

    /// Wraps an already-open live source.
    #[must_use]
    pub fn from_source(source: LiveEventRingSource) -> Self {
        Self {
            source,
            commit_states: CommitStateTracker::new(),
            transaction_flows: TxnFlowTracker::new(),
        }
    }

    /// Returns the underlying source metadata and configuration.
    #[must_use]
    pub const fn source(&self) -> &LiveEventRingSource {
        &self.source
    }

    /// Returns commit-state issues detected while normalizing live events.
    #[must_use]
    pub fn commit_state_issues(&self) -> &[CommitStateIssue] {
        self.commit_states.issues()
    }

    /// Returns transaction-flow counters for the live stream.
    #[must_use]
    pub fn transaction_flow_summary(&self) -> TransactionFlowSummary {
        self.transaction_flows.summary()
    }

    fn normalize_tracked_item(
        commit_states: &mut CommitStateTracker,
        transaction_flows: &mut TxnFlowTracker,
        item: StreamItem<RawExecEvent>,
    ) -> StreamItem<crate::ChainEvent> {
        match item {
            StreamItem::Event(envelope) => {
                let envelope = commit_states.observe(envelope);
                let envelope = transaction_flows.observe(envelope).envelope;
                StreamItem::Event(normalize_raw_event(envelope))
            }
            other => normalize_stream_item(other),
        }
    }
}

impl ExecutionEventPoller for LiveExecutionEventStream {
    fn poll_next(&mut self) -> Result<Option<StreamItem<crate::ChainEvent>>> {
        let Some(item) = self.source.poll_descriptor()? else {
            return Ok(None);
        };
        Ok(Some(Self::normalize_tracked_item(
            &mut self.commit_states,
            &mut self.transaction_flows,
            item,
        )))
    }
}

/// Live observation counters.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct LiveMetrics {
    /// Raw descriptors observed.
    pub descriptors_seen: u64,
    /// Normalized events emitted.
    pub events_emitted: u64,
    /// Sequence gaps observed.
    pub gaps: u64,
    /// Payload expiration records observed.
    pub payload_expired: u64,
    /// Schema mismatch records observed.
    pub schema_mismatches: u64,
    /// Source-ended records observed.
    pub source_ended: u64,
}

impl LiveMetrics {
    fn record<T>(&mut self, item: &StreamItem<T>) {
        match item {
            StreamItem::Event(_) => {
                self.descriptors_seen += 1;
                self.events_emitted += 1;
            }
            StreamItem::Gap(_) => self.gaps += 1,
            StreamItem::PayloadExpired(_) => self.payload_expired += 1,
            StreamItem::SchemaMismatch(_) => self.schema_mismatches += 1,
            StreamItem::SourceEnded => self.source_ended += 1,
        }
    }
}

/// Creates the bounded live poller channel.
///
/// # Errors
///
/// Returns an error when capacity is zero.
pub fn bounded_live_channel<T>(capacity: usize) -> Result<(SyncSender<T>, Receiver<T>)> {
    if capacity == 0 {
        return Err(Error::Message(
            "live channel capacity must be greater than zero".to_owned(),
        ));
    }
    Ok(sync_channel(capacity))
}

/// Applies live gap policy to a gap event.
#[must_use]
pub const fn live_gap_policy_action(policy: GapPolicy, gap: &GapEvent) -> LiveGapAction {
    match policy {
        GapPolicy::FailFast => LiveGapAction::Fail,
        GapPolicy::LogAndContinue => LiveGapAction::Continue,
        GapPolicy::RiskOffThenFail => {
            if gap.missed_count == 0 {
                LiveGapAction::Continue
            } else {
                LiveGapAction::RiskOffThenFail
            }
        }
    }
}

/// Live gap handling action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveGapAction {
    /// Continue observe mode.
    Continue,
    /// Stop immediately.
    Fail,
    /// Stop after risk-off notification.
    RiskOffThenFail,
}

/// Converts raw live stream items to normalized chain stream items.
#[must_use]
pub fn normalize_live_stream_item(item: StreamItem<RawExecEvent>) -> StreamItem<crate::ChainEvent> {
    normalize_stream_item(item)
}

/// Runs an observe-only loop over prebuilt stream items for tests and examples.
#[must_use]
pub fn observe_fake_stream<T>(items: impl IntoIterator<Item = StreamItem<T>>) -> LiveMetrics {
    let mut metrics = LiveMetrics::default();
    for item in items {
        metrics.record(&item);
        if item.is_source_end() {
            break;
        }
    }
    metrics
}

/// Returns whether this host can open Monad live event rings directly.
#[must_use]
pub const fn host_supports_live_event_ring() -> bool {
    cfg!(target_os = "linux")
}

/// Returns a stable live availability message for diagnostics.
#[must_use]
pub const fn live_availability_reason() -> &'static str {
    if host_supports_live_event_ring() {
        "live event-ring observation is available on this host"
    } else {
        "live event-ring observation requires Linux in v0.1"
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::mpsc::TrySendError;

    use monad_mev_core::{EventEnvelope, EventKind, EventMeta, FlowTags, PayloadExpired};

    use super::*;
    use crate::{fixture_block_tag_payload, fixture_raw_envelope, ExecEventType};

    fn hash(byte: u8) -> B256 {
        B256::from([byte; 32])
    }

    #[test]
    fn live_stream_attaches_block_and_transaction_context_before_normalizing() {
        let block_id = hash(9);
        let mut commits = CommitStateTracker::new();
        let mut flows = TxnFlowTracker::new();
        let start = fixture_raw_envelope(
            10,
            ExecEventType::BlockStart,
            [0; 4],
            fixture_block_tag_payload(block_id, 77),
        )
        .expect("block start fixture");
        let log = fixture_raw_envelope(11, ExecEventType::TxnLog, [10, 4, 0, 0], Vec::new())
            .expect("transaction log fixture");

        let _ = LiveExecutionEventStream::normalize_tracked_item(
            &mut commits,
            &mut flows,
            StreamItem::Event(start),
        );
        let normalized = LiveExecutionEventStream::normalize_tracked_item(
            &mut commits,
            &mut flows,
            StreamItem::Event(log),
        );
        let StreamItem::Event(normalized) = normalized else {
            panic!("expected normalized event");
        };

        assert_eq!(
            normalized.meta.commit_state,
            monad_mev_core::CommitState::Proposed
        );
        assert_eq!(normalized.block_number(), Some(77));
        assert_eq!(normalized.txn_idx(), Some(3));
    }

    #[test]
    fn live_duration_parser_accepts_expected_units() {
        assert_eq!(parse_duration_millis("250ms").expect("duration"), 250);
        assert_eq!(parse_duration_millis("10s").expect("duration"), 10_000);
        assert_eq!(parse_duration_millis("2m").expect("duration"), 120_000);
        assert!(parse_duration_millis("0s").is_err());
        assert!(parse_duration_millis("10").is_err());
    }

    #[test]
    fn live_config_resolves_named_ring_under_default_dir() {
        let config = LiveConfig::named("monad-test-ring");
        let path = resolve_event_ring_path(&config).expect("path");

        assert!(path.ends_with(Path::new("monad-test-ring")));
    }

    #[test]
    fn live_config_explicit_path_wins() {
        let config = LiveConfig::path("/tmp/monad-ring");
        let path = resolve_event_ring_path(&config).expect("path");

        assert_eq!(path, PathBuf::from("/tmp/monad-ring"));
    }

    #[test]
    fn live_source_validates_content_and_schema() {
        let source = LiveEventRingSource::from_existing_path(
            LiveConfig::default(),
            "/tmp/ring",
            Some(hash(1)),
        );

        let result = source
            .validate_live_source(hash(1))
            .expect("schema should match");

        assert_eq!(result, SchemaValidation::Match);
    }

    #[test]
    fn live_bounded_channel_applies_backpressure() {
        let (sender, _receiver) = bounded_live_channel::<u64>(1).expect("channel");

        sender.try_send(1).expect("first item fits");
        let error = sender.try_send(2).expect_err("second item should block");

        assert!(matches!(error, TrySendError::Full(2)));
    }

    #[test]
    fn live_gap_policy_risks_off_on_missed_descriptors() {
        let gap = GapEvent::new(1, 3, EventSourceKind::Live);

        assert_eq!(
            live_gap_policy_action(GapPolicy::RiskOffThenFail, &gap),
            LiveGapAction::RiskOffThenFail
        );
        assert_eq!(
            live_gap_policy_action(GapPolicy::LogAndContinue, &gap),
            LiveGapAction::Continue
        );
    }

    #[test]
    fn live_fake_observer_counts_stream_items() {
        let event = EventEnvelope::new(
            (),
            EventMeta {
                seqno: 1,
                record_epoch_nanos: 1,
                event_kind: EventKind::TxnLog,
                source: EventSourceKind::Live,
                block: None,
                txn: None,
                flow: FlowTags::default(),
                commit_state: monad_mev_core::CommitState::Unknown,
                schema_hash: None,
            },
        );
        let metrics = observe_fake_stream([
            StreamItem::Event(event),
            StreamItem::Gap(GapEvent::new(2, 4, EventSourceKind::Live)),
            StreamItem::PayloadExpired(PayloadExpired {
                seqno: 4,
                event_kind: Some(EventKind::TxnLog),
                source: EventSourceKind::Live,
            }),
            StreamItem::SourceEnded,
        ]);

        assert_eq!(metrics.descriptors_seen, 1);
        assert_eq!(metrics.events_emitted, 1);
        assert_eq!(metrics.gaps, 1);
        assert_eq!(metrics.payload_expired, 1);
        assert_eq!(metrics.source_ended, 1);
    }

    #[test]
    fn live_raw_items_normalize_to_chain_items() {
        let raw = fixture_raw_envelope(1, ExecEventType::TxnLog, [0; 4], Vec::new())
            .expect("fixture raw");

        let item = normalize_live_stream_item(StreamItem::Event(raw));

        assert!(matches!(item, StreamItem::Event(_)));
    }

    #[test]
    fn live_open_reports_unsupported_host_without_panicking() {
        if host_supports_live_event_ring() {
            return;
        }

        let error = LiveEventRingSource::open(LiveConfig::default()).expect_err("unsupported");

        assert!(error.to_string().contains("requires Linux"));
    }
}
