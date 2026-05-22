use monad_mev_core::{keccak256, Address, B256, U256};
use serde::{Deserialize, Serialize};

use crate::LogEvent;

/// ERC20 Transfer event signature.
pub const ERC20_TRANSFER_SIGNATURE: &str = "Transfer(address,address,uint256)";
/// ERC20 Approval event signature.
pub const ERC20_APPROVAL_SIGNATURE: &str = "Approval(address,address,uint256)";
/// Uniswap V2-style Swap event signature.
pub const UNISWAP_V2_SWAP_SIGNATURE: &str = "Swap(address,uint256,uint256,uint256,uint256,address)";
/// Uniswap V2-style Sync event signature.
pub const UNISWAP_V2_SYNC_SIGNATURE: &str = "Sync(uint112,uint112)";
/// Uniswap V3-style Swap event signature.
pub const UNISWAP_V3_SWAP_SIGNATURE: &str =
    "Swap(address,address,int256,int256,uint160,uint128,int24)";

/// Decoded built-in `DeFi` event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeFiEvent {
    /// ERC20 `Transfer`.
    Erc20Transfer(Box<Erc20Transfer>),
    /// ERC20 `Approval`.
    Erc20Approval(Box<Erc20Approval>),
    /// DEX swap event.
    DexSwap(Box<DexSwap>),
    /// DEX reserve sync event.
    DexSync(Box<DexSync>),
    /// Unknown or malformed log.
    UnknownLog(Box<UnknownLog>),
}

/// Decoded ERC20 Transfer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Erc20Transfer {
    /// Decoder name.
    pub decoder: String,
    /// Event signature.
    pub signature: String,
    /// Token contract.
    pub token: Address,
    /// Sender.
    pub from: Address,
    /// Recipient.
    pub to: Address,
    /// Transfer amount.
    pub value: U256,
    /// Original normalized log.
    pub log: LogEvent,
}

/// Decoded ERC20 Approval.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Erc20Approval {
    /// Decoder name.
    pub decoder: String,
    /// Event signature.
    pub signature: String,
    /// Token contract.
    pub token: Address,
    /// Owner.
    pub owner: Address,
    /// Spender.
    pub spender: Address,
    /// Approval amount.
    pub value: U256,
    /// Original normalized log.
    pub log: LogEvent,
}

/// Supported DEX swap family.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DexSwapKind {
    /// Uniswap V2-compatible swap.
    UniswapV2,
    /// Uniswap V3-compatible swap.
    UniswapV3,
}

/// Decoded DEX swap.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DexSwap {
    /// Decoder name.
    pub decoder: String,
    /// Event signature.
    pub signature: String,
    /// Swap family.
    pub kind: DexSwapKind,
    /// Pool contract.
    pub pool: Address,
    /// V2 sender or V3 sender.
    pub sender: Option<Address>,
    /// V2 recipient or V3 recipient.
    pub recipient: Option<Address>,
    /// V2 amount0 in.
    pub amount0_in: Option<U256>,
    /// V2 amount1 in.
    pub amount1_in: Option<U256>,
    /// V2 amount0 out.
    pub amount0_out: Option<U256>,
    /// V2 amount1 out.
    pub amount1_out: Option<U256>,
    /// V3 signed amount0 word.
    pub amount0_delta_raw: Option<B256>,
    /// V3 signed amount1 word.
    pub amount1_delta_raw: Option<B256>,
    /// V3 sqrt price.
    pub sqrt_price_x96: Option<U256>,
    /// V3 liquidity.
    pub liquidity: Option<U256>,
    /// V3 tick.
    pub tick: Option<i32>,
    /// Original normalized log.
    pub log: LogEvent,
}

/// Decoded DEX reserve sync.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DexSync {
    /// Decoder name.
    pub decoder: String,
    /// Event signature.
    pub signature: String,
    /// Pool contract.
    pub pool: Address,
    /// Reserve0.
    pub reserve0: U256,
    /// Reserve1.
    pub reserve1: U256,
    /// Original normalized log.
    pub log: LogEvent,
}

