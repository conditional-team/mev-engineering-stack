//! Bundle Builder module

use crate::config::Config;
use crate::types::{Bundle, BundleTransaction, Opportunity, OpportunityType};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{info, debug};

/// Bundle builder for MEV extraction
pub struct BundleBuilder {
    config: Arc<Config>,
    count: AtomicU64,
    contract_address: Option<String>,
}

impl BundleBuilder {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            count: AtomicU64::new(0),
            contract_address: None,
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        info!("Bundle builder started");
        Ok(())
    }

    pub async fn stop(&self) -> anyhow::Result<()> {
        info!("Bundle builder stopped");
        Ok(())
    }

    /// Set the contract address for bundle execution
    pub fn set_contract(&mut self, address: String) {
        self.contract_address = Some(address);
    }

    /// Build a bundle from an opportunity
    pub async fn build(&self, opportunity: &Opportunity) -> anyhow::Result<Bundle> {
        let contract = self.contract_address.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Contract address not set"))?;

        let transactions = match opportunity.opportunity_type {
            OpportunityType::Arbitrage => self.build_arbitrage_bundle(opportunity, contract)?,
            OpportunityType::Backrun => self.build_backrun_bundle(opportunity, contract)?,
            OpportunityType::Liquidation => self.build_liquidation_bundle(opportunity, contract)?,
            OpportunityType::Sandwich => self.build_sandwich_bundle(opportunity, contract)?,
        };

        self.count.fetch_add(1, Ordering::Relaxed);

        Ok(Bundle {
            transactions,
            target_block: None,
            max_block_number: None,
            min_timestamp: None,
            max_timestamp: None,
            reverting_tx_hashes: vec![],
        })
    }

    fn build_arbitrage_bundle(
        &self, 
        opportunity: &Opportunity,
        contract: &str,
    ) -> anyhow::Result<Vec<BundleTransaction>> {
        // Build calldata for FlashArbitrage.executeArbitrage
        let swap_data = self.encode_swap_path(opportunity)?;
        
        let calldata = self.encode_execute_arbitrage(
            &opportunity.token_in,
            opportunity.amount_in,
            &swap_data,
        )?;

        Ok(vec![BundleTransaction {
            to: contract.to_string(),
            value: 0,
            gas_limit: opportunity.gas_estimate,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: Some(1_000_000_000), // 1 gwei tip
            data: calldata,
            nonce: None,
        }])
    }

    fn build_backrun_bundle(
        &self,
        opportunity: &Opportunity,
        contract: &str,
    ) -> anyhow::Result<Vec<BundleTransaction>> {
        // Include target transaction first (if available)
        let mut txs = Vec::new();

        // Our backrun transaction
        let swap_data = self.encode_swap_path(opportunity)?;
        let calldata = self.encode_execute_arbitrage(
            &opportunity.token_in,
            opportunity.amount_in,
            &swap_data,
        )?;

        txs.push(BundleTransaction {
            to: contract.to_string(),
            value: 0,
            gas_limit: opportunity.gas_estimate,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: Some(2_000_000_000), // Higher tip for backrun
            data: calldata,
            nonce: None,
        });

        Ok(txs)
    }

    fn build_liquidation_bundle(
        &self,
        opportunity: &Opportunity,
        contract: &str,
    ) -> anyhow::Result<Vec<BundleTransaction>> {
        // Flash loan -> Repay debt -> Receive collateral -> Swap collateral -> Repay flash loan
        
        let calldata = self.encode_liquidation(
            &opportunity.token_in,
            &opportunity.token_out,
            opportunity.amount_in,
        )?;

        Ok(vec![BundleTransaction {
            to: contract.to_string(),
            value: 0,
            gas_limit: opportunity.gas_estimate,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: Some(1_000_000_000),
            data: calldata,
            nonce: None,
        }])
    }

    fn build_sandwich_bundle(
        &self,
        opportunity: &Opportunity,
        contract: &str,
    ) -> anyhow::Result<Vec<BundleTransaction>> {
        // Frontrun -> Target -> Backrun
        let mut txs = Vec::new();

        // Frontrun transaction
        let frontrun_data = self.encode_swap_path(opportunity)?;
        txs.push(BundleTransaction {
            to: contract.to_string(),
            value: 0,
            gas_limit: 150_000,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: Some(10_000_000_000), // High tip for frontrun
            data: frontrun_data.clone(),
            nonce: None,
        });

        // Note: Target TX would be included by the bundle relay

        // Backrun transaction
        txs.push(BundleTransaction {
            to: contract.to_string(),
            value: 0,
            gas_limit: 150_000,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: Some(1_000_000_000),
            data: frontrun_data, // Reverse swap
            nonce: None,
        });

        Ok(txs)
    }

    fn encode_swap_path(&self, opportunity: &Opportunity) -> anyhow::Result<Vec<u8>> {
        // Encode swap path for the contract
        // Format: [swapCount][swapType][target][params]...
        
        let mut data = Vec::new();
        data.push(opportunity.path.len() as u8);
        
        for dex in &opportunity.path {
            // Swap type
            data.push(match dex {
                crate::types::DexType::UniswapV2 => 1,
                crate::types::DexType::UniswapV3 => 2,
                crate::types::DexType::SushiSwap => 1,
                crate::types::DexType::Curve => 3,
                crate::types::DexType::Balancer => 4,
            });
            
            // Target address (placeholder)
            data.extend_from_slice(&[0u8; 20]);
            
            // Params (placeholder)
            data.extend_from_slice(&[0u8; 32]);
        }

        Ok(data)
    }

    fn encode_execute_arbitrage(
        &self,
        token: &str,
        amount: u128,
        swap_data: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        // Function selector: executeArbitrage(address,uint256,bytes)
        let selector = [0x12, 0x34, 0x56, 0x78]; // Placeholder
        
        let mut calldata = selector.to_vec();
        
        // Encode token address (32 bytes, right-padded)
        calldata.extend_from_slice(&[0u8; 12]);
        if let Ok(bytes) = hex::decode(token.trim_start_matches("0x")) {
            calldata.extend_from_slice(&bytes);
        } else {
            calldata.extend_from_slice(&[0u8; 20]);
        }
        
        // Encode amount (32 bytes)
        calldata.extend_from_slice(&amount.to_be_bytes());
        calldata.extend_from_slice(&[0u8; 16]);
        
        // Encode swap data offset and data
        calldata.extend_from_slice(&[0u8; 28]);
        calldata.extend_from_slice(&(96u32).to_be_bytes());
        
        calldata.extend_from_slice(&[0u8; 28]);
        calldata.extend_from_slice(&(swap_data.len() as u32).to_be_bytes());
        
        calldata.extend_from_slice(swap_data);

        Ok(calldata)
    }

    fn encode_liquidation(
        &self,
        debt_token: &str,
        collateral_token: &str,
        amount: u128,
    ) -> anyhow::Result<Vec<u8>> {
        // Placeholder liquidation encoding
        let selector = [0xab, 0xcd, 0xef, 0x12];
        let mut calldata = selector.to_vec();
        
        // Add parameters...
        calldata.extend_from_slice(&[0u8; 64]);
        
        Ok(calldata)
    }

    pub async fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}
