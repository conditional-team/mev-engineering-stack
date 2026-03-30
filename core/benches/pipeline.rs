//! Criterion benchmarks for the MEV detection pipeline.
//!
//! Run with: `cargo bench`
//! Reports: `target/criterion/report/index.html`

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ethers::types::{Address, U256};
use std::collections::HashMap;
use std::hint::black_box as bb;

// ─── Keccak-256 ─────────────────────────────────────────────────────────────

fn bench_keccak256(c: &mut Criterion) {
    use tiny_keccak::{Keccak, Hasher};

    let mut group = c.benchmark_group("crypto");
    for size in [20, 32, 64, 256] {
        let input = vec![0xABu8; size];
        group.bench_with_input(
            BenchmarkId::new("keccak256", size),
            &input,
            |b, data| {
                b.iter(|| {
                    let mut hasher = Keccak::v256();
                    let mut output = [0u8; 32];
                    hasher.update(data);
                    hasher.finalize(&mut output);
                    black_box(output)
                });
            },
        );
    }
    group.finish();
}

// ─── Constant-Product AMM Swap ──────────────────────────────────────────────

#[inline(always)]
fn constant_product_swap(amount_in: u128, reserve_in: u128, reserve_out: u128, fee_bps: u128) -> u128 {
    let fee_factor = 10_000 - fee_bps;
    let amount_with_fee = amount_in * fee_factor;
    let numerator = amount_with_fee * reserve_out;
    let denominator = reserve_in * 10_000 + amount_with_fee;
    numerator / denominator
}

fn bench_swap_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("amm");

    // Realistic pool reserves
    let weth_reserve: u128 = 5_000_000_000_000_000_000_000;   // 5000 ETH
    let usdc_reserve: u128 = 10_000_000_000_000;               // 10M USDC

    let amounts: &[(&str, u128)] = &[
        ("0.1_ETH",  100_000_000_000_000_000),
        ("1_ETH",   1_000_000_000_000_000_000),
        ("10_ETH", 10_000_000_000_000_000_000),
    ];

    for (label, amount) in amounts {
        group.bench_with_input(
            BenchmarkId::new("constant_product", label),
            amount,
            |b, &amt| {
                b.iter(|| {
                    black_box(constant_product_swap(amt, weth_reserve, usdc_reserve, 30))
                });
            },
        );
    }

    // Two-hop arbitrage (buy on DEX A, sell on DEX B)
    group.bench_function("two_hop_arbitrage", |b| {
        let amount_in: u128 = 1_000_000_000_000_000_000; // 1 ETH
        let pool_a_r0: u128 = 5_000_000_000_000_000_000_000;
        let pool_a_r1: u128 = 10_000_000_000_000;
        let pool_b_r0: u128 = 10_200_000_000_000; // slightly different = arb
        let pool_b_r1: u128 = 5_000_000_000_000_000_000_000;

        b.iter(|| {
            let mid = constant_product_swap(amount_in, pool_a_r0, pool_a_r1, 30);
            let out = constant_product_swap(mid, pool_b_r0, pool_b_r1, 30);
            black_box(out)
        });
    });

    group.finish();
}

// ─── Pool Lookup (DashMap) ──────────────────────────────────────────────────

fn bench_pool_lookup(c: &mut Criterion) {
    use dashmap::DashMap;

    let map: DashMap<[u8; 20], (u128, u128)> = DashMap::new();

    // Fill 10k pools
    for i in 0u64..10_000 {
        let mut addr = [0u8; 20];
        addr[12..20].copy_from_slice(&i.to_be_bytes());
        map.insert(addr, (5_000_000_000_000_000_000_000u128, 10_000_000_000_000u128));
    }

    let target = {
        let mut addr = [0u8; 20];
        addr[12..20].copy_from_slice(&5_000u64.to_be_bytes());
        addr
    };

    c.bench_function("pool_lookup_10k", |b| {
        b.iter(|| black_box(map.get(&target)))
    });
}

// ─── ABI Encoding ───────────────────────────────────────────────────────────

