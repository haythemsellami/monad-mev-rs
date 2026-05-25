use std::path::{Path, PathBuf};

use monad_mev_core::{
    Address, BlockRef, CommitState, Error, EventSourceKind, ReplayReport, Result, StreamItem,
    TxnRef, B256,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    event_topic, fixture_block_tag_payload, fixture_block_verified_payload, fixture_log_payload,
    fixture_raw_envelope, fixture_txn_header_start_payload, normalize_raw_event, ChainEvent,
    ExecEventType, ERC20_TRANSFER_SIGNATURE,
};

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

/// Converts a deterministic fixture document into normalized stream items.
///
/// Sequence gaps are inserted when fixture `seqno` values skip ahead.
///
/// # Errors
///
/// Returns an error when a fixture event record is malformed.
pub fn fixture_stream_items(fixture: &FixtureDocument) -> Result<Vec<StreamItem<ChainEvent>>> {
    let mut items = Vec::new();
    let mut next_seqno = None;

    for event in &fixture.events {
        let seqno = required_u64(event, "seqno")?;
        if let Some(expected) = next_seqno {
            if seqno > expected {
                items.push(StreamItem::Gap(monad_mev_core::GapEvent::new(
                    expected,
                    seqno,
                    EventSourceKind::Fixture,
                )));
            }
        }

        items.push(StreamItem::Event(fixture_event(event)?));
        next_seqno = seqno.checked_add(1);
    }

    items.push(StreamItem::SourceEnded);
    Ok(items)
}

fn fixture_event(event: &Value) -> Result<monad_mev_core::EventEnvelope<ChainEvent>> {
    let seqno = required_u64(event, "seqno")?;
    let kind = required_str(event, "kind")?;
    let block_number = optional_u64(event, "block")?.unwrap_or(100);
    let block_id = block_id(block_number);
    let txn_idx = optional_u64(event, "txn")?;
    let event_type = fixture_exec_event_type(kind)?;
    let payload = fixture_payload(event, event_type, block_id, block_number)?;
    let flow = [
        if optional_u64(event, "block")?.is_some() {
            block_number
        } else {
            0
        },
        txn_idx.map_or(0, |idx| idx.saturating_add(1)),
        0,
        0,
    ];
    let mut raw = fixture_raw_envelope(seqno, event_type, flow, payload)?;

    if optional_u64(event, "block")?.is_some()
        || matches!(
            event_type,
            ExecEventType::BlockStart
                | ExecEventType::BlockQc
                | ExecEventType::BlockFinalized
                | ExecEventType::BlockVerified
        )
    {
        raw.meta.block = Some(BlockRef {
            block_id,
            proposed_block_number: block_number,
            block_start_seqno: if matches!(event_type, ExecEventType::BlockStart) {
                seqno
            } else {
                1
            },
        });
    }
    if let Some(txn_idx) = txn_idx {
        raw.meta.txn = Some(TxnRef {
            txn_idx,
            txn_hash: if matches!(event_type, ExecEventType::TxnHeaderStart) {
                Some(txn_hash(seqno, txn_idx))
            } else {
                None
            },
        });
    }
    raw.meta.commit_state = fixture_commit_state(event, event_type)?;

    Ok(normalize_raw_event(raw))
}

fn fixture_exec_event_type(kind: &str) -> Result<ExecEventType> {
    match kind {
        "block_start" => Ok(ExecEventType::BlockStart),
        "block_qc" => Ok(ExecEventType::BlockQc),
        "block_finalized" => Ok(ExecEventType::BlockFinalized),
        "block_verified" => Ok(ExecEventType::BlockVerified),
        "txn_header_start" => Ok(ExecEventType::TxnHeaderStart),
        "txn_header_end" => Ok(ExecEventType::TxnHeaderEnd),
        "txn_log" | "erc20_transfer" => Ok(ExecEventType::TxnLog),
        "txn_end" => Ok(ExecEventType::TxnEnd),
        "txn_evm_output" => Ok(ExecEventType::TxnEvmOutput),
        other => Err(Error::Message(format!(
            "unsupported fixture event kind `{other}`"
        ))),
    }
}

