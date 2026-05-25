# Writing Strategies

Strategies implement `monad_mev_core::Strategy<T>` and return framework
actions. v0.1 executors are recording and dry-run only.

```rust
use monad_mev_core::{Action, EventEnvelope, RecordAction, Result, Strategy, StrategyContext};

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
```

Create a scaffold:

```bash
cargo run -p monad-mev-cli -- strategy new /tmp/monad-mev-strategy
cargo test --manifest-path /tmp/monad-mev-strategy/Cargo.toml
```

Use `RecordingExecutor` for deterministic tests and `DryRunExecutor` for
transaction-shape validation. v0.1 intentionally has no production submission
executor.
