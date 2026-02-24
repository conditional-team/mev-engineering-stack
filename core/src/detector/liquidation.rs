//! Liquidation detection

use crate::config::Config;
use crate::types::{Opportunity, OpportunityType, DexType};
use std::sync::Arc;
use std::collections::HashMap;

/// Position data from lending protocol
#[derive(Debug, Clone)]
pub struct Position {
    pub user: String,
    pub protocol: LendingProtocol,
    pub collateral_token: String,
    pub collateral_amount: u128,
    pub debt_token: String,
    pub debt_amount: u128,
    pub health_factor: u128, // 18 decimals, <1e18 = liquidatable
}

#[derive(Debug, Clone)]
pub enum LendingProtocol {
    AaveV3,
    Compound,
    Morpho,
}

/// Liquidation detector for lending protocols
pub struct LiquidationDetector {
    config: Arc<Config>,
    positions: HashMap<String, Position>,
}

impl LiquidationDetector {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            positions: HashMap::new(),
        }
    }

    /// Update tracked positions
    pub fn update_positions(&mut self, positions: Vec<Position>) {
        for pos in positions {
            self.positions.insert(pos.user.clone(), pos);
        }
    }

    /// Find liquidatable positions
    pub fn find_liquidatable(&self) -> Vec<Opportunity> {
        let mut opportunities = Vec::new();
        
        for (_, position) in &self.positions {
            // Health factor < 1e18 means liquidatable
            if position.health_factor < 1_000_000_000_000_000_000 {
                if let Some(opp) = self.create_liquidation_opportunity(position) {
                    opportunities.push(opp);
                }
            }
        }

        opportunities
    }

    fn create_liquidation_opportunity(&self, position: &Position) -> Option<Opportunity> {
        // Calculate liquidation profit
        // Typically 5-10% bonus on liquidated collateral
        
        let liquidation_bonus_bps = match position.protocol {
            LendingProtocol::AaveV3 => 500,   // 5%
            LendingProtocol::Compound => 800, // 8%
            LendingProtocol::Morpho => 500,   // 5%
        };

        // Max liquidation is typically 50% of position
        let max_liquidation = position.debt_amount / 2;
        
        // Profit = liquidation_amount * bonus - gas - flash_loan_fee
        let gross_profit = max_liquidation * liquidation_bonus_bps / 10000;
        
        // Flash loan fee (Balancer = 0, Aave = 0.05%)
        let flash_fee = max_liquidation * 5 / 10000;
        
        // Gas cost (liquidations are expensive ~500k gas)
        let gas_cost = 500_000u128 * 50_000_000_000u128;
        
        let net_profit = gross_profit.checked_sub(flash_fee)?.checked_sub(gas_cost)?;

        if net_profit < self.config.strategy.min_profit_wei {
            return None;
        }

        Some(Opportunity {
            opportunity_type: OpportunityType::Liquidation,
            token_in: position.debt_token.clone(),
            token_out: position.collateral_token.clone(),
            amount_in: max_liquidation,
            expected_profit: net_profit,
            gas_estimate: 500_000,
            deadline: u64::MAX, // Liquidations don't have strict deadlines
            path: vec![DexType::UniswapV3],
            target_tx: None,
        })
    }

    /// Monitor health factors via events
    pub async fn subscribe_health_updates(&self) -> anyhow::Result<()> {
        // TODO: Subscribe to Borrow/Repay/Liquidation events
        // Update internal position tracking
        Ok(())
    }
}