fn bench_abi_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("abi");

    // Encode address (20 bytes → 32 bytes zero-padded)
    group.bench_function("encode_address", |b| {
        let addr_hex = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
        b.iter(|| {
            let addr = &addr_hex[2..];
            let bytes = hex::decode(addr).unwrap();
            let mut padded = vec![0u8; 12];
            padded.extend_from_slice(&bytes);
            black_box(padded)
        });
    });

    // Encode uint256
    group.bench_function("encode_u256", |b| {
        let value: u128 = 1_000_000_000_000_000_000;
        b.iter(|| {
            let mut buf = [0u8; 32];
            buf[16..].copy_from_slice(&value.to_be_bytes());
            black_box(buf)
        });
    });

    // Encode swap path (3 hops × 24 bytes = 72 bytes)
    group.bench_function("encode_swap_path_3hop", |b| {
        b.iter(|| {
            let mut data = Vec::with_capacity(72);
            for _ in 0..3 {
                data.push(0x01u8); // dex type
                data.extend_from_slice(&[0u8; 20]); // pool address
                data.extend_from_slice(&[0x00, 0x0B, 0xB8]); // fee 3000
            }
            black_box(data)
        });
    });

    group.finish();
}

// ─── Full Detection Pipeline (simulated) ────────────────────────────────────

fn bench_detection_pipeline(c: &mut Criterion) {
    use tiny_keccak::{Keccak, Hasher};

    c.bench_function("full_pipeline_detect_simulate_build", |b| {
        let mut calldata = vec![0x38u8, 0xed, 0x17, 0x39]; // swapExactTokensForTokens
        calldata.extend_from_slice(&[0u8; 128]);
        let pool_r0: u128 = 5_000_000_000_000_000_000_000;
        let pool_r1: u128 = 10_000_000_000_000;
        let pool_b_r0: u128 = 10_200_000_000_000;
        let pool_b_r1: u128 = 5_000_000_000_000_000_000_000;

        b.iter(|| {
            // Step 1: Classify (check function selector — 4 bytes)
            let selector = &calldata[0..4];
            let is_swap = selector == [0x38, 0xed, 0x17, 0x39];
            if !is_swap { return black_box(0u128); }

            // Step 2: Hash tx for dedup
            let mut hasher = Keccak::v256();
            let mut tx_hash = [0u8; 32];
            hasher.update(&calldata);
            hasher.finalize(&mut tx_hash);

            // Step 3: Detect arbitrage (two-hop simulation)
            let amount_in: u128 = 1_000_000_000_000_000_000;
            let mid = constant_product_swap(amount_in, pool_r0, pool_r1, 30);
            let out = constant_product_swap(mid, pool_b_r0, pool_b_r1, 30);
            let profit = if out > amount_in { out - amount_in } else { 0 };

            // Step 4: Gas check
            let gas_cost: u128 = 200_000 * 1_000_000_000; // 200k gas × 1 gwei
            let net_profit = if profit > gas_cost { profit - gas_cost } else { 0 };

            // Step 5: Build calldata (ABI encode)
            let mut bundle_data = Vec::with_capacity(100);
            bundle_data.extend_from_slice(&[0xa0, 0x71, 0x2d, 0x68]); // selector
            let mut buf = [0u8; 32];
            buf[16..].copy_from_slice(&amount_in.to_be_bytes());
            bundle_data.extend_from_slice(&buf);

            black_box(net_profit)
        });
    });
}

// ─── U256 Arithmetic ────────────────────────────────────────────────────────

fn bench_u256_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("u256");

    let a = U256::from(1_000_000_000_000_000_000u64);
    let b = U256::from(2_000_000_000_000_000_000u64);
    let d = U256::from(10_000u64);

    group.bench_function("mul_div", |b_iter| {
        b_iter.iter(|| black_box((a * b) / d))
    });

    group.bench_function("add_sub", |b_iter| {
        b_iter.iter(|| black_box(a + b - d))
    });

    group.bench_function("compare", |b_iter| {
        b_iter.iter(|| black_box(a > b))
    });

    group.finish();
}

// ─── Crossbeam Channel ──────────────────────────────────────────────────────

fn bench_channel(c: &mut Criterion) {
    use crossbeam_channel::bounded;

    let (tx, rx) = bounded::<u64>(4096);

    c.bench_function("crossbeam_send_recv", |b| {
        b.iter(|| {
            tx.send(42).ok();
            black_box(rx.recv().ok())
        })
    });
}

// ─── Register ───────────────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_keccak256,
    bench_swap_simulation,
    bench_pool_lookup,
    bench_abi_encode,
    bench_detection_pipeline,
    bench_u256_ops,
    bench_channel,
);
criterion_main!(benches);
