use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use monad_mev_core::{Error, EventKind, EventSourceKind, GapPolicy, Result, StreamItem};
use monad_mev_events::{ChainEvent, ExecutionEventPoller, StreamHealthAction, StreamHealthTracker};
use serde::{Deserialize, Serialize};

use crate::{Engine, EngineRun, Opportunity};

/// Boundary at which a continuous runner invokes opportunity detectors.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionBoundary {
    /// Invoke detectors after every normalized event.
    EveryEvent,
    /// Invoke detectors after each transaction end.
    #[default]
    TransactionEnd,
    /// Invoke detectors after each block end.
    BlockEnd,
}

impl DetectionBoundary {
    const fn matches(self, event_kind: &EventKind) -> bool {
        match self {
            Self::EveryEvent => true,
            Self::TransactionEnd => matches!(event_kind, EventKind::TxnEnd),
            Self::BlockEnd => matches!(event_kind, EventKind::BlockEnd),
        }
    }
}

/// Policy for stream faults other than descriptor gaps.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamFaultPolicy {
    /// Record the fault and continue observing.
    Continue,
    /// Return an error immediately.
    Fail,
    /// Enter risk-off mode and stop the runner without processing later items.
    #[default]
    RiskOffThenFail,
}

/// Configuration for a continuous engine runner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContinuousEngineConfig {
    /// Source identity used by implicit sequence-gap tracking.
    pub source: EventSourceKind,
    /// Policy for explicit or implicit descriptor gaps.
    pub gap_policy: GapPolicy,
    /// Policy for overwritten payloads.
    pub payload_expired_policy: StreamFaultPolicy,
    /// Policy for source/decoder schema mismatches.
    pub schema_mismatch_policy: StreamFaultPolicy,
    /// Boundary at which detectors run.
    pub detection_boundary: DetectionBoundary,
    /// Backoff after a healthy poll returns no item.
    pub idle_sleep_millis: u64,
}

impl ContinuousEngineConfig {
    /// Creates risk-off defaults for a source.
    #[must_use]
    pub const fn for_source(source: EventSourceKind) -> Self {
        Self {
            source,
            gap_policy: GapPolicy::RiskOffThenFail,
            payload_expired_policy: StreamFaultPolicy::RiskOffThenFail,
            schema_mismatch_policy: StreamFaultPolicy::RiskOffThenFail,
            detection_boundary: DetectionBoundary::TransactionEnd,
            idle_sleep_millis: 1,
        }
    }

    /// Sets the detector boundary.
    #[must_use]
    pub const fn with_detection_boundary(mut self, boundary: DetectionBoundary) -> Self {
        self.detection_boundary = boundary;
        self
    }

    /// Sets idle-poll backoff in milliseconds. Zero yields the current thread.
    #[must_use]
    pub const fn with_idle_sleep_millis(mut self, idle_sleep_millis: u64) -> Self {
        self.idle_sleep_millis = idle_sleep_millis;
        self
    }
}

impl Default for ContinuousEngineConfig {
    fn default() -> Self {
        Self::for_source(EventSourceKind::Live)
    }
}

/// Thread-safe cooperative shutdown handle.
#[derive(Clone, Debug, Default)]
pub struct ShutdownHandle {
    requested: Arc<AtomicBool>,
}

impl ShutdownHandle {
    /// Requests shutdown. The runner stops before its next source poll.
    pub fn request_shutdown(&self) {
        self.requested.store(true, Ordering::Release);
    }

    /// Returns whether shutdown has been requested.
    #[must_use]
    pub fn is_shutdown_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }
}

/// Reason a continuous run stopped normally.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuousStopReason {
    /// A caller requested cooperative shutdown.
    ShutdownRequested,
    /// A finite source emitted its terminal item.
    SourceEnded,
    /// Stream integrity policy moved the runner into risk-off mode.
    RiskOff {
        /// Human-readable health fault.
        reason: String,
    },
}

/// Control decision after processing one stream item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContinuousControl {
    /// Poll the next item.
    Continue,
    /// Stop with a stable reason.
    Stop(ContinuousStopReason),
}