/// Unknown or malformed log.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnknownLog {
    /// Decoder name.
    pub decoder: String,
    /// Event signature if topic0 is known to this decoder.
    pub signature: Option<String>,
    /// Reason the log could not be decoded.
    pub reason: String,
    /// Original normalized log.
    pub log: LogEvent,
}

/// Decodes a log using the built-in V1 `DeFi` decoder set.
#[must_use]
pub fn decode_basic_defi_log(log: LogEvent) -> DeFiEvent {
    if log.malformed {
        return unknown_log(log, None, "malformed log payload");
    }

    match log.topic0() {
        Some(topic) if topic == event_topic(ERC20_TRANSFER_SIGNATURE) => decode_erc20_transfer(log),
        Some(topic) if topic == event_topic(ERC20_APPROVAL_SIGNATURE) => decode_erc20_approval(log),
        Some(topic) if topic == event_topic(UNISWAP_V2_SWAP_SIGNATURE) => decode_v2_swap(log),
        Some(topic) if topic == event_topic(UNISWAP_V2_SYNC_SIGNATURE) => decode_v2_sync(log),
        Some(topic) if topic == event_topic(UNISWAP_V3_SWAP_SIGNATURE) => decode_v3_swap(log),
        Some(_) => unknown_log(log, None, "unknown topic0"),
        None => unknown_log(log, None, "missing topic0"),
    }
}

/// Computes an event topic hash.
#[must_use]
pub fn event_topic(signature: &str) -> B256 {
    keccak256(signature.as_bytes())
}

fn decode_erc20_transfer(log: LogEvent) -> DeFiEvent {
    let Some(token) = log.address else {
        return unknown_log(log, Some(ERC20_TRANSFER_SIGNATURE), "missing token address");
    };
    if log.topics.len() != 3 {
        return unknown_log(log, Some(ERC20_TRANSFER_SIGNATURE), "expected 3 topics");
    }
    let Some(value) = word(&log.data, 0) else {
        return unknown_log(
            log,
            Some(ERC20_TRANSFER_SIGNATURE),
            "expected 32 bytes of data",
        );
    };

    DeFiEvent::Erc20Transfer(Box::new(Erc20Transfer {
        decoder: "erc20_transfer".to_owned(),
        signature: ERC20_TRANSFER_SIGNATURE.to_owned(),
        token,
        from: address_from_topic(log.topics[1]),
        to: address_from_topic(log.topics[2]),
        value,
        log,
    }))
}

fn decode_erc20_approval(log: LogEvent) -> DeFiEvent {
    let Some(token) = log.address else {
        return unknown_log(log, Some(ERC20_APPROVAL_SIGNATURE), "missing token address");
    };
    if log.topics.len() != 3 {
        return unknown_log(log, Some(ERC20_APPROVAL_SIGNATURE), "expected 3 topics");
    }
    let Some(value) = word(&log.data, 0) else {
        return unknown_log(
            log,
            Some(ERC20_APPROVAL_SIGNATURE),
            "expected 32 bytes of data",
        );
    };

    DeFiEvent::Erc20Approval(Box::new(Erc20Approval {
        decoder: "erc20_approval".to_owned(),
        signature: ERC20_APPROVAL_SIGNATURE.to_owned(),
        token,
        owner: address_from_topic(log.topics[1]),
        spender: address_from_topic(log.topics[2]),
        value,
        log,
    }))
}

