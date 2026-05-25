//! Generic engine runtime for `monad-mev-rs`.

use std::collections::{BTreeMap, BTreeSet};

use monad_mev_core::{
    Action, Address, CommitState, Error, EventEnvelope, EventKind, RecordAction, Result,
    StreamItem, B256,
};
use monad_mev_events::{ChainEvent, LogEvent};
use monad_mev_store::{InMemoryStateStore, SourceEventRef, StateKey, StateUpdate, StateVersion};
use serde::{Deserialize, Serialize};

/// Capture filter for normalized Execution Events.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CaptureFilter {
    /// Stable filter name.
    pub name: String,
    /// Allowed log emitter addresses. Empty means any address.
    pub addresses: BTreeSet<Address>,
    /// Allowed log topic0 values. Empty means any topic.
    pub topic0: BTreeSet<B256>,
    /// Allowed framework event kinds. Empty means any kind.
    pub event_kinds: BTreeSet<EventKind>,
    /// Required transaction index.
    pub txn_idx: Option<u64>,
    /// Inclusive first proposed block number.
    pub from_block: Option<u64>,
    /// Inclusive last proposed block number.
    pub to_block: Option<u64>,
    /// Minimum commit state required.
    pub min_commit_state: Option<CommitState>,
}

impl CaptureFilter {
    /// Creates a named filter.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Adds an address criterion.
    #[must_use]
    pub fn with_address(mut self, address: Address) -> Self {
        self.addresses.insert(address);
        self
    }

    /// Adds a topic0 criterion.
    #[must_use]
    pub fn with_topic0(mut self, topic: B256) -> Self {
        self.topic0.insert(topic);
        self
    }

    /// Adds an event-kind criterion.
    #[must_use]
    pub fn with_event_kind(mut self, kind: EventKind) -> Self {
        self.event_kinds.insert(kind);
        self
    }

    /// Sets a minimum commit state.
    #[must_use]
    pub fn with_min_commit_state(mut self, state: CommitState) -> Self {
        self.min_commit_state = Some(state);
        self
    }

    /// Returns whether an event matches this filter.
    #[must_use]
    pub fn matches(&self, event: &EventEnvelope<ChainEvent>) -> bool {
        self.explain(event).matched
    }

    /// Returns a deterministic explanation for a match decision.
    #[must_use]
    pub fn explain(&self, event: &EventEnvelope<ChainEvent>) -> CaptureDecision {
        let mut reasons = Vec::new();

        if !self.event_kinds.is_empty() && !self.event_kinds.contains(&event.meta.event_kind) {
            reasons.push(format!("event kind {} not selected", event.meta.event_kind));
            return CaptureDecision::ignored(&self.name, reasons);
        }

        if let Some(txn_idx) = self.txn_idx {
            if event.txn_idx() != Some(txn_idx) {
                reasons.push(format!(
                    "txn_idx {:?} did not match {txn_idx}",
                    event.txn_idx()
                ));
                return CaptureDecision::ignored(&self.name, reasons);
            }
        }

        if let Some(from_block) = self.from_block {
            if event
                .block_number()
                .map_or(true, |block| block < from_block)
            {
                reasons.push(format!(
                    "block {:?} is before {from_block}",
                    event.block_number()
                ));
                return CaptureDecision::ignored(&self.name, reasons);
            }
        }

        if let Some(to_block) = self.to_block {
            if event.block_number().map_or(true, |block| block > to_block) {
                reasons.push(format!(
                    "block {:?} is after {to_block}",
                    event.block_number()
                ));
                return CaptureDecision::ignored(&self.name, reasons);
            }
        }

        if let Some(min_state) = self.min_commit_state {
            if !event.meta.commit_state.satisfies_min(min_state) {
                reasons.push(format!(
                    "commit state {:?} does not satisfy {:?}",
                    event.meta.commit_state, min_state
                ));
                return CaptureDecision::ignored(&self.name, reasons);
            }
        }

        if !self.addresses.is_empty() || !self.topic0.is_empty() {
            let ChainEvent::Log(log) = &event.payload else {
                reasons.push("address/topic filters only match log events".to_owned());
                return CaptureDecision::ignored(&self.name, reasons);
            };
            if !self.addresses.is_empty()
                && log
                    .address
                    .map_or(true, |addr| !self.addresses.contains(&addr))
            {
                reasons.push(format!("address {:?} not selected", log.address));
                return CaptureDecision::ignored(&self.name, reasons);
            }
            if !self.topic0.is_empty()
                && log
                    .topic0()
                    .map_or(true, |topic| !self.topic0.contains(&topic))
            {
                reasons.push(format!("topic0 {:?} not selected", log.topic0()));
                return CaptureDecision::ignored(&self.name, reasons);
            }
        }

        reasons.push("matched all criteria".to_owned());
        CaptureDecision {
            capture: self.name.clone(),
            matched: true,
            reasons,
        }
    }
}

