# Configuration Reference

This document describes all configuration files in `config/` and the environment variables consumed by the pipeline.

---

## Environment Variables (`.env`)

Copy `.env.example` to `.env` and populate with your own credentials. **Never commit populated `.env` files.**

| Variable | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `MEV_RPC_ENDPOINTS` | Comma-separated URLs | Yes | — | WebSocket RPC URLs for mempool subscription |
| `ARBITRUM_RPC_URL` | URL | Yes | — | Arbitrum HTTP endpoint (Alchemy, Infura, or self-hosted) |
| `ARBITRUM_WS_URL` | URL | Yes | — | Arbitrum WebSocket endpoint for pending tx stream |
| `FLASHBOTS_SIGNING_KEY` | Hex string (64 chars) | Yes | — | ECDSA private key for EIP-191 bundle signing |
| `MEV_PIPELINE_WORKERS` | Integer | No | `4` | Number of parallel classification workers in Go pipeline |
| `MEV_METRICS_ADDR` | `host:port` | No | `:9090` | Prometheus metrics bind address for Rust core |
| `MEV_NODE_METRICS_ADDR` | `host:port` | No | `:9091` | Prometheus metrics bind address for Go node |
| `LOG_LEVEL` | `debug` / `info` / `warn` / `error` | No | `info` | Log verbosity for both Rust and Go |
| `MEV_CONFIG` | File path | No | `config/config.json` | Override path for main config file |

---

## `config/chains.json`

Multi-chain configuration. Keyed by chain ID (string).

```jsonc
{
  "chains": {
    "<chain_id>": {
      "name":             "string  — Human-readable chain name",
      "chain_id":         "uint    — EIP-155 chain ID (must match key)",
      "rpc_url":          "URL     — HTTP JSON-RPC endpoint (required)",
      "ws_url":           "URL     — WebSocket endpoint for subscriptions (required)",
      "flashbots_relay":  "URL|null — Flashbots relay endpoint. null disables Flashbots on this chain",
      "bloxroute_relay":  "URL|null — bloXroute relay endpoint. null disables bloXroute",
      "balancer_vault":   "address|null — Balancer V2 Vault for flash loans. null if Balancer unavailable",
      "contract_address": "address|null — Deployed FlashArbitrage contract. null if not yet deployed"
    }
  }
}
```

### Field Details

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `name` | string | Yes | Display name only; not used in logic |
| `chain_id` | uint | Yes | Must match the object key. Used for EIP-155 signing |
| `rpc_url` | URL | Yes | Replace `YOUR_KEY` with your Alchemy/Infura API key |
| `ws_url` | URL | Yes | Required for `newPendingTransactions` subscription |
| `flashbots_relay` | URL or `null` | No | Set to `null` on chains without Flashbots (Arbitrum, Optimism, Polygon) |
| `bloxroute_relay` | URL or `null` | No | Alternative relay. Used when Flashbots is unavailable |
| `balancer_vault` | address or `null` | No | `0xBA1222...` on most chains. `null` on BSC |
| `contract_address` | address or `null` | No | Set after deploying `FlashArbitrage.sol` via `scripts/deploy.sh` |

### Supported Chains

| Chain ID | Name | Flashbots | Balancer | Status |
|----------|------|-----------|----------|--------|
| `1` | Ethereum | Yes | Yes | Supported |
| `42161` | Arbitrum | No | Yes | **Primary target** |
| `8453` | Base | Yes | Yes | Supported |
| `10` | Optimism | No | Yes | Supported |
| `137` | Polygon | No | Yes | Supported |
| `56` | BSC | No | No | Experimental |

---

## `config/arbitrum.json`

Arbitrum-specific configuration used by the Rust core engine.

### `chain` — Network metadata

| Field | Type | Description |
|-------|------|-------------|
| `id` | uint | Chain ID (`42161` for mainnet, `421614` for Sepolia) |
| `name` | string | Human-readable name |
| `rpc_url` | URL | HTTP RPC endpoint |
| `ws_url` | URL | WebSocket endpoint |
| `explorer` | URL | Block explorer base URL |
| `block_time_ms` | uint | Target block time in milliseconds (250 for Arbitrum) |

### `testnet` — Arbitrum Sepolia testnet

Same fields as `chain`. Used by `testnet-verify` tool.

### `contracts` — Deployed contract addresses

