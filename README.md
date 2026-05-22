# monad-mev-rs

`monad-mev-rs` is a V1 Rust framework for building Monad searcher,
monitoring, and MEV-style applications on top of Monad Execution Events.

The V1 focus is replay-first development:

- Inspect historical Monad Execution Events snapshots.
- Normalize raw execution events into safer framework-owned types.
- Decode common EVM and DeFi logs.
- Run deterministic strategy tests against fixtures and snapshots.
- Graduate the same event pipeline to live Linux event-ring ingestion in observe-only mode.

V1 is not a production trading stack. Transaction submission, counterfactual simulation, private relays, risk engines, and full protocol state stores are intentionally delayed to V2.

## Project Plan

V1 development follows:

- [V1 spec](v1-spec.md)
- [V1 implementation plan](v1-implementation-plan.md)

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
