//! MEV Engine CLI
//! Sub-microsecond latency optimized

use mev_core::{Config, MevEngine, MempoolConfig, MempoolMonitor};
use mev_core::detector::{MultiThreadedDetector, DetectorConfig};
use mev_core::ffi::rdtsc_native;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use tokio::sync::mpsc;
use std::env;

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)?;

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║          MEV PROTOCOL ENGINE v0.1.0 - ARBITRUM                ║");
    println!("║          Sub-microsecond latency | Flash Loan Arbitrage       ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // Check CLI args
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 && args[1] == "benchmark" {
        info!("Running latency benchmarks...");
        mev_core::run_all_benchmarks();
        return Ok(());
    }

    // Load configuration
    let config = Config::from_env()?;
    
    // Get Arbitrum chain config (chain_id 42161)
    let arbitrum_config = config.chains.get(&42161)
        .expect("Arbitrum config not found! Add chain 42161 to config.");
    
    info!("✅ Configuration loaded");
    info!("   Chain ID: {}", arbitrum_config.chain_id);
    info!("   Contract: {:?}", arbitrum_config.contract_address);

    // Verify FFI is working
    let tsc_start = rdtsc_native();
    let tsc_end = rdtsc_native();
    info!("✅ FFI verified | TSC delta: {} cycles", tsc_end - tsc_start);

    // Create mempool monitor
    let mempool_config = MempoolConfig {
        ws_url: arbitrum_config.ws_url.clone(),
        backup_ws_urls: vec![],
        max_pending_txs: 10_000,
        cpu_core: Some(0),
        batch_size: 32,
    };
    let mempool_monitor = MempoolMonitor::new(mempool_config);
    info!("✅ Mempool monitor initialized");

    // Create multi-threaded detector
    let detector_config = DetectorConfig {
        num_workers: 4,
        min_profit_wei: ethers::types::U256::from(500_000_000_000_000u64), // 0.0005 ETH
        max_hops: 3,
        gas_price: ethers::types::U256::from(100_000_000), // 0.1 gwei
        batch_size: 64,
    };
    let _detector = MultiThreadedDetector::new(detector_config);
    info!("✅ Multi-threaded detector initialized (4 workers)");

    // Create engine
    let engine = MevEngine::new(config);

    // Handle shutdown
    let engine_clone = engine.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("🛑 Shutdown signal received");
        engine_clone.stop().await.ok();
    });

    // Start mempool monitoring
    let (tx_sender, mut tx_receiver) = mpsc::unbounded_channel();
    
    let monitor = mempool_monitor;
    tokio::spawn(async move {
        if let Err(e) = monitor.start(tx_sender).await {
            warn!("Mempool monitor error: {}", e);
        }
    });

    info!("\n🚀 MEV ENGINE STARTED");
    info!("   Monitoring mempool for opportunities...\n");

    // Main loop - process transactions
    let mut tx_count = 0u64;
    let mut last_log = std::time::Instant::now();
    
    loop {
        tokio::select! {
            Some(tx) = tx_receiver.recv() => {
                tx_count += 1;
                
                // Log stats every 10 seconds
                if last_log.elapsed() > std::time::Duration::from_secs(10) {
                    info!("📊 Stats | TXs: {} | Rate: {:.1}/s", 
                          tx_count, 
                          tx_count as f64 / last_log.elapsed().as_secs_f64());
                    tx_count = 0;
                    last_log = std::time::Instant::now();
                }
                
                // Check if it's a swap
                if tx.is_swap {
                    info!("🔄 Swap detected: {:?}", tx.hash);
                    // Detector will process this
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("🛑 Shutting down...");
                break;
            }
        }
    }

    info!("✅ MEV Engine shutdown complete");
    Ok(())
}
