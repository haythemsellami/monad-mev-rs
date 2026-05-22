//! Command-line interface scaffolding for `monad-mev-rs`.
//!
//! WP-03 provides only the binary skeleton and help text. Real commands such as
//! `doctor`, `inspect`, `decode`, `replay`, and `strategy new` are implemented
//! in WP-18.

/// CLI binary name.
pub const BIN_NAME: &str = "monad-mev";

/// CLI crate version, exposed for diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Exit code used for invalid CLI usage.
pub const USAGE_ERROR: i32 = 2;

/// Returns the placeholder help text for the V1 CLI skeleton.
#[must_use]
pub const fn help_text() -> &'static str {
    "monad-mev 0.0.0\n\nUsage: monad-mev [--help] [--version]\n\nV1 commands are implemented in later work packages.\n"
}

/// Returns the placeholder version text.
#[must_use]
pub fn version_text() -> String {
    format!("{BIN_NAME} {VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_mentions_binary_name() {
        assert!(help_text().contains(BIN_NAME));
    }

    #[test]
    fn version_mentions_binary_name() {
        assert!(version_text().starts_with(BIN_NAME));
    }
}
