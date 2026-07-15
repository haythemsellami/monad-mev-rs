use monad_mev_core::{
    Error, EventEnvelope, EventSourceKind, GapEvent, GapPolicy, PayloadExpired, ReplayReport,
    Result, RunMode, SchemaMismatch, StreamItem,
};
use serde::{Deserialize, Serialize};

/// Warning counters suitable for CLI inspect summaries.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StreamHealthWarningSummary {
    /// Descriptor gaps observed.
    pub gaps: u64,
    /// Payload expirations observed.
    pub payload_expired: u64,
    /// Schema mismatches observed.
    pub schema_mismatches: u64,
}

impl StreamHealthWarningSummary {
    /// Returns true when no warning counters are set.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.gaps == 0 && self.payload_expired == 0 && self.schema_mismatches == 0
    }
}

/// Action selected after a stream health event is handled.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamHealthAction {
    /// Continue processing events.
    Continue,
    /// Enter risk-off behavior and then stop processing.
    RiskOffThenFail,
    /// Stop processing immediately.
    Fail,
}

/// Strategy-level override for a stream gap.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapPolicyOverride {
    /// Use the configured default policy.
    UseDefault,
    /// Continue processing.
    Continue,
    /// Enter risk-off behavior and then stop processing.
    RiskOffThenFail,
    /// Stop processing immediately.
    Fail,
}

/// Hook implemented by strategies that want to inspect gap decisions.
pub trait GapObserver {
    /// Called before the default gap policy is applied.
    fn on_gap(&mut self, gap: &GapEvent) -> GapPolicyOverride;
}

/// Tracks descriptor sequence numbers and emits gap events.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequenceTracker {
    expected_seqno: Option<u64>,
    source: EventSourceKind,
}

impl SequenceTracker {
    /// Creates a new tracker with no expected sequence number.
    #[must_use]
    pub const fn new(source: EventSourceKind) -> Self {
        Self {
            expected_seqno: None,
            source,
        }
    }

    /// Creates a tracker with an explicit first expected sequence number.
    #[must_use]
    pub const fn with_expected(source: EventSourceKind, expected_seqno: u64) -> Self {
        Self {
            expected_seqno: Some(expected_seqno),
            source,
        }
    }

    /// Returns the next expected sequence number.
    #[must_use]
    pub const fn expected_seqno(&self) -> Option<u64> {
        self.expected_seqno
    }

    /// Observes a sequence number and returns a gap when one is detected.
    pub fn observe(&mut self, observed_seqno: u64) -> Option<GapEvent> {
        let gap = self.expected_seqno.and_then(|expected| {
            (observed_seqno != expected)
                .then(|| GapEvent::new(expected, observed_seqno, self.source.clone()))
        });

        self.expected_seqno = observed_seqno.checked_add(1);
        gap
    }

    /// Resets tracking to a new expected sequence number.
    pub const fn reset(&mut self, expected_seqno: Option<u64>) {
        self.expected_seqno = expected_seqno;
    }
}

/// Aggregates stream health counters and gap-policy decisions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamHealthTracker {
    sequence: SequenceTracker,
    gap_policy: GapPolicy,
    report: ReplayReport,
}

impl StreamHealthTracker {
    /// Creates a tracker for a run mode using the mode's default gap policy.
    #[must_use]
    pub fn for_run_mode(source: EventSourceKind, run_mode: RunMode) -> Self {
        Self::new(source, GapPolicy::default_for_run_mode(run_mode))
    }

    /// Creates a tracker with an explicit gap policy.
    #[must_use]
    pub fn new(source: EventSourceKind, gap_policy: GapPolicy) -> Self {
        Self {
            sequence: SequenceTracker::new(source),
            gap_policy,
            report: ReplayReport::default(),
        }
    }

    /// Creates a tracker with an explicit expected first sequence number.
    #[must_use]
    pub fn with_expected(
        source: EventSourceKind,
        gap_policy: GapPolicy,
        expected_seqno: u64,
    ) -> Self {
        Self {
            sequence: SequenceTracker::with_expected(source, expected_seqno),
            gap_policy,
            report: ReplayReport::default(),
        }
    }

    /// Returns the configured gap policy.
    #[must_use]
    pub const fn gap_policy(&self) -> GapPolicy {
        self.gap_policy
    }

    /// Returns accumulated counters.
    #[must_use]
    pub const fn report(&self) -> &ReplayReport {
        &self.report
    }

