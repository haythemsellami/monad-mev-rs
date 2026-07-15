# Continuous Engine Runner

The continuous runner connects a non-blocking normalized Execution Events
source to the same adapters, state store, and opportunity detectors used by
finite replay. It is observe-only and does not sign or submit transactions.

## Pipeline

```text
Live event ring
-> owned raw descriptor
-> block commit-state tracker
-> transaction-flow tracker
-> normalized ChainEvent
-> capture filters and adapters
-> persistent state store
-> opportunity detectors at transaction or block boundaries
```

`ExecutionEventPoller::poll_next` returns:

- `Some(item)` when an event or health item is ready.
- `None` when a continuous source is healthy but currently idle.
- `Err` on a source failure.

Finite pollers terminate with `StreamItem::SourceEnded`. Live event rings do
not synthesize a terminal item.

## Live observe example

On a Linux host that can read the Monad event ring:

```bash
MONAD_MEV_EVENT_RING=monad-exec-events \
MONAD_MEV_DURATION_MILLIS=10000 \
cargo run -p live-observe
```

The example opens `LiveExecutionEventStream`, runs an empty protocol-agnostic
engine for the configured duration, and prints a JSON report. External packages
add capture filters, adapters, and detectors to `Engine` before constructing
`ContinuousEngineRunner`.

## Safety defaults

Live defaults stop in risk-off mode after descriptor gaps, overwritten
payloads, or schema mismatches. Callers can opt into continue or fail-fast
behavior explicitly. Opportunity detectors run after `TxnEnd` by default and
can instead be configured for every event or `BlockEnd`.

`ShutdownHandle` provides cooperative thread-safe shutdown. The runner checks
it before every source poll, so shutdown never interrupts an item halfway
through adapter or state processing.

Repeated opportunities with the same ID and state version are suppressed.
This prevents a source-end flush or an unchanged later boundary from emitting
the same candidate twice.
