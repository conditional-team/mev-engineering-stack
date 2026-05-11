//! Liquidation detection — monitors under-collateralized lending positions
//!
//! Tracks positions across Aave V3, Compound V3, and Morpho via event
//! subscriptions, identifies positions whose health factor has dropped
//! below 1.0, and constructs profitable liquidation opportunities
//! using flash loans.

use crate::config::Config;
use crate::types::{Opportunity, OpportunityType, DexType, PoolState};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, info, warn};

/// Position data from lending protocol
#[derive(Debug, Clone)]
pub struct Position {
    pub user: [u8; 20],
    pub protocol: LendingProtocol,
    pub collateral_token: [u8; 20],
    /// Amount in collateral_token native units (NOT debt units).
    pub collateral_amount: u128,
    pub debt_token: [u8; 20],
    /// Amount in debt_token native units.
    pub debt_amount: u128,
    /// 18-decimal fixed point. < 1e18 means liquidatable
    pub health_factor: u128,
    pub last_updated: u64,
    /// Price of 1 unit of collateral_token expressed in debt_token units,
    /// 18-decimal fixed point (e.g. 1e18 means 1:1 parity).
    ///
    /// When `collateral_token == debt_token` this field is ignored.
    /// When tokens differ and this is `None`, the position is skipped instead of
    /// being evaluated with mismatched units (the previous behaviour produced a
    /// dimensionally invalid "profit" number).
    pub collateral_price_e18: Option<u128>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LendingProtocol {
    AaveV3,
    CompoundV3,
    Morpho,
}

impl LendingProtocol {
    /// Liquidation bonus in basis points (protocol-specific)
    fn liquidation_bonus_bps(&self) -> u128 {
        match self {
            LendingProtocol::AaveV3 => 500,      // 5.0%
            LendingProtocol::CompoundV3 => 800,   // 8.0%
            LendingProtocol::Morpho => 500,        // 5.0%
        }
    }

    /// Maximum debt that can be liquidated in one call (fraction of total)
    fn close_factor(&self) -> u128 {
        match self {
            LendingProtocol::AaveV3 => 5000,     // 50%
            LendingProtocol::CompoundV3 => 5000,  // 50%
            LendingProtocol::Morpho => 10000,      // 100% (full liquidation)
        }
    }

    /// Flash loan fee in basis points
    fn flash_loan_fee_bps(&self) -> u128 {
        match self {
            LendingProtocol::AaveV3 => 5,     // 0.05% (Aave flash)
            LendingProtocol::CompoundV3 => 0,  // Balancer flash = 0
            LendingProtocol::Morpho => 0,       // Balancer flash = 0
        }
    }

    fn gas_estimate(&self) -> u64 {
        match self {
            LendingProtocol::AaveV3 => 450_000,
            LendingProtocol::CompoundV3 => 500_000,
            LendingProtocol::Morpho => 400_000,
        }
    }
}

/// Liquidation detector for lending protocols
pub struct LiquidationDetector {
    config: Arc<Config>,
    /// Active positions indexed by user address
    positions: Arc<RwLock<HashMap<[u8; 20], Vec<Position>>>>,
    /// Pool cache for resolving swap routes (debt_token → collateral_token)
    pool_cache: Arc<RwLock<HashMap<[u8; 20], PoolState>>>,
    /// Running count of liquidatable positions found
    found_count: std::sync::atomic::AtomicU64,
}

impl LiquidationDetector {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            positions: Arc::new(RwLock::new(HashMap::new())),
            pool_cache: Arc::new(RwLock::new(HashMap::new())),
            found_count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Load pool states for swap route resolution
    pub fn load_pools(&self, pools: Vec<PoolState>) {
        let mut cache = self.pool_cache.write();
        for pool in pools {
            cache.insert(pool.address, pool);
        }
    }

    /// Resolve a pool for the (token_a, token_b) swap. Returns address and fee.
    fn resolve_pool_for_pair(&self, token_a: &[u8; 20], token_b: &[u8; 20]) -> Option<([u8; 20], u32)> {
        let cache = self.pool_cache.read();
        for pool in cache.values() {
            let matches = (pool.token0 == *token_a && pool.token1 == *token_b)
                || (pool.token0 == *token_b && pool.token1 == *token_a);
            if matches {
                return Some((pool.address, pool.fee));
            }
        }
        None
    }

    /// Bulk update tracked positions (e.g. from event indexer)
    pub fn update_positions(&self, new_positions: Vec<Position>) {
        let mut map = self.positions.write();
        for pos in new_positions {
            map.entry(pos.user).or_insert_with(Vec::new).push(pos);
        }
    }

