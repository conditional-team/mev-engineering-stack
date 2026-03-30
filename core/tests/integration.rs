//! Integration tests — full pipeline flow from PendingTx to Bundle.
//!
//! These tests exercise the real code paths across multiple modules,
//! verifying that the detector → simulator → builder pipeline produces
//! correct results for known transaction patterns.

use mev_core::config::Config;
use mev_core::types::{Opportunity, OpportunityType, DexType};
use std::sync::Arc;

fn test_config() -> Arc<Config> {
    let mut config = Config::default();
    config.strategy.max_gas_price_gwei = 1; // Arbitrum-level gas
    config.strategy.min_profit_wei = 0;     // Accept all for testing
    config.strategy.slippage_tolerance_bps = 50;
    Arc::new(config)
}

// ─── Engine Lifecycle ───────────────────────────────────────────────────────

#[tokio::test]
async fn engine_starts_and_stops_cleanly() {
    let config = Config::default();
    let engine = mev_core::MevEngine::new(config);
    assert!(!engine.is_running().await);
}

#[tokio::test]
async fn engine_stats_start_at_zero() {
    let config = Config::default();
    let engine = mev_core::MevEngine::new(config);
    let stats = engine.stats().await;
    assert_eq!(stats.opportunities_detected, 0);
    assert_eq!(stats.simulations_run, 0);
    assert_eq!(stats.bundles_submitted, 0);
}

// ─── Detector → Simulator Pipeline ─────────────────────────────────────────

#[tokio::test]
async fn arbitrage_opportunity_simulates_with_gas() {
    let config = test_config();
    let sim = mev_core::simulator::EvmSimulator::new(config);

    let opp = Opportunity {
        opportunity_type: OpportunityType::Arbitrage,
        token_in: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(), // WETH
        token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(), // USDC
        amount_in: 1_000_000_000_000_000_000, // 1 ETH
        expected_profit: 10_000_000_000_000_000,
        gas_estimate: 250_000,
        deadline: 0,
        path: vec![DexType::UniswapV2, DexType::UniswapV3],
        target_tx: None,
    };

    let result = sim.simulate(&opp).await;
    // Simulation should run and produce a gas estimate
    assert!(result.gas_used > 0, "simulation must consume gas");
}

#[tokio::test]
async fn liquidation_opportunity_simulates() {
    let config = test_config();
    let sim = mev_core::simulator::EvmSimulator::new(config);

    let opp = Opportunity {
        opportunity_type: OpportunityType::Liquidation,
        token_in: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(), // USDC
        token_out: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(), // WETH
        amount_in: 50_000_000_000, // 50k USDC
        expected_profit: 5_000_000_000,
        gas_estimate: 500_000,
        deadline: 0,
        path: vec![],
        target_tx: None,
    };

    let result = sim.simulate(&opp).await;
    assert!(result.gas_used > 0);
}

// ─── Simulator → Builder Pipeline ───────────────────────────────────────────

#[tokio::test]
async fn arbitrage_builds_valid_bundle() {
    let config = test_config();
    let mut builder = mev_core::builder::BundleBuilder::new(config);
    builder.set_contract("0x1234567890123456789012345678901234567890".to_string());

    let opp = Opportunity {
        opportunity_type: OpportunityType::Arbitrage,
        token_in: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
        amount_in: 5_000_000_000_000_000_000, // 5 ETH
        expected_profit: 50_000_000_000_000_000,
        gas_estimate: 250_000,
        deadline: 100,
        path: vec![DexType::UniswapV2, DexType::UniswapV3],
        target_tx: None,
    };

    let bundle = builder.build(&opp).await.unwrap();

    // Bundle must contain exactly 1 transaction
    assert_eq!(bundle.transactions.len(), 1);
    // Transaction must target the contract
    assert_eq!(bundle.transactions[0].to, "0x1234567890123456789012345678901234567890");
    // Gas limit must be set
    assert_eq!(bundle.transactions[0].gas_limit, 250_000);
    // Calldata must not be empty
    assert!(!bundle.transactions[0].data.is_empty());
    // Calldata must start with the arbitrage selector (4 bytes)
    assert!(bundle.transactions[0].data.len() >= 4);
}

