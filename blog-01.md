# Introducing monad-mev-rs: An Execution Events-First MEV Framework For Monad

Monad is an EVM-compatible blockchain designed for high throughput and low
latency. That changes the shape of developer infrastructure.

On slower chains, many bots, indexers, and monitoring systems can get pretty
far with JSON-RPC polling or WebSocket subscriptions. On Monad, that approach
can become the bottleneck. Blocks are proposed quickly, blocks can contain many
transactions, and the amount of data emitted by execution can be much larger
than what traditional Ethereum-era data APIs were designed to handle.

That is where Monad Execution Events come in.

Execution Events are a low-latency data stream emitted by the Monad execution
daemon. Instead of asking an RPC node for data after the fact, a colocated
process can read execution records directly from a shared-memory event ring.
These records are not just Solidity logs. They include lower-level execution
activity such as block boundaries, transaction headers, account access, storage
access, EVM output, logs, call frames, transaction end, and consensus-related
state changes.

For high-performance systems, this is powerful. It gives searchers, market
makers, liquidation bots, indexers, and protocol monitors a path to observe what
is happening on-chain with very low latency.

It also creates a new developer problem.

## The Problem

Execution Events are closer to infrastructure than to the abstractions most
Solidity and DeFi developers use every day.

To use the raw SDK directly, a developer needs to understand things like:

- live event rings and snapshot event rings
- descriptor sequence numbers
- payload buffers and payload expiration
- gap handling when a reader falls behind
- raw event types
- block and transaction flow metadata
- commit-state transitions
- schema compatibility
- platform requirements for live shared-memory ingestion

That is a reasonable interface for systems programmers building high-performance
data consumers. It is not the ideal starting point for a DeFi developer who
wants to answer a more direct question:

> "When protocol X emits the events I care about, can I run strategy Y?"

There is also a local development issue. Live Execution Events require an
environment near a Monad node. A MacBook is a fine place to write Rust and test
logic, but it is not where a live Linux shared-memory event ring is available.
For bot development, this matters. You want to test strategy behavior before
deploying infrastructure near a validator or node.

`monad-mev-rs` starts from that gap.

## The Product

`monad-mev-rs` is an Execution Events-first MEV framework for Monad.

The intended product shape is:

```text
capture events for protocol X
-> attach strategy Y
-> maintain protocol state
-> simulate and risk-check
-> execute or record the outcome
```

The goal is to let developers build searcher, monitoring, and keeper logic on
top of the semantics Monad exposes through Execution Events.

The framework is modular, but the Execution Events pipeline is the spine.
Protocol adapters, strategy packages, state stores, simulators, risk gates, and
executors plug into that pipeline.

The current `v0.1` implementation is the foundation for that product. It focuses
on replay-first development, normalized event types, deterministic testing, and
safe strategy execution through recording and dry-run executors.

The planned `v0.2` direction continues the same design toward protocol event
capture, imported/local strategy packages, state stores, simulation, risk, and
explicit production execution.

## What Exists Today

The current implementation includes:

- raw Execution Event conversion into framework-owned Rust types
- normalized `ChainEvent` records
- explicit stream items for events, gaps, payload expiration, schema mismatch,
  and source end
- commit-state tracking
- transaction flow grouping
- deterministic replay reports
- built-in ERC20 and DEX log decoders
- generic ABI event decoding
- strategy and executor traits
- recording and dry-run executors
- deterministic fixtures and golden tests
- a CLI for inspection, decoding, replay, and strategy scaffolding
- observe-only live event-ring scaffolding behind a feature flag

The practical workflow today is:

```text
fixture or snapshot
  -> raw execution event
  -> normalized chain event
  -> optional DeFi/ABI decode
  -> strategy
  -> recording or dry-run executor
```

The important part is that strategy code does not need to care about raw
descriptor buffers or payload lifetimes.

## What This Looks Like In Rust

Here is the high-level replay API:

```rust
use monad_mev_events::{
    fixture_stream_items, load_workspace_fixture, ReplayConfig, ReplayRunner,
};

fn main() -> monad_mev_core::Result<()> {
    let fixture = load_workspace_fixture("raw-events.json")?;
    let items = fixture_stream_items(&fixture)?;
    let run = ReplayRunner::new(ReplayConfig::default()).run(items)?;

    println!("{}", run.human_summary());
    Ok(())
}
```

