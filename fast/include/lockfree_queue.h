#ifndef MEV_LOCKFREE_QUEUE_H
#define MEV_LOCKFREE_QUEUE_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct mev_queue_t mev_queue_t;

// Create/destroy
mev_queue_t* mev_queue_create(size_t capacity);
void mev_queue_destroy(mev_queue_t* q);

// Push/pop
int mev_queue_push(mev_queue_t* q, void* item);
void* mev_queue_pop(mev_queue_t* q);
void* mev_queue_try_pop(mev_queue_t* q);

// Batch operations
size_t mev_queue_pop_batch(mev_queue_t* q, void** items, size_t max_items);

// Status
size_t mev_queue_size(mev_queue_t* q);
int mev_queue_empty(mev_queue_t* q);

#ifdef __cplusplus
}
#endif

#endif // MEV_LOCKFREE_QUEUE_H
