//! gRPC service implementation for MevEngine
//!
//! Bridges the Go network layer to the Rust MEV detection pipeline.
//! Each incoming ClassifiedTransaction is: decoded → detected → simulated → bundled.

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, mpsc};
use tonic::{Request, Response, Status};
use tracing::{info, debug, warn};

use super::mev;
use super::mev::mev_engine_server::MevEngine;
use crate::config::Config;
use crate::detector::OpportunityDetector;
use crate::simulator::EvmSimulator;
use crate::builder::BundleBuilder;
use crate::types::{PendingTx, OpportunityType};

/// Capacity for the broadcast channel used by StreamOpportunities.
const BROADCAST_CAPACITY: usize = 256;

/// gRPC server wrapping the full MEV pipeline
pub struct MevGrpcServer {
    detector: Arc<OpportunityDetector>,
    simulator: Arc<EvmSimulator>,
    builder: Arc<BundleBuilder>,
    start_time: Instant,
    /// Broadcast sender — every successful detection is published here
    /// so that all StreamOpportunities subscribers receive it.
    opportunity_tx: broadcast::Sender<mev::DetectionResult>,
}

impl MevGrpcServer {
    pub fn new(config: Arc<Config>) -> Self {
        let (opportunity_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let mut builder = BundleBuilder::new(config.clone());

        if let Some(contract_address) = config
            .chains
            .get(&42161)
            .and_then(|chain| chain.contract_address.clone())
        {
            builder.set_contract(contract_address);
        } else {
            warn!("No contract address configured for chain 42161 — bundle building may fail");
        }

        Self {
            detector: Arc::new(OpportunityDetector::new(config.clone())),
            simulator: Arc::new(EvmSimulator::new(config.clone())),
            builder: Arc::new(builder),
            start_time: Instant::now(),
            opportunity_tx,
        }
    }

    /// Convert proto ClassifiedTransaction to internal PendingTx
    fn decode_tx(proto_tx: &mev::ClassifiedTransaction) -> Result<PendingTx, Status> {
        let hash: [u8; 32] = proto_tx.tx_hash.as_slice()
            .try_into()
            .map_err(|_| Status::invalid_argument("tx_hash must be 32 bytes"))?;

        let from: [u8; 20] = proto_tx.from.as_slice()
            .try_into()
            .map_err(|_| Status::invalid_argument("from must be 20 bytes"))?;

        let to: Option<[u8; 20]> = if proto_tx.to.len() == 20 {
            Some(proto_tx.to.as_slice().try_into().unwrap())
        } else {
            None
        };

        // Decode value from big-endian bytes (up to u128)
        let value = bytes_to_u128(&proto_tx.value);

        Ok(PendingTx {
            hash,
            from,
            to,
            value,
            gas_price: proto_tx.gas_price as u128,
            gas_limit: proto_tx.gas_limit,
            input: proto_tx.input.clone(),
            nonce: proto_tx.nonce,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }

    /// Convert internal Opportunity to proto Opportunity
    fn encode_opportunity(opp: &crate::types::Opportunity) -> mev::Opportunity {
        mev::Opportunity {
            r#type: match opp.opportunity_type {
                OpportunityType::Arbitrage   => mev::OpportunityType::Arbitrage as i32,
                OpportunityType::Backrun     => mev::OpportunityType::Backrun as i32,
                OpportunityType::Liquidation => mev::OpportunityType::LiquidationOpp as i32,
            },
            token_in: opp.token_in.clone(),
            token_out: opp.token_out.clone(),
            amount_in: opp.amount_in.to_be_bytes().to_vec(),
            expected_profit: opp.expected_profit.to_be_bytes().to_vec(),
            gas_estimate: opp.gas_estimate,
            bundle: None, // Built separately if simulation passes
        }
    }
}

#[tonic::async_trait]
impl MevEngine for MevGrpcServer {
    /// Process a single classified transaction through the full MEV pipeline
    async fn detect_opportunity(
        &self,
        request: Request<mev::ClassifiedTransaction>,
    ) -> Result<Response<mev::DetectionResult>, Status> {
        let start = Instant::now();
        let proto_tx = request.into_inner();

        let pending_tx = Self::decode_tx(&proto_tx)?;

        // Run detector
        let opportunities = self.detector.process_tx(pending_tx).await;

        if opportunities.is_empty() {
            return Ok(Response::new(mev::DetectionResult {
                found: false,
                opportunities: vec![],
                detection_latency_ns: start.elapsed().as_nanos() as u64,
            }));
        }

        // Simulate + build bundles for each opportunity
        let mut proto_opps = Vec::with_capacity(opportunities.len());

        for opp in &opportunities {
            let sim = self.simulator.simulate(opp).await;
            if !sim.success || sim.profit <= 0 {
                debug!(kind = ?opp.opportunity_type, "Simulation rejected opportunity");
                continue;
            }

            let mut proto_opp = Self::encode_opportunity(opp);

            // Build bundle
            match self.builder.build(opp).await {
                Ok(bundle) => {
                    proto_opp.bundle = Some(mev::Bundle {
                        transactions: bundle.transactions.iter().map(|tx| {
                            mev::BundleTx {
                                to: hex::decode(tx.to.trim_start_matches("0x"))
                                    .unwrap_or_default(),
                                value: tx.value.to_be_bytes().to_vec(),
                                gas_limit: tx.gas_limit,
                                max_fee_per_gas: tx.max_fee_per_gas
                                    .map(|v| v.to_be_bytes().to_vec())
                                    .unwrap_or_default(),
                                max_priority_fee_per_gas: tx.max_priority_fee_per_gas
                                    .map(|v| v.to_be_bytes().to_vec())
                                    .unwrap_or_default(),
                                data: tx.data.clone(),
                            }
                        }).collect(),
                        target_block: bundle.target_block.unwrap_or(proto_tx.target_block),
                    });
                }
                Err(e) => {
                    warn!(err = %e, "Bundle build failed");
                }
            }

            proto_opps.push(proto_opp);
        }

        let result = mev::DetectionResult {
            found: !proto_opps.is_empty(),
            opportunities: proto_opps,
            detection_latency_ns: start.elapsed().as_nanos() as u64,
        };

        // Broadcast to StreamOpportunities subscribers (best-effort)
        if result.found {
            let _ = self.opportunity_tx.send(result.clone());
        }

        Ok(Response::new(result))
    }

    /// Stream — subscribe to all detected opportunities in real time.
    /// Each subscriber receives every opportunity that passes simulation.
    type StreamOpportunitiesStream = tokio_stream::wrappers::ReceiverStream<Result<mev::DetectionResult, Status>>;

    async fn stream_opportunities(
        &self,
        request: Request<mev::StreamRequest>,
    ) -> Result<Response<Self::StreamOpportunitiesStream>, Status> {
        let min_profit = request.into_inner().min_profit;
        let min_profit_u128 = if min_profit.is_empty() {
            0u128
        } else {
            super::server::bytes_to_u128(&min_profit)
        };

        let (tx, rx) = mpsc::channel(128);
        let mut broadcast_rx = self.opportunity_tx.subscribe();

        // Spawn a task that forwards broadcast messages to this subscriber's stream
        tokio::spawn(async move {
            loop {
                match broadcast_rx.recv().await {
                    Ok(detection_result) => {
                        // Apply optional min_profit filter
                        if min_profit_u128 > 0 {
                            let passes = detection_result.opportunities.iter().any(|opp| {
                                let profit = bytes_to_u128(&opp.expected_profit);
                                profit >= min_profit_u128
                            });
                            if !passes {
                                continue;
                            }
                        }

                        if tx.send(Ok(detection_result)).await.is_err() {
                            // Subscriber disconnected
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "StreamOpportunities subscriber lagged");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }

    /// Return engine status and counters
    async fn get_status(
        &self,
        _request: Request<mev::StatusRequest>,
    ) -> Result<Response<mev::StatusResponse>, Status> {
        Ok(Response::new(mev::StatusResponse {
            running: true,
            opportunities_detected: self.detector.count().await,
            simulations_run: self.simulator.count().await,
            bundles_built: self.builder.count().await,
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }))
    }
}

/// Convert big-endian byte slice to u128 (clamped to 16 bytes)
fn bytes_to_u128(b: &[u8]) -> u128 {
    if b.is_empty() {
        return 0;
    }
    let start = if b.len() > 16 { b.len() - 16 } else { 0 };
    let slice = &b[start..];
    let mut buf = [0u8; 16];
    buf[16 - slice.len()..].copy_from_slice(slice);
    u128::from_be_bytes(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_u128_empty() {
        assert_eq!(bytes_to_u128(&[]), 0);
    }

    #[test]
    fn test_bytes_to_u128_one_eth() {
        // 1 ETH = 1e18 = 0x0DE0B6B3A7640000
        let bytes = vec![0x0D, 0xE0, 0xB6, 0xB3, 0xA7, 0x64, 0x00, 0x00];
        assert_eq!(bytes_to_u128(&bytes), 1_000_000_000_000_000_000);
    }

    #[test]
    fn test_bytes_to_u128_32_byte_input() {
        // Simulate a uint256 where only the low 16 bytes matter
        let mut bytes = vec![0u8; 32];
        bytes[31] = 42;
        assert_eq!(bytes_to_u128(&bytes), 42);
    }
}
