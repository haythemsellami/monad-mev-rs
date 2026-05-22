# Troubleshooting

## Live Mode Reports Unavailable

This is expected on macOS. Real execution event rings require Linux and access
to a Monad validator or compatible environment.

## Missing Fixture

Fixture names resolve relative to `fixtures/`. Both `raw-events` and
`raw-events.json` are accepted.

```bash
cargo run -p monad-mev-cli -- inspect --fixture raw-events
```

## Schema Mismatch

Schema mismatch means the source event schema does not match the compiled SDK
schema. V1 defaults to strict validation for replay-style sources and warning
behavior for observe-only live metadata.

## SDK Fetch Is Slow

The upstream SDK dependency is pinned but not activated in default builds. The
reserved `sdk` feature exists so SDK-backed ingestion can be enabled once the
fetch/vendor strategy is settled.

## Generated Strategy Does Not Compile

Regenerate it from the repo checkout so the local path dependency points at the
current `monad-mev-core` crate:

```bash
cargo run -p monad-mev-cli -- strategy new /tmp/monad-mev-strategy
cargo test --manifest-path /tmp/monad-mev-strategy/Cargo.toml
```
