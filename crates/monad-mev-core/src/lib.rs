//! Core framework types for `monad-mev-rs`.
//!
//! This crate owns framework-level types and traits.
//!
//! During WP-03 it only exposes diagnostics metadata. Framework-owned event,
//! replay, strategy, and executor types are implemented in later work packages.

/// Crate version, exposed for diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_version() {
        assert!(!VERSION.is_empty());
    }
}
