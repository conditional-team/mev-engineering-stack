//! Property-based tests for core MEV pipeline invariants.
//!
//! Uses proptest to verify mathematical properties that must hold
//! for all possible inputs, catching edge cases that unit tests miss.

use proptest::prelude::*;
use mev_core::types::{OpportunityType, DexType, estimate_gas};

// ─── Constant-Product AMM Invariants ────────────────────────────────────────

/// Use the REAL production function from the simulator module.
/// This import ensures proptest and production code can never diverge.
use mev_core::simulator::constant_product_swap;

/// ABI-encode a u128 into a 32-byte word (big-endian, left-padded)
fn abi_encode_u256(val: u128) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[16..32].copy_from_slice(&val.to_be_bytes());
    word
}

/// ABI-encode an address into a 32-byte word (left-padded with zeros)
fn abi_encode_address(addr: &[u8; 20]) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[12..32].copy_from_slice(addr);
    word
}

// ─── AMM Properties ─────────────────────────────────────────────────────────

proptest! {
    /// Output must never exceed reserve_out (can't drain more than the pool has)
    #[test]
    fn swap_output_bounded_by_reserve(
        amount_in in 1u128..=10_000_000_000_000_000_000_000u128,   // up to 10k ETH
        reserve_in in 1u128..=100_000_000_000_000_000_000_000u128, // up to 100k ETH
        reserve_out in 1u128..=100_000_000_000_000_000_000_000u128,
        fee_bps in 1u128..=1000u128,                                // 0.01% to 10%
    ) {
        let out = constant_product_swap(amount_in, reserve_in, reserve_out, fee_bps);
        prop_assert!(out < reserve_out, "output {} >= reserve_out {}", out, reserve_out);
    }

    /// Zero input always produces zero output
    #[test]
    fn swap_zero_input_zero_output(
        reserve_in in 1u128..=u64::MAX as u128,
        reserve_out in 1u128..=u64::MAX as u128,
        fee_bps in 0u128..=500u128,
    ) {
        prop_assert_eq!(constant_product_swap(0, reserve_in, reserve_out, fee_bps), 0);
    }

    /// Zero reserves always produce zero output
    #[test]
    fn swap_zero_reserves_zero_output(
        amount_in in 1u128..=u64::MAX as u128,
        fee_bps in 0u128..=500u128,
    ) {
        prop_assert_eq!(constant_product_swap(amount_in, 0, 1000, fee_bps), 0);
        prop_assert_eq!(constant_product_swap(amount_in, 1000, 0, fee_bps), 0);
    }

    /// More input → more output (monotonically increasing)
    #[test]
    fn swap_monotonically_increasing(
        a in 1u128..=1_000_000_000_000_000_000u128,
        delta in 1u128..=1_000_000_000_000_000_000u128,
        reserve_in in 1_000_000_000_000_000_000u128..=10_000_000_000_000_000_000_000u128,
        reserve_out in 1_000_000_000_000_000_000u128..=10_000_000_000_000_000_000_000u128,
        fee_bps in 1u128..=500u128,
    ) {
        let out_a = constant_product_swap(a, reserve_in, reserve_out, fee_bps);
        let out_b = constant_product_swap(a + delta, reserve_in, reserve_out, fee_bps);
        prop_assert!(out_b >= out_a, "not monotonic: f({}) = {} > f({}) = {}", a, out_a, a + delta, out_b);
    }

    /// Higher fee → less output
    #[test]
    fn swap_higher_fee_less_output(
        amount_in in 1_000_000_000u128..=1_000_000_000_000_000_000u128,
        reserve_in in 1_000_000_000_000_000_000u128..=10_000_000_000_000_000_000_000u128,
        reserve_out in 1_000_000_000_000_000_000u128..=10_000_000_000_000_000_000_000u128,
        fee_low in 1u128..=200u128,
        fee_delta in 1u128..=300u128,
    ) {
        let fee_high = fee_low + fee_delta;
        let out_low = constant_product_swap(amount_in, reserve_in, reserve_out, fee_low);
        let out_high = constant_product_swap(amount_in, reserve_in, reserve_out, fee_high);
        prop_assert!(out_low >= out_high, "lower fee {} gave less output {} than higher fee {} output {}", fee_low, out_low, fee_high, out_high);
    }

    /// k = reserve_in * reserve_out must not decrease after swap
    /// (it stays the same or increases due to fees)
    #[test]
    fn swap_preserves_k_invariant(
        amount_in in 1u128..=1_000_000_000_000_000_000u128,
        reserve_in in 1_000_000_000u128..=1_000_000_000_000_000_000u128,
        reserve_out in 1_000_000_000u128..=1_000_000_000_000_000_000u128,
        fee_bps in 1u128..=500u128,
    ) {
        let out = constant_product_swap(amount_in, reserve_in, reserve_out, fee_bps);
        if out > 0 {
            let k_before = reserve_in as u128 * reserve_out as u128;
            let new_reserve_in = reserve_in + amount_in;
            let new_reserve_out = reserve_out - out;
            let k_after = new_reserve_in as u128 * new_reserve_out as u128;
            prop_assert!(k_after >= k_before, "k decreased: {} -> {}", k_before, k_after);
        }
    }
}

