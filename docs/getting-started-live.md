# Getting Started: Live Observe Mode

V1 live mode is observe-only. It is intended to validate event-ring attachment,
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

## Current V1 Boundary

`LiveEventRingSource` validates source metadata and exposes the observe-only
polling boundary. SDK-backed descriptor polling remains behind the V1 live
feature until the Linux event-ring reader is wired to the pinned SDK dependency
strategy.
