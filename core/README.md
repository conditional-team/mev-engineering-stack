# mev-core

**Rust MEV Detection Engine** — sub-microsecond opportunity detection, EVM simulation, and bundle construction.

## Build

```bash
cargo build --release     # opt-level=3, lto=fat, codegen-units=1
cargo test                # unit + property-based tests
cargo bench               # Criterion benchmarks (7 groups)
```

Release profile: `opt-level=3`, `lto="fat"`, `codegen-units=1`, `panic="abort"`, `strip=true`.

## Architecture

```
                          ┌──────────────────────────────┐
                          │         MevEngine            │
                          │         (lib.rs)             │
                          └──────┬─────────┬─────────┬───┘
                                 │         │         │
                    ┌────────────┘         │         └────────────┐
                    ▼                      ▼                      ▼
           ┌────────────────┐    ┌──────────────────┐   ┌─────────────────┐
           │ Detector       │    │ EvmSimulator     │   │ BundleBuilder   │
           │                │    │                  │   │                 │
           │ Arbitrage      │───▶│ constant_product │──▶│ encode_arb_call │
           │ Backrun        │    │ revm fork sim    │   │ encode_liq_call │
           │ Liquidation    │    │ gas estimation   │   │ swap_path encode│
           │ MultiThreaded  │    │                  │   │ Flashbots format│
           └────────┬───────┘    └──────────────────┘   └─────────────────┘
                    │
                    │ FFI (optional)
                    ▼
           ┌────────────────┐
           │ fast/ (C)      │
           │ keccak, RLP    │
           │ SIMD, queue    │
           └────────────────┘
```

## Modules

| Module | Path | Description |
|--------|------|-------------|
| **detector** | `src/detector/` | Opportunity detection pipeline |
| ├ arbitrage | `detector/arbitrage.rs` | Cross-DEX price discrepancy — 8 V2/V3 selectors, cached pool state |
| ├ backrun | `detector/backrun.rs` | Price recovery capture after large swaps |
| ├ liquidation | `detector/liquidation.rs` | Under-collateralized position liquidation (Aave V3, Compound V3, Morpho) |
| └ multi_threaded | `detector/multi_threaded.rs` | Parallel worker pool with crossbeam channels |
| **simulator** | `src/simulator/` | EVM simulation |
| | | Constant-product x·y=k fast filter (35 ns) + revm 8.0 fork validation |
| **builder** | `src/builder/` | Bundle construction |
| | | ABI encoding, swap path packing, gas pricing, Flashbots-compatible format |
| **grpc** | `src/grpc/` | gRPC server (tonic 0.11) |
| | | `DetectOpportunity`, `StreamOpportunities`, `GetStatus` RPCs |
| **ffi** | `src/ffi/` | C FFI bindings |
| | | Keccak-256, RLP, SIMD utilities, lock-free queue (graceful Rust fallback) |
| **arbitrum** | `src/arbitrum/` | Arbitrum-specific detection + pool management |
| **mempool** | `src/mempool/` | WebSocket ultra-latency transaction polling |
| **config** | `src/config.rs` | Typed configuration with chain/strategy/performance sections |
| **types** | `src/types.rs` | Shared pipeline types: `PendingTx → SwapInfo → Opportunity → SimulationResult → Bundle` |

## Binaries

| Binary | Source | Purpose |
|--------|--------|---------|
| `mev-engine` | `src/main.rs` | Main MEV extraction engine |
| `scanner` | `src/bin/scanner.rs` | Standalone transaction scanner |
| `benchmark` | `src/bin/benchmark.rs` | Custom latency benchmarks |

## Benchmark Results

Criterion 0.5 on Intel i5-8250U @ 1.60GHz. Run `cargo bench` to reproduce.

| Group | Benchmark | Latency |
|-------|-----------|---------|
| crypto | keccak256/20 | 515 ns |
| crypto | keccak256/32 | 552 ns |
| crypto | keccak256/64 | 554 ns |
| crypto | keccak256/256 | 1.09 µs |
| amm | constant_product/0.1 ETH | 35 ns |
| amm | constant_product/1 ETH | 35 ns |
| amm | constant_product/10 ETH | 35 ns |
| amm | two_hop_arbitrage | 69 ns |
| lookup | pool_lookup_10k | 55 ns |
| abi | encode_address | 17 ns |
| abi | encode_u256 | 17 ns |
| abi | encode_swap_path_3hop | 53 ns |
| **pipeline** | **detect → simulate → build** | **608 ns** |
| u256 | mul_div | 83 ns |
| u256 | add_sub | 0.6 ns |
| u256 | compare | 0.6 ns |
| channel | crossbeam_send_recv | 23 ns |

HTML reports: `target/criterion/report/index.html`

## Tests

33+ tests across all modules. Run with `cargo test`.

| Module | Tests | What's Verified |
|--------|-------|-----------------|
| `detector/arbitrage` | 5 | V2/V3 calldata parsing, profit calculation, ABI word decoding |
| `simulator` | 3 | Constant-product math, zero-reserve edge case, simulation count |
| `builder` | 5 | ABI encoding (address, u256), arbitrage/liquidation calldata, swap path |
| `grpc/server` | 3 | u128 byte conversion (empty, 1 ETH, 32-byte overflow) |
| `ffi` | 3 | FFI binding correctness, Rust fallback paths |
| `lib` | 1 | Engine creation and state |
| **proptest** | ∞ | Constant-product invariants, ABI encode/decode roundtrip, swap parsing boundaries |

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1.35 | Async runtime (full features) |
| revm | 8.0 | EVM simulation (std, serde) |
| alloy | 0.1 | Ethereum type primitives |
| ethers | 2.0 | ABI generation, WebSocket |
| tonic | 0.11 | gRPC server |
| prost | 0.12 | Protocol buffer codegen |
| tiny-keccak | 2.0 | Keccak-256 hashing |
| dashmap | 5.5 | Concurrent hash map |
| crossbeam-channel | 0.5 | Lock-free MPSC channels |
| metrics | 0.22 | Prometheus metrics |
| criterion | 0.5 | Benchmarking (dev) |
| proptest | 1.4 | Property-based testing (dev) |

## Type Flow

```
PendingTx              Raw mempool transaction (hash, from, to, calldata, gas)
    │
    ▼ parse_swap()
SwapInfo               Decoded swap (dex, token_in, token_out, amount, fee)
    │
    ▼ detect()
Opportunity            MEV opportunity (type, path, expected_profit, gas_estimate)
    │
    ▼ simulate()
SimulationResult       EVM result (success, actual_profit, gas_used, state_changes)
    │
    ▼ build()
Bundle                 Flashbots bundle (txs, target_block, timing constraints)
```