fn decode_v2_swap(log: LogEvent) -> DeFiEvent {
    let Some(pool) = log.address else {
        return unknown_log(log, Some(UNISWAP_V2_SWAP_SIGNATURE), "missing pool address");
    };
    if log.topics.len() != 3 {
        return unknown_log(log, Some(UNISWAP_V2_SWAP_SIGNATURE), "expected 3 topics");
    }
    let Some(amount0_in) = word(&log.data, 0) else {
        return unknown_log(log, Some(UNISWAP_V2_SWAP_SIGNATURE), "missing amount0_in");
    };
    let Some(amount1_in) = word(&log.data, 1) else {
        return unknown_log(log, Some(UNISWAP_V2_SWAP_SIGNATURE), "missing amount1_in");
    };
    let Some(amount0_out) = word(&log.data, 2) else {
        return unknown_log(log, Some(UNISWAP_V2_SWAP_SIGNATURE), "missing amount0_out");
    };
    let Some(amount1_out) = word(&log.data, 3) else {
        return unknown_log(log, Some(UNISWAP_V2_SWAP_SIGNATURE), "missing amount1_out");
    };

    DeFiEvent::DexSwap(Box::new(DexSwap {
        decoder: "uniswap_v2_swap".to_owned(),
        signature: UNISWAP_V2_SWAP_SIGNATURE.to_owned(),
        kind: DexSwapKind::UniswapV2,
        pool,
        sender: Some(address_from_topic(log.topics[1])),
        recipient: Some(address_from_topic(log.topics[2])),
        amount0_in: Some(amount0_in),
        amount1_in: Some(amount1_in),
        amount0_out: Some(amount0_out),
        amount1_out: Some(amount1_out),
        amount0_delta_raw: None,
        amount1_delta_raw: None,
        sqrt_price_x96: None,
        liquidity: None,
        tick: None,
        log,
    }))
}

fn decode_v2_sync(log: LogEvent) -> DeFiEvent {
    let Some(pool) = log.address else {
        return unknown_log(log, Some(UNISWAP_V2_SYNC_SIGNATURE), "missing pool address");
    };
    let Some(reserve0) = word(&log.data, 0) else {
        return unknown_log(log, Some(UNISWAP_V2_SYNC_SIGNATURE), "missing reserve0");
    };
    let Some(reserve1) = word(&log.data, 1) else {
        return unknown_log(log, Some(UNISWAP_V2_SYNC_SIGNATURE), "missing reserve1");
    };

    DeFiEvent::DexSync(Box::new(DexSync {
        decoder: "uniswap_v2_sync".to_owned(),
        signature: UNISWAP_V2_SYNC_SIGNATURE.to_owned(),
        pool,
        reserve0,
        reserve1,
        log,
    }))
}

fn decode_v3_swap(log: LogEvent) -> DeFiEvent {
    let Some(pool) = log.address else {
        return unknown_log(log, Some(UNISWAP_V3_SWAP_SIGNATURE), "missing pool address");
    };
    if log.topics.len() != 3 {
        return unknown_log(log, Some(UNISWAP_V3_SWAP_SIGNATURE), "expected 3 topics");
    }
    let Some(amount0_delta_raw) = raw_word(&log.data, 0) else {
        return unknown_log(log, Some(UNISWAP_V3_SWAP_SIGNATURE), "missing amount0");
    };
    let Some(amount1_delta_raw) = raw_word(&log.data, 1) else {
        return unknown_log(log, Some(UNISWAP_V3_SWAP_SIGNATURE), "missing amount1");
    };
    let Some(sqrt_price_x96) = word(&log.data, 2) else {
        return unknown_log(
            log,
            Some(UNISWAP_V3_SWAP_SIGNATURE),
            "missing sqrt_price_x96",
        );
    };
    let Some(liquidity) = word(&log.data, 3) else {
        return unknown_log(log, Some(UNISWAP_V3_SWAP_SIGNATURE), "missing liquidity");
    };
    let Some(tick) = int24_word(&log.data, 4) else {
        return unknown_log(log, Some(UNISWAP_V3_SWAP_SIGNATURE), "missing tick");
    };

    DeFiEvent::DexSwap(Box::new(DexSwap {
        decoder: "uniswap_v3_swap".to_owned(),
        signature: UNISWAP_V3_SWAP_SIGNATURE.to_owned(),
        kind: DexSwapKind::UniswapV3,
        pool,
        sender: Some(address_from_topic(log.topics[1])),
        recipient: Some(address_from_topic(log.topics[2])),
        amount0_in: None,
        amount1_in: None,
        amount0_out: None,
        amount1_out: None,
        amount0_delta_raw: Some(amount0_delta_raw),
        amount1_delta_raw: Some(amount1_delta_raw),
        sqrt_price_x96: Some(sqrt_price_x96),
        liquidity: Some(liquidity),
        tick: Some(tick),
        log,
    }))
}

