//! Configuration module

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main configuration struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Chain configurations
    pub chains: HashMap<u64, ChainConfig>,
    
    /// RPC endpoints
    pub rpc: RpcConfig,
    
    /// Strategy settings
    pub strategy: StrategyConfig,
    
    /// Performance settings
    pub performance: PerformanceConfig,
    
    /// Logging settings
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub name: String,
    pub chain_id: u64,
    pub rpc_url: String,
    pub ws_url: String,
    pub flashbots_relay: Option<String>,
    pub balancer_vault: String,
    pub contract_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub primary: Vec<String>,
    pub fallback: Vec<String>,
    pub max_connections: usize,
    pub request_timeout_ms: u64,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub min_profit_wei: u128,
    pub min_profit_bps: u16,
    pub max_gas_price_gwei: u64,
    pub slippage_tolerance_bps: u16,
    pub enabled_strategies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub detector_threads: usize,
    pub simulator_threads: usize,
    pub max_pending_opportunities: usize,
    pub simulation_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub json_output: bool,
    pub file_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut chains = HashMap::new();
        
        chains.insert(1, ChainConfig {
            name: "Ethereum".to_string(),
            chain_id: 1,
            rpc_url: "https://eth.llamarpc.com".to_string(),
            ws_url: "wss://eth.llamarpc.com".to_string(),
            flashbots_relay: Some("https://relay.flashbots.net".to_string()),
            balancer_vault: "0xBA12222222228d8Ba445958a75a0704d566BF2C8".to_string(),
            contract_address: None,
        });

        chains.insert(42161, ChainConfig {
            name: "Arbitrum".to_string(),
            chain_id: 42161,
            rpc_url: "https://arb1.arbitrum.io/rpc".to_string(),
            ws_url: "wss://arb1.arbitrum.io/ws".to_string(),
            flashbots_relay: None,
            balancer_vault: "0xBA12222222228d8Ba445958a75a0704d566BF2C8".to_string(),
            contract_address: None,
        });

        Self {
            chains,
            rpc: RpcConfig {
                primary: vec![],
                fallback: vec![],
                max_connections: 10,
                request_timeout_ms: 5000,
                retry_count: 3,
            },
            strategy: StrategyConfig {
                min_profit_wei: 1_000_000_000_000_000, // 0.001 ETH
                min_profit_bps: 10,
                max_gas_price_gwei: 100,
                slippage_tolerance_bps: 50,
                enabled_strategies: vec![
                    "arbitrage".to_string(),
                    "backrun".to_string(),
                ],
            },
            performance: PerformanceConfig {
                detector_threads: num_cpus::get(),
                simulator_threads: num_cpus::get() / 2,
                max_pending_opportunities: 1000,
                simulation_timeout_ms: 100,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                json_output: true,
                file_path: Some("logs/mev-engine.log".to_string()),
            },
        }
    }
}

impl Config {
    /// Load config from environment
    pub fn from_env() -> anyhow::Result<Self> {
        // Try to load from file first
        let config_path = std::env::var("MEV_CONFIG")
            .unwrap_or_else(|_| "config/config.json".to_string());
        
        if std::path::Path::new(&config_path).exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Save config to file
    pub fn save(&self, path: &str) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