/// Continuous-stream counters independent of protocol adapters.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContinuousEngineReport {
    /// Source poll attempts.
    pub polls: u64,
    /// Healthy polls that had no item ready.
    pub idle_polls: u64,
    /// Stream items received, including health and terminal items.
    pub stream_items: u64,
    /// Descriptor gaps observed.
    pub gaps: u64,
    /// Payload expirations observed.
    pub payload_expired: u64,
    /// Schema mismatches observed.
    pub schema_mismatches: u64,
    /// Detector invocations.
    pub detector_runs: u64,
    /// Repeated opportunity versions suppressed.
    pub duplicate_opportunities: u64,
    /// True after risk-off policy has fired.
    pub risk_off: bool,
}

/// Snapshot returned when a continuous runner stops normally.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContinuousEngineRun {
    /// Persistent lifecycle state and emitted opportunities.
    pub engine: EngineRun,
    /// Continuous source and detector counters.
    pub continuous: ContinuousEngineReport,
    /// Normal stop reason.
    pub stop_reason: ContinuousStopReason,
}

/// Stateful engine runner for non-blocking finite or live sources.
pub struct ContinuousEngineRunner {
    engine: Engine,
    config: ContinuousEngineConfig,
    run: EngineRun,
    health: StreamHealthTracker,
    report: ContinuousEngineReport,
    emitted_opportunities: BTreeSet<(String, u64, u64)>,
}

impl ContinuousEngineRunner {
    /// Creates a stateful runner around a configured engine.
    #[must_use]
    pub fn new(engine: Engine, config: ContinuousEngineConfig) -> Self {
        let health = StreamHealthTracker::new(config.source.clone(), config.gap_policy);
        Self {
            engine,
            config,
            run: EngineRun::default(),
            health,
            report: ContinuousEngineReport::default(),
            emitted_opportunities: BTreeSet::new(),
        }
    }

    /// Returns accumulated lifecycle state.
    #[must_use]
    pub const fn engine_run(&self) -> &EngineRun {
        &self.run
    }

    /// Returns continuous counters.
    #[must_use]
    pub const fn report(&self) -> &ContinuousEngineReport {
        &self.report
    }

    /// Processes one normalized stream item.
    ///
    /// # Errors
    ///
    /// Returns adapter, detector, or fail-fast stream-health errors.
    pub fn process_item(&mut self, item: StreamItem<ChainEvent>) -> Result<ContinuousControl> {
        self.report.stream_items += 1;
        let health_action = self.health.observe_item(&item)?;
        self.sync_health_counters();
        if matches!(health_action, StreamHealthAction::RiskOffThenFail) {
            return Ok(self.enter_risk_off(health_reason(&item)));
        }

        match item {
            StreamItem::Event(event) => {
                let event_kind = event.meta.event_kind.clone();
                self.engine.process_event(&mut self.run, &event)?;
                if self.config.detection_boundary.matches(&event_kind) {
                    self.detect_opportunities()?;
                }
                Ok(ContinuousControl::Continue)
            }
            StreamItem::Gap(_) => Ok(ContinuousControl::Continue),
            StreamItem::PayloadExpired(expired) => {
                self.apply_fault_policy(self.config.payload_expired_policy, expired.to_string())
            }
            StreamItem::SchemaMismatch(mismatch) => {
                self.apply_fault_policy(self.config.schema_mismatch_policy, mismatch.to_string())
            }
            StreamItem::SourceEnded => {
                self.detect_opportunities()?;
                Ok(ContinuousControl::Stop(ContinuousStopReason::SourceEnded))
            }
        }
    }

    /// Polls until cooperative shutdown, source completion, or risk-off policy.
    ///
    /// # Errors
    ///
    /// Returns source, adapter, detector, or fail-fast stream-health errors.
    pub fn run_until_stopped(
        &mut self,
        source: &mut impl ExecutionEventPoller,
        shutdown: &ShutdownHandle,
    ) -> Result<ContinuousEngineRun> {
        loop {
            if shutdown.is_shutdown_requested() {
                return Ok(self.snapshot(ContinuousStopReason::ShutdownRequested));
            }

            self.report.polls += 1;
            let Some(item) = source.poll_next()? else {
                self.report.idle_polls += 1;
                self.idle_backoff();
                continue;
            };

            if let ContinuousControl::Stop(reason) = self.process_item(item)? {
                return Ok(self.snapshot(reason));
            }
        }
    }