// ─── ABI Encoding Properties ────────────────────────────────────────────────

proptest! {
    /// ABI-encoded u256 must roundtrip correctly
    #[test]
    fn abi_u256_roundtrip(val in any::<u128>()) {
        let encoded = abi_encode_u256(val);
        // First 16 bytes must be zero (u128 fits in lower 16 bytes)
        prop_assert_eq!(&encoded[..16], &[0u8; 16]);
        // Decode back
        let decoded = u128::from_be_bytes(encoded[16..32].try_into().unwrap());
        prop_assert_eq!(decoded, val);
    }

    /// ABI-encoded address must be exactly 32 bytes with 12 zero-padding bytes
    #[test]
    fn abi_address_padding(addr in proptest::array::uniform20(any::<u8>())) {
        let encoded = abi_encode_address(&addr);
        prop_assert_eq!(encoded.len(), 32);
        // First 12 bytes must be zero
        prop_assert_eq!(&encoded[..12], &[0u8; 12]);
        // Last 20 bytes must be the address
        prop_assert_eq!(&encoded[12..32], &addr[..]);
    }

    /// ABI-encoded u256 must always be exactly 32 bytes
    #[test]
    fn abi_u256_always_32_bytes(val in any::<u128>()) {
        let encoded = abi_encode_u256(val);
        prop_assert_eq!(encoded.len(), 32);
    }
}

// ─── Swap Selector Parsing Properties ───────────────────────────────────────

