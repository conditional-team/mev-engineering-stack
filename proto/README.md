# proto

**gRPC Service Definition** — cross-language contract between the Go network layer (client) and the Rust core engine (server).

## File

- [`mev.proto`](mev.proto) — Protocol Buffers 3 definition

## Service

```protobuf
service MevEngine {
  rpc DetectOpportunity(ClassifiedTransaction) returns (DetectionResult);
  rpc StreamOpportunities(StreamRequest) returns (stream DetectionResult);
  rpc GetStatus(StatusRequest) returns (StatusResponse);
}
```

| RPC | Direction | Description |
|-----|-----------|-------------|
| `DetectOpportunity` | Unary | Single classified tx → detection result |
| `StreamOpportunities` | Server-streaming | Subscribe to all opportunities in real time |
| `GetStatus` | Unary | Engine health, uptime, detection count |

## Message Flow

```
Go pipeline             proto/mev.proto              Rust core
──────────               ──────────                 ──────────
ClassifiedTransaction ──▶ gRPC (tonic) ──────────▶ OpportunityDetector
                                                        │
                                                   EvmSimulator
                                                        │
                                                   BundleBuilder
                                                        │
DetectionResult ◀─────── gRPC response ◀──────────── Bundle
```

## Types

| Message | Fields | Purpose |
|---------|--------|---------|
| `ClassifiedTransaction` | tx_hash, from, to, value, gas, calldata, tx_class, swap_info, target_block, base_fee | Pending mempool tx with Go classification |
| `DetectionResult` | found, opportunities[], detection_latency_ns | Pipeline response |
| `Opportunity` | type, tokens, amounts, gas_estimate, bundle | Detected MEV opportunity |
| `Bundle` | transactions[], target_block | Flashbots-format bundle |
| `BundleTx` | to, value, gas_limit, max_fee, priority_fee, data | Single tx in bundle |

## Enums

| Enum | Values | Source |
|------|--------|--------|
| `TxClass` | UNKNOWN, SWAP_V2, SWAP_V3, TRANSFER, APPROVAL, LIQUIDATION, FLASH_LOAN | Go pipeline classifier |
| `OpportunityType` | ARBITRAGE, BACKRUN, LIQUIDATION_OPP | Rust detector output |

## Code Generation

| Language | Tool | Output |
|----------|------|--------|
| Rust | `tonic-build` (build.rs) | `core/src/grpc/mev.rs` |
| Go | `protoc-gen-go-grpc` | `network/internal/strategy/proto/mev.pb.go` |

## Design Notes

- All `uint256` values encoded as big-endian byte arrays (preserves full 256-bit precision)
- `go_package` option: `github.com/mev-protocol/network/internal/strategy/proto`
- Latency target: < 10ms round-trip on co-located infrastructure
