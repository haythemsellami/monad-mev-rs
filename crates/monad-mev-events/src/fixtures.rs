use std::path::{Path, PathBuf};

use monad_mev_core::{Error, ReplayReport, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Deterministic JSON fixture document.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FixtureDocument {
    /// Fixture name.
    pub name: String,
    /// Fixture description.
    pub description: String,
    /// Fixture event records.
    pub events: Vec<Value>,
    /// Expected stable report counters.
    pub expected_report: Option<FixtureReport>,
}

/// Stable subset of replay report counters used in golden fixtures.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FixtureReport {
    /// Events seen.
    pub events_seen: u64,
    /// Decoded events.
    pub events_decoded: u64,
    /// Gaps.
    pub gaps: u64,
    /// Payload expirations.
    pub payload_expired: u64,
    /// Logs seen.
    pub logs_seen: u64,
}

impl From<&ReplayReport> for FixtureReport {
    fn from(report: &ReplayReport) -> Self {
        Self {
            events_seen: report.events_seen,
            events_decoded: report.events_decoded,
            gaps: report.gaps,
            payload_expired: report.payload_expired,
            logs_seen: report.logs_seen,
        }
    }
}

/// Returns the workspace fixture directory.
#[must_use]
pub fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Returns a path inside the workspace fixture directory.
#[must_use]
pub fn fixture_path(name: impl AsRef<Path>) -> PathBuf {
    fixture_root().join(name)
}

/// Loads a fixture JSON document by path.
///
/// # Errors
///
/// Returns an error when the fixture cannot be read or parsed.
pub fn load_fixture(path: impl AsRef<Path>) -> Result<FixtureDocument> {
    let path = path.as_ref();
    let json = std::fs::read_to_string(path).map_err(|err| {
        Error::Message(format!("failed to read fixture {}: {err}", path.display()))
    })?;
    serde_json::from_str(&json)
        .map_err(|err| Error::Message(format!("failed to parse fixture {}: {err}", path.display())))
}

/// Loads a fixture JSON document from the workspace fixture directory.
///
/// # Errors
///
/// Returns an error when the fixture cannot be read or parsed.
pub fn load_workspace_fixture(name: &str) -> Result<FixtureDocument> {
    load_fixture(fixture_path(name))
}

/// Loads a golden file as raw text.
///
/// # Errors
///
/// Returns an error when the golden file cannot be read.
pub fn load_golden(name: &str) -> Result<String> {
    let path = fixture_path("golden").join(name);
    std::fs::read_to_string(&path)
        .map_err(|err| Error::Message(format!("failed to read golden {}: {err}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_loader_reads_required_fixtures() {
        for name in [
            "empty.json",
            "raw-events.json",
            "chain-events.json",
            "defi-decoded.json",
            "interleaved-transactions.json",
            "malformed-logs.json",
            "gap.json",
            "commit-states.json",
        ] {
            let fixture = load_workspace_fixture(name).expect("fixture should load");
            assert!(!fixture.name.is_empty());
            assert!(fixture.expected_report.is_some());
        }
    }

    #[test]
    fn golden_report_matches_raw_events_fixture() {
        let fixture = load_workspace_fixture("raw-events.json").expect("fixture should load");
        let golden = load_golden("report.json").expect("golden report should load");
        let golden_report: FixtureReport =
            serde_json::from_str(&golden).expect("golden report should parse");

        assert_eq!(fixture.expected_report, Some(golden_report));
    }

    #[test]
    fn golden_decoded_jsonl_is_stable_json() {
        let decoded = load_golden("decoded.jsonl").expect("golden decoded should load");

        for line in decoded.lines() {
            let value: Value = serde_json::from_str(line).expect("JSONL line should parse");
            assert!(value.get("seqno").is_some());
        }
    }

    #[test]
    fn golden_action_jsonl_is_stable_json() {
        let actions = load_golden("actions.jsonl").expect("golden actions should load");

        for line in actions.lines() {
            let value: Value = serde_json::from_str(line).expect("JSONL line should parse");
            assert_eq!(value.get("type").and_then(Value::as_str), Some("record"));
        }
    }

    #[test]
    fn fixture_determinism_loads_same_document_twice() {
        let first = load_workspace_fixture("interleaved-transactions.json").expect("fixture");
        let second = load_workspace_fixture("interleaved-transactions.json").expect("fixture");

        assert_eq!(first, second);
    }
}