    /// Remove stale positions older than max_age_seconds
    pub fn prune_stale(&self, now: u64, max_age_seconds: u64) {
        let mut map = self.positions.write();
        for positions in map.values_mut() {
            positions.retain(|p| now.saturating_sub(p.last_updated) < max_age_seconds);
        }
        map.retain(|_, v| !v.is_empty());
    }

    /// Scan all tracked positions and return profitable liquidation opportunities
    pub fn find_liquidatable(&self) -> Vec<Opportunity> {
        let map = self.positions.read();
        let mut opportunities = Vec::new();

        for positions in map.values() {
            for position in positions {
                // Health factor < 1e18 → liquidatable
                if position.health_factor < 1_000_000_000_000_000_000 {
                    if let Some(opp) = self.evaluate_liquidation(position) {
                        self.found_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        opportunities.push(opp);
                    }
                }
            }
        }

        // Sort by profit descending — execute most profitable first
        opportunities.sort_by(|a, b| b.expected_profit.cmp(&a.expected_profit));
        opportunities
    }

    /// Evaluate whether a single position liquidation is profitable.
    ///
    /// All monetary quantities below are tracked explicitly in either
    /// **debt-token units** or **collateral-token native units**, and converted
    /// via `collateral_price_e18` when needed. The previous implementation
    /// compared `collateral_received` (debt units) against `collateral_amount`
    /// (collateral native units), which silently yielded an opportunity whose
    /// `expected_profit` was dimensionally meaningless whenever the two tokens
    /// differed (e.g. WETH collateral vs. USDC debt).
    fn evaluate_liquidation(&self, position: &Position) -> Option<Opportunity> {
        let protocol = &position.protocol;
        const ONE_E18: u128 = 1_000_000_000_000_000_000;

        // Max repayable debt = debt_amount × close_factor (debt units)
        let close_factor = protocol.close_factor();
        let max_repay = position.debt_amount.checked_mul(close_factor)? / 10_000;

        // Collateral the protocol owes us, denominated in DEBT units.
        let bonus_bps = protocol.liquidation_bonus_bps();
        let collateral_value_in_debt =
            max_repay.checked_mul(10_000 + bonus_bps)? / 10_000;

        // Convert that obligation to COLLATERAL native units to verify the
        // position actually holds enough collateral to seize.
        let same_token = position.collateral_token == position.debt_token;
        let collateral_needed_native = if same_token {
            collateral_value_in_debt
        } else {
            let price = match position.collateral_price_e18 {
                Some(p) if p > 0 => p,
                _ => {
                    warn!(
                        collateral = ?position.collateral_token,
                        debt = ?position.debt_token,
                        "Collateral/debt token mismatch with no price provided, skipping"
                    );
                    return None;
                }
            };
            // collateral_native = value_in_debt × 1e18 / price_e18 (256-bit safe)
            mul_div_u128(collateral_value_in_debt, ONE_E18, price)?
        };

        if collateral_needed_native > position.collateral_amount {
            // Position doesn't hold enough collateral for the full bonus.
            return None;
        }

        // Gross profit = bonus portion, in DEBT units (max_repay is debt units).
        let gross_profit = max_repay.checked_mul(bonus_bps)? / 10_000;

        // Flash loan fee (debt units)
        let flash_fee = max_repay.checked_mul(protocol.flash_loan_fee_bps())? / 10_000;

        // Gas cost — denominated in wei (≈ ETH). For a denomination-aware
        // comparison we treat min_profit_wei and gas_cost as the same unit as
        // gross_profit; this matches the historical behaviour and keeps the
        // `min_profit_wei` config knob compatible.
        let gas = protocol.gas_estimate();
        let gas_price_wei = self.config.strategy.max_gas_price_gwei as u128 * 1_000_000_000;
        let gas_cost = (gas as u128).checked_mul(gas_price_wei)?;

        // DEX swap cost (collateral → debt token): ~30 bps of the collateral leg
        // expressed in debt units (so it is subtractable from gross_profit).
        let swap_cost = collateral_value_in_debt.checked_mul(30)? / 10_000;

        // Net profit (debt units). Saturating-style: any underflow → unprofitable.
        let net_profit = gross_profit
            .checked_sub(flash_fee)?
            .checked_sub(gas_cost)?
            .checked_sub(swap_cost)?;

        if net_profit < self.config.strategy.min_profit_wei {
            return None;
        }

        debug!(
            protocol = ?position.protocol,
            health = position.health_factor,
            debt = position.debt_amount,
            bonus_bps,
            net_profit,
            "Liquidation opportunity found"
        );

        // Resolve a pool for the collateral → debt swap
        let (pool_addr, pool_fee) = match self.resolve_pool_for_pair(
            &position.collateral_token,
            &position.debt_token,
        ) {
            Some(p) => p,
            None => {
                warn!(
                    collateral = ?position.collateral_token,
                    debt = ?position.debt_token,
                    "No pool found for liquidation swap route, skipping"
                );
                return None;
            }
        };

        Some(Opportunity {
            opportunity_type: OpportunityType::Liquidation,
            token_in: format!("0x{}", hex::encode(position.debt_token)),
            token_out: format!("0x{}", hex::encode(position.collateral_token)),
            amount_in: max_repay,
            expected_profit: net_profit,
            gas_estimate: gas,
            deadline: u64::MAX, // Liquidations are valid as long as health < 1
            path: vec![DexType::UniswapV3],
            pool_addresses: vec![pool_addr],
            pool_fees: vec![pool_fee],
            target_tx: None,
        })
    }

