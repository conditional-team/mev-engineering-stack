//! FFI hot-path integration tests — exercises the C/C++ compiled library
//! through the safe Rust wrappers to verify correctness after linking.

use mev_core::ffi::hot_path::safe;
use mev_core::ffi::simulator;
use ethers::types::{Address, H256, U256};
use std::str::FromStr;

// ─── Keccak-256 ─────────────────────────────────────────────────────────────

#[test]
fn keccak256_empty_input_matches_known_hash() {
    // Keccak-256("") = c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470
    let hash = safe::keccak256_fast(b"");
    let expected = H256::from_str(
        "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
    ).unwrap();
    assert_eq!(hash, expected, "empty keccak mismatch");
}

#[test]
fn keccak256_hello_world_matches_known_hash() {
    // Keccak-256("hello") = 1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8
    let hash = safe::keccak256_fast(b"hello");
    let expected = H256::from_str(
        "1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8",
    ).unwrap();
    assert_eq!(hash, expected, "hello keccak mismatch");
}

// ─── Function Selector ──────────────────────────────────────────────────────

#[test]
fn function_selector_transfer() {
    // transfer(address,uint256) = 0xa9059cbb
    let sel = safe::function_selector("transfer(address,uint256)");
    assert_eq!(sel, [0xa9, 0x05, 0x9c, 0xbb]);
}

#[test]
fn function_selector_swap_exact_tokens() {
    // swapExactTokensForTokens(uint256,uint256,address[],address,uint256) = 0x38ed1739
    let sel = safe::function_selector(
        "swapExactTokensForTokens(uint256,uint256,address[],address,uint256)",
    );
    assert_eq!(sel, [0x38, 0xed, 0x17, 0x39]);
}

// ─── Address Comparison ─────────────────────────────────────────────────────

#[test]
fn address_eq_same() {
    let addr = Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
    assert!(safe::address_eq(&addr, &addr));
}

#[test]
fn address_eq_different() {
    let a = Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
    let b = Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();
    assert!(!safe::address_eq(&a, &b));
}

// ─── Batch Price Impact ─────────────────────────────────────────────────────

#[test]
fn batch_price_impact_basic() {
    // 4 identical pools: 1M/1M reserves, swap 1000 in
    let r0 = [1_000_000u64; 4];
    let r1 = [1_000_000u64; 4];
    let amount_in = 1_000u64;

    let outputs = safe::calc_price_impact_batch(&r0, &r1, amount_in);

    // out = (1000 * 997 * 1_000_000) / (1_000_000 * 1000 + 1000 * 997)
    //     = 997_000_000_000 / 1_000_997_000 ≈ 996
    for out in &outputs {
        assert!(*out > 0, "output must be positive");
        assert!(*out < amount_in, "output must be less than input (fees)");
    }
    // All 4 should be identical
    assert_eq!(outputs[0], outputs[1]);
    assert_eq!(outputs[1], outputs[2]);
    assert_eq!(outputs[2], outputs[3]);
}

#[test]
fn batch_price_impact_zero_reserves() {
    let r0 = [0u64; 4];
    let r1 = [1_000_000u64; 4];
    let outputs = safe::calc_price_impact_batch(&r0, &r1, 1_000);
    // Zero reserve_in → zero output
    for out in &outputs {
        assert_eq!(*out, 0);
    }
}

// ─── RLP Encoding ───────────────────────────────────────────────────────────

#[test]
fn rlp_encode_address_length() {
    let addr = Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
    let encoded = safe::rlp_encode_address(&addr);
    // RLP: 0x94 prefix + 20 address bytes = 21
    assert_eq!(encoded.len(), 21);
    assert_eq!(encoded[0], 0x94); // 0x80 + 20
}

#[test]
fn rlp_encode_u256_zero() {
    let encoded = safe::rlp_encode_u256(U256::zero());
    // RLP(0) = [0x80]
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0], 0x80);
}

#[test]
fn rlp_encode_u256_small() {
    let encoded = safe::rlp_encode_u256(U256::from(42u64));
    // Single byte for values < 0x80
    assert_eq!(encoded.len(), 1);
    assert_eq!(encoded[0], 42);
}

// ─── AMM Simulator (C++ FFI) ────────────────────────────────────────────────

#[test]
fn v2_amount_out_basic() {
    // Standard Uniswap V2: r0=1M, r1=1M, fee=3000bps (0.3%), swap 1000 in
    let out = simulator::v2_amount_out(1_000_000, 1_000_000, 3000, 1_000);
    assert!(out.is_some());
    let out = out.unwrap();
    assert!(out > 0, "must produce output");
    assert!(out < 1_000, "output < input due to fee + slippage");
}

#[test]
fn v2_amount_out_zero_input() {
    let out = simulator::v2_amount_out(1_000_000, 1_000_000, 3000, 0);
    // C sentinel: zero input is invalid → None
    assert_eq!(out, None);
}

#[test]
fn v2_amount_out_zero_reserves() {
    let out = simulator::v2_amount_out(0, 1_000_000, 3000, 1_000);
    // C sentinel: zero reserves is invalid → None
    assert_eq!(out, None);
}

#[test]
fn v2_amount_in_roundtrips_with_amount_out() {
    let reserve_in = 10_000_000u64;
    let reserve_out = 10_000_000u64;
    let fee = 3000u32;
    let target_out = 50_000u64;

    // Get the amount_in needed for target_out
    let amount_in = simulator::v2_amount_in(reserve_in, reserve_out, fee, target_out);
    assert!(amount_in.is_some());
    let amount_in = amount_in.unwrap();

    // Verify: swapping amount_in should produce >= target_out
    let actual_out = simulator::v2_amount_out(reserve_in, reserve_out, fee, amount_in);
    assert!(actual_out.is_some());
    assert!(
        actual_out.unwrap() >= target_out,
        "roundtrip: in={} → out={}, expected >={}",
        amount_in, actual_out.unwrap(), target_out
    );
}

// ─── RDTSC ──────────────────────────────────────────────────────────────────

#[test]
fn rdtsc_monotonically_increasing() {
    let t1 = safe::rdtsc();
    // Do some work
    let mut x = 0u64;
    for i in 0..1000 { x = x.wrapping_add(i); }
    let _ = x;
    let t2 = safe::rdtsc();
    assert!(t2 > t1, "RDTSC must increase: {} vs {}", t1, t2);
}
