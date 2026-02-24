//! Arbitrage detection

use crate::config::Config;
use crate::types::{Opportunity, OpportunityType, PendingTx, SwapInfo, DexType};
use std::sync::Arc;
use tracing::debug;

/// Arbitrage detector for cross-DEX opportunities
pub struct ArbitrageDetector {
    config: Arc<Config>,
}

impl ArbitrageDetector {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    /// Detect arbitrage opportunity from pending transaction
    pub async fn detect(&self, tx: &PendingTx) -> Option<Opportunity> {
        // Parse swap from transaction
        let swap = self.parse_swap(tx)?;
        
        // Get prices from other DEXes
        let prices = self.get_cross_dex_prices(&swap).await?;
        
        // Calculate potential profit
        let profit = self.calculate_profit(&swap, &prices)?;
        
        // Check minimum profit threshold
        let min_profit = self.config.strategy.min_profit_wei;
        if profit < min_profit {
            return None;
        }

        Some(Opportunity {
            opportunity_type: OpportunityType::Arbitrage,
            token_in: swap.token_in,
            token_out: swap.token_out,
            amount_in: swap.amount_in,
            expected_profit: profit,
            gas_estimate: 250_000,
            deadline: tx.timestamp + 12, // 1 block
            path: vec![swap.dex.clone(), self.find_best_exit_dex(&prices)],
            target_tx: Some(tx.hash),
        })
    }

    fn parse_swap(&self, tx: &PendingTx) -> Option<SwapInfo> {
        // Parse calldata to extract swap info
        let data = &tx.input;
        
        if data.len() < 4 {
            return None;
        }

        // Check function selector
        let selector = &data[0..4];
        
        match selector {
            // swapExactTokensForTokens (UniswapV2)
            [0x38, 0xed, 0x17, 0x39] => self.parse_v2_swap(data),
            // exactInputSingle (UniswapV3)
            [0x41, 0x4b, 0xf3, 0x89] => self.parse_v3_swap(data),
            _ => None,
        }
    }

    fn parse_v2_swap(&self, data: &[u8]) -> Option<SwapInfo> {
        if data.len() < 132 {
            return None;
        }

        // Decode: amountIn, amountOutMin, path[], to, deadline
        let amount_in = u128::from_be_bytes(data[4..36].try_into().ok()?);
        
        // Path is dynamic, first address is token_in, last is token_out
        // Simplified: assume 2-hop path at offset 128
        let token_in = format!("0x{}", hex::encode(&data[100..120]));
        let token_out = format!("0x{}", hex::encode(&data[132..152]));

        Some(SwapInfo {
            dex: DexType::UniswapV2,
            token_in,
            token_out,
            amount_in,
            amount_out_min: 0,
            fee: 3000, // 0.3%
        })
    }

    fn parse_v3_swap(&self, _data: &[u8]) -> Option<SwapInfo> {
        // TODO: Implement V3 parsing
        None
    }

    async fn get_cross_dex_prices(&self, swap: &SwapInfo) -> Option<Vec<(DexType, u128)>> {
        // TODO: Query multiple DEXes for prices
        // This would use on-chain calls or cached pool data
        Some(vec![
            (DexType::UniswapV2, 1_000_000),
            (DexType::SushiSwap, 1_010_000),
            (DexType::UniswapV3, 1_005_000),
        ])
    }

    fn calculate_profit(&self, swap: &SwapInfo, prices: &[(DexType, u128)]) -> Option<u128> {
        // Find best exit price
        let best_exit = prices.iter().max_by_key(|(_, p)| p)?;
        let entry_price = 1_000_000u128; // Base price
        
        // Profit = (exit - entry) * amount - gas
        let gross_profit = (best_exit.1.saturating_sub(entry_price)) * swap.amount_in / entry_price;
        
        // Estimate gas cost (assume 50 gwei, 250k gas)
        let gas_cost = 250_000u128 * 50_000_000_000u128;
        
        gross_profit.checked_sub(gas_cost)
    }

    fn find_best_exit_dex(&self, prices: &[(DexType, u128)]) -> DexType {
        prices
            .iter()
            .max_by_key(|(_, p)| p)
            .map(|(d, _)| d.clone())
            .unwrap_or(DexType::UniswapV3)
    }
}
