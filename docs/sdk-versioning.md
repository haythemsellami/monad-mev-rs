# SDK Versioning

Status date: 2026-07-15

This document records the Monad Execution Events Rust SDK source used by
`monad-mev-rs` 0.1.

## Pinned SDK

The integration records the official v1.1 baseline and pins active Cargo
dependencies to an exact compatibility revision:

```text
repository: https://github.com/category-labs/monad
tag:        release/exec-events-sdk-v1.1
tag commit: b7c13e1565f40556cb717090eae245e34bb5c6e7
revision:   4f2289307196a1b70dfa1fb5282600a07ca40767
```

The tag identifies the upstream SDK release line. Cargo uses the exact
`revision`, which is the only commit compiled by the active integration. The
SDK crates live in the execution repository under:

```text
rust/crates/monad-event-ring
rust/crates/monad-exec-events
```

All pin metadata and direct SDK dependencies are isolated to
`crates/monad-mev-events`.

## Why SDK v1.1

The Monad Execution Events v1.1 release moved the Rust SDK from the consensus
repository to the execution repository. Older v1.0 examples use
`category-labs/monad-bft`; those dependencies are not used here.

Upstream references:

- https://docs.monad.xyz/execution-events/release-notes
- https://docs.monad.xyz/execution-events/getting-started/rust
- https://github.com/category-labs/monad/tree/release/exec-events-sdk-v1.1/rust/crates

## Cargo Feature Policy

`monad-mev-events` uses explicit `sdk` and `live` features:

```bash
cargo test -p monad-mev-events --no-default-features
cargo test -p monad-mev-events --features live
```

The dependencies are active only for Linux targets. macOS builds retain replay,
snapshot, normalization, and diagnostic APIs without compiling the native live
event-ring SDK.

The active declarations are:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
monad-event-ring = { git = "https://github.com/category-labs/monad", rev = "4f2289307196a1b70dfa1fb5282600a07ca40767", optional = true }
monad-exec-events = { git = "https://github.com/category-labs/monad", rev = "4f2289307196a1b70dfa1fb5282600a07ca40767", optional = true }

[features]
default = []
live = ["sdk"]
sdk = ["dep:monad-event-ring", "dep:monad-exec-events"]
```

## Schema Compatibility

The upstream `monad-exec-events` crate exposes the compiled schema hash through
`ExecEventDecoder::ring_schema_hash()`. The live source attaches that hash to
framework source metadata and copied descriptors.

Missing or mismatched schemas are compatibility errors by default. Warning and
skip behavior require explicit caller configuration. A real snapshot and live
ring conformance run is required before a stable tag.

## Platform Dependencies

The official SDK requires native tooling because its Rust crates bind to the C
event-ring implementation.

The workspace minimum supported Rust version is 1.90, matching the highest
declared requirement in the locked Alloy/cryptography dependency graph
(`ruint` 1.18). CI checks that exact toolchain rather than treating the current
stable compiler as an MSRV proxy.

Ubuntu requirements:

- Rust toolchain.
- `git` and `curl`.
- GCC 13 or Clang 19.
- CMake 3.23 or newer.
- `libzstd-dev`.
- `libhugetlbfs-dev` for live shared-memory ring resolution.
- Clang 19/libclang 19 or newer for C23-compatible bindgen.

macOS supports snapshots, fixtures, examples, and non-live framework APIs. A
real live ring requires Linux and access to a Monad node.

## License Decision

The Category Labs `monad` repository and SDK sources are GPL-3.0-or-later.
Because `monad-mev-rs` links the SDK directly when enabled, this repository uses
GPL-3.0-or-later. `NOTICE` records the baseline tag and exact active revision.

## Known Build Friction

Cargo may fetch more of the upstream execution repository than these two SDK
crates require. Linux all-feature CI therefore takes longer than replay-only
builds. Linux CI compiles the native SDK path on every change, while macOS CI
covers replay and cross-platform public APIs.
