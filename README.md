# monad-mev-rs

`monad-mev-rs` is a V1-planned Rust framework for building Monad searcher, monitoring, and MEV-style applications on top of Monad Execution Events.

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

## Development Process

Every work package lands through a GitHub pull request. Large work packages should be split into multiple smaller PRs. Commits inside a PR should group related changes and avoid unrelated code changes in a single commit.

## Local Checks

For WP-01:

```bash
cargo metadata --no-deps
cargo fmt --all -- --check
```

Broader V1 checks will be added as the workspace crates are implemented.
