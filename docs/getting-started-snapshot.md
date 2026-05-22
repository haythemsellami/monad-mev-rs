# Getting Started: Snapshot And Fixture Replay

The V1 development loop is replay-first. You can build and test strategies on
macOS or Linux with deterministic fixtures before touching a live event ring.

## Inspect Fixtures

```bash
cargo run -p monad-mev-cli -- inspect --fixture raw-events
cargo run -p monad-mev-cli -- inspect --fixture chain-events --json
```

Fixtures live in `fixtures/` and are converted into normalized `ChainEvent`
stream items by `monad_mev_events::fixture_stream_items`.

## Decode Events

```bash
cargo run -p monad-mev-cli -- decode --fixture raw-events
cargo run -p monad-mev-cli -- decode --fixture defi-decoded --defi
```

`--defi` runs the built-in log decoders for ERC20 transfers/approvals and
Uniswap-style swaps/syncs. Unknown or malformed logs are preserved as
structured `unknown_log` records.

## Replay

```bash
cargo run -p monad-mev-cli -- replay --fixture raw-events
cargo run -p monad-mev-cli -- replay --fixture raw-events --report /tmp/report.json --events-jsonl /tmp/events.jsonl
```

Useful filters:

```bash
cargo run -p monad-mev-cli -- replay --fixture chain-events --from-seqno 2 --to-seqno 4
cargo run -p monad-mev-cli -- replay --fixture chain-events --kind txn_log
```

## Strategy Scaffold

```bash
cargo run -p monad-mev-cli -- strategy new /tmp/monad-mev-strategy
cargo test --manifest-path /tmp/monad-mev-strategy/Cargo.toml
```

The generated strategy uses `RecordingExecutor` and includes a fixture-backed
unit test. It does not submit transactions.
