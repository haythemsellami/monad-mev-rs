# Event Semantics

v0.1 separates events into three layers.

## Raw Execution Events

`RawExecEvent` preserves the upstream execution event type, descriptor sequence
number, record timestamp, payload size, flow tags, payload bytes, and schema
hash when known. Unknown event IDs are preserved instead of dropped.

The pinned event IDs come from Monad Execution Events SDK v1.1.

## Chain Events

`ChainEvent` is the normalized EVM-facing layer:

- `Block` for block boundary records.
- `Transaction` for transaction boundary records.
- `Log` for EVM logs.
- `CallFrame`, `AccountAccess`, `StorageAccess`, and `TxnOutput` for lower
  level execution records.
- `CommitState` for proposed, voted, finalized, and verified block state.
- `UnknownRaw` when v0.1 does not yet define a higher-level shape.

Every event is wrapped in `EventEnvelope<T>` with stable metadata: `seqno`,
`event_kind`, `source`, optional block and transaction refs, flow tags, commit
state, and schema hash.

## Health Items

Streams may emit non-event items:

- `Gap` when descriptor sequence numbers skip.
- `PayloadExpired` when a live payload was overwritten before it could be read.
- `SchemaMismatch` when source schema does not match the compiled decoder.
- `SourceEnded` when replay or observe mode finishes.

Snapshot replay defaults to fail-fast gap handling. CLI inspection logs and
continues. Live observe mode uses risk-off then fail semantics.
