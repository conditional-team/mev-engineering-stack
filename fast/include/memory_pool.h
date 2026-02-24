#ifndef MEV_MEMORY_POOL_H
#define MEV_MEMORY_POOL_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// Initialize all pools - call once at startup
int mev_pools_init(void);

// Transaction buffer pool (512 bytes each)
void* mev_alloc_tx(void);
void mev_free_tx(void* ptr);

// Calldata buffer pool (2KB each)
void* mev_alloc_calldata(void);
void mev_free_calldata(void* ptr);

// Result buffer pool (256 bytes each)
void* mev_alloc_result(void);
void mev_free_result(void* ptr);

// Batch operations
int mev_alloc_batch(void** ptrs, size_t count, size_t size);
void mev_free_batch(void** ptrs, size_t count, size_t size);

// Stats
void mev_pool_stats(size_t* tx_avail, size_t* calldata_avail, size_t* result_avail);

#ifdef __cplusplus
}
#endif

#endif // MEV_MEMORY_POOL_H
