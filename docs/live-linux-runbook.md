# Live Linux Runbook

V1 live mode is observe-only. It never submits transactions and it reports
availability before trying to attach to a Monad execution event ring.

## Prerequisites

- Linux host with access to a Monad validator or execution-event ring.
- `MONAD_MEV_EVENT_RING` set to a ring name such as `monad-exec-events`, or to
  an explicit path.
- Optional `MONAD_MEV_EVENT_RING_DIR` when named rings are not under `/dev/shm`.

## Smoke Test

```bash
cargo test -p monad-mev-events --features live live
MONAD_MEV_EVENT_RING=monad-exec-events cargo test -p monad-mev-events --test live_ring --features live -- --ignored
cargo run -p monad-mev-cli -- inspect monad-exec-events --live --duration 10s --summary
```

On non-Linux hosts, the first command should pass unit tests and the CLI should
report live mode as unavailable without panicking.