/// Captured-event decision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CaptureDecision {
    /// Filter name.
    pub capture: String,
    /// True when the event matched.
    pub matched: bool,
    /// Deterministic explanation.
    pub reasons: Vec<String>,
}

impl CaptureDecision {
    fn ignored(capture: &str, reasons: Vec<String>) -> Self {
        Self {
            capture: capture.to_owned(),
            matched: false,
            reasons,
        }
    }
}

/// Captured event with the filter that matched it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapturedEvent {
    /// Capture filter name.
    pub capture: String,
    /// Event envelope.
    pub event: EventEnvelope<ChainEvent>,
}

/// Applies filters to stream items while preserving health events.
#[must_use]
pub fn capture_stream(
    items: impl IntoIterator<Item = StreamItem<ChainEvent>>,
    filters: &[CaptureFilter],
) -> Vec<CapturedEvent> {
    let mut captured = Vec::new();
    for item in items {
        let StreamItem::Event(event) = item else {
            continue;
        };
        for filter in filters {
            if filter.matches(&event) {
                captured.push(CapturedEvent {
                    capture: filter.name.clone(),
                    event: event.clone(),
                });
            }
        }
    }
    captured
}

/// Package subscription declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Subscription {
    /// Subscription name.
    pub name: String,
    /// Capture filter to apply.
    pub filter: CaptureFilter,
}

/// Package manifest for external strategy or adapter packages.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackageManifest {
    /// Package name.
    pub name: String,
    /// Package version.
    pub version: String,
    /// Package capabilities.
    pub capabilities: BTreeSet<PackageCapability>,
    /// Event subscriptions.
    pub subscriptions: Vec<Subscription>,
}

impl PackageManifest {
    /// Validates package metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when required fields are missing.
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::Message("package name is required".to_owned()));
        }
        if self.version.trim().is_empty() {
            return Err(Error::Message("package version is required".to_owned()));
        }
        if self.capabilities.is_empty() {
            return Err(Error::Message(
                "at least one package capability is required".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Capability exposed by a package.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageCapability {
    /// Provides capture filters.
    Capture,
    /// Adapts framework events.
    EventAdapter,
    /// Updates state.
    StateAdapter,
    /// Detects opportunities.
    OpportunityDetector,
    /// Builds transaction candidates.
    TransactionCandidateBuilder,
    /// Provides a strategy implementation.
    Strategy,
}

/// Adapter metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AdapterMetadata {
    /// Adapter name.
    pub name: String,
    /// Adapter version.
    pub version: String,
    /// Adapter capabilities.
    pub capabilities: BTreeSet<PackageCapability>,
}

/// Adapter-produced domain event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DomainEvent {
    /// Domain event kind.
    pub kind: String,
    /// Domain payload.
    pub payload: serde_json::Value,
    /// Source framework event.
    pub source: SourceEventRef,
}

/// Converts framework events into domain events.
pub trait EventAdapter {
    /// Adapter metadata.
    fn metadata(&self) -> AdapterMetadata;

    /// Converts one event.
    ///
    /// # Errors
    ///
    /// Returns adapter-specific failures.
    fn adapt_event(&mut self, event: &EventEnvelope<ChainEvent>) -> Result<Vec<DomainEvent>>;
}

/// Converts domain events into state updates.
pub trait StateAdapter {
    /// Converts one domain event into state updates.
    ///
    /// # Errors
    ///
    /// Returns adapter-specific failures.
    fn state_updates(&mut self, event: &DomainEvent) -> Result<Vec<StateUpdate>>;
}

/// Detects opportunities from state.
pub trait OpportunityDetector {
    /// Detects opportunities from store state.
    ///
    /// # Errors
    ///
    /// Returns detector-specific failures.
    fn detect(&mut self, store: &InMemoryStateStore) -> Result<Vec<Opportunity>>;
}

/// Boxed event adapter.
pub type BoxEventAdapter = Box<dyn EventAdapter>;

/// Boxed state adapter.
pub type BoxStateAdapter = Box<dyn StateAdapter>;

/// Boxed opportunity detector.
pub type BoxOpportunityDetector = Box<dyn OpportunityDetector>;

/// Generic signal emitted by adapters or strategies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    /// Signal kind.
    pub kind: String,
    /// Signal strength in basis points.
    pub strength_bps: i64,
    /// Source events.
    pub sources: Vec<SourceEventRef>,
    /// Additional structured details.
    pub attributes: BTreeMap<String, serde_json::Value>,
}

