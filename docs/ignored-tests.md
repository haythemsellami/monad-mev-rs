# Ignored Integration Tests

Ignored tests require external Monad artifacts or host capabilities.

## Snapshot Replay

```bash
MONAD_MEV_SNAPSHOT=/path/to/snapshot.zst cargo test -p monad-mev-events --test snapshot_replay -- --ignored
```

Use a real Monad execution events snapshot. The test opens the snapshot and
replays raw descriptors through normalization.

## Live Ring

```bash
MONAD_MEV_EVENT_RING=monad-exec-events cargo test -p monad-mev-events --test live_ring --features live -- --ignored
```

Run this on Linux with access to a readable execution event ring. Set
`MONAD_MEV_EVENT_RING_DIR` when named rings are not under `/dev/shm`.
