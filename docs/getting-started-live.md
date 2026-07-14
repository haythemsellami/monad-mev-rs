# Getting Started: Live Observe Mode

v0.1 live mode is observe-only. It is intended to validate event-ring attachment,
metadata checks, gap handling, and replay pipeline readiness. It does not submit
transactions.

## Platform Support

Real live event rings require Linux and access to a Monad validator or an
environment that exposes the execution event ring. macOS can compile and run
the CLI diagnostics, but it cannot attach to a live event ring.

## Check Availability

```bash
cargo run -p monad-mev-cli -- doctor
cargo run -p monad-mev-cli -- inspect monad-exec-events --live --duration 10s --summary
```

On non-Linux hosts this should exit successfully and report live mode as
unavailable.

## Linux Smoke Test

```bash
cargo test -p monad-mev-events --features live live
MONAD_MEV_EVENT_RING=monad-exec-events cargo test -p monad-mev-events --test live_ring --features live -- --ignored
```

Set `MONAD_MEV_EVENT_RING_DIR` if the named ring is not under `/dev/shm`.

## Reader Semantics

The `live` feature pins the official `monad-event-ring` and
`monad-exec-events` crates to a reviewed Monad revision. On Linux,
`LiveEventRingSource::open` starts a bounded SDK reader and
`poll_descriptor` returns:

- `Ok(Some(item))` for an owned event, sequence gap, or expired payload;
- `Ok(None)` when the producer has not published another descriptor yet;
- `Err(_)` when the SDK reader fails or disconnects.

The SDK payload is copied before the ring can overwrite it. Sequence gaps and
payload expiry stay explicit `StreamItem` variants so callers can apply their
configured risk-off policy. The reader remains observe-only and never signs or
submits transactions.
