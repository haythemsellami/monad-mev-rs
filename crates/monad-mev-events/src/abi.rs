use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use monad_mev_core::{Address, Error, Result, B256, U256};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{event_topic, LogEvent};

/// Generic ABI event decoder.
#[derive(Clone, Debug)]
pub struct AbiDecoder {
    abi_name: String,
    source_path: Option<PathBuf>,
    events_by_topic: BTreeMap<B256, AbiEventDefinition>,
    address_filter: Option<BTreeSet<Address>>,
}

/// ABI event definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AbiEventDefinition {
    /// Event name.
    pub name: String,
    /// Canonical event signature.
    pub signature: String,
    /// Event topic0.
    pub topic0: B256,
    /// Event inputs.
    pub inputs: Vec<AbiEventInput>,
}

/// ABI event input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AbiEventInput {
    /// Input name.
    pub name: String,
    /// Solidity type.
    pub kind: String,
    /// Whether the field is indexed.
    pub indexed: bool,
}

/// Decoded ABI event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DecodedAbiEvent {
    /// ABI name.
    pub abi_name: String,
    /// ABI source path, when loaded from a path.
    pub source_path: Option<PathBuf>,
    /// Event name.
    pub event_name: String,
    /// Canonical signature.
    pub signature: String,
    /// Emitting contract address.
    pub address: Option<Address>,
    /// Decoded fields in ABI order.
    pub fields: Vec<DecodedAbiField>,
    /// Original normalized log.
    pub log: LogEvent,
}

/// Decoded ABI field.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DecodedAbiField {
    /// Field name.
    pub name: String,
    /// Solidity type.
    pub kind: String,
    /// Whether the field came from topics.
    pub indexed: bool,
    /// JSON-friendly decoded value.
    pub value: AbiValue,
}

/// JSON-friendly ABI value representation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum AbiValue {
    /// Address value.
    Address(Address),
    /// Unsigned integer value.
    Uint(U256),
    /// Signed integer stored as the raw ABI word.
    Int(B256),
    /// Boolean value.
    Bool(bool),
    /// Fixed bytes32 value.
    Bytes32(B256),
    /// Unsupported or dynamic value represented as a raw ABI word.
    RawWord(B256),
}

impl AbiDecoder {
    /// Builds a decoder from an ABI JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid JSON, invalid event entries, or anonymous events.
    pub fn from_json_str(abi_name: impl Into<String>, json: &str) -> Result<Self> {
        let value = serde_json::from_str(json)
            .map_err(|err| Error::Message(format!("invalid ABI JSON: {err}")))?;
        Self::from_json_value(abi_name, None, &value)
    }

    /// Loads a decoder from an ABI JSON file.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or the JSON cannot be parsed.
    pub fn load_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let json = std::fs::read_to_string(path).map_err(|err| {
            Error::Message(format!("failed to read ABI file {}: {err}", path.display()))
        })?;
        let abi_name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("abi")
            .to_owned();

        let value = serde_json::from_str(&json)
            .map_err(|err| Error::Message(format!("invalid ABI JSON {}: {err}", path.display())))?;

        Self::from_json_value(abi_name, Some(path.to_path_buf()), &value)
    }

    /// Restricts decoding to the provided emitter addresses.
    #[must_use]
    pub fn with_address_filter(mut self, addresses: impl IntoIterator<Item = Address>) -> Self {
        self.address_filter = Some(addresses.into_iter().collect());
        self
    }

    /// Decodes a normalized log, returning `None` when topic0 or address filter do not match.
    ///
    /// # Errors
    ///
    /// Returns an error when a matching log is malformed for the ABI definition.
    pub fn decode_log(&self, log: LogEvent) -> Result<Option<DecodedAbiEvent>> {
        let Some(topic0) = log.topic0() else {
            return Ok(None);
        };
        let Some(definition) = self.events_by_topic.get(&topic0) else {
            return Ok(None);
        };
        if let Some(filter) = &self.address_filter {
            let Some(address) = log.address else {
                return Ok(None);
            };
            if !filter.contains(&address) {
                return Ok(None);
            }
        }

        let mut topic_index = 1_usize;
        let mut data_index = 0_usize;
        let mut fields = Vec::with_capacity(definition.inputs.len());

        for input in &definition.inputs {
            let word = if input.indexed {
                let Some(topic) = log.topics.get(topic_index).copied() else {
                    return Err(Error::Message(format!(
                        "log missing indexed topic {} for {}",
                        input.name, definition.signature
                    )));
                };
                topic_index += 1;
                topic
            } else {
                let Some(word) = data_word(&log.data, data_index) else {
                    return Err(Error::Message(format!(
                        "log missing data word {} for {}",
                        input.name, definition.signature
                    )));
                };
                data_index += 1;
                word
            };

            fields.push(DecodedAbiField {
                name: input.name.clone(),
                kind: input.kind.clone(),
                indexed: input.indexed,
                value: decode_word(&input.kind, word),
            });
        }

        Ok(Some(DecodedAbiEvent {
            abi_name: self.abi_name.clone(),
            source_path: self.source_path.clone(),
            event_name: definition.name.clone(),
            signature: definition.signature.clone(),
            address: log.address,
            fields,
            log,
        }))
    }

    fn from_json_value(
        abi_name: impl Into<String>,
        source_path: Option<PathBuf>,
        value: &Value,
    ) -> Result<Self> {
        let entries = value
            .as_array()
            .or_else(|| value.get("abi").and_then(Value::as_array))
            .ok_or_else(|| {
                Error::Message("ABI JSON must be an array or object with abi array".to_owned())
            })?;

        let mut events_by_topic = BTreeMap::new();
        for entry in entries {
            if entry.get("type").and_then(Value::as_str) != Some("event") {
                continue;
            }
            let definition = parse_event_definition(entry)?;
            events_by_topic.insert(definition.topic0, definition);
        }

        Ok(Self {
            abi_name: abi_name.into(),
            source_path,
            events_by_topic,
            address_filter: None,
        })
    }
}

