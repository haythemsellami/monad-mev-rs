use monad_mev_core::{Action, EventEnvelope, RecordAction, Result, Strategy, StrategyContext};

fn main() {
    let strategy = CountStrategy;
    println!(
        "replay strategy example: {}",
        std::any::type_name_of_val(&strategy)
    );
}

#[derive(Default)]
struct CountStrategy;

impl Strategy<String> for CountStrategy {
    fn on_event(
        &mut self,
        _context: &mut StrategyContext,
        event: &EventEnvelope<String>,
    ) -> Result<Vec<Action>> {
        Ok(vec![Action::Record(RecordAction {
            topic: "example.event".to_owned(),
            payload: serde_json::json!({ "seqno": event.seqno() }),
        })])
    }
}

#[cfg(test)]
mod tests {
    use monad_mev_core::{
        run_strategy, CommitState, EventKind, EventMeta, EventSourceKind, FlowTags,
        RecordingExecutor, StreamItem, B256,
    };

    use super::*;

    #[test]
    fn replay_strategy_example_runs() {
        let mut strategy = CountStrategy;
        let mut executor = RecordingExecutor::default();
        let mut context = StrategyContext::new("example");
        let event = EventEnvelope::new(
            "event".to_owned(),
            EventMeta {
                seqno: 1,
                record_epoch_nanos: 1,
                event_kind: EventKind::TxnLog,
                source: EventSourceKind::Fixture,
                block: None,
                txn: None,
                flow: FlowTags::default(),
                commit_state: CommitState::Unknown,
                schema_hash: Some(B256::from([1_u8; 32])),
            },
        );

        run_strategy(
            [StreamItem::Event(event)],
            &mut strategy,
            &mut executor,
            &mut context,
        )
        .expect("strategy should run");
        assert_eq!(executor.receipts().len(), 1);
    }
}