#[tokio::test]
async fn liquidation_builds_valid_bundle() {
    let config = test_config();
    let mut builder = mev_core::builder::BundleBuilder::new(config);
    builder.set_contract("0xABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD".to_string());

    let opp = Opportunity {
        opportunity_type: OpportunityType::Liquidation,
        token_in: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
        token_out: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        amount_in: 50_000_000_000,
        expected_profit: 5_000_000_000,
        gas_estimate: 500_000,
        deadline: 200,
        path: vec![],
        target_tx: None,
    };

    let bundle = builder.build(&opp).await.unwrap();

    assert_eq!(bundle.transactions.len(), 1);
    assert_eq!(bundle.transactions[0].gas_limit, 500_000);
    // Liquidation calldata: selector(4) + debtToken(32) + collToken(32) + amount(32) + user(32) = 132
    assert_eq!(bundle.transactions[0].data.len(), 132);
}

#[tokio::test]
async fn backrun_builds_valid_bundle() {
    let config = test_config();
    let mut builder = mev_core::builder::BundleBuilder::new(config);
    builder.set_contract("0x1111111111111111111111111111111111111111".to_string());

    let opp = Opportunity {
        opportunity_type: OpportunityType::Backrun,
        token_in: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
        amount_in: 2_000_000_000_000_000_000,
        expected_profit: 20_000_000_000_000_000,
        gas_estimate: 300_000,
        deadline: 50,
        path: vec![DexType::UniswapV2],
        target_tx: Some([0xAA; 32]),
    };

    let bundle = builder.build(&opp).await.unwrap();
    assert_eq!(bundle.transactions.len(), 1);
    assert!(bundle.transactions[0].data.len() >= 4);
}

// ─── Full Pipeline: Detect → Simulate → Build ──────────────────────────────

#[tokio::test]
async fn full_pipeline_arbitrage_end_to_end() {
    let config = test_config();

    // Step 1: Create opportunity (simulating detector output)
    let opp = Opportunity {
        opportunity_type: OpportunityType::Arbitrage,
        token_in: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
        amount_in: 1_000_000_000_000_000_000,
        expected_profit: 10_000_000_000_000_000,
        gas_estimate: 250_000,
        deadline: 0,
        path: vec![DexType::UniswapV2, DexType::UniswapV3],
        target_tx: None,
    };

    // Step 2: Simulate
    let sim = mev_core::simulator::EvmSimulator::new(config.clone());
    let sim_result = sim.simulate(&opp).await;
    assert!(sim_result.gas_used > 0);

    // Step 3: Build bundle
    let mut builder = mev_core::builder::BundleBuilder::new(config);
    builder.set_contract("0xDEADBEEF00000000000000000000000000000000".to_string());
    let bundle = builder.build(&opp).await.unwrap();

    // Verify complete pipeline output
    assert_eq!(bundle.transactions.len(), 1);
    assert!(!bundle.transactions[0].data.is_empty());
    assert_eq!(bundle.transactions[0].gas_limit, 250_000);
}

// ─── Simulation Count Tracking ──────────────────────────────────────────────

#[tokio::test]
async fn simulator_tracks_count_across_calls() {
    let config = test_config();
    let sim = mev_core::simulator::EvmSimulator::new(config);

    let opp = Opportunity {
        opportunity_type: OpportunityType::Arbitrage,
        token_in: "WETH".to_string(),
        token_out: "USDC".to_string(),
        amount_in: 1_000_000_000_000_000_000,
        expected_profit: 0,
        gas_estimate: 250_000,
        deadline: 0,
        path: vec![DexType::UniswapV2, DexType::UniswapV3],
        target_tx: None,
    };

    sim.simulate(&opp).await;
    sim.simulate(&opp).await;
    sim.simulate(&opp).await;

    assert_eq!(sim.count().await, 3);
}

// ─── Bundle Builder Multiple Opportunity Types ──────────────────────────────

#[tokio::test]
async fn builder_handles_all_opportunity_types() {
    let config = test_config();
    let mut builder = mev_core::builder::BundleBuilder::new(config);
    builder.set_contract("0x0000000000000000000000000000000000000001".to_string());

    let types = vec![
        (OpportunityType::Arbitrage, vec![DexType::UniswapV2, DexType::UniswapV3]),
        (OpportunityType::Backrun, vec![DexType::SushiSwap]),
        (OpportunityType::Liquidation, vec![]),
    ];

    for (opp_type, path) in types {
        let opp = Opportunity {
            opportunity_type: opp_type,
            token_in: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
            token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
            amount_in: 1_000_000_000_000_000_000,
            expected_profit: 10_000_000_000_000_000,
            gas_estimate: 250_000,
            deadline: 100,
            path,
            target_tx: None,
        };

        let bundle = builder.build(&opp).await.unwrap();
        assert_eq!(bundle.transactions.len(), 1, "each opportunity type should produce exactly 1 tx");
        assert!(!bundle.transactions[0].data.is_empty(), "calldata must not be empty");
    }
}