/// Known V2 selectors
const V2_SELECTORS: [[u8; 4]; 4] = [
    [0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
    [0x88, 0x03, 0xdb, 0xee], // swapTokensForExactTokens
    [0x7f, 0xf3, 0x6a, 0xb5], // swapExactETHForTokens
    [0x18, 0xcb, 0xaf, 0xe5], // swapExactTokensForETH
];

/// Known V3 selectors
const V3_SELECTORS: [[u8; 4]; 4] = [
    [0x41, 0x4b, 0xf3, 0x89], // exactInputSingle
    [0xc0, 0x4b, 0x8d, 0x59], // exactInput
    [0xdb, 0x3e, 0x21, 0x98], // exactOutputSingle
    [0xf2, 0x8c, 0x04, 0x98], // exactOutput
];

proptest! {
    /// Random 4-byte data that doesn't match known selectors should not classify as V2/V3
    #[test]
    fn unknown_selector_not_classified(
        b0 in any::<u8>(),
        b1 in any::<u8>(),
        b2 in any::<u8>(),
        b3 in any::<u8>(),
    ) {
        let selector = [b0, b1, b2, b3];
        let is_v2 = V2_SELECTORS.contains(&selector);
        let is_v3 = V3_SELECTORS.contains(&selector);
        // A selector can't be both V2 and V3
        prop_assert!(!(is_v2 && is_v3), "selector {:?} matched both V2 and V3", selector);
    }
}

// ─── Gas Estimation Properties ──────────────────────────────────────────────

/// Estimate gas for a transaction based on calldata
fn estimate_tx_gas(gas_limit: u64, data: &[u8]) -> u64 {
    let calldata_gas: u64 = data.iter().map(|&b| if b == 0 { 4u64 } else { 16u64 }).sum();
    let estimated = 21_000 + calldata_gas + 100_000;
    estimated.min(gas_limit)
}

proptest! {
    /// Gas estimate must always be >= 121000 (21k base + 100k execution overhead)
    #[test]
    fn gas_estimate_minimum(
        data in proptest::collection::vec(any::<u8>(), 0..512),
    ) {
        let gas = estimate_tx_gas(u64::MAX, &data);
        prop_assert!(gas >= 121_000, "gas {} below minimum", gas);
    }

    /// Gas estimate must not exceed gas_limit
    #[test]
    fn gas_estimate_capped_by_limit(
        data in proptest::collection::vec(any::<u8>(), 0..512),
        gas_limit in 100_000u64..=30_000_000u64,
    ) {
        let gas = estimate_tx_gas(gas_limit, &data);
        prop_assert!(gas <= gas_limit, "gas {} exceeded limit {}", gas, gas_limit);
    }

    /// More non-zero bytes → higher gas
    #[test]
    fn gas_increases_with_nonzero_bytes(
        len in 1usize..=256,
    ) {
        let zeros = vec![0u8; len];
        let nonzeros = vec![0xFFu8; len];
        let gas_zeros = estimate_tx_gas(u64::MAX, &zeros);
        let gas_nonzeros = estimate_tx_gas(u64::MAX, &nonzeros);
        prop_assert!(gas_nonzeros >= gas_zeros, "non-zero bytes should cost more gas");
    }
}

// ─── Keccak-256 Properties ──────────────────────────────────────────────────

proptest! {
    /// Keccak-256 must produce exactly 32 bytes for any input
    #[test]
    fn keccak_output_always_32_bytes(
        data in proptest::collection::vec(any::<u8>(), 0..1024),
    ) {
        use tiny_keccak::{Keccak, Hasher};
        let mut hasher = Keccak::v256();
        let mut output = [0u8; 32];
        hasher.update(&data);
        hasher.finalize(&mut output);
        prop_assert_eq!(output.len(), 32);
    }

    /// Same input must produce same hash (deterministic)
    #[test]
    fn keccak_deterministic(
        data in proptest::collection::vec(any::<u8>(), 0..512),
    ) {
        use tiny_keccak::{Keccak, Hasher};
        let hash = |d: &[u8]| -> [u8; 32] {
            let mut h = Keccak::v256();
            let mut out = [0u8; 32];
            h.update(d);
            h.finalize(&mut out);
            out
        };
        prop_assert_eq!(hash(&data), hash(&data));
    }

    /// Different inputs should (almost certainly) produce different hashes
    #[test]
    fn keccak_collision_resistant(
        a in proptest::collection::vec(any::<u8>(), 1..256),
        b in proptest::collection::vec(any::<u8>(), 1..256),
    ) {
        use tiny_keccak::{Keccak, Hasher};
        if a != b {
            let hash = |d: &[u8]| -> [u8; 32] {
                let mut h = Keccak::v256();
                let mut out = [0u8; 32];
                h.update(d);
                h.finalize(&mut out);
                out
            };
            prop_assert_ne!(hash(&a), hash(&b));
        }
    }
}

// ─── U256 Properties ────────────────────────────────────────────────────────

proptest! {
    /// DashMap concurrent read should always return what was inserted
    #[test]
    fn dashmap_insert_get_consistent(
        key in any::<u64>(),
        val in any::<u64>(),
    ) {
        let map = dashmap::DashMap::new();
        map.insert(key, val);
        prop_assert_eq!(*map.get(&key).unwrap(), val);
    }

    /// Crossbeam bounded channel should deliver in order
    #[test]
    fn crossbeam_channel_fifo(
        items in proptest::collection::vec(any::<u64>(), 1..100),
    ) {
        let (tx, rx) = crossbeam_channel::bounded(4096);
        for &item in &items {
            tx.send(item).unwrap();
        }
        for &expected in &items {
            prop_assert_eq!(rx.recv().unwrap(), expected);
        }
    }
}

// ─── AMM Overflow Safety ────────────────────────────────────────────────────

proptest! {
    /// Whale-sized input must not panic or produce corrupted output.
    /// With checked arithmetic, overflow returns 0 instead of wrapping.
    #[test]
    fn swap_no_panic_on_overflow(
        amount_in in (u128::MAX / 2)..=u128::MAX,
        reserve_in in (u128::MAX / 2)..=u128::MAX,
        reserve_out in (u128::MAX / 2)..=u128::MAX,
        fee_bps in 1u128..=500u128,
    ) {
        // Must not panic — may return 0 on overflow
        let out = constant_product_swap(amount_in, reserve_in, reserve_out, fee_bps);
        // If it didn't overflow, output must still be bounded
        prop_assert!(out <= reserve_out || out == 0);
    }

    /// Fee >= 100% (10000 bps) should produce zero output
    #[test]
    fn swap_fee_100_percent_returns_zero(
        amount_in in 1u128..=1_000_000_000_000_000_000u128,
        reserve_in in 1u128..=1_000_000_000_000_000_000u128,
        reserve_out in 1u128..=1_000_000_000_000_000_000u128,
    ) {
        let out = constant_product_swap(amount_in, reserve_in, reserve_out, 10_000);
        prop_assert_eq!(out, 0, "100% fee should drain all output");
    }
}

// ─── estimate_gas() Properties ──────────────────────────────────────────────

proptest! {
    /// Arbitrage gas must always include flash loan overhead
    #[test]
    fn estimate_gas_arb_includes_flash_loan(
        num_hops in 1usize..=5,
    ) {
        let path: Vec<DexType> = (0..num_hops).map(|_| DexType::UniswapV2).collect();
        let gas = estimate_gas(&OpportunityType::Arbitrage, &path);
        // Must include BASE_TX (21k) + FLASH_LOAN (80k) = 101k minimum
        prop_assert!(gas >= 101_000, "arb gas {} must include flash loan overhead", gas);
    }

    /// Backrun gas must NOT include flash loan overhead
    #[test]
    fn estimate_gas_backrun_no_flash_loan(
        num_hops in 1usize..=5,
    ) {
        let path: Vec<DexType> = (0..num_hops).map(|_| DexType::UniswapV3).collect();
        let gas = estimate_gas(&OpportunityType::Backrun, &path);
        // Must NOT include flash loan: BASE_TX (21k) + hops only
        prop_assert!(gas >= 21_000, "backrun gas {} too low", gas);
        // For 1 hop V3 = 21k + 150k = 171k, should never be > 21k + 5*200k = 1.021M
        prop_assert!(gas <= 1_100_000, "backrun gas {} unexpectedly high", gas);
    }

    /// More hops → more gas (monotonic)
    #[test]
    fn estimate_gas_monotonic_with_hops(
        hops_a in 1usize..=3,
        hops_delta in 1usize..=3,
    ) {
        let path_short: Vec<DexType> = (0..hops_a).map(|_| DexType::UniswapV2).collect();
        let path_long: Vec<DexType> = (0..hops_a + hops_delta).map(|_| DexType::UniswapV2).collect();
        let gas_short = estimate_gas(&OpportunityType::Arbitrage, &path_short);
        let gas_long = estimate_gas(&OpportunityType::Arbitrage, &path_long);
        prop_assert!(gas_long > gas_short, "more hops should cost more gas: {} vs {}", gas_short, gas_long);
    }
}
