# v0.2 Capture

v0.2 capture is protocol-agnostic. A `CaptureFilter` selects normalized
Execution Events by address, topic, event kind, transaction index, block range,
and minimum commit state. Protocol packages can compose these filters without
touching raw event-ring descriptors.

```rust
use monad_mev_engine::CaptureFilter;
use monad_mev_core::{CommitState, EventKind};

let capture = CaptureFilter::named("my-protocol")
    .with_event_kind(EventKind::TxnLog)
    .with_min_commit_state(CommitState::Finalized);
```

Use `monad-mev lifecycle --json` for a local end-to-end capture smoke test.