/// Generic opportunity record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Opportunity {
    /// Stable opportunity ID.
    pub id: String,
    /// Opportunity kind.
    pub kind: String,
    /// Human-readable summary.
    pub summary: String,
    /// State version used to derive the opportunity.
    pub state_version: StateVersion,
    /// Source events used to derive the opportunity.
    pub sources: Vec<SourceEventRef>,
    /// Additional structured details.
    pub attributes: BTreeMap<String, serde_json::Value>,
}

impl Opportunity {
    /// Converts the opportunity into a recording action.
    #[must_use]
    pub fn into_record_action(self) -> Action {
        Action::Record(RecordAction {
            topic: format!("opportunity.{}", self.kind),
            payload: serde_json::to_value(self).unwrap_or_else(|err| {
                serde_json::json!({
                    "serialization_error": err.to_string(),
                })
            }),
        })
    }
}

/// Rejection emitted when an opportunity cannot be created or advanced.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OpportunityRejection {
    /// Rejection kind.
    pub kind: String,
    /// Human-readable reason.
    pub reason: String,
    /// Source events involved.
    pub sources: Vec<SourceEventRef>,
}

/// Deterministic engine report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineReport {
    /// Stream events seen.
    pub events_seen: u64,
    /// Events captured by filters.
    pub events_captured: u64,
    /// Domain events emitted by adapters.
    pub domain_events: u64,
    /// State updates applied.
    pub state_updates: u64,
    /// Opportunities emitted.
    pub opportunities: u64,
    /// Rejections emitted.
    pub rejections: u64,
}

/// Output from a lifecycle run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineRun {
    /// Run report.
    pub report: EngineReport,
    /// Final state.
    pub store: InMemoryStateStore,
    /// Opportunities.
    pub opportunities: Vec<Opportunity>,
    /// Rejections.
    pub rejections: Vec<OpportunityRejection>,
}

/// Protocol-agnostic lifecycle engine.
pub struct Engine {
    captures: Vec<CaptureFilter>,
    event_adapters: Vec<BoxEventAdapter>,
    state_adapters: Vec<BoxStateAdapter>,
    detectors: Vec<BoxOpportunityDetector>,
}