fn parse_event_definition(entry: &Value) -> Result<AbiEventDefinition> {
    let name = entry
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Message("ABI event missing name".to_owned()))?
        .to_owned();
    if entry
        .get("anonymous")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(Error::Message(format!(
            "anonymous ABI event {name} is not supported"
        )));
    }
    let inputs_value = entry
        .get("inputs")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Message(format!("ABI event {name} missing inputs")))?;
    let inputs = inputs_value
        .iter()
        .map(parse_event_input)
        .collect::<Result<Vec<_>>>()?;
    let signature = format!(
        "{}({})",
        name,
        inputs
            .iter()
            .map(|input| input.kind.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    let topic0 = event_topic(&signature);

    Ok(AbiEventDefinition {
        name,
        signature,
        topic0,
        inputs,
    })
}

fn parse_event_input(value: &Value) -> Result<AbiEventInput> {
    Ok(AbiEventInput {
        name: value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        kind: value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Message("ABI event input missing type".to_owned()))?
            .to_owned(),
        indexed: value
            .get("indexed")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn decode_word(kind: &str, word: B256) -> AbiValue {
    match kind {
        "address" => AbiValue::Address(Address::from_slice(&word.as_slice()[12..32])),
        "bool" => AbiValue::Bool(word.as_slice()[31] != 0),
        "bytes32" => AbiValue::Bytes32(word),
        kind if kind.starts_with("uint") => AbiValue::Uint(U256::from_be_slice(word.as_slice())),
        kind if kind.starts_with("int") => AbiValue::Int(word),
        _ => AbiValue::RawWord(word),
    }
}

fn data_word(data: &[u8], index: usize) -> Option<B256> {
    let start = index.checked_mul(32)?;
    let bytes: [u8; 32] = data.get(start..start.checked_add(32)?)?.try_into().ok()?;
    Some(B256::from(bytes))
}

#[cfg(test)]
mod tests {
    use monad_mev_core::Address;

    use super::*;
    use crate::{
        fixture_log_payload, fixture_raw_envelope, normalize_raw_event, ChainEvent, ExecEventType,
    };

    const ERC20_ABI: &str = r#"[
      {"type":"event","name":"Transfer","inputs":[
        {"name":"from","type":"address","indexed":true},
        {"name":"to","type":"address","indexed":true},
        {"name":"value","type":"uint256","indexed":false}
      ]}
    ]"#;

    fn topic_for_address(address: Address) -> B256 {
        let mut topic = [0_u8; 32];
        topic[12..32].copy_from_slice(address.as_slice());
        B256::from(topic)
    }

    fn word_bytes(value: u64) -> [u8; 32] {
        let mut word = [0_u8; 32];
        word[24..32].copy_from_slice(&value.to_be_bytes());
        word
    }

    fn log(signature: &str, emitter: Address, topics: &[B256], words: &[u64]) -> LogEvent {
        let mut all_topics = Vec::with_capacity(topics.len() + 1);
        all_topics.push(event_topic(signature));
        all_topics.extend_from_slice(topics);
        let data = words
            .iter()
            .flat_map(|value| word_bytes(*value))
            .collect::<Vec<_>>();
        let payload = fixture_log_payload(emitter, &all_topics, &data).expect("fixture log");
        let raw = fixture_raw_envelope(1, ExecEventType::TxnLog, [1, 1, 0, 0], payload)
            .expect("fixture raw event");
        let ChainEvent::Log(log) = normalize_raw_event(raw).payload else {
            panic!("expected log");
        };
        log
    }

    #[test]
    fn abi_decoder_decodes_simple_erc20_event() {
        let decoder = AbiDecoder::from_json_str("erc20", ERC20_ABI).expect("ABI should parse");
        let from = Address::from([1_u8; 20]);
        let to = Address::from([2_u8; 20]);
        let event = decoder
            .decode_log(log(
                "Transfer(address,address,uint256)",
                Address::from([9_u8; 20]),
                &[topic_for_address(from), topic_for_address(to)],
                &[100],
            ))
            .expect("decode should not fail")
            .expect("log should match");

        assert_eq!(event.event_name, "Transfer");
        assert_eq!(event.fields[0].value, AbiValue::Address(from));
        assert_eq!(event.fields[2].value, AbiValue::Uint(U256::from(100_u64)));
    }

    #[test]
    fn abi_decoder_decodes_custom_mixed_event() {
        let abi = r#"[{"type":"event","name":"Mixed","inputs":[
          {"name":"who","type":"address","indexed":true},
          {"name":"enabled","type":"bool","indexed":false},
          {"name":"salt","type":"bytes32","indexed":false}
        ]}]"#;
        let decoder = AbiDecoder::from_json_str("mixed", abi).expect("ABI should parse");
        let who = Address::from([3_u8; 20]);
        let event = decoder
            .decode_log(log(
                "Mixed(address,bool,bytes32)",
                Address::from([4_u8; 20]),
                &[topic_for_address(who)],
                &[1, 9],
            ))
            .expect("decode should not fail")
            .expect("log should match");

        assert_eq!(event.fields[0].value, AbiValue::Address(who));
        assert_eq!(event.fields[1].value, AbiValue::Bool(true));
        assert_eq!(
            event.fields[2].value,
            AbiValue::Bytes32(B256::from(word_bytes(9)))
        );
    }

    #[test]
    fn abi_decoder_unknown_topic_returns_no_match() {
        let decoder = AbiDecoder::from_json_str("erc20", ERC20_ABI).expect("ABI should parse");
        let result = decoder
            .decode_log(log(
                "Unknown(uint256)",
                Address::from([1_u8; 20]),
                &[],
                &[1],
            ))
            .expect("decode should not fail");

        assert!(result.is_none());
    }

    #[test]
    fn abi_decoder_invalid_json_errors() {
        let error = AbiDecoder::from_json_str("bad", "{").expect_err("invalid JSON should fail");

        assert!(error.to_string().contains("invalid ABI JSON"));
    }

    #[test]
    fn abi_decoder_overloaded_events_match_by_topic() {
        let abi = r#"[{"type":"event","name":"Over","inputs":[{"name":"a","type":"uint256"}]},
          {"type":"event","name":"Over","inputs":[{"name":"a","type":"address","indexed":true}]}]"#;
        let decoder = AbiDecoder::from_json_str("over", abi).expect("ABI should parse");
        let who = Address::from([7_u8; 20]);
        let event = decoder
            .decode_log(log(
                "Over(address)",
                Address::from([1_u8; 20]),
                &[topic_for_address(who)],
                &[],
            ))
            .expect("decode should not fail")
            .expect("log should match");

        assert_eq!(event.signature, "Over(address)");
        assert_eq!(event.fields[0].value, AbiValue::Address(who));
    }

    #[test]
    fn abi_decoder_address_filter_includes_and_excludes() {
        let emitter = Address::from([8_u8; 20]);
        let decoder = AbiDecoder::from_json_str("erc20", ERC20_ABI)
            .expect("ABI should parse")
            .with_address_filter([emitter]);
        let matching = decoder
            .decode_log(log(
                "Transfer(address,address,uint256)",
                emitter,
                &[
                    topic_for_address(Address::from([1_u8; 20])),
                    topic_for_address(Address::from([2_u8; 20])),
                ],
                &[1],
            ))
            .expect("decode should not fail");
        let excluded = decoder
            .decode_log(log(
                "Transfer(address,address,uint256)",
                Address::from([9_u8; 20]),
                &[
                    topic_for_address(Address::from([1_u8; 20])),
                    topic_for_address(Address::from([2_u8; 20])),
                ],
                &[1],
            ))
            .expect("decode should not fail");

        assert!(matching.is_some());
        assert!(excluded.is_none());
    }

    #[test]
    fn abi_decoder_rejects_anonymous_events() {
        let abi = r#"[{"type":"event","name":"Hidden","anonymous":true,"inputs":[]}]"#;
        let error =
            AbiDecoder::from_json_str("anon", abi).expect_err("anonymous events should fail");

        assert!(error.to_string().contains("anonymous"));
    }
}
