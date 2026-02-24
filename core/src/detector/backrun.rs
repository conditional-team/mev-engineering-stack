//! Backrun detection

use crate::config::Config;
use crate::types::{Opportunity, OpportunityType, PendingTx, DexType};
use std::sync::Arc;

/// Backrun detector for large swaps
pub struct BackrunDetector {
    config: Arc<Config>,
    min_swap_size_eth: u128,
}

impl BackrunDetector {
    pub fn new(config: Arc<Config>) -> Self {
        Self { 
            config,
            min_swap_size_eth: 10_000_000_000_000_000_000, // 10 ETH
        }
    }

    /// Detect backrun opportunity
    pub async fn detect(&self, tx: &PendingTx) -> Option<Opportunity> {
        // Check if transaction is a large swap
        let swap_size = self.estimate_swap_size(tx)?;
        
        if swap_size < self.min_swap_size_eth {
            return None;
        }

        // Estimate price impact
        let price_impact = self.estimate_price_impact(swap_size)?;
        
        // Calculate backrun profit
        let profit = self.calculate_backrun_profit(swap_size, price_impact)?;

        // Check minimum profit
        if profit < self.config.strategy.min_profit_wei {
            return None;
        }

        Some(Opportunity {
            opportunity_type: OpportunityType::Backrun,
            token_in: "WETH".to_string(),
            token_out: "TARGET".to_string(),
            amount_in: swap_size / 10, // Use 10% of target size
            expected_profit: profit,
            gas_estimate: 150_000,
            deadline: tx.timestamp + 12,
            path: vec![DexType::UniswapV3],
            target_tx: Some(tx.hash),
        })
    }

    fn estimate_swap_size(&self, tx: &PendingTx) -> Option<u128> {
        // Check value field for ETH swaps
        if tx.value > 0 {
            return Some(tx.value);
        }

        // Parse calldata for token swaps
        if tx.input.len() >= 36 {
            let amount = u128::from_be_bytes(
                tx.input[4..20].try_into().ok()?
            );
            return Some(amount);
        }

        None
    }

    fn estimate_price_impact(&self, swap_size: u128) -> Option<u128> {
        // Simplified price impact model
        // Real implementation would query pool liquidity
        
        // Assume 0.1% impact per 10 ETH
        let impact_bps = (swap_size / 10_000_000_000_000_000_000) * 10;
        Some(impact_bps.min(500)) // Cap at 5%
    }

    fn calculate_backrun_profit(&self, swap_size: u128, price_impact_bps: u128) -> Option<u128> {
        // Profit = swap_size * price_recovery * our_portion - gas
        
        // Assume we capture 50% of price recovery
        let recovery_rate = 50; // 50%
        
        // Our portion of the recovery
        let gross = swap_size * price_impact_bps * recovery_rate / (10000 * 100);
        
        // Gas cost
        let gas_cost = 150_000u128 * 50_000_000_000u128;
        
        gross.checked_sub(gas_cost)
    }
}
