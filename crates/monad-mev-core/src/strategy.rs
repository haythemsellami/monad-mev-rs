use serde::{Deserialize, Serialize};

use crate::{Address, Error, EventEnvelope, GapEvent, Result, StreamItem, U256};

/// Strategy-level decision after a stream gap.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapDecision {
    /// Use the runner's default gap policy.
    UseDefault,
    /// Continue processing.
    Continue,
    /// Stop processing.
    Fail,
    /// Enter risk-off behavior and then stop processing.
    RiskOffThenFail,
}

/// Context passed to strategies.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StrategyContext {
    /// Stable run identifier.
    pub run_id: String,
    /// Number of events delivered to the strategy.
    pub events_seen: u64,
    /// Number of gaps delivered to the strategy.
    pub gaps_seen: u64,
}

impl StrategyContext {
    /// Creates a strategy context.
    #[must_use]
    pub fn new(run_id: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            ..Self::default()
        }
    }
}

/// Strategy interface for replay and live observe modes.
pub trait Strategy<E> {
    /// Handles one event and returns actions.
    ///
    /// # Errors
    ///
    /// Returns an error when strategy logic fails.
    fn on_event(
        &mut self,
        context: &mut StrategyContext,
        event: &EventEnvelope<E>,
    ) -> Result<Vec<Action>>;

    /// Handles a stream gap.
    ///
    /// # Errors
    ///
    /// Returns an error when strategy gap handling fails.
    fn on_gap(&mut self, _context: &mut StrategyContext, _gap: &GapEvent) -> Result<GapDecision> {
        Ok(GapDecision::UseDefault)
    }
}

/// Framework action emitted by a v0.1 strategy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Record structured data.
    Record(RecordAction),
    /// Emit an alert.
    Alert(AlertAction),
    /// Dry-run transaction candidate. v0.1 never submits it.
    SubmitTxDryRun(SubmitTxDryRun),
}

/// Structured record action.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecordAction {
    /// Record topic.
    pub topic: String,
    /// JSON payload.
    pub payload: serde_json::Value,
}

/// Alert action.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AlertAction {
    /// Alert level.
    pub level: String,
    /// Alert message.
    pub message: String,
}

/// Dry-run transaction action. This is never submitted by v0.1 executors.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubmitTxDryRun {
    /// Optional recipient. `None` represents contract creation.
    pub to: Option<Address>,
    /// Calldata or initcode.
    pub data: Vec<u8>,
    /// Native value.
    pub value: U256,
}

/// Executor receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    /// Monotonic executor receipt ID.
    pub id: u64,
    /// Whether this receipt came from dry-run behavior.
    pub dry_run: bool,
    /// Human-readable status.
    pub status: String,
    /// Executed action.
    pub action: Action,
}

/// Action executor interface.
pub trait Executor {
    /// Executes one action.
    ///
    /// # Errors
    ///
    /// Returns an error when the action is invalid or execution fails.
    fn execute(&mut self, action: Action) -> Result<ExecutionReceipt>;
}

/// Executor that records every action as JSONL.
#[derive(Clone, Debug, Default)]
pub struct RecordingExecutor {
    next_id: u64,
    receipts: Vec<ExecutionReceipt>,
    jsonl: String,
}

impl RecordingExecutor {
    /// Returns recorded receipts.
    #[must_use]
    pub fn receipts(&self) -> &[ExecutionReceipt] {
        &self.receipts
    }

    /// Returns recorded action JSONL.
    #[must_use]
    pub fn jsonl(&self) -> &str {
        &self.jsonl
    }
}

impl Executor for RecordingExecutor {
    fn execute(&mut self, action: Action) -> Result<ExecutionReceipt> {
        self.next_id += 1;
        let receipt = ExecutionReceipt {
            id: self.next_id,
            dry_run: false,
            status: "recorded".to_owned(),
            action,
        };
        self.jsonl
            .push_str(&serde_json::to_string(&receipt.action).map_err(|err| {
                Error::Message(format!("failed to serialize recorded action: {err}"))
            })?);
        self.jsonl.push('\n');
        self.receipts.push(receipt.clone());
        Ok(receipt)
    }
}

/// Executor that validates actions but never submits transactions.
#[derive(Clone, Debug, Default)]
pub struct DryRunExecutor {
    next_id: u64,
}

impl Executor for DryRunExecutor {
    fn execute(&mut self, action: Action) -> Result<ExecutionReceipt> {
        if let Action::SubmitTxDryRun(tx) = &action {
            if tx.data.is_empty() {
                return Err(Error::Message(
                    "dry-run transaction action requires non-empty data".to_owned(),
                ));
            }
        }

        self.next_id += 1;
        Ok(ExecutionReceipt {
            id: self.next_id,
            dry_run: true,
            status: "dry_run_validated".to_owned(),
            action,
        })
    }
}