    /// Returns warning counters for CLI inspect summary output.
    #[must_use]
    pub const fn warning_summary(&self) -> StreamHealthWarningSummary {
        StreamHealthWarningSummary {
            gaps: self.report.gaps,
            payload_expired: self.report.payload_expired,
            schema_mismatches: self.report.schema_mismatches,
        }
    }

    /// Consumes the tracker and returns accumulated counters.
    #[must_use]
    pub fn into_report(self) -> ReplayReport {
        self.report
    }

    /// Observes one event envelope and applies the configured gap policy.
    ///
    /// # Errors
    ///
    /// Returns an error when the selected policy is fail-fast.
    pub fn observe_event<T>(&mut self, envelope: &EventEnvelope<T>) -> Result<StreamHealthAction> {
        if let Some(gap) = self.sequence.observe(envelope.seqno()) {
            return self.handle_gap(&gap);
        }

        self.report.record_event();
        Ok(StreamHealthAction::Continue)
    }

    /// Records a stream item and applies health handling when needed.
    ///
    /// # Errors
    ///
    /// Returns an error when a gap is handled with a fail-fast policy.
    pub fn observe_item<T>(&mut self, item: &StreamItem<T>) -> Result<StreamHealthAction> {
        match item {
            StreamItem::Event(envelope) => self.observe_event(envelope),
            StreamItem::Gap(gap) => {
                self.sequence.reset(gap.observed_seqno.checked_add(1));
                self.handle_gap(gap)
            }
            StreamItem::PayloadExpired(expired) => Ok(self.handle_payload_expired(expired)),
            StreamItem::SchemaMismatch(mismatch) => Ok(self.handle_schema_mismatch(mismatch)),
            StreamItem::SourceEnded => Ok(StreamHealthAction::Continue),
        }
    }

    /// Handles a gap with an observer override.
    ///
    /// # Errors
    ///
    /// Returns an error when the observer or default policy selects fail-fast.
    pub fn handle_gap_with_observer(
        &mut self,
        gap: &GapEvent,
        observer: &mut impl GapObserver,
    ) -> Result<StreamHealthAction> {
        let action = match observer.on_gap(gap) {
            GapPolicyOverride::UseDefault => action_for_gap_policy(self.gap_policy),
            GapPolicyOverride::Continue => StreamHealthAction::Continue,
            GapPolicyOverride::RiskOffThenFail => StreamHealthAction::RiskOffThenFail,
            GapPolicyOverride::Fail => StreamHealthAction::Fail,
        };

        self.record_gap_action(gap, action)
    }

    fn handle_gap(&mut self, gap: &GapEvent) -> Result<StreamHealthAction> {
        self.record_gap_action(gap, action_for_gap_policy(self.gap_policy))
    }

    const fn handle_payload_expired(&mut self, _expired: &PayloadExpired) -> StreamHealthAction {
        self.report.record_payload_expired();
        StreamHealthAction::Continue
    }

    const fn handle_schema_mismatch(&mut self, _mismatch: &SchemaMismatch) -> StreamHealthAction {
        self.report.record_schema_mismatch();
        StreamHealthAction::Continue
    }

    fn record_gap_action(
        &mut self,
        gap: &GapEvent,
        action: StreamHealthAction,
    ) -> Result<StreamHealthAction> {
        self.report.record_gap();

        match action {
            StreamHealthAction::Continue => Ok(StreamHealthAction::Continue),
            StreamHealthAction::RiskOffThenFail => Ok(StreamHealthAction::RiskOffThenFail),
            StreamHealthAction::Fail => Err(Error::Message(gap.to_string())),
        }
    }
}

/// Returns the health action selected by a gap policy.
#[must_use]
pub const fn action_for_gap_policy(policy: GapPolicy) -> StreamHealthAction {
    match policy {
        GapPolicy::FailFast => StreamHealthAction::Fail,
        GapPolicy::LogAndContinue => StreamHealthAction::Continue,
        GapPolicy::RiskOffThenFail => StreamHealthAction::RiskOffThenFail,
    }
}

#[cfg(test)]
mod tests {
    use monad_mev_core::{CommitState, EventKind, EventMeta, FlowTags, B256};

    use super::*;