This is intentionally much smaller than the raw SDK workflow. With the raw SDK,
you open an event ring, create a reader, poll descriptors, handle gap states,
try to read payloads, handle payload expiration, match raw execution event
types, and build your own report counters. `monad-mev-rs` turns that into a
stream of framework-owned items.

For DeFi logs, the framework includes basic decoders:

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

And here is a minimal strategy using the recording executor:

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
    let mut context = StrategyContext::new("example");

    run_strategy(items, &mut strategy, &mut executor, &mut context)?;

    print!("{}", executor.jsonl());
    Ok(())
}
```

This is not a liquidation bot yet. But it is the skeleton you want before a
liquidation bot exists: event ingestion, normalized records, deterministic
testing, strategy callbacks, and auditable outputs.

## The CLI

The project also ships a CLI:

```bash
cargo run -p monad-mev-cli -- doctor
cargo run -p monad-mev-cli -- inspect --fixture raw-events
cargo run -p monad-mev-cli -- decode --fixture defi-decoded --defi
cargo run -p monad-mev-cli -- replay --fixture raw-events
```

You can scaffold a strategy project:

```bash
cargo run -p monad-mev-cli -- strategy new /tmp/monad-mev-strategy
cargo test --manifest-path /tmp/monad-mev-strategy/Cargo.toml
```

And you can check live observe availability:

```bash
cargo run -p monad-mev-cli -- inspect monad-exec-events --live --duration 10s --summary
```

On macOS, this reports live mode as unavailable. That is expected. Real live
event rings require a Linux host with access to a Monad node exposing the
execution event ring.

## What It Enables

Today, `monad-mev-rs` is useful for:

- learning the shape of Monad Execution Events without starting at the raw SDK
- writing strategy logic against deterministic fixtures
- replaying known event streams and comparing output to goldens
- decoding common ERC20 and DEX logs
- testing stream-health behavior such as gaps and payload expiration
- building monitors and read-only strategy prototypes
- preparing code that can later run against live event-ring ingestion

Some concrete examples:

- ERC20 transfer monitor
- DEX swap monitor
- strategy replay test
- raw event printer
- read-only opportunity detector prototype

The most important use case is developer iteration. Before deploying near a
node, a developer can write a strategy locally and ask:

- Does it react to the right event?
- Does it ignore malformed or irrelevant logs?
- Does it produce deterministic output?
- What happens if the stream has a gap?
- What commit-state assumptions does it make?

Those questions should be answered before transaction submission exists.

## What Is Still Ahead

The current implementation intentionally does not include:

- production transaction submission
- private relays or bundles
- key management
- nonce management
- risk engine
- counterfactual EVM simulation
- lending protocol state stores
- full DEX routing
- production liquidation templates

Those are planned parts of the product, but they need to be built in the right
order. Transaction submission without simulation, risk controls, nonce policy,
and audit logs is not a framework feature. It is an unsafe shortcut.

The next major direction is to close the gap between:

```text
I can replay normalized events
```

and:

```text
I can capture protocol-specific events, attach a strategy, maintain state, and
produce opportunity records
```

After that, simulation, risk, and explicit execution can be layered in.

## Closing

Execution Events are one of the most interesting parts of building high
performance applications on Monad. They expose a lower-level, lower-latency view
of execution than ordinary RPC or WebSocket flows.

But low-level power should not force every strategy developer to start from
payload buffers and sequence numbers.

`monad-mev-rs` is an attempt to make that power usable: replay first,
deterministic by default, explicit about stream health, and built around the
semantics that make Monad Execution Events different.

It is not just a wrapper around the SDK. It is the beginning of an
Execution Events-first MEV framework for Monad.

## References

- Monad Execution Events: https://docs.monad.xyz/execution-events
- Execution Events overview: https://docs.monad.xyz/execution-events/overview
- Snapshot replay guide: https://docs.monad.xyz/execution-events/getting-started/snapshot
- Rust API: https://docs.monad.xyz/execution-events/rust-api
- `monad-mev-rs` product spec: `v0.2-spec.md`
- `monad-mev-rs` implementation plan: `v0.2-implementation-plan.md`
