# 0.1 Release Scope

`0.1.x` is the first supported Git release line for `monad-mev-rs`.

## Supported

- Deterministic fixture and snapshot replay.
- Framework-owned raw and normalized Monad Execution Events.
- Linux live event-ring observation through the pinned official SDK.
- Continuous capture, adapter, state, and opportunity-detection lifecycles.
- Auditable simulation interfaces, risk policies, recording executors, and
  fake-tested transport boundaries.
- macOS development for replay, fixtures, documentation, and non-live APIs.

## Explicitly experimental

- Production transaction submission and private-key integration.
- Protocol-specific adapters and strategies.
- Profitability claims or latency guarantees.
- Crates.io distribution.

The stable release gate requires real Execution Events conformance, correct
speculative-block rollback and finality behavior, cross-platform/MSRV CI, and a
sustained observe-only live-ring run. Experimental APIs remain available for
dogfooding but are not part of the production-support claim.
