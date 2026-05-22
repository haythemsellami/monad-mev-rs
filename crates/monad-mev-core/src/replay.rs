use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Clock behavior for deterministic or timed replay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayClock {
    /// Process events as fast as the runner can read them.
    #[default]
    AsFastAsPossible,
    /// Sleep a fixed duration between events.
    FixedDelay(Duration),
    /// Scale source timing by a multiplier.
    SpeedMultiplier(f64),
}

impl ReplayClock {
    /// Parses replay speed strings such as `10x` and `1.5x`.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is not a positive multiplier ending in `x`.
    pub fn parse_speed_multiplier(value: &str) -> Result<Self> {
        let Some(number) = value.strip_suffix('x') else {
            return Err(Error::Message(format!(
                "replay speed must end with `x`, got {value}"
            )));
        };
        let multiplier = number.parse::<f64>().map_err(|err| {
            Error::Message(format!("invalid replay speed multiplier {value}: {err}"))
        })?;

        if !multiplier.is_finite() || multiplier <= 0.0 {
            return Err(Error::Message(format!(
                "replay speed multiplier must be positive, got {value}"
            )));
        }

        Ok(Self::SpeedMultiplier(multiplier))
    }

    /// Returns true when replay should avoid wall-clock sleeping.
    #[must_use]
    pub const fn is_deterministic_default(self) -> bool {
        matches!(self, Self::AsFastAsPossible)
    }
}

/// Aggregated replay counters and metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReplayReport {
    /// Source path, if replay was path-backed.
    pub source_path: Option<PathBuf>,
    /// Replay start timestamp in epoch nanoseconds, if recorded.
    pub started_epoch_nanos: Option<u64>,
    /// Replay finish timestamp in epoch nanoseconds, if recorded.
    pub finished_epoch_nanos: Option<u64>,
    /// Raw stream items observed.
    pub events_seen: u64,
    /// Events decoded into a higher-level representation.
    pub events_decoded: u64,
    /// Descriptor gaps observed.
    pub gaps: u64,
    /// Payload expirations observed.
    pub payload_expired: u64,
    /// Schema mismatches observed.
    pub schema_mismatches: u64,
    /// Distinct or inferred blocks observed.
    pub blocks_seen: u64,
    /// Distinct or inferred transactions observed.
    pub transactions_seen: u64,
    /// Logs observed.
    pub logs_seen: u64,
    /// `DeFi` events observed.
    pub defi_events_seen: u64,
    /// Actions recorded by an executor.
    pub actions_recorded: u64,
    /// Strategy errors observed.
    pub strategy_errors: u64,
}

impl ReplayReport {
    /// Creates a new empty replay report for a source path.
    #[must_use]
    pub fn for_source(path: impl Into<PathBuf>) -> Self {
        Self {
            source_path: Some(path.into()),
            ..Self::default()
        }
    }

    /// Records one raw event.
    pub const fn record_event(&mut self) {
        self.events_seen += 1;
    }

    /// Records one decoded event.
    pub const fn record_decoded_event(&mut self) {
        self.events_decoded += 1;
    }

    /// Records one descriptor gap.
    pub const fn record_gap(&mut self) {
        self.gaps += 1;
    }

    /// Records one payload expiration.
    pub const fn record_payload_expired(&mut self) {
        self.payload_expired += 1;
    }

    /// Records one schema mismatch.
    pub const fn record_schema_mismatch(&mut self) {
        self.schema_mismatches += 1;
    }

    /// Records one observed block.
    pub const fn record_block(&mut self) {
        self.blocks_seen += 1;
    }

    /// Records one observed transaction.
    pub const fn record_transaction(&mut self) {
        self.transactions_seen += 1;
    }

    /// Records one observed log.
    pub const fn record_log(&mut self) {
        self.logs_seen += 1;
    }

    /// Returns a stable human summary of report counters.
    #[must_use]
    pub fn human_summary(&self) -> String {
        format!(
            "events={} decoded={} gaps={} payload_expired={} blocks={} txs={} logs={} actions={}",
            self.events_seen,
            self.events_decoded,
            self.gaps,
            self.payload_expired,
            self.blocks_seen,
            self.transactions_seen,
            self.logs_seen,
            self.actions_recorded
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_speed_parser_accepts_valid_multipliers() {
        assert_eq!(
            ReplayClock::parse_speed_multiplier("10x").expect("speed should parse"),
            ReplayClock::SpeedMultiplier(10.0)
        );
        assert_eq!(
            ReplayClock::parse_speed_multiplier("1.5x").expect("speed should parse"),
            ReplayClock::SpeedMultiplier(1.5)
        );
    }

    #[test]
    fn replay_speed_parser_rejects_invalid_values() {
        assert!(ReplayClock::parse_speed_multiplier("10").is_err());
        assert!(ReplayClock::parse_speed_multiplier("0x").is_err());
        assert!(ReplayClock::parse_speed_multiplier("-1x").is_err());
        assert!(ReplayClock::parse_speed_multiplier("abcx").is_err());
    }

    #[test]
    fn replay_report_aggregates_stable_counters() {
        let mut report = ReplayReport::default();

        report.record_event();
        report.record_decoded_event();
        report.record_gap();
        report.record_payload_expired();
        report.record_log();
        report.record_block();
        report.record_transaction();

        assert_eq!(report.events_seen, 1);
        assert_eq!(report.events_decoded, 1);
        assert_eq!(report.gaps, 1);
        assert_eq!(report.payload_expired, 1);
        assert!(report.human_summary().contains("events=1"));
    }
}
