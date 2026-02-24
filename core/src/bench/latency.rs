//! Precision benchmarks for sub-microsecond verification
//! Tests every critical hot path component

use std::time::Instant;
use std::hint::black_box;
use ethers::types::{U256, Address};

/// Benchmark result
#[derive(Debug, Clone)]
pub struct BenchResult {
    pub name: String,
    pub iterations: u64,
    pub total_ns: u64,
    pub avg_ns: f64,
    pub min_ns: u64,
    pub max_ns: u64,
    pub p50_ns: u64,
    pub p99_ns: u64,
    pub throughput_ops: f64,
}

impl std::fmt::Display for BenchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:<30} | avg: {:>8.2}ns | min: {:>6}ns | p50: {:>6}ns | p99: {:>6}ns | throughput: {:>12.0} ops/s",
            self.name, self.avg_ns, self.min_ns, self.p50_ns, self.p99_ns, self.throughput_ops
        )
    }
}

/// Run a benchmark with high precision
pub fn run_bench<F>(name: &str, iterations: u64, mut f: F) -> BenchResult
where
    F: FnMut(),
{
    // Warmup
    for _ in 0..1000 {
        black_box(f());
    }
    
    // Collect samples
    let mut samples = Vec::with_capacity(iterations as usize);
    
    for _ in 0..iterations {
        let start = Instant::now();
        black_box(f());
        let elapsed = start.elapsed().as_nanos() as u64;
        samples.push(elapsed);
    }
    
    // Calculate stats
    samples.sort_unstable();
    
    let total: u64 = samples.iter().sum();
    let avg = total as f64 / iterations as f64;
    let min = *samples.first().unwrap_or(&0);
    let max = *samples.last().unwrap_or(&0);
    let p50 = samples.get(samples.len() / 2).copied().unwrap_or(0);
    let p99 = samples.get(samples.len() * 99 / 100).copied().unwrap_or(0);
    let throughput = 1_000_000_000.0 / avg;
    
    BenchResult {
        name: name.to_string(),
        iterations,
        total_ns: total,
        avg_ns: avg,
        min_ns: min,
        max_ns: max,
        p50_ns: p50,
        p99_ns: p99,
        throughput_ops: throughput,
    }
}

/// Benchmark Rust Keccak256
pub fn bench_keccak256_rust() -> BenchResult {
    use tiny_keccak::{Keccak, Hasher};
    
    let input = vec![0u8; 32];
    
    run_bench("Rust Keccak256 (32 bytes)", 100_000, || {
        let mut hasher = Keccak::v256();
        let mut output = [0u8; 32];
        hasher.update(&input);
        hasher.finalize(&mut output);
        black_box(output);
    })
}

/// Benchmark SIMD memory compare (pure Rust fallback)
pub fn bench_memcmp() -> BenchResult {
    let a = [0x42u8; 64];
    let b = [0x42u8; 64];
    
    run_bench("memcmp (64 bytes)", 100_000, || {
        black_box(a == b);
    })
}

/// Benchmark address equality check
pub fn bench_address_eq() -> BenchResult {
    let addr1 = Address::repeat_byte(0x42);
    let addr2 = Address::repeat_byte(0x42);
    
    run_bench("Address equality", 100_000, || {
        black_box(addr1 == addr2);
    })
}

/// Benchmark U256 arithmetic
pub fn bench_u256_math() -> BenchResult {
    let a = U256::from(1_000_000_000_000_000_000u64);
    let b = U256::from(2_000_000_000_000_000_000u64);
    let c = U256::from(10000u64);
    
    run_bench("U256 mul/div", 100_000, || {
        let result = (a * b) / c;
        black_box(result);
    })
}

/// Benchmark swap simulation
pub fn bench_swap_simulation() -> BenchResult {
    let reserve0 = U256::from(1_000_000_000_000_000_000_000u128); // 1000 ETH
    let reserve1 = U256::from(2_000_000_000_000_000_000_000_000u128); // 2M tokens
    let amount_in = U256::from(1_000_000_000_000_000_000u64); // 1 ETH
    let fee = 30u32;
    
    run_bench("Swap simulation (constant product)", 100_000, || {
        let fee_factor = 10000 - fee;
        let amount_with_fee = amount_in * U256::from(fee_factor);
        let numerator = amount_with_fee * reserve1;
        let denominator = reserve0 * U256::from(10000) + amount_with_fee;
        let amount_out = numerator / denominator;
        black_box(amount_out);
    })
}

/// Benchmark hash lookup (simulated pool lookup)
pub fn bench_hashmap_lookup() -> BenchResult {
    use dashmap::DashMap;
    
    let map: DashMap<Address, u64> = DashMap::new();
    
    // Pre-populate
    for i in 0..10_000u64 {
        let addr = Address::from_low_u64_be(i);
        map.insert(addr, i);
    }
    
    let lookup_addr = Address::from_low_u64_be(5000);
    
    run_bench("DashMap lookup", 100_000, || {
        black_box(map.get(&lookup_addr));
    })
}

/// Benchmark channel send/recv
pub fn bench_channel() -> BenchResult {
    use crossbeam_channel::bounded;
    
    let (tx, rx) = bounded::<u64>(1024);
    
    run_bench("Crossbeam channel send+recv", 100_000, || {
        tx.send(42).ok();
        black_box(rx.recv().ok());
    })
}

/// Benchmark TSC read (CPU timestamp)
pub fn bench_rdtsc() -> BenchResult {
    run_bench("RDTSC read", 100_000, || {
        #[cfg(target_arch = "x86_64")]
        {
            black_box(unsafe { std::arch::x86_64::_rdtsc() });
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            black_box(std::time::Instant::now());
        }
    })
}

/// Run all benchmarks
pub fn run_all_benchmarks() -> Vec<BenchResult> {
    println!("\n╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                    MEV PROTOCOL - LATENCY BENCHMARKS                         ║");
    println!("╠══════════════════════════════════════════════════════════════════════════════╣");
    
    let results = vec![
        // Crypto
        bench_keccak256_rust(),
        
        // Memory
        bench_memcmp(),
        bench_address_eq(),
        
        // Math
        bench_u256_math(),
        bench_swap_simulation(),
        
        // Data structures
        bench_hashmap_lookup(),
        bench_channel(),
        
        // System
        bench_rdtsc(),
    ];
    
    for r in &results {
        println!("║ {} ║", r);
    }
    
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");
    
    // Summary
    println!("\n📊 LATENCY SUMMARY:");
    let critical_ops = ["Keccak256", "Swap simulation", "DashMap lookup"];
    let total_critical: f64 = results.iter()
        .filter(|r| critical_ops.iter().any(|op| r.name.contains(op)))
        .map(|r| r.avg_ns)
        .sum();
    
    println!("   Critical path total: {:.2}ns ({:.2}µs)", total_critical, total_critical / 1000.0);
    
    if total_critical < 1000.0 {
        println!("   ✅ SUB-MICROSECOND ACHIEVED!");
    } else if total_critical < 10000.0 {
        println!("   ⚠️  Under 10µs - good but can improve");
    } else {
        println!("   ❌ Over 10µs - needs optimization");
    }
    
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bench_framework() {
        let result = run_bench("test_noop", 1000, || {
            black_box(42);
        });
        
        assert!(result.avg_ns < 1000.0); // Should be very fast
        assert!(result.min_ns <= result.p50_ns);
        assert!(result.p50_ns <= result.p99_ns);
    }
}