| Field | Type | Description |
|-------|------|-------------|
| `balancer_vault` | address | Balancer V2 Vault on Arbitrum |
| `flash_arbitrage` | address or `null` | Your deployed FlashArbitrage contract. `null` until deployed |

### `dexes` — DEX router/factory addresses

Each DEX entry contains:

| Field | Type | Description |
|-------|------|-------------|
| `factory` | address | Pool factory contract (used to validate pool existence) |
| `router` | address | Swap router contract |
| `quoter` | address | Price quoter (V3 only) |
| `fees` | array of uint | Supported fee tiers in basis points (V3 only, e.g. `[100, 500, 3000, 10000]`) |
| `fee_bps` | uint | Fixed fee in basis points (V2 only, typically `30` = 0.3%) |

### `tokens` — Monitored token list

| Field | Type | Description |
|-------|------|-------------|
| `address` | address | ERC-20 contract address (checksummed) |
| `decimals` | uint | Token decimals (6 for USDC/USDT, 18 for ETH/ARB, 8 for WBTC) |
| `is_base` | bool | If `true`, used as quote asset for arbitrage scanning (typically WETH) |

### `strategy` — Detection parameters

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `min_profit_bps` | uint | `10` | Minimum profit threshold in basis points (10 = 0.1%) |
| `max_gas_price_gwei` | float | `1` | Skip opportunities if gas price exceeds this (Arbitrum is cheap) |
| `slippage_bps` | uint | `50` | Maximum acceptable slippage (50 = 0.5%) |
| `scan_amounts_eth` | array of float | `[0.1, 0.5, 1, 5, 10]` | ETH amounts to scan for arbitrage opportunities |
| `max_position_eth` | float | `50` | Maximum position size per trade |

---

## `config/dex.json`

Global DEX registry across all supported chains.

### Structure

```jsonc
{
  "dexes": {
    "<dex_id>": {
      "name":    "string — Display name",
      "type":    "string — AMM type: v2 | v3 | solidly | curve | balancer",
      "fee_bps": "uint   — Fixed fee (V2/solidly only)",
      "chains": {
        "<chain_id>": {
          "router":          "address — Swap router",
          "router2":         "address — Secondary router (V3 SwapRouter02, optional)",
          "factory":         "address — Pool factory",
          "quoter":          "address — Price quoter (V3 only, optional)",
          "quoter2":         "address — QuoterV2 (V3 only, optional)",
          "init_code_hash":  "bytes32 — Pool init code hash (V2 only, for CREATE2)",
          "vault":           "address — Protocol vault (Balancer only)",
          "registry":        "address — Pool registry (Curve only)",
          "address_provider": "address — Address provider (Curve only)"
        }
      }
    }
  }
}
```

### AMM Types

| Type | Protocol Examples | Fee Model |
|------|-------------------|-----------|
| `v2` | Uniswap V2, SushiSwap, Camelot, PancakeSwap V2 | Fixed `fee_bps` (typically 30) |
| `v3` | Uniswap V3, PancakeSwap V3 | Per-pool fee tier (100, 500, 3000, 10000 bps) |
| `solidly` | Aerodrome, Velodrome | Variable stable/volatile |
| `curve` | Curve | Pool-specific |
| `balancer` | Balancer | Pool-specific |

---

## `config/tokens.json`

Token registry keyed by chain ID.

### Structure

```jsonc
{
  "tokens": {
    "<chain_id>": {
      "<SYMBOL>": {
        "address":  "address — ERC-20 contract address",
        "decimals": "uint    — Token decimals",
        "priority": "uint    — Scanning priority (1 = highest, scanned first)"
      }
    }
  }
}
```

### Priority

Tokens with lower priority numbers are scanned first for arbitrage opportunities. Typical ordering:

1. **WETH** — Most liquid base pair
2. **USDC** — Primary stablecoin
3. **USDT** — Secondary stablecoin
4. **Chain-native tokens** (ARB, OP, etc.)
5. **Others** (WBTC, DAI, GMX, etc.)

---

## Validation

Configuration is loaded at startup by `core/src/config.rs` (Rust) and `network/pkg/config/config.go` (Go). Both layers validate:

- Required fields are present and non-empty
- Chain IDs are consistent between key and value
- Addresses are valid hex (0x-prefixed, 40 hex chars)
- URLs are well-formed

If validation fails, the binary exits with a descriptive error message. Missing optional fields (`null` values) are handled gracefully with documented fallback behaviour.