    fn detect_opportunities(&mut self) -> Result<()> {
        self.report.detector_runs += 1;
        let opportunities = self.engine.detect(&self.run.store)?;
        for opportunity in opportunities {
            let key = opportunity_key(&opportunity);
            if self.emitted_opportunities.insert(key) {
                self.run.report.opportunities += 1;
                self.run.opportunities.push(opportunity);
            } else {
                self.report.duplicate_opportunities += 1;
            }
        }
        Ok(())
    }

    fn apply_fault_policy(
        &mut self,
        policy: StreamFaultPolicy,
        reason: String,
    ) -> Result<ContinuousControl> {
        match policy {
            StreamFaultPolicy::Continue => Ok(ContinuousControl::Continue),
            StreamFaultPolicy::Fail => Err(Error::Message(reason)),
            StreamFaultPolicy::RiskOffThenFail => Ok(self.enter_risk_off(reason)),
        }
    }

    fn enter_risk_off(&mut self, reason: String) -> ContinuousControl {
        self.report.risk_off = true;
        ContinuousControl::Stop(ContinuousStopReason::RiskOff { reason })
    }

    fn sync_health_counters(&mut self) {
        let health = self.health.report();
        self.report.gaps = health.gaps;
        self.report.payload_expired = health.payload_expired;
        self.report.schema_mismatches = health.schema_mismatches;
    }

    fn idle_backoff(&self) {
        if self.config.idle_sleep_millis == 0 {
            thread::yield_now();
        } else {
            thread::sleep(Duration::from_millis(self.config.idle_sleep_millis));
        }
    }

    fn snapshot(&self, stop_reason: ContinuousStopReason) -> ContinuousEngineRun {
        ContinuousEngineRun {
            engine: self.run.clone(),
            continuous: self.report.clone(),
            stop_reason,
        }
    }
}

fn opportunity_key(opportunity: &Opportunity) -> (String, u64, u64) {
    (
        opportunity.id.clone(),
        opportunity.state_version.revision,
        opportunity.state_version.last_seqno,
    )
}