fn unknown_log(log: LogEvent, signature: Option<&str>, reason: &str) -> DeFiEvent {
    DeFiEvent::UnknownLog(Box::new(UnknownLog {
        decoder: "basic_defi".to_owned(),
        signature: signature.map(ToOwned::to_owned),
        reason: reason.to_owned(),
        log,
    }))
}

fn address_from_topic(topic: B256) -> Address {
    Address::from_slice(&topic.as_slice()[12..32])
}

fn word(data: &[u8], index: usize) -> Option<U256> {
    raw_word(data, index).map(|word| U256::from_be_slice(word.as_slice()))
}

fn raw_word(data: &[u8], index: usize) -> Option<B256> {
    let start = index.checked_mul(32)?;
    let bytes: [u8; 32] = data.get(start..start.checked_add(32)?)?.try_into().ok()?;
    Some(B256::from(bytes))
}

fn int24_word(data: &[u8], index: usize) -> Option<i32> {
    let word = raw_word(data, index)?;
    let bytes = word.as_slice();
    let raw = i32::from_be_bytes([0, bytes[29], bytes[30], bytes[31]]);
    Some(if raw & 0x0080_0000 != 0 {
        raw | !0x00ff_ffff
    } else {
        raw
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fixture_log_payload, fixture_raw_envelope, normalize_raw_event, ChainEvent, ExecEventType,
    };

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

    fn log_event(signature: &str, topics: &[B256], data_words: &[u64]) -> LogEvent {
        let address = Address::from([0xaa_u8; 20]);
        let mut all_topics = Vec::with_capacity(topics.len() + 1);
        all_topics.push(event_topic(signature));
        all_topics.extend_from_slice(topics);
        let data = data_words
            .iter()
            .flat_map(|value| word_bytes(*value))
            .collect::<Vec<_>>();
        let payload = fixture_log_payload(address, &all_topics, &data).expect("fixture log");
        let raw = fixture_raw_envelope(1, ExecEventType::TxnLog, [1, 1, 0, 0], payload)
            .expect("fixture raw event");
        let normalized = normalize_raw_event(raw);

        let ChainEvent::Log(log) = normalized.payload else {
            panic!("expected normalized log");
        };
        log
    }

    #[test]
    fn defi_decoder_decodes_erc20_transfer() {
        let from = Address::from([1_u8; 20]);
        let to = Address::from([2_u8; 20]);
        let log = log_event(
            ERC20_TRANSFER_SIGNATURE,
            &[topic_for_address(from), topic_for_address(to)],
            &[100],
        );

        let DeFiEvent::Erc20Transfer(transfer) = decode_basic_defi_log(log) else {
            panic!("expected transfer");
        };

        assert_eq!(transfer.from, from);
        assert_eq!(transfer.to, to);
        assert_eq!(transfer.value, U256::from(100_u64));
    }

    #[test]
    fn defi_decoder_decodes_erc20_approval() {
        let owner = Address::from([3_u8; 20]);
        let spender = Address::from([4_u8; 20]);
        let log = log_event(
            ERC20_APPROVAL_SIGNATURE,
            &[topic_for_address(owner), topic_for_address(spender)],
            &[200],
        );

        let DeFiEvent::Erc20Approval(approval) = decode_basic_defi_log(log) else {
            panic!("expected approval");
        };

        assert_eq!(approval.owner, owner);
        assert_eq!(approval.spender, spender);
        assert_eq!(approval.value, U256::from(200_u64));
    }

    #[test]
    fn defi_decoder_decodes_v2_swap_and_sync() {
        let sender = Address::from([5_u8; 20]);
        let recipient = Address::from([6_u8; 20]);
        let swap_log = log_event(
            UNISWAP_V2_SWAP_SIGNATURE,
            &[topic_for_address(sender), topic_for_address(recipient)],
            &[1, 2, 3, 4],
        );

        let DeFiEvent::DexSwap(swap) = decode_basic_defi_log(swap_log) else {
            panic!("expected swap");
        };
        assert_eq!(swap.kind, DexSwapKind::UniswapV2);
        assert_eq!(swap.amount0_in, Some(U256::from(1_u64)));
        assert_eq!(swap.amount1_out, Some(U256::from(4_u64)));

        let sync_log = log_event(UNISWAP_V2_SYNC_SIGNATURE, &[], &[7, 8]);
        let DeFiEvent::DexSync(sync) = decode_basic_defi_log(sync_log) else {
            panic!("expected sync");
        };
        assert_eq!(sync.reserve0, U256::from(7_u64));
        assert_eq!(sync.reserve1, U256::from(8_u64));
    }

    #[test]
    fn defi_decoder_decodes_v3_swap() {
        let sender = Address::from([7_u8; 20]);
        let recipient = Address::from([8_u8; 20]);
        let log = log_event(
            UNISWAP_V3_SWAP_SIGNATURE,
            &[topic_for_address(sender), topic_for_address(recipient)],
            &[1, 2, 3, 4, 5],
        );

        let DeFiEvent::DexSwap(swap) = decode_basic_defi_log(log) else {
            panic!("expected v3 swap");
        };

        assert_eq!(swap.kind, DexSwapKind::UniswapV3);
        assert_eq!(swap.sqrt_price_x96, Some(U256::from(3_u64)));
        assert_eq!(swap.liquidity, Some(U256::from(4_u64)));
        assert_eq!(swap.tick, Some(5));
    }

    #[test]
    fn defi_decoder_malformed_missing_topics_returns_unknown() {
        let log = log_event(ERC20_TRANSFER_SIGNATURE, &[], &[1]);

        let DeFiEvent::UnknownLog(unknown) = decode_basic_defi_log(log) else {
            panic!("expected unknown");
        };

        assert!(unknown.reason.contains("expected 3 topics"));
    }

    #[test]
    fn defi_decoder_wrong_data_length_returns_unknown() {
        let from = Address::from([1_u8; 20]);
        let to = Address::from([2_u8; 20]);
        let address = Address::from([0xaa_u8; 20]);
        let topics = [
            event_topic(ERC20_TRANSFER_SIGNATURE),
            topic_for_address(from),
            topic_for_address(to),
        ];
        let payload = fixture_log_payload(address, &topics, &[1, 2, 3]).expect("fixture log");
        let raw = fixture_raw_envelope(1, ExecEventType::TxnLog, [1, 1, 0, 0], payload)
            .expect("fixture raw event");
        let ChainEvent::Log(log) = normalize_raw_event(raw).payload else {
            panic!("expected log");
        };

        let DeFiEvent::UnknownLog(unknown) = decode_basic_defi_log(log) else {
            panic!("expected unknown");
        };

        assert!(unknown.reason.contains("expected 32 bytes"));
    }

    #[test]
    fn defi_decoder_unknown_signature_returns_unknown() {
        let log = log_event("Unknown(uint256)", &[], &[1]);

        let DeFiEvent::UnknownLog(unknown) = decode_basic_defi_log(log) else {
            panic!("expected unknown");
        };

        assert_eq!(unknown.reason, "unknown topic0");
    }

    #[test]
    fn defi_decoder_round_trip_json_serialization() {
        let log = log_event(
            ERC20_TRANSFER_SIGNATURE,
            &[
                topic_for_address(Address::from([1_u8; 20])),
                topic_for_address(Address::from([2_u8; 20])),
            ],
            &[100],
        );
        let event = decode_basic_defi_log(log);
        let json = serde_json::to_string(&event).expect("defi event should serialize");
        let decoded: DeFiEvent =
            serde_json::from_str(&json).expect("defi event should deserialize");

        assert_eq!(decoded, event);
    }
}
