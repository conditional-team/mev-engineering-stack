# network

**Go Network Layer** — mempool monitoring, transaction classification, Flashbots relay, and Prometheus metrics for the MEV pipeline.

## Build

```bash
go build ./...            # all binaries
go test ./... -v          # 23 tests across 4 packages
go test -bench . ./...    # selector dispatch + gas oracle benchmarks
```

## Architecture

```
  Arbitrum Node (WSS)
        │
        ▼
  ┌─────────────┐     ┌──────────────┐     ┌──────────────┐
  │  mempool/   │────▶│  pipeline/   │────▶│  strategy/   │
  │  subscribe  │     │  classify    │     │  gRPC client │
  │  (gethclient)     │  (4 workers) │     │  (tonic)     │
  └─────────────┘     └──────────────┘     └──────┬───────┘
        │                    │                     │
        ▼                    ▼                     ▼
  ┌─────────────┐     ┌──────────────┐     ┌──────────────┐
  │  block/     │     │  gas/        │     │  relay/      │
  │  new heads  │     │  EIP-1559    │     │  Flashbots   │
  │  reorg det. │     │  base fee    │     │  Multi-relay │
  └─────────────┘     └──────────────┘     └──────────────┘
        │                    │                     │
        └────────────────────┼─────────────────────┘
                             ▼
                      ┌──────────────┐     ┌──────────────┐
                      │  metrics/    │     │  rpc/        │
                      │  Prometheus  │     │  conn pool   │
                      │  :9090       │     │  health + LB │
                      └──────────────┘     └──────────────┘
```

## Packages

### `internal/`

| Package | Description |
|---------|-------------|
| **mempool** | WebSocket pending-tx subscription via `gethclient`. Configurable selector filtering, 10k tx buffer, backpressure handling. |
| **pipeline** | Multi-worker transaction classifier. Dispatches on function selector: UniswapV2 (6 selectors), V3 (4), ERC20 transfers, Aave V2/V3 liquidations, flash loans. Zero-allocation hot path at **40.7 ns/op**. |
| **block** | New-head subscription with configurable reorg detection depth. Automatic polling fallback if WebSocket drops. |
| **gas** | EIP-1559 base fee oracle. Real formula: `baseFee * (1 + elasticity * gasUsedDelta / gasTarget)`. Multi-block prediction for bundle gas pricing at **425 ns/op**. |
| **relay** | Flashbots relay client with EIP-191 bundle signing (`eth_sendBundle`, `eth_callBundle`, `flashbots_getBundleStats`). Automatic retry with exponential backoff. |
| **relay (multi)** | Multi-relay manager — 3 strategies: `Race` (first response wins), `Primary` (fallback chain), `All` (broadcast to all relays). Concurrent submission with context cancellation. |
| **rpc** | Connection pool with health checking, latency-based routing, automatic reconnection. Supports multiple RPC endpoints with weighted selection. |
| **metrics** | Prometheus instrumentation for RPC latency, mempool throughput, pipeline classification breakdown, relay submission stats. Bind address configurable. |
| **strategy** | gRPC client to Rust core. 100ms timeout, keepalive, graceful fallback to monitor-only mode when core is offline. |

### `pkg/`

| Package | Description |
|---------|-------------|
| **config** | Environment-based configuration with typed parsing. Reads from `.env` with sensible defaults. |
| **types** | Shared types: `OpportunityType`, `TxClass`, cross-package data structures. |

### `cmd/`

| Command | Description |
|---------|-------------|
| **mev-node** | Main binary. Orchestrates mempool → pipeline → strategy → relay loop. Prometheus server on `:9090`. |
| **testnet-verify** | Testnet signing verification. Generates ECDSA key, signs EIP-1559 tx on Arbitrum Sepolia (421614), constructs Flashbots bundle with EIP-191, verifies via ecrecover. `--submit` flag for live submission. |

## Benchmarks

Intel i5-8250U @ 1.60GHz. Run `go test -bench . ./...`

| Operation | Latency | Allocs | Throughput |
|-----------|---------|--------|------------|
| Tx classification (selector dispatch) | **40.7 ns/op** | 0 B / 0 alloc | ~24.5M tx/sec |
| EIP-1559 base fee calculation | **425 ns/op** | 152 B / 6 alloc | ~2.3M/sec |

Classification throughput is **1500×** Arbitrum's 250ms block production.

## Tests

23 tests across 4 packages:

| Package | Tests | Coverage |
|---------|-------|----------|
| `pkg/config` | Config parsing, env vars, defaults | Typed field validation |
| `internal/gas` | EIP-1559 base fee, multi-block prediction | Real formula correctness |
| `internal/pipeline` | V2/V3 selector matching, ERC20 parsing, decoder | All 10 selectors tested |
| `internal/relay` | Race/Primary/All strategies, timeout, fallback | Mock relay with delays |

## Pipeline Classification

```
Input: raw calldata (4-byte selector)
    │
    ├── 0x38ed1739  swapExactTokensForTokens     ──▶ SwapV2
    ├── 0x8803dbee  swapTokensForExactTokens     ──▶ SwapV2
    ├── 0x7ff36ab5  swapExactETHForTokens         ──▶ SwapV2
    ├── 0x18cbafe5  swapExactTokensForETH         ──▶ SwapV2
    ├── 0x5c11d795  swapExactTokensForTokensSFOT  ──▶ SwapV2
    ├── 0xfb3bdb41  swapETHForExactTokens         ──▶ SwapV2
    ├── 0x414bf389  exactInputSingle              ──▶ SwapV3
    ├── 0xc04b8d59  exactInput                    ──▶ SwapV3
    ├── 0xdb3e2198  exactOutputSingle             ──▶ SwapV3
    ├── 0xf28c0498  exactOutput                   ──▶ SwapV3
    └── *           unknown                       ──▶ Other
```

## Dependencies

| Module | Version | Purpose |
|--------|---------|---------|
| go-ethereum | v1.13.8 | Ethereum types, ABI, crypto, RLP |
| prometheus/client_golang | v1.18.0 | Metrics instrumentation |
| rs/zerolog | v1.31.0 | Structured JSON logging |
| google.golang.org/grpc | v1.60.1 | gRPC client to Rust core |
| gorilla/websocket | v1.5.1 | WebSocket transport (indirect) |
