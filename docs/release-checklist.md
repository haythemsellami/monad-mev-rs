# Release Checklist

Before tagging a V1 release:

- Run `cargo fmt --all -- --check`.
- Run `cargo clippy --all-targets --all-features -- -D warnings`.
- Run `cargo test --workspace --all-features`.
- Run `cargo test --doc --workspace --all-features`.
- Run CLI smoke commands from `README.md`.
- Confirm ignored snapshot and live tests are documented.
- Confirm `docs/sdk-versioning.md` still matches the pinned SDK metadata.
- Confirm live docs state Linux-only support.
- Confirm V1 docs do not claim production transaction submission.
- Update `CHANGELOG.md`.
- Tag the release from a clean `main`.