    fn envelope(seqno: u64) -> EventEnvelope<()> {
        EventEnvelope::new(
            (),
            EventMeta {
                seqno,
                record_epoch_nanos: seqno * 10,
                event_kind: EventKind::TxnLog,
                source: EventSourceKind::Fixture,
                block: None,
                txn: None,
                flow: FlowTags::default(),
                commit_state: CommitState::Unknown,
                schema_hash: Some(B256::from([1_u8; 32])),
            },
        )
    }

    #[test]
    fn gap_tracker_detects_expected_sequence_cases() {
        let mut tracker = SequenceTracker::with_expected(EventSourceKind::Fixture, 1);

        assert_eq!(tracker.observe(1), None);
        assert_eq!(tracker.observe(2), None);

        let gap = tracker.observe(5).expect("gap should be detected");
        assert_eq!(gap.expected_seqno, 3);
        assert_eq!(gap.observed_seqno, 5);
        assert_eq!(gap.missed_count, 2);
        assert_eq!(tracker.expected_seqno(), Some(6));
    }

    #[test]
    fn gap_tracker_handles_restart_like_lower_sequence() {
        let mut tracker = SequenceTracker::with_expected(EventSourceKind::Fixture, 10);

        let gap = tracker.observe(1).expect("lower seqno should be surfaced");

        assert_eq!(gap.expected_seqno, 10);
        assert_eq!(gap.observed_seqno, 1);
        assert_eq!(gap.missed_count, 0);
        assert_eq!(tracker.expected_seqno(), Some(2));
    }

    #[test]
    fn snapshot_fail_fast_returns_error_on_gap() {
        let mut tracker =
            StreamHealthTracker::with_expected(EventSourceKind::Snapshot, GapPolicy::FailFast, 1);

        tracker
            .observe_event(&envelope(1))
            .expect("first event should pass");
        let error = tracker
            .observe_event(&envelope(3))
            .expect_err("gap should fail");

        assert!(error.to_string().contains("event stream gap"));
        assert_eq!(tracker.report().gaps, 1);
    }

    #[test]
    fn inspect_policy_continues_and_counts_gap() {
        let mut tracker = StreamHealthTracker::with_expected(
            EventSourceKind::Snapshot,
            GapPolicy::LogAndContinue,
            1,
        );

        let action = tracker
            .observe_event(&envelope(3))
            .expect("inspect should continue");

        assert_eq!(action, StreamHealthAction::Continue);
        assert_eq!(tracker.report().gaps, 1);
        assert_eq!(tracker.warning_summary().gaps, 1);
    }

    #[test]
    fn payload_expiration_is_counted() {
        let mut tracker =
            StreamHealthTracker::new(EventSourceKind::Snapshot, GapPolicy::LogAndContinue);
        let expired = PayloadExpired {
            seqno: 4,
            event_kind: Some(EventKind::TxnLog),
            source: EventSourceKind::Snapshot,
        };

        let action = tracker
            .observe_item::<()>(&StreamItem::PayloadExpired(expired))
            .expect("payload expiration should not fail");

        assert_eq!(action, StreamHealthAction::Continue);
        assert_eq!(tracker.report().payload_expired, 1);
    }

    #[test]
    fn gap_observer_is_called_before_policy_decision() {
        #[derive(Default)]
        struct Observer {
            calls: u64,
        }

        impl GapObserver for Observer {
            fn on_gap(&mut self, _gap: &GapEvent) -> GapPolicyOverride {
                self.calls += 1;
                GapPolicyOverride::Continue
            }
        }

        let mut observer = Observer::default();
        let mut tracker = StreamHealthTracker::new(EventSourceKind::Snapshot, GapPolicy::FailFast);
        let gap = GapEvent::new(1, 4, EventSourceKind::Snapshot);

        let action = tracker
            .handle_gap_with_observer(&gap, &mut observer)
            .expect("observer should override fail-fast");

        assert_eq!(observer.calls, 1);
        assert_eq!(action, StreamHealthAction::Continue);
        assert_eq!(tracker.report().gaps, 1);
    }

    #[test]
    fn explicit_gap_synchronizes_the_next_expected_sequence() {
        let mut tracker =
            StreamHealthTracker::new(EventSourceKind::Live, GapPolicy::LogAndContinue);

        tracker
            .observe_item::<()>(&StreamItem::Gap(GapEvent::new(2, 5, EventSourceKind::Live)))
            .expect("explicit gap should continue");
        tracker
            .observe_event(&envelope(6))
            .expect("the event after an explicit gap should not create a duplicate gap");

        assert_eq!(tracker.report().gaps, 1);
    }
}
