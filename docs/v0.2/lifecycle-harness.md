# v0.2 Lifecycle Harness

The lifecycle harness is the protocol-agnostic end-to-end proof for v0.2. It
uses a fixture adapter to show that the framework can capture an event, update
state, detect an opportunity, emit metrics, and produce deterministic output.

```bash
cargo run -p monad-mev-cli -- lifecycle --json
```

The harness is intentionally generic. Real DEX, lending, oracle, NFT, bridge,
or custom protocol logic should live in external adapter packages.
