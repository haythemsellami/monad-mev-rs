use monad_mev_events::{UNISWAP_V2_SWAP_SIGNATURE, UNISWAP_V3_SWAP_SIGNATURE};

fn main() {
    println!("{UNISWAP_V2_SWAP_SIGNATURE} {UNISWAP_V3_SWAP_SIGNATURE}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use monad_mev_events::event_topic;

    #[test]
    fn dex_swap_topics_are_distinct() {
        assert_ne!(
            event_topic(UNISWAP_V2_SWAP_SIGNATURE),
            event_topic(UNISWAP_V3_SWAP_SIGNATURE)
        );
    }
}
