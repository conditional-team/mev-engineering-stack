#ifndef MEV_SIMD_UTILS_H
#define MEV_SIMD_UTILS_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// Fast memory operations
int mev_memcmp_fast(const void* a, const void* b, size_t len);
void mev_memcpy_nt(void* dst, const void* src, size_t len);
void mev_xor_block_256(uint8_t* dst, const uint8_t* src);

// Hex encoding
int mev_hex_decode_fast(const char* hex, size_t hex_len, uint8_t* out);

// Address operations
int mev_address_eq(const uint8_t* a, const uint8_t* b);
int mev_address_find(const uint8_t addresses[][20], size_t count, const uint8_t* target);

// Batch price calculations
void mev_calc_price_impact_batch(
    const uint64_t reserves0[4],
    const uint64_t reserves1[4],
    uint64_t amount_in,
    uint64_t outputs[4]
);

// Prefetch and timing
void mev_prefetch_pool(const void* pool_data);
uint64_t mev_rdtsc(void);
void mev_cpu_pause(void);

#ifdef __cplusplus
}
#endif

#endif // MEV_SIMD_UTILS_H
