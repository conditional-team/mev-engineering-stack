# MEV Protocol

**High-Performance Multi-Language MEV Engineering Stack**

Sub-microsecond mempool monitoring, transaction classification, bundle construction, and relay submission targeting **Arbitrum**. Four-language architecture where each layer uses the optimal tool for its domain.

![Rust](https://img.shields.io/badge/Rust-Core_Engine-orange?style=flat-square&logo=rust)
![Go](https://img.shields.io/badge/Go-Network_Layer-00ADD8?style=flat-square&logo=go)
![C](https://img.shields.io/badge/C-Hot_Path-A8B9CC?style=flat-square&logo=c)
![Solidity](https://img.shields.io/badge/Solidity-Contracts-363636?style=flat-square&logo=solidity)

---

## Architecture

```
 ┌────────────────────────────────────────────────────────────────────────────┐
 │                          MEV Protocol Pipeline                             │
 │                                                                            │
 │   Mempool ──▶ Classify ──▶ Detect ──▶ Simulate ──▶ Build ──▶ Submit      │
 │   (Go)        (Go)         (Rust)     (Rust/revm)   (Rust)    (Go relay)  │
 └────────────────────────────────────────────────────────────────────────────┘

          ┌─────────────┐        gRPC         ┌──────────────┐
          │  network/   │◄═══════════════════▶│    core/      │
          │  Go 1.21    │   proto/mev.proto   │  Rust 2021   │
          │             │                      │              │
          │ • mempool   │                      │ • detector   │
          │ • pipeline  │                      │ • simulator  │
          │ • relay     │                      │ • builder    │
          │ • metrics   │                      │ • grpc srv   │
          │ • gas oracle│                      │ • ffi bridge │
          └──────┬──────┘                      └──────┬───────┘
                 │                                     │ FFI
                 │ eth_sendBundle                       │
          ┌──────┴──────┐                      ┌──────┴───────┐
          │ Flashbots   │                      │    fast/      │
          │ Relay       │                      │     C         │
          │             │                      │              │
          │ EIP-191     │                      │ • keccak256  │
          │ Multi-Relay │                      │ • RLP encode │
          │ Race/All    │                      │ • SIMD AVX2  │
          └─────────────┘                      │ • lock-free Q│
                                               │ • mem pool   │
          ┌─────────────┐                      └──────────────┘
          │ contracts/  │
          │ Solidity    │
          │             │
          │ FlashArb    │ ◄── Balancer V2 flash loans (0% fee)
          │ MultiDexRtr │ ◄── V2/V3/Sushi/Curve routing
          └─────────────┘
```

### Layer Breakdown

| Layer | Language | Purpose | Entry Point |
|-------|----------|---------|-------------|
| **network/** | Go 1.21 | Mempool monitoring, tx classification, Flashbots relay, Prometheus metrics | `cmd/mev-node/main.go` |
| **core/** | Rust 2021 | MEV detection, revm simulation, bundle construction, gRPC server | `src/main.rs` |
| **fast/** | C (GCC/Clang) | SIMD keccak, RLP encoding, lock-free MPSC queue, arena allocator | `src/keccak.c` |
| **contracts/** | Solidity | Flash loan arbitrage (Balancer V2), multi-DEX routing | `src/FlashArbitrage.sol` |
| **proto/** | Protocol Buffers | Cross-language service contract (Go ↔ Rust) | `mev.proto` |

---

## Benchmark Results

Measured with [Criterion 0.5](https://bheisler.github.io/criterion.rs/book/) on Intel i5-8250U @ 1.60GHz. Production targets co-located bare-metal.

### Rust Core — `cargo bench`

| Operation | Latency | Notes |
|-----------|---------|-------|
| Keccak-256 (32 bytes) | **552 ns** | `tiny-keccak` — address hashing |
| Constant-product swap (1 ETH) | **35 ns** | x·y = k with 30 bps fee |
| Two-hop arbitrage (buy DEX A → sell DEX B) | **69 ns** | Cross-DEX price discrepancy |
| Pool lookup (10k DashMap) | **55 ns** | Concurrent read, pre-populated |
| ABI encode swap path (3-hop) | **53 ns** | 72-byte packed path |
| **Full pipeline (detect → simulate → build)** | **608 ns** | End-to-end per opportunity |
| U256 mul + div | **83 ns** | `alloy` 256-bit arithmetic |
| Crossbeam channel send+recv | **23 ns** | Bounded 4096, single item |

### Go Network — `go test -bench .`

| Operation | Latency | Allocs | Throughput |
|-----------|---------|--------|------------|
| Tx classification (selector dispatch) | **40.7 ns/op** | 0 B / 0 alloc | ~24.5M tx/sec |
| EIP-1559 base fee calculation | **425 ns/op** | 152 B / 6 alloc | ~2.3M/sec |

> Full pipeline processes a transaction **1500× faster** than Arbitrum's 250ms block time.

---

## Go Network Layer — `network/`

Production-grade mempool monitoring used as the entry point for the MEV pipeline. See [network/README.md](network/README.md) for full documentation.

| Package | Role |
|---------|------|
| `internal/mempool` | WebSocket pending-tx subscription (`gethclient`), selector filtering |
| `internal/pipeline` | Multi-worker classifier — UniswapV2 (6 selectors), V3 (4), ERC20, Aave, flash loans |
| `internal/block` | New-head subscription with reorg detection, polling fallback |
| `internal/gas` | EIP-1559 base fee oracle with multi-block prediction |
| `internal/relay` | Flashbots `eth_sendBundle` + multi-relay manager (Race / Primary / All) |
| `internal/rpc` | Connection pool, health checks, latency-based routing |
| `internal/metrics` | Prometheus instrumentation (RPC latency, mempool throughput, relay stats) |
| `cmd/mev-node` | Main binary — pipeline orchestration |
| `cmd/testnet-verify` | Testnet signing verification (EIP-1559 tx + EIP-191 bundle proof) |

```bash
cd network
go build ./...            # compile all binaries
go test ./... -v          # 23 tests across 4 packages
go test -bench . ./...    # selector + gas oracle benchmarks
```

---

## Rust Core Engine — `core/`

High-performance detection, simulation, and bundle construction. See [core/README.md](core/README.md) for full documentation.

- **revm 8.0** — local EVM simulation without forked geth
- **Tokio 1.35** — async multi-threaded runtime
- **crossbeam** — lock-free channels for detector→simulator pipeline
- **alloy + ethers** — type-safe Ethereum primitives and ABI encoding
- **Prometheus** — `metrics-exporter-prometheus` for hot-path instrumentation
- **tonic + prost** — gRPC server exposing detection pipeline to Go

```bash
cd core
cargo build --release     # opt-level=3, lto=fat, codegen-units=1
cargo test                # 33 unit tests + property-based tests
cargo bench               # 7 Criterion benchmark groups
```

### Detection Pipeline

```
PendingTx → parse_swap() → ArbitrageDetector ──┐
            (8 selectors)   BackrunDetector  ────┼─▶ EvmSimulator ──▶ BundleBuilder ──▶ Bundle
                            LiquidationDetector─┘    (revm 8.0)       (ABI encode)
```

### gRPC Bridge (Go ↔ Rust)

| Component | Location | Protocol |
|-----------|----------|----------|
| Service definition | `proto/mev.proto` | `MevEngine` — 3 RPCs |
| Rust server | `core/src/grpc/server.rs` | tonic 0.11 |
| Go client | `network/internal/strategy/client.go` | google.golang.org/grpc 1.60 |

RPCs: `DetectOpportunity`, `StreamOpportunities`, `GetStatus`

Target: **< 10ms** round-trip for detect + simulate + bundle on co-located infra.

---

## Smart Contracts — `contracts/`

Foundry-based Solidity with hardened callback validation.

| Contract | Purpose | Security |
|----------|---------|----------|
| **FlashArbitrage.sol** | Balancer V2 flash loan → multi-DEX execution | Callback bound to (executor, token, amount, swap hash) |
| **MultiDexRouter.sol** | Aggregated routing across V2/V3/Sushi/Curve | Trusted factory/router whitelist, strict ERC20 checks |

```bash
cd contracts
forge build
forge test -vvv
```

---

## C Hot Path — `fast/`

SIMD-accelerated primitives linked into Rust via FFI (`core/src/ffi/`).

| File | Function | Optimization |
|------|----------|-------------|
| `keccak.c` | Keccak-256 (24-round permutation) | Batch hashing |
| `rlp.c` | RLP encoding (tx serialization) | Zero-copy output |
| `simd_utils.c` | Byte comparison, address matching | AVX2 + SSE4.2 |
| `lockfree_queue.c` | MPSC queue (detector→simulator) | CAS-only, no mutex |
| `memory_pool.c` | Arena allocator | Zero-alloc per tx |
| `parser.c` | Binary calldata parsing | Unrolled loops |

Compile flags: `-O3 -march=native -mavx2 -msse4.2 -flto -falign-functions=64`

```bash
cd fast
make            # → lib/libmev_fast.a + lib/libmev_fast.so
make test       # correctness + SIMD validation
make bench      # hot-path benchmarks
```

---

## Quick Start

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.75+ | Core engine |
| Go | 1.21+ | Network layer |
| Foundry | Latest | Contract compilation + testing |
| GCC / Clang | 11+ | C hot path (AVX2 support) |

### Build & Run

```bash
# 1. Clone
git clone https://github.com/ivanpiardi/mev-engineering-stack.git
cd mev-engineering-stack

# 2. Configure
cp .env.example .env
# Edit .env with your RPC endpoints and signing key

# 3. Build
make build        # all layers (or build individually below)

# 4. Test
make test         # all layers

# 5. Run
cd network && go run ./cmd/mev-node/

# 6. Benchmark
cd core && cargo bench
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `MEV_RPC_ENDPOINTS` | Comma-separated WebSocket RPC URLs | — |
| `ARBITRUM_RPC_URL` | Arbitrum HTTP endpoint | — |
| `ARBITRUM_WS_URL` | Arbitrum WebSocket endpoint | — |
| `FLASHBOTS_SIGNING_KEY` | ECDSA key for bundle signing (EIP-191) | — |
| `MEV_PIPELINE_WORKERS` | Parallel classification workers | `4` |
| `MEV_METRICS_ADDR` | Prometheus metrics bind address | `:9090` |

See [.env.example](.env.example) for the full list.

---

## Testnet Verification

The `testnet-verify` tool proves the entire signing + bundle pipeline end-to-end against Arbitrum Sepolia without spending gas:

```bash
cd network
go run ./cmd/testnet-verify/

# Output:
# ✓ Signing Key    : 0xa2F3...
# ✓ Chain          : Arbitrum Sepolia (421614)
# ✓ EIP-1559 Tx    : Signed (111 bytes)
# ✓ EIP-191 Sign   : Verified (Flashbots format)
# ✓ Bundle         : Target block N+1
# ○ Submission     : Dry-run (use --submit)
```

Flags: `--key` (reuse signing key), `--rpc` (custom RPC), `--submit` (live submission).

---

## Test Coverage

| Layer | Tests | Framework | What's Tested |
|-------|-------|-----------|---------------|
| **Go network** | 23 tests, 2 benchmarks | `go test` | Config parsing, EIP-1559 oracle, tx classification (V2/V3 selectors), multi-relay strategies |
| **Rust core** | 33+ tests | `cargo test` + proptest | ABI encoding, constant-product math, V2/V3 calldata parsing, gRPC serialization, bundle construction, property-based invariants |
| **Rust bench** | 7 groups | Criterion 0.5 | Full pipeline, keccak, AMM, pool lookup, ABI, U256, crossbeam |
| **Solidity** | Foundry suite | `forge test` | Flash arbitrage execution, multi-DEX routing, callback validation |
| **C hot path** | `make test` | Custom runner | Keccak correctness, RLP encoding, SIMD validation |

---

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| **4 languages** | Go for concurrent network I/O, Rust for safe high-perf compute, C for SIMD hot paths, Solidity for on-chain. Mirrors production MEV infra. |
| **gRPC over FFI for Go↔Rust** | cgo pins goroutines to OS threads, defeating Go's scheduler. gRPC gives process separation, independent scaling, and proto-defined contracts. |
| **revm over forked geth** | Pure Rust, no cgo dependency, fork-mode simulation, deterministic gas. 10-50× faster for single-tx sim. |
| **Balancer flash loans** | 0% fee vs Aave's 0.09%. When margins are basis points, eliminating the fee is critical. |
| **Constant-product fast filter** | x·y=k math at 35 ns screens candidates before expensive revm simulation. Only analytical survivors hit EVM. |
| **Arbitrum-first** | 10-100× cheaper gas, 250ms blocks, less MEV competition. Proof-of-concept before L1. |
| **Monitor-only fallback** | Go node degrades gracefully when Rust core is offline — keeps logging rather than crashing. |

---

## Project Structure

```
mev-engineering-stack/
├── core/                   # Rust — detection, simulation, bundle construction
│   ├── src/
│   │   ├── detector/       # ArbitrageDetector, BackrunDetector, LiquidationDetector
│   │   ├── simulator/      # EvmSimulator (revm 8.0, constant-product)
│   │   ├── builder/        # BundleBuilder (ABI encoding, gas pricing)
│   │   ├── grpc/           # tonic server (MevEngine service)
│   │   ├── arbitrum/       # Arbitrum-specific detection + execution
│   │   ├── ffi/            # C FFI bindings (keccak, RLP, SIMD, queue)
│   │   └── mempool/        # WebSocket data handling
│   └── benches/            # Criterion benchmarks (7 groups)
├── network/                # Go — mempool monitor, pipeline, relay
│   ├── cmd/mev-node/       # Main binary
│   ├── cmd/testnet-verify/ # Testnet signing verification tool
│   ├── internal/           # block, gas, mempool, pipeline, relay, rpc, metrics
│   └── pkg/                # config, types (public packages)
├── contracts/              # Solidity — FlashArbitrage, MultiDexRouter
│   ├── src/                # Contract source + interfaces
│   └── test/               # Foundry test suite
├── fast/                   # C — SIMD keccak, RLP, lock-free queue, memory pool
│   ├── src/                # Implementation (6 files)
│   ├── include/            # Headers
│   └── test/               # Test suite
├── proto/                  # gRPC service definition
├── dashboard/              # Real-time monitoring (HTML/JS)
├── config/                 # Chain + DEX + token configs (JSON)
├── docker/                 # Dockerfile, docker-compose, prometheus.yml
├── scripts/                # Build and deploy automation
├── Makefile                # Top-level build orchestration
└── .env.example            # Environment template (no secrets)
```

---

## License

Proprietary. See [LICENSE](LICENSE) for details.