impl Engine {
    /// Creates an empty engine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            captures: Vec::new(),
            event_adapters: Vec::new(),
            state_adapters: Vec::new(),
            detectors: Vec::new(),
        }
    }

    /// Adds a capture filter.
    #[must_use]
    pub fn with_capture(mut self, capture: CaptureFilter) -> Self {
        self.captures.push(capture);
        self
    }

    /// Adds an event adapter.
    #[must_use]
    pub fn with_event_adapter(mut self, adapter: BoxEventAdapter) -> Self {
        self.event_adapters.push(adapter);
        self
    }

    /// Adds a state adapter.
    #[must_use]
    pub fn with_state_adapter(mut self, adapter: BoxStateAdapter) -> Self {
        self.state_adapters.push(adapter);
        self
    }

    /// Adds an opportunity detector.
    #[must_use]
    pub fn with_detector(mut self, detector: BoxOpportunityDetector) -> Self {
        self.detectors.push(detector);
        self
    }

    /// Runs the engine over a stream.
    ///
    /// # Errors
    ///
    /// Returns adapter or detector errors.
    pub fn run(
        &mut self,
        items: impl IntoIterator<Item = StreamItem<ChainEvent>>,
    ) -> Result<EngineRun> {
        let mut run = EngineRun::default();

        for item in items {
            let StreamItem::Event(event) = item else {
                continue;
            };
            run.report.events_seen += 1;
            let matched = self.captures.is_empty()
                || self.captures.iter().any(|capture| capture.matches(&event));
            if !matched {
                continue;
            }

            run.report.events_captured += 1;
            for adapter in &mut self.event_adapters {
                let domain_events = adapter.adapt_event(&event)?;
                run.report.domain_events += domain_events.len() as u64;
                for domain_event in domain_events {
                    for state_adapter in &mut self.state_adapters {
                        let updates = state_adapter.state_updates(&domain_event)?;
                        run.report.state_updates += updates.len() as u64;
                        run.store.apply_all(updates);
                    }
                }
            }
        }

        for detector in &mut self.detectors {
            let opportunities = detector.detect(&run.store)?;
            run.report.opportunities += opportunities.len() as u64;
            run.opportunities.extend(opportunities);
        }

        Ok(run)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

/// Generic monitor strategy over opportunity records.
#[derive(Clone, Debug, Default)]
pub struct OpportunityMonitor {
    records: Vec<Action>,
}

impl OpportunityMonitor {
    /// Records opportunities as actions.
    pub fn record(&mut self, opportunities: impl IntoIterator<Item = Opportunity>) {
        self.records.extend(
            opportunities
                .into_iter()
                .map(Opportunity::into_record_action),
        );
    }

    /// Returns recorded actions.
    #[must_use]
    pub fn actions(&self) -> &[Action] {
        &self.records
    }
}

/// Fixture adapter used by the core lifecycle tests.
#[derive(Clone, Debug)]
pub struct FixtureDomainAdapter {
    watched_address: Address,
    stale_after_seqno: Option<u64>,
}

impl FixtureDomainAdapter {
    /// Creates a fixture adapter.
    #[must_use]
    pub fn new(watched_address: Address) -> Self {
        Self {
            watched_address,
            stale_after_seqno: None,
        }
    }

    /// Rejects opportunities derived after this sequence number.
    #[must_use]
    pub fn with_stale_after_seqno(mut self, seqno: u64) -> Self {
        self.stale_after_seqno = Some(seqno);
        self
    }
}

impl EventAdapter for FixtureDomainAdapter {
    fn metadata(&self) -> AdapterMetadata {
        let mut capabilities = BTreeSet::new();
        capabilities.insert(PackageCapability::EventAdapter);
        AdapterMetadata {
            name: "fixture-domain".to_owned(),
            version: "0.0.0".to_owned(),
            capabilities,
        }
    }

    fn adapt_event(&mut self, event: &EventEnvelope<ChainEvent>) -> Result<Vec<DomainEvent>> {
        let ChainEvent::Log(LogEvent {
            address: Some(address),
            data,
            ..
        }) = &event.payload
        else {
            return Ok(Vec::new());
        };
        if *address != self.watched_address || data.is_empty() {
            return Ok(Vec::new());
        }

        Ok(vec![DomainEvent {
            kind: "fixture_signal".to_owned(),
            payload: serde_json::json!({
                "score": data[0],
                "stale_after_seqno": self.stale_after_seqno,
            }),
            source: SourceEventRef::from(event),
        }])
    }
}

impl StateAdapter for FixtureDomainAdapter {
    fn state_updates(&mut self, event: &DomainEvent) -> Result<Vec<StateUpdate>> {
        Ok(vec![StateUpdate {
            key: StateKey::new("fixture-domain", "latest-signal"),
            value: event.payload.clone(),
            source: event.source.clone(),
        }])
    }
}

impl OpportunityDetector for FixtureDomainAdapter {
    fn detect(&mut self, state_store: &InMemoryStateStore) -> Result<Vec<Opportunity>> {
        let key = StateKey::new("fixture-domain", "latest-signal");
        let Some(entry) = state_store.get(&key) else {
            return Ok(Vec::new());
        };
        if entry
            .value
            .get("stale_after_seqno")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|limit| entry.version.last_seqno > limit)
        {
            return Ok(Vec::new());
        }
        let score = entry
            .value
            .get("score")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        if score < 128 {
            return Ok(Vec::new());
        }

        Ok(vec![Opportunity {
            id: format!("fixture-domain-{}", entry.version.last_seqno),
            kind: "fixture_signal".to_owned(),
            summary: format!("fixture signal score {score}"),
            state_version: entry.version,
            sources: entry.sources.clone(),
            attributes: BTreeMap::from([("score".to_owned(), serde_json::json!(score))]),
        }])
    }
}

