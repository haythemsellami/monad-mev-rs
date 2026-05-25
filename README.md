# monad-mev-rs

`monad-mev-rs` is an Execution Events-first Rust framework for building Monad
searcher, monitoring, and MEV-style applications.

The current v0.1 focus is replay-first development:

- Inspect historical Monad Execution Events snapshots.
- Normalize raw execution events into safer framework-owned types.
- Decode common EVM and DeFi logs.
- Run deterministic strategy tests against fixtures and snapshots.
- Graduate the same event pipeline to live Linux event-ring ingestion in observe-only mode.

v0.1 is not a production trading stack. Transaction submission, counterfactual
simulation, private relays, risk engines, and full protocol state stores are
intentionally delayed to later product milestones.

## Project Plan

Planning documents:

- [v0.1 spec](v0.1-spec.md)
- [v0.1 implementation plan](v0.1-implementation-plan.md)
- [Product spec](v0.2-spec.md)
- [Product implementation plan](v0.2-implementation-plan.md)

## Quickstart

Run the deterministic fixture workflow:

```bash
cargo run -p monad-mev-cli -- doctor
cargo run -p monad-mev-cli -- inspect --fixture raw-events
cargo run -p monad-mev-cli -- decode --fixture defi-decoded --defi
cargo run -p monad-mev-cli -- replay --fixture raw-events
```

Create and test a strategy scaffold:

```bash
cargo run -p monad-mev-cli -- strategy new /tmp/monad-mev-strategy
cargo test --manifest-path /tmp/monad-mev-strategy/Cargo.toml
```

Check live observe availability:

```bash
cargo run -p monad-mev-cli -- inspect monad-exec-events --live --duration 10s --summary
```

On macOS this reports live mode as unavailable. Real live event rings require a
Linux host with access to a Monad execution event ring.

## Rust API Examples

Inside this repository, example crates use local path dependencies. Adjust the
relative paths for your crate location:

```toml
[dependencies]
monad-mev-core = { path = "crates/monad-mev-core" }
monad-mev-events = { path = "crates/monad-mev-events" }
serde_json = "1.0"
```

Replay a deterministic fixture:

```rust
use monad_mev_events::{fixture_stream_items, load_workspace_fixture, ReplayConfig, ReplayRunner};

fn main() -> monad_mev_core::Result<()> {
    let fixture = load_workspace_fixture("raw-events.json")?;
    let items = fixture_stream_items(&fixture)?;
    let run = ReplayRunner::new(ReplayConfig::default()).run(items)?;

    println!("{}", run.human_summary());
    Ok(())
}
```

Decode built-in DeFi logs:

```rust
use monad_mev_core::StreamItem;
use monad_mev_events::{
    decode_basic_defi_log, fixture_stream_items, load_workspace_fixture, ChainEvent, DeFiEvent,
};

fn main() -> monad_mev_core::Result<()> {
    let fixture = load_workspace_fixture("defi-decoded.json")?;

    for item in fixture_stream_items(&fixture)? {
        let StreamItem::Event(event) = item else {
            continue;
        };
        let ChainEvent::Log(log) = event.payload else {
            continue;
        };

        if let DeFiEvent::Erc20Transfer(transfer) = decode_basic_defi_log(log) {
            println!(
                "transfer token={} from={} to={} value={}",
                transfer.token, transfer.from, transfer.to, transfer.value
            );
        }
    }

    Ok(())
}
```

Run a strategy with the recording executor:

```rust
use monad_mev_core::{
    run_strategy, Action, EventEnvelope, RecordAction, RecordingExecutor, Result, Strategy,
    StrategyContext,
};
use monad_mev_events::{fixture_stream_items, load_workspace_fixture, ChainEvent};

#[derive(Default)]
struct LogCounter;

impl Strategy<ChainEvent> for LogCounter {
    fn on_event(
        &mut self,
        _context: &mut StrategyContext,
        event: &EventEnvelope<ChainEvent>,
    ) -> Result<Vec<Action>> {
        if !matches!(&event.payload, ChainEvent::Log(_)) {
            return Ok(Vec::new());
        }

        Ok(vec![Action::Record(RecordAction {
            topic: "log.seen".to_owned(),
            payload: serde_json::json!({ "seqno": event.seqno() }),
        })])
    }
}

fn main() -> Result<()> {
    let fixture = load_workspace_fixture("raw-events.json")?;
    let items = fixture_stream_items(&fixture)?;
    let mut strategy = LogCounter;
    let mut executor = RecordingExecutor::default();
    let mut context = StrategyContext::new("readme-example");

    run_strategy(items, &mut strategy, &mut executor, &mut context)?;

    print!("{}", executor.jsonl());
    Ok(())
}
```

Current executors record or validate actions only. They do not sign or submit
transactions.

## Development Process

The current workflow is direct commits on `main`. Commits should group related
changes and avoid unrelated code changes in a single commit.

## Local Checks

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --doc --workspace --all-features
```
