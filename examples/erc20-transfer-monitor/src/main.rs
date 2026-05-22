use monad_mev_core::{Address, B256};
use monad_mev_events::{
    decode_basic_defi_log, event_topic, fixture_log_payload, fixture_raw_envelope,
    normalize_raw_event, ChainEvent, DeFiEvent, ExecEventType, ERC20_TRANSFER_SIGNATURE,
};

fn main() {
    let _ = sample_transfer();
    println!("{ERC20_TRANSFER_SIGNATURE}");
}

fn sample_transfer() -> DeFiEvent {
    let mut from = [0_u8; 32];
    from[31] = 1;
    let mut to = [0_u8; 32];
    to[31] = 2;
    let mut value = [0_u8; 32];
    value[31] = 100;
    let payload = fixture_log_payload(
        Address::from([0xaa_u8; 20]),
        &[
            event_topic(ERC20_TRANSFER_SIGNATURE),
            B256::from(from),
            B256::from(to),
        ],
        &value,
    )
    .expect("fixture log should build");
    let raw = fixture_raw_envelope(1, ExecEventType::TxnLog, [1, 1, 0, 0], payload)
        .expect("fixture raw should build");
    let ChainEvent::Log(log) = normalize_raw_event(raw).payload else {
        panic!("expected log");
    };

    decode_basic_defi_log(log)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erc20_transfer_monitor_decodes_sample() {
        assert!(matches!(sample_transfer(), DeFiEvent::Erc20Transfer(_)));
    }
}