/// Runs a strategy over stream items and sends actions to an executor.
///
/// # Errors
///
/// Returns an error from the strategy or executor.
pub fn run_strategy<E>(
    items: impl IntoIterator<Item = StreamItem<E>>,
    strategy: &mut impl Strategy<E>,
    executor: &mut impl Executor,
    context: &mut StrategyContext,
) -> Result<Vec<ExecutionReceipt>> {
    let mut receipts = Vec::new();

    for item in items {
        match item {
            StreamItem::Event(event) => {
                context.events_seen += 1;
                for action in strategy.on_event(context, &event)? {
                    receipts.push(executor.execute(action)?);
                }
            }
            StreamItem::Gap(gap) => {
                context.gaps_seen += 1;
                if matches!(
                    strategy.on_gap(context, &gap)?,
                    GapDecision::Fail | GapDecision::RiskOffThenFail
                ) {
                    return Err(Error::Message(gap.to_string()));
                }
            }
            StreamItem::PayloadExpired(_)
            | StreamItem::SchemaMismatch(_)
            | StreamItem::SourceEnded => {}
        }
    }

    Ok(receipts)
}

#[cfg(test)]
mod tests {
    use crate::{CommitState, EventKind, EventMeta, EventSourceKind, FlowTags, StreamItem, B256};

    use super::*;

    #[derive(Default)]
    struct RecordingStrategy {
        seen: Vec<u64>,
    }

    impl Strategy<String> for RecordingStrategy {
        fn on_event(
            &mut self,
            _context: &mut StrategyContext,
            event: &EventEnvelope<String>,
        ) -> Result<Vec<Action>> {
            self.seen.push(event.seqno());
            Ok(vec![Action::Record(RecordAction {
                topic: "seen".to_owned(),
                payload: serde_json::json!({ "seqno": event.seqno() }),
            })])
        }

        fn on_gap(
            &mut self,
            _context: &mut StrategyContext,
            _gap: &GapEvent,
        ) -> Result<GapDecision> {
            Ok(GapDecision::Continue)
        }
    }

    fn event(seqno: u64) -> EventEnvelope<String> {
        EventEnvelope::new(
            format!("event-{seqno}"),
            EventMeta {
                seqno,
                record_epoch_nanos: seqno,
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
    fn strategy_receives_events_in_order_and_actions_reach_executor() {
        let mut strategy = RecordingStrategy::default();
        let mut executor = RecordingExecutor::default();
        let mut context = StrategyContext::new("test");

        let receipts = run_strategy(
            [StreamItem::Event(event(1)), StreamItem::Event(event(2))],
            &mut strategy,
            &mut executor,
            &mut context,
        )
        .expect("strategy should run");

        assert_eq!(strategy.seen, vec![1, 2]);
        assert_eq!(receipts.len(), 2);
        assert_eq!(executor.jsonl().lines().count(), 2);
    }

    #[test]
    fn recording_executor_writes_jsonl() {
        let mut executor = RecordingExecutor::default();
        executor
            .execute(Action::Alert(AlertAction {
                level: "info".to_owned(),
                message: "hello".to_owned(),
            }))
            .expect("action should record");

        assert!(executor.jsonl().contains("alert"));
        assert_eq!(executor.receipts().len(), 1);
    }

    #[test]
    fn dry_run_executor_accepts_well_formed_action() {
        let mut executor = DryRunExecutor::default();
        let receipt = executor
            .execute(Action::SubmitTxDryRun(SubmitTxDryRun {
                to: Some(Address::from([1_u8; 20])),
                data: vec![0xab],
                value: U256::ZERO,
            }))
            .expect("dry run should validate");

        assert!(receipt.dry_run);
    }

    #[test]
    fn dry_run_executor_rejects_malformed_action() {
        let mut executor = DryRunExecutor::default();
        let error = executor
            .execute(Action::SubmitTxDryRun(SubmitTxDryRun {
                to: Some(Address::from([1_u8; 20])),
                data: Vec::new(),
                value: U256::ZERO,
            }))
            .expect_err("empty data should be rejected");

        assert!(error.to_string().contains("non-empty data"));
    }

    #[test]
    fn strategy_gap_callback_can_continue() {
        let mut strategy = RecordingStrategy::default();
        let mut executor = RecordingExecutor::default();
        let mut context = StrategyContext::new("test");

        run_strategy(
            [StreamItem::Gap(GapEvent::new(
                1,
                3,
                EventSourceKind::Fixture,
            ))],
            &mut strategy,
            &mut executor,
            &mut context,
        )
        .expect("gap callback should continue");

        assert_eq!(context.gaps_seen, 1);
    }
}