fn fixture_payload(
    event: &Value,
    event_type: ExecEventType,
    block_id: B256,
    block_number: u64,
) -> Result<Vec<u8>> {
    match event_type {
        ExecEventType::BlockStart | ExecEventType::BlockQc | ExecEventType::BlockFinalized => {
            Ok(fixture_block_tag_payload(block_id, block_number))
        }
        ExecEventType::BlockVerified => Ok(fixture_block_verified_payload(block_number)),
        ExecEventType::TxnHeaderStart => Ok(fixture_txn_header_start_payload(txn_hash(
            required_u64(event, "seqno")?,
            optional_u64(event, "txn")?.unwrap_or_default(),
        ))),
        ExecEventType::TxnLog => fixture_log(event),
        _ => Ok(Vec::new()),
    }
}

fn fixture_log(event: &Value) -> Result<Vec<u8>> {
    if optional_bool(event, "malformed")?.unwrap_or(false) {
        if let Some(data) = optional_str(event, "data")? {
            return parse_hex_bytes(data);
        }
        return Ok(vec![0, 1, 2]);
    }

    if required_str(event, "kind")? == "erc20_transfer" {
        let token = optional_address(event, "token")?.unwrap_or_else(default_address);
        let from = required_address(event, "from")?;
        let to = required_address(event, "to")?;
        let value = required_u64_string(event, "value")?;
        return fixture_log_payload(
            token,
            &[
                event_topic(ERC20_TRANSFER_SIGNATURE),
                topic_address(from),
                topic_address(to),
            ],
            &u256_word(value),
        );
    }

    let address = optional_address(event, "address")?.unwrap_or_else(default_address);
    let data = optional_str(event, "data")?
        .map(parse_hex_bytes)
        .transpose()?
        .unwrap_or_default();
    fixture_log_payload(address, &[event_topic("FixtureSignal(uint8)")], &data)
}

fn fixture_commit_state(event: &Value, event_type: ExecEventType) -> Result<CommitState> {
    if let Some(state) = optional_str(event, "state")? {
        return match state {
            "unknown" => Ok(CommitState::Unknown),
            "proposed" => Ok(CommitState::Proposed),
            "voted" => Ok(CommitState::Voted),
            "finalized" => Ok(CommitState::Finalized),
            "verified" => Ok(CommitState::Verified),
            "abandoned" => Ok(CommitState::Abandoned),
            other => Err(Error::Message(format!(
                "unsupported commit state `{other}`"
            ))),
        };
    }

    Ok(match event_type {
        ExecEventType::BlockStart => CommitState::Proposed,
        ExecEventType::BlockQc => CommitState::Voted,
        ExecEventType::BlockFinalized => CommitState::Finalized,
        ExecEventType::BlockVerified => CommitState::Verified,
        _ => CommitState::Unknown,
    })
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    optional_str(value, field)?.ok_or_else(|| Error::Message(format!("missing `{field}`")))
}

fn optional_str<'a>(value: &'a Value, field: &str) -> Result<Option<&'a str>> {
    value
        .get(field)
        .map(|field_value| {
            field_value
                .as_str()
                .ok_or_else(|| Error::Message(format!("`{field}` must be a string")))
        })
        .transpose()
}

fn required_u64(value: &Value, field: &str) -> Result<u64> {
    optional_u64(value, field)?.ok_or_else(|| Error::Message(format!("missing `{field}`")))
}

fn optional_u64(value: &Value, field: &str) -> Result<Option<u64>> {
    value
        .get(field)
        .map(|field_value| {
            field_value
                .as_u64()
                .ok_or_else(|| Error::Message(format!("`{field}` must be an unsigned integer")))
        })
        .transpose()
}