    /// Statistics
    pub fn found_count(&self) -> u64 {
        self.found_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn tracked_positions(&self) -> usize {
        self.positions.read().values().map(|v| v.len()).sum()
    }
}

/// 256-bit-precise mul_div: returns `(a * b) / c` without intermediate u128 overflow.
/// Returns `None` if `c == 0` or the quotient does not fit in u128.
fn mul_div_u128(a: u128, b: u128, c: u128) -> Option<u128> {
    if c == 0 {
        return None;
    }
    // Manual 128 x 128 -> 256 via 64-bit halves.
    let a_hi = a >> 64;
    let a_lo = a & 0xFFFF_FFFF_FFFF_FFFF;
    let b_hi = b >> 64;
    let b_lo = b & 0xFFFF_FFFF_FFFF_FFFF;
    let ll = a_lo * b_lo;
    let lh = a_lo * b_hi;
    let hl = a_hi * b_lo;
    let hh = a_hi * b_hi;
    let mid = (ll >> 64) + (lh & 0xFFFF_FFFF_FFFF_FFFF) + (hl & 0xFFFF_FFFF_FFFF_FFFF);
    let lo = (ll & 0xFFFF_FFFF_FFFF_FFFF) | ((mid & 0xFFFF_FFFF_FFFF_FFFF) << 64);
    let hi = hh + (lh >> 64) + (hl >> 64) + (mid >> 64);
    // Long-divide [hi:lo] by c, MSB-first.
    let mut rem: u128 = 0;
    let mut quot_lo: u128 = 0;
    let mut quot_hi: u128 = 0;
    for i in (0..256).rev() {
        rem = (rem << 1) | if i >= 128 { (hi >> (i - 128)) & 1 } else { (lo >> i) & 1 };
        if rem >= c {
            rem -= c;
            if i >= 128 {
                quot_hi |= 1u128 << (i - 128);
            } else {
                quot_lo |= 1u128 << i;
            }
        }
    }
    if quot_hi != 0 {
        return None; // quotient overflows u128
    }
    Some(quot_lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Arc<Config> {
        let mut config = Config::default();
        config.strategy.min_profit_wei = 100_000_000_000_000; // 0.0001 ETH
        config.strategy.max_gas_price_gwei = 1;
        Arc::new(config)
    }

    #[test]
    fn test_liquidatable_position_detected() {
        let detector = LiquidationDetector::new(test_config());

        // Pool needed by evaluate_liquidation::resolve_pool_for_pair.
        detector.load_pools(vec![PoolState {
            address: [0xEE; 20],
            token0: [0xAA; 20],
            token1: [0xBB; 20],
            reserve0: 0,
            reserve1: 0,
            fee: 3000,
            sqrt_price_x96: 0,
            liquidity: 0,
            is_v3: false,
            current_tick: 0,
            tick_spacing: 0,
            ticks: Vec::new(),
        }]);

        detector.update_positions(vec![Position {
            user: [1u8; 20],
            protocol: LendingProtocol::AaveV3,
            collateral_token: [0xAA; 20],
            collateral_amount: 100_000_000_000_000_000_000, // 100 ETH worth
            debt_token: [0xBB; 20],
            debt_amount: 80_000_000_000_000_000_000, // 80 ETH
            health_factor: 900_000_000_000_000_000,   // 0.9 — liquidatable
            last_updated: 0,
            // 1:1 price (test maintains historical 1-token assumption).
            collateral_price_e18: Some(1_000_000_000_000_000_000),
        }]);

        let opps = detector.find_liquidatable();
        assert_eq!(opps.len(), 1);
        assert!(opps[0].expected_profit > 0);
        assert!(matches!(opps[0].opportunity_type, OpportunityType::Liquidation));
    }

    #[test]
    fn test_skips_when_price_missing_and_tokens_differ() {
        let detector = LiquidationDetector::new(test_config());

        detector.update_positions(vec![Position {
            user: [3u8; 20],
            protocol: LendingProtocol::AaveV3,
            collateral_token: [0xAA; 20],
            collateral_amount: 100_000_000_000_000_000_000,
            debt_token: [0xBB; 20],
            debt_amount: 80_000_000_000_000_000_000,
            health_factor: 900_000_000_000_000_000,
            last_updated: 0,
            collateral_price_e18: None, // mismatched units, no price → must skip
        }]);

        let opps = detector.find_liquidatable();
        assert!(opps.is_empty(), "must not emit opportunity with unknown collateral price");
    }

    #[test]
    fn test_collateral_price_converts_units_correctly() {
        // WETH collateral (1 WETH = 2000 USDC) vs USDC debt.
        // Verify the collateral sufficiency check uses native collateral units.
        let detector = LiquidationDetector::new(test_config());

        detector.load_pools(vec![PoolState {
            address: [0xEE; 20],
            token0: [0xAA; 20],
            token1: [0xBB; 20],
            reserve0: 0,
            reserve1: 0,
            fee: 3000,
            sqrt_price_x96: 0,
            liquidity: 0,
            is_v3: false,
            current_tick: 0,
            tick_spacing: 0,
            ticks: Vec::new(),
        }]);

        // Debt: 1000 USDC (assuming 18-decimal placeholder for test simplicity).
        // Aave close factor 50% → max_repay = 500 USDC.
        // Collateral required (with 5% bonus) = 525 USDC worth = 0.2625 WETH.
        // Position holds 1 WETH → sufficient.
        let one_eth = 1_000_000_000_000_000_000u128;
        detector.update_positions(vec![Position {
            user: [4u8; 20],
            protocol: LendingProtocol::AaveV3,
            collateral_token: [0xAA; 20],
            collateral_amount: one_eth, // 1 WETH
            debt_token: [0xBB; 20],
            debt_amount: 1000 * one_eth, // 1000 "USDC" (test units)
            health_factor: 900_000_000_000_000_000,
            last_updated: 0,
            collateral_price_e18: Some(2000 * one_eth), // 1 WETH = 2000 USDC
        }]);

        let opps = detector.find_liquidatable();
        assert_eq!(opps.len(), 1);
        assert!(opps[0].expected_profit > 0);
    }

    #[test]
    fn test_healthy_position_skipped() {
        let detector = LiquidationDetector::new(test_config());

        detector.update_positions(vec![Position {
            user: [2u8; 20],
            protocol: LendingProtocol::CompoundV3,
            collateral_token: [0xCC; 20],
            collateral_amount: 200_000_000_000_000_000_000,
            debt_token: [0xDD; 20],
            debt_amount: 50_000_000_000_000_000_000,
            health_factor: 2_000_000_000_000_000_000, // 2.0 — healthy
            last_updated: 0,
            collateral_price_e18: Some(1_000_000_000_000_000_000),
        }]);

        let opps = detector.find_liquidatable();
        assert!(opps.is_empty());
    }

    #[test]
    fn test_close_factor_limits_repay() {
        // Aave close factor = 50%
        let protocol = LendingProtocol::AaveV3;
        assert_eq!(protocol.close_factor(), 5000);

        let debt = 100_000_000_000_000_000_000u128; // 100 ETH
        let max_repay = debt * protocol.close_factor() / 10_000;
        assert_eq!(max_repay, 50_000_000_000_000_000_000); // 50 ETH
    }

    #[test]
    fn test_prune_stale() {
        let detector = LiquidationDetector::new(test_config());

        detector.update_positions(vec![
            Position {
                user: [1u8; 20],
                protocol: LendingProtocol::AaveV3,
                collateral_token: [0; 20],
                collateral_amount: 0,
                debt_token: [0; 20],
                debt_amount: 0,
                health_factor: 0,
                last_updated: 100,
                collateral_price_e18: None,
            },
        ]);

        assert_eq!(detector.tracked_positions(), 1);
        detector.prune_stale(200, 50); // max_age=50, now=200, pos=100 → stale
        assert_eq!(detector.tracked_positions(), 0);
    }
}