#[cfg(test)]
mod tests {
    use monad_mev_core::{BlockRef, EventMeta, EventSourceKind, FlowTags, TxnRef};
    use monad_mev_events::{
        event_topic, fixture_log_payload, fixture_raw_envelope, normalize_raw_event, ExecEventType,
    };

    use super::*;

    fn word(value: u8) -> B256 {
        B256::from([value; 32])
    }

    fn log_event(seqno: u64, address: Address, data: &[u8]) -> EventEnvelope<ChainEvent> {
        let payload = fixture_log_payload(address, &[event_topic("Fixture(uint256)")], data)
            .expect("fixture log");
        let raw = fixture_raw_envelope(seqno, ExecEventType::TxnLog, [1, 2, 0, 0], payload)
            .expect("fixture raw");
        let mut event = normalize_raw_event(raw);
        event.meta.block = Some(BlockRef {
            block_id: word(9),
            proposed_block_number: 10,
            block_start_seqno: 1,
        });
        event.meta.txn = Some(TxnRef {
            txn_idx: 7,
            txn_hash: None,
        });
        event.meta.commit_state = CommitState::Finalized;
        event
    }

    fn block_event(seqno: u64) -> EventEnvelope<ChainEvent> {
        EventEnvelope::new(
            ChainEvent::UnknownRaw(monad_mev_events::UnknownChainEvent {
                kind: EventKind::BlockStart,
                raw: fixture_raw_envelope(seqno, ExecEventType::BlockStart, [0; 4], Vec::new())
                    .expect("fixture raw")
                    .payload,
            }),
            EventMeta {
                seqno,
                record_epoch_nanos: seqno,
                event_kind: EventKind::BlockStart,
                source: EventSourceKind::Fixture,
                block: None,
                txn: None,
                flow: FlowTags::default(),
                commit_state: CommitState::Proposed,
                schema_hash: None,
            },
        )
    }

    #[test]
    fn protocol_capture_filters_address_topic_kind_and_commit_state() {
        let address = Address::from([1_u8; 20]);
        let topic = event_topic("Fixture(uint256)");
        let event = log_event(5, address, &[200]);
        let filter = CaptureFilter::named("fixture")
            .with_address(address)
            .with_topic0(topic)
            .with_event_kind(EventKind::TxnLog)
            .with_min_commit_state(CommitState::Finalized);

        let decision = filter.explain(&event);

        assert!(decision.matched);
        assert_eq!(decision.reasons, vec!["matched all criteria"]);
    }

    #[test]
    fn protocol_capture_explains_ignored_events() {
        let event = block_event(6);
        let filter = CaptureFilter::named("logs").with_event_kind(EventKind::TxnLog);

        let decision = filter.explain(&event);

        assert!(!decision.matched);
        assert!(decision.reasons[0].contains("event kind"));
    }