fn optional_bool(value: &Value, field: &str) -> Result<Option<bool>> {
    value
        .get(field)
        .map(|field_value| {
            field_value
                .as_bool()
                .ok_or_else(|| Error::Message(format!("`{field}` must be a boolean")))
        })
        .transpose()
}

fn required_u64_string(value: &Value, field: &str) -> Result<u64> {
    let string = required_str(value, field)?;
    string
        .parse::<u64>()
        .map_err(|err| Error::Message(format!("`{field}` must fit u64: {err}")))
}

fn required_address(value: &Value, field: &str) -> Result<Address> {
    optional_address(value, field)?.ok_or_else(|| Error::Message(format!("missing `{field}`")))
}

fn optional_address(value: &Value, field: &str) -> Result<Option<Address>> {
    optional_str(value, field)?.map(parse_address).transpose()
}

fn parse_address(value: &str) -> Result<Address> {
    value
        .parse()
        .map_err(|err| Error::Message(format!("invalid address `{value}`: {err}")))
}

fn parse_hex_bytes(value: &str) -> Result<Vec<u8>> {
    let hex = value.strip_prefix("0x").unwrap_or(value);
    if hex.len() % 2 != 0 {
        return Err(Error::Message(format!(
            "hex byte string `{value}` must have an even length"
        )));
    }

    (0..hex.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&hex[index..index + 2], 16)
                .map_err(|err| Error::Message(format!("invalid hex byte string `{value}`: {err}")))
        })
        .collect()
}

fn default_address() -> Address {
    Address::from([0xaa_u8; 20])
}

fn block_id(block_number: u64) -> B256 {
    let mut bytes = [0_u8; 32];
    bytes[24..].copy_from_slice(&block_number.to_be_bytes());
    B256::from(bytes)
}

fn txn_hash(seqno: u64, txn_idx: u64) -> B256 {
    let mut bytes = [0_u8; 32];
    bytes[16..24].copy_from_slice(&seqno.to_be_bytes());
    bytes[24..].copy_from_slice(&txn_idx.to_be_bytes());
    B256::from(bytes)
}

fn topic_address(address: Address) -> B256 {
    let mut bytes = [0_u8; 32];
    bytes[12..].copy_from_slice(address.as_slice());
    B256::from(bytes)
}

fn u256_word(value: u64) -> Vec<u8> {
    let mut bytes = [0_u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    bytes.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_basic_defi_log, DeFiEvent, ReplayConfig, ReplayRunner};

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
            "lifecycle-signal.json",
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

    #[test]
    fn fixture_stream_items_replay_to_expected_report() {
        for name in ["raw-events.json", "chain-events.json", "gap.json"] {
            let fixture = load_workspace_fixture(name).expect("fixture");
            let items = fixture_stream_items(&fixture).expect("stream items");
            let run = ReplayRunner::new(ReplayConfig::default())
                .run(items)
                .expect("replay should run");

            assert_eq!(
                fixture.expected_report,
                Some(FixtureReport::from(&run.report))
            );
        }
    }

    #[test]
    fn fixture_stream_items_insert_gap_events() {
        let fixture = load_workspace_fixture("gap.json").expect("fixture");
        let items = fixture_stream_items(&fixture).expect("stream items");

        assert!(matches!(items.get(1), Some(StreamItem::Gap(gap)) if gap.missed_count == 1));
    }

    #[test]
    fn defi_fixture_decodes_transfer_log() {
        let fixture = load_workspace_fixture("defi-decoded.json").expect("fixture");
        let items = fixture_stream_items(&fixture).expect("stream items");
        let Some(StreamItem::Event(event)) = items.first() else {
            panic!("expected first fixture event");
        };
        let ChainEvent::Log(log) = &event.payload else {
            panic!("expected log");
        };

        assert!(matches!(
            decode_basic_defi_log(log.clone()),
            DeFiEvent::Erc20Transfer(_)
        ));
    }
}
