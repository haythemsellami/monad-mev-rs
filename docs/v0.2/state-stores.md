# v0.2 State Stores

`monad-mev-store` provides commit-state-aware in-memory state with audit
records, deterministic rollback, projections, and JSON file snapshots.

Adapters write `StateUpdate` values. Strategies and detectors read filtered
`StateView` values so speculative and finalized state can be handled
explicitly.
