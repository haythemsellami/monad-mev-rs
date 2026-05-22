//! Command-line interface scaffolding for `monad-mev-rs`.
//!
//! WP-03 provides only the binary skeleton and help text. Real commands such as
//! `doctor`, `inspect`, `decode`, `replay`, and `strategy new` are implemented
//! in WP-18.

use std::path::{Path, PathBuf};

/// CLI binary name.
pub const BIN_NAME: &str = "monad-mev";

/// CLI crate version, exposed for diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Exit code used for invalid CLI usage.
pub const USAGE_ERROR: i32 = 2;

/// Returns the placeholder help text for the V1 CLI skeleton.
#[must_use]
pub const fn help_text() -> &'static str {
    "monad-mev 0.0.0\n\nUsage: monad-mev [--help] [--version]\n       monad-mev strategy new <destination>\n\nV1 analysis commands are implemented in WP-18.\n"
}

/// Returns the placeholder version text.
#[must_use]
pub fn version_text() -> String {
    format!("{BIN_NAME} {VERSION}")
}

fn core_crate_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../monad-mev-core")
}

/// Creates a minimal strategy project that compiles immediately.
///
/// # Errors
///
/// Returns an error when the destination exists or files cannot be written.
pub fn scaffold_strategy_project(destination: impl AsRef<Path>) -> std::io::Result<()> {
    let destination = destination.as_ref();
    let core_path = core_crate_path();
    std::fs::create_dir(destination)?;
    std::fs::create_dir(destination.join("src"))?;
    std::fs::write(
        destination.join("Cargo.toml"),
        format!(
            r#"[package]
name = "generated-monad-mev-strategy"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
monad-mev-core = {{ path = "{}" }}
serde_json = "1.0"
"#,
            core_path.display()
        ),
    )?;
    std::fs::write(
        destination.join("src/lib.rs"),
        r#"use monad_mev_core::{Action, EventEnvelope, RecordAction, Result, Strategy, StrategyContext};

#[derive(Default)]
pub struct GeneratedStrategy;

impl Strategy<String> for GeneratedStrategy {
    fn on_event(
        &mut self,
        _context: &mut StrategyContext,
        event: &EventEnvelope<String>,
    ) -> Result<Vec<Action>> {
        Ok(vec![Action::Record(RecordAction {
            topic: "generated.event".to_owned(),
            payload: serde_json::json!({ "seqno": event.seqno() }),
        })])
    }
}

#[cfg(test)]
mod tests {
    use monad_mev_core::{
        run_strategy, CommitState, EventKind, EventMeta, EventSourceKind, FlowTags,
        RecordingExecutor, StreamItem, B256,
    };

    use super::*;

    #[test]
    fn generated_strategy_records_fixture_event() {
        let event = EventEnvelope::new(
            "fixture".to_owned(),
            EventMeta {
                seqno: 1,
                record_epoch_nanos: 1,
                event_kind: EventKind::TxnLog,
                source: EventSourceKind::Fixture,
                block: None,
                txn: None,
                flow: FlowTags::default(),
                commit_state: CommitState::Unknown,
                schema_hash: Some(B256::from([1_u8; 32])),
            },
        );
        let mut strategy = GeneratedStrategy;
        let mut executor = RecordingExecutor::default();
        let mut context = StrategyContext::new("generated");

        run_strategy(
            [StreamItem::Event(event)],
            &mut strategy,
            &mut executor,
            &mut context,
        )
        .expect("strategy should run");
        assert_eq!(executor.receipts().len(), 1);
    }
}
"#,
    )?;
    Ok(())
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

    #[test]
    fn strategy_new_scaffold_creates_expected_files() {
        let destination = std::env::temp_dir().join(format!(
            "monad-mev-generated-strategy-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&destination).ok();

        scaffold_strategy_project(&destination).expect("scaffold should be created");

        assert!(destination.join("Cargo.toml").is_file());
        assert!(destination.join("src/lib.rs").is_file());
        assert!(scaffold_strategy_project(&destination).is_err());

        std::fs::remove_dir_all(destination).ok();
    }
}