    #[test]
    fn capture_stream_is_deterministic() {
        let address = Address::from([1_u8; 20]);
        let filters = vec![CaptureFilter::named("fixture").with_address(address)];
        let items = vec![
            StreamItem::Event(block_event(1)),
            StreamItem::Event(log_event(2, address, &[200])),
        ];

        let captured = capture_stream(items, &filters);

        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].capture, "fixture");
        assert_eq!(captured[0].event.seqno(), 2);
    }

    #[test]
    fn package_manifest_requires_capabilities() {
        let manifest = PackageManifest {
            name: "pkg".to_owned(),
            version: "0.1.0".to_owned(),
            capabilities: BTreeSet::new(),
            subscriptions: Vec::new(),
        };

        assert!(manifest.validate().is_err());
    }

    #[test]
    fn adapter_metadata_serializes_capabilities() {
        let adapter = FixtureDomainAdapter::new(Address::from([1_u8; 20]));
        let metadata = adapter.metadata();
        let json = serde_json::to_value(metadata).expect("json");

        assert_eq!(json["name"], "fixture-domain");
        assert_eq!(json["capabilities"][0], "event_adapter");
    }

    #[test]
    fn lifecycle_fixture_emits_opportunity() {
        let address = Address::from([1_u8; 20]);
        let adapter = FixtureDomainAdapter::new(address);
        let mut engine = Engine::new()
            .with_capture(CaptureFilter::named("fixture").with_address(address))
            .with_event_adapter(Box::new(adapter.clone()))
            .with_state_adapter(Box::new(adapter.clone()))
            .with_detector(Box::new(adapter));

        let run = engine
            .run(vec![StreamItem::Event(log_event(10, address, &[200]))])
            .expect("engine run");

        assert_eq!(run.report.events_seen, 1);
        assert_eq!(run.report.events_captured, 1);
        assert_eq!(run.report.domain_events, 1);
        assert_eq!(run.report.state_updates, 1);
        assert_eq!(run.report.opportunities, 1);
        assert_eq!(run.opportunities[0].id, "fixture-domain-10");
    }

    #[test]
    fn lifecycle_fixture_ignores_non_matching_state() {
        let address = Address::from([1_u8; 20]);
        let adapter = FixtureDomainAdapter::new(address);
        let mut engine = Engine::new()
            .with_capture(CaptureFilter::named("fixture").with_address(address))
            .with_event_adapter(Box::new(adapter.clone()))
            .with_state_adapter(Box::new(adapter.clone()))
            .with_detector(Box::new(adapter));

        let run = engine
            .run(vec![StreamItem::Event(log_event(10, address, &[10]))])
            .expect("engine run");

        assert_eq!(run.report.opportunities, 0);
        assert!(run.opportunities.is_empty());
    }

    #[test]
    fn lifecycle_fixture_rejects_stale_state() {
        let address = Address::from([1_u8; 20]);
        let adapter = FixtureDomainAdapter::new(address).with_stale_after_seqno(5);
        let mut engine = Engine::new()
            .with_capture(CaptureFilter::named("fixture").with_address(address))
            .with_event_adapter(Box::new(adapter.clone()))
            .with_state_adapter(Box::new(adapter.clone()))
            .with_detector(Box::new(adapter));

        let run = engine
            .run(vec![StreamItem::Event(log_event(10, address, &[200]))])
            .expect("engine run");

        assert_eq!(run.report.opportunities, 0);
    }

    #[test]
    fn opportunity_monitor_records_json_actions() {
        let address = Address::from([1_u8; 20]);
        let adapter = FixtureDomainAdapter::new(address);
        let mut engine = Engine::new()
            .with_capture(CaptureFilter::named("fixture").with_address(address))
            .with_event_adapter(Box::new(adapter.clone()))
            .with_state_adapter(Box::new(adapter.clone()))
            .with_detector(Box::new(adapter));
        let run = engine
            .run(vec![StreamItem::Event(log_event(10, address, &[200]))])
            .expect("engine run");
        let mut monitor = OpportunityMonitor::default();

        monitor.record(run.opportunities);

        assert_eq!(monitor.actions().len(), 1);
    }
}
