# SDK Versioning

Status date: 2026-05-22

This document records the Monad Execution Events Rust SDK source pinned by `monad-mev-rs` v0.1.

## Pinned SDK

v0.1 pins the SDK to:

```text
repository: https://github.com/category-labs/monad
tag:        release/exec-events-sdk-v1.1
commit:     b7c13e1565f40556cb717090eae245e34bb5c6e7
```

The SDK crates live in the execution repository under:

```text
rust/crates/monad-event-ring
rust/crates/monad-exec-events
```

The local SDK pin metadata is intentionally isolated to `crates/monad-mev-events`.

## Why SDK v1.1

The Monad Execution Events release notes say v1.1 moved the Rust SDK from the consensus repository to the execution repository.

The older Rust getting-started page still shows v1.0 snippets from `category-labs/monad-bft`:

```toml
[dependencies.monad-exec-events]
git = "https://github.com/category-labs/monad-bft"
tag = "release/exec-events-sdk-v1.0"

[dependencies.monad-event-ring]
git = "https://github.com/category-labs/monad-bft"
tag = "release/exec-events-sdk-v1.0"
```

Those snippets are useful historical context, but v0.1 should use the newer v1.1 execution-repo tag.

References:

- https://docs.monad.xyz/execution-events/release-notes
- https://docs.monad.xyz/execution-events/getting-started/rust
- https://github.com/category-labs/monad/tree/release/exec-events-sdk-v1.1/rust/crates

## Cargo Feature Policy

`monad-mev-events` reserves an explicit `sdk` feature for upstream SDK-backed APIs:

```bash
cargo test -p monad-mev-events
cargo test -p monad-mev-events --features sdk
```

Default builds should stay fast and should not require the upstream SDK toolchain.

This feature gate is pragmatic. The official docs state that the first SDK build fetches the upstream repo and transitive git submodules, most of which are not needed by the SDK but are checked out by Cargo by default.

During WP-02, active optional git dependencies were tested and found to trigger the upstream fetch even for ordinary `cargo test` lockfile resolution. To keep default development usable, WP-02 records the exact pinned dependency declarations but defers activating them in `Cargo.toml` until the ingestion work packages decide the fetch/vendor strategy.

## Dependency Declarations

The pinned SDK dependency declarations are:

```toml
[dependencies]
monad-event-ring = { git = "https://github.com/category-labs/monad", tag = "release/exec-events-sdk-v1.1", package = "monad-event-ring", optional = true }
monad-exec-events = { git = "https://github.com/category-labs/monad", tag = "release/exec-events-sdk-v1.1", package = "monad-exec-events", optional = true }

[features]
default = []
sdk = ["dep:monad-event-ring", "dep:monad-exec-events"]
```

No other v0.1 crate should depend directly on `monad-event-ring` or `monad-exec-events`.

When these dependencies are activated, they must be activated only in `crates/monad-mev-events`.

## Schema Compatibility

The upstream `monad-exec-events` crate exposes the execution event schema hash through `ExecEventDecoder::ring_schema_hash()`.

`monad-mev-events` should expose this through:

```rust
monad_mev_events::exec_event_schema_hash()
monad_mev_events::exec_event_schema_hash_hex()
```

These APIs should be available only when the `sdk` feature is enabled and the upstream dependencies are active.

v0.1 ingestion code must compare the compiled SDK schema hash against the schema hash recorded in a live event ring or snapshot before decoding payloads. A mismatch must be treated as a compatibility error by default.

## Platform Dependencies

The official Rust SDK build currently requires native tooling because the Rust crates bind to the C event-ring implementation.

Ubuntu requirements listed by the docs:

- Rust toolchain.
- `git`.
- `curl`.
- C compiler: `gcc-13` or `clang-19`.
- C++ compiler: `g++-13` or `clang-19`.
- CMake 3.23 or newer.
- `libzstd-dev`.
- `libhugetlbfs-dev` for live shared-memory ring path resolution.
- `clang-19` or a recent enough `libclang` for bindgen.

macOS snapshot development:

- Live mode is not supported because it requires a Linux host running a Monad node.
- Historical snapshot mode can compile and run.
- `libhugetlbfs` is not required.
- `libzstd`, CMake, and a recent LLVM/clang/libclang are required.

## License Decision

The Category Labs `monad` and `monad-bft` repositories are GPL-3.0 licensed. The SDK crate source also carries GPL-3.0-or-later headers.

Because `monad-mev-rs` links the SDK directly when the `sdk` feature is enabled, this repository uses:

```text
GPL-3.0-or-later
```

The root `NOTICE` file records the pinned upstream SDK source.

## Known Build Friction

The first SDK-backed Cargo build can be slow because Cargo fetches the execution repository and its submodules. During WP-02, `cargo test -p monad-mev-events sdk` began fetching upstream submodules and was stopped after several minutes without reaching compilation.

Default v0.1 checks avoid this by not activating the upstream git dependencies yet. SDK-backed checks should still be run before implementing or releasing ingestion code.
