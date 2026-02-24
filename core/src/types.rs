//! Core types for MEV engine

use serde::{Deserialize, Serialize};

/// Opportunity types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpportunityType {
    Arbitrage,
    Backrun,
    Liquidation,
    Sandwich,
}

/// DEX types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DexType {
    UniswapV2,
    UniswapV3,
    SushiSwap,
    Curve,
    Balancer,
}

/// Pending transaction
#[derive(Debug, Clone)]
pub struct PendingTx {
    pub hash: [u8; 32],
    pub from: [u8; 20],
    pub to: Option<[u8; 20]>,
    pub value: u128,
    pub gas_price: u128,
    pub gas_limit: u64,
    pub input: Vec<u8>,
    pub nonce: u64,
    pub timestamp: u64,
}

/// Swap information
#[derive(Debug, Clone)]
pub struct SwapInfo {
    pub dex: DexType,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: u128,
    pub amount_out_min: u128,
    pub fee: u32, // In hundredths of a bip
}

/// MEV opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opportunity {
    pub opportunity_type: OpportunityType,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: u128,
    pub expected_profit: u128,
    pub gas_estimate: u64,
    pub deadline: u64,
    pub path: Vec<DexType>,
    pub target_tx: Option<[u8; 32]>,
}

/// Simulation result
#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub success: bool,
    pub profit: i128,
    pub gas_used: u64,
    pub error: Option<String>,
    pub state_changes: Vec<StateChange>,
}

/// State change from simulation
#[derive(Debug, Clone)]
pub struct StateChange {
    pub address: [u8; 20],
    pub slot: [u8; 32],
    pub old_value: [u8; 32],
    pub new_value: [u8; 32],
}

/// Bundle for submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub transactions: Vec<BundleTransaction>,
    pub target_block: Option<u64>,
    pub max_block_number: Option<u64>,
    pub min_timestamp: Option<u64>,
    pub max_timestamp: Option<u64>,
    pub reverting_tx_hashes: Vec<[u8; 32]>,
}

/// Transaction in a bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleTransaction {
    pub to: String,
    pub value: u128,
    pub gas_limit: u64,
    pub gas_price: Option<u128>,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
    pub data: Vec<u8>,
    pub nonce: Option<u64>,
}

/// Bundle submission result
#[derive(Debug, Clone)]
pub struct BundleResult {
    pub bundle_hash: [u8; 32],
    pub submitted: bool,
    pub included_block: Option<u64>,
    pub error: Option<String>,
}

/// Pool state for simulation
#[derive(Debug, Clone)]
pub struct PoolState {
    pub address: [u8; 20],
    pub token0: [u8; 20],
    pub token1: [u8; 20],
    pub reserve0: u128,
    pub reserve1: u128,
    pub fee: u32,
}
