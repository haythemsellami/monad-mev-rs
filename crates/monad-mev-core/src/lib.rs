//! Core framework types for `monad-mev-rs`.
//!
//! This crate is intentionally minimal during WP-01. Framework-owned types are
//! implemented in later work packages.

/// Crate version, exposed for diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