fn health_reason(item: &StreamItem<ChainEvent>) -> String {
    match item {
        StreamItem::Event(event) => {
            format!("implicit descriptor gap before seqno {}", event.meta.seqno)
        }
        StreamItem::Gap(gap) => gap.to_string(),
        StreamItem::PayloadExpired(expired) => expired.to_string(),
        StreamItem::SchemaMismatch(mismatch) => mismatch.to_string(),
        StreamItem::SourceEnded => "source ended".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use monad_mev_core::{
        Address, EventKind, EventSourceKind, GapEvent, PayloadExpired, SchemaMismatch, B256,
    };
    use monad_mev_events::{
        fixture_log_payload, fixture_raw_envelope, normalize_raw_event, ExecEventType,
    };

    use super::*;
    use crate::FixtureDomainAdapter;

    struct ScriptedPoller {
        items: VecDeque<Option<StreamItem<ChainEvent>>>,
    }

    impl ScriptedPoller {
        fn new(items: impl IntoIterator<Item = Option<StreamItem<ChainEvent>>>) -> Self {
            Self {
                items: items.into_iter().collect(),
            }
        }
    }

    impl ExecutionEventPoller for ScriptedPoller {
        fn poll_next(&mut self) -> Result<Option<StreamItem<ChainEvent>>> {
            Ok(self
                .items
                .pop_front()
                .unwrap_or(Some(StreamItem::SourceEnded)))
        }
    }

    fn event(
        seqno: u64,
        event_type: ExecEventType,
        address: Address,
        data: &[u8],
    ) -> StreamItem<ChainEvent> {
        let payload = if matches!(event_type, ExecEventType::TxnLog) {
            fixture_log_payload(address, &[], data).expect("fixture log")
        } else {
            Vec::new()
        };
        let raw =
            fixture_raw_envelope(seqno, event_type, [1, 1, 0, 0], payload).expect("fixture event");
        StreamItem::Event(normalize_raw_event(raw))
    }

    fn fixture_engine(address: Address) -> Engine {
        let adapter = FixtureDomainAdapter::new(address);
        Engine::new()
            .with_event_adapter(Box::new(adapter.clone()))
            .with_state_adapter(Box::new(adapter.clone()))
            .with_detector(Box::new(adapter))
    }

    #[test]
    fn continuous_runner_retains_state_and_detects_at_transaction_end() {
        let address = Address::from([7_u8; 20]);
        let mut source = ScriptedPoller::new([
            None,
            Some(event(10, ExecEventType::TxnLog, address, &[200])),
            Some(event(11, ExecEventType::TxnEnd, address, &[])),
            None,
            Some(StreamItem::SourceEnded),
        ]);
        let config =
            ContinuousEngineConfig::for_source(EventSourceKind::Fixture).with_idle_sleep_millis(0);
        let mut runner = ContinuousEngineRunner::new(fixture_engine(address), config);

        let run = runner
            .run_until_stopped(&mut source, &ShutdownHandle::default())
            .expect("continuous run");

        assert_eq!(run.stop_reason, ContinuousStopReason::SourceEnded);
        assert_eq!(run.continuous.polls, 5);
        assert_eq!(run.continuous.idle_polls, 2);
        assert_eq!(run.continuous.detector_runs, 2);
        assert_eq!(run.continuous.duplicate_opportunities, 1);
        assert_eq!(run.engine.report.events_seen, 2);
        assert_eq!(run.engine.opportunities.len(), 1);
    }

    #[test]
    fn live_gap_moves_default_runner_to_risk_off() {
        let mut runner = ContinuousEngineRunner::new(
            Engine::new(),
            ContinuousEngineConfig::default().with_idle_sleep_millis(0),
        );

        let control = runner
            .process_item(StreamItem::Gap(GapEvent::new(1, 3, EventSourceKind::Live)))
            .expect("risk-off is a controlled stop");

        assert!(matches!(
            control,
            ContinuousControl::Stop(ContinuousStopReason::RiskOff { .. })
        ));
        assert!(runner.report().risk_off);
        assert_eq!(runner.report().gaps, 1);
    }

    #[test]
    fn payload_expiration_moves_default_runner_to_risk_off() {
        let mut runner = ContinuousEngineRunner::new(
            Engine::new(),
            ContinuousEngineConfig::default().with_idle_sleep_millis(0),
        );

        let control = runner
            .process_item(StreamItem::PayloadExpired(PayloadExpired {
                seqno: 8,
                event_kind: Some(EventKind::TxnLog),
                source: EventSourceKind::Live,
            }))
            .expect("risk-off is a controlled stop");

        assert!(matches!(
            control,
            ContinuousControl::Stop(ContinuousStopReason::RiskOff { .. })
        ));
        assert!(runner.report().risk_off);
        assert_eq!(runner.report().payload_expired, 1);
    }

    #[test]
    fn schema_mismatch_can_be_configured_fail_fast() {
        let mut config = ContinuousEngineConfig::default().with_idle_sleep_millis(0);
        config.schema_mismatch_policy = StreamFaultPolicy::Fail;
        let mut runner = ContinuousEngineRunner::new(Engine::new(), config);

        let error = runner
            .process_item(StreamItem::SchemaMismatch(SchemaMismatch {
                expected: B256::from([1_u8; 32]),
                observed: Some(B256::from([2_u8; 32])),
                source: EventSourceKind::Live,
            }))
            .expect_err("schema mismatch should fail fast");

        assert!(error.to_string().contains("schema mismatch"));
        assert_eq!(runner.report().schema_mismatches, 1);
        assert!(!runner.report().risk_off);
    }

    struct ShutdownPoller {
        polls: u64,
        shutdown: ShutdownHandle,
    }

    impl ExecutionEventPoller for ShutdownPoller {
        fn poll_next(&mut self) -> Result<Option<StreamItem<ChainEvent>>> {
            self.polls += 1;
            if self.polls == 2 {
                self.shutdown.request_shutdown();
            }
            Ok(None)
        }
    }

    #[test]
    fn shutdown_stops_after_the_current_poll() {
        let shutdown = ShutdownHandle::default();
        let mut source = ShutdownPoller {
            polls: 0,
            shutdown: shutdown.clone(),
        };
        let config = ContinuousEngineConfig::for_source(EventSourceKind::Synthetic)
            .with_idle_sleep_millis(0);
        let mut runner = ContinuousEngineRunner::new(Engine::new(), config);

        let run = runner
            .run_until_stopped(&mut source, &shutdown)
            .expect("shutdown run");

        assert_eq!(run.stop_reason, ContinuousStopReason::ShutdownRequested);
        assert_eq!(run.continuous.polls, 2);
        assert_eq!(run.continuous.idle_polls, 2);
    }
}
