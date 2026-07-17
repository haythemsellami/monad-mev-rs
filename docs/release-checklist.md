# Release Checklist

Before tagging a v0.1 release:

- Confirm every workspace package reports the intended release version.
- Confirm Linux all-feature, macOS all-feature, feature-matrix, MSRV, and
  rustdoc CI jobs pass.
- Run `cargo fmt --all -- --check`.
- Run `cargo clippy --all-targets --all-features -- -D warnings`.
- Run `cargo test --workspace --all-features`.
- Run `cargo test --doc --workspace --all-features`.
- Run CLI smoke commands from `README.md`.
- Confirm ignored snapshot and live tests are documented.
- Confirm `docs/sdk-versioning.md` still matches the pinned SDK metadata.
- Run the real snapshot conformance suite and ignored live-ring smoke test.
- Confirm speculative block rejection and finality tests pass.
- Confirm live docs state Linux-only support.
- Confirm v0.1 docs do not claim production transaction submission.
- Update `CHANGELOG.md`.
- Confirm the release scope still excludes unaudited production submission.
- Tag the release from a clean `main`.
