/**
 * Lock-Free MPSC Queue for Opportunity Pipeline
 * Multiple producers (detectors), single consumer (executor)
 *
 * Implementation: Dmitry Vyukov's bounded MPMC queue (per-slot sequence counter).
 * Safe for multi-producer / multi-consumer; we only need MPSC but the algorithm
 * costs nothing extra and removes the classic "claim-then-write" race that a
 * naive CAS-on-tail design exhibits, where a producer that has bumped `tail`
 * but not yet stored its payload causes the consumer to read a stale/zero slot.
 *
 * Reference: http://www.1024cores.net/home/lock-free-algorithms/queues/bounded-mpmc-queue
 */

#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <stdalign.h>
#include <stdatomic.h>

#ifdef _WIN32
#include <malloc.h>
#define aligned_alloc(align, size) _aligned_malloc(size, align)
#define aligned_free(ptr) _aligned_free(ptr)
#else
#define aligned_free(ptr) free(ptr)
#endif

#define QUEUE_CAPACITY 4096  // Must be power of 2

typedef struct {
    atomic_size_t     sequence;  // Vyukov ticket: marks who may write/read this slot
    atomic_uintptr_t  data;
} queue_slot_t;

typedef struct {
    queue_slot_t* slots;
    size_t capacity;
    size_t mask;

    // Aligned to separate cache lines to avoid false sharing between
    // the consumer-owned head and the producer-owned tail.
    alignas(64) atomic_size_t head;  // Consumer cursor
    alignas(64) atomic_size_t tail;  // Producer cursor
} mev_queue_t;

/**
 * Create a new queue
 */
mev_queue_t* mev_queue_create(size_t capacity) {
    if (capacity & (capacity - 1)) {
        // Not power of 2, round up
        size_t n = 1;
        while (n < capacity) n <<= 1;
        capacity = n;
    }

    mev_queue_t* q = (mev_queue_t*)aligned_alloc(64, sizeof(mev_queue_t));
    if (!q) return NULL;

    q->slots = (queue_slot_t*)calloc(capacity, sizeof(queue_slot_t));
    if (!q->slots) {
        free(q);
        return NULL;
    }

    q->capacity = capacity;
    q->mask = capacity - 1;
    atomic_store(&q->head, 0);
    atomic_store(&q->tail, 0);

    // Vyukov initialization: slot[i].sequence = i means "ready to be written
    // by the producer holding ticket i". Without this initialization the
    // queue would refuse all pushes because diff would be non-zero.
    for (size_t i = 0; i < capacity; i++) {
        atomic_store_explicit(&q->slots[i].sequence, i, memory_order_relaxed);
        atomic_store_explicit(&q->slots[i].data, 0, memory_order_relaxed);
    }
    atomic_thread_fence(memory_order_release);

    return q;
}

/**
 * Destroy queue
 */
void mev_queue_destroy(mev_queue_t* q) {
    if (q) {
        free(q->slots);
        aligned_free(q);
    }
}

/**
 * Push item to queue (multi-producer safe, lock-free)
 * Returns 0 on success, -1 if full.
 *
 * Algorithm:
 *   1. Read tail, look at slot[tail].sequence.
 *   2. diff = sequence - tail
 *      diff == 0 → slot is ours to claim, CAS-bump tail and write.
 *      diff <  0 → consumer hasn't drained the lapped slot yet → queue full.
 *      diff >  0 → another producer already advanced tail, reload and retry.
 *   3. After writing data, publish by setting slot.sequence = tail + 1 (release).
 *      The consumer spins on this exact value to know the payload is committed.
 */
int mev_queue_push(mev_queue_t* q, void* item) {
    size_t pos = atomic_load_explicit(&q->tail, memory_order_relaxed);

    for (;;) {
        queue_slot_t* slot = &q->slots[pos & q->mask];
        size_t seq = atomic_load_explicit(&slot->sequence, memory_order_acquire);
        intptr_t diff = (intptr_t)seq - (intptr_t)pos;

        if (diff == 0) {
            if (atomic_compare_exchange_weak_explicit(
                    &q->tail, &pos, pos + 1,
                    memory_order_relaxed, memory_order_relaxed)) {
                // Slot reserved. Write payload, then release-publish via sequence.
                atomic_store_explicit(&slot->data, (uintptr_t)item, memory_order_relaxed);
                atomic_store_explicit(&slot->sequence, pos + 1, memory_order_release);
                return 0;
            }
            // CAS lost the race; pos already updated, retry.
        } else if (diff < 0) {
            return -1; // Full: lap in progress, consumer is behind.
        } else {
            // Another producer claimed this position; reload tail and retry.
            pos = atomic_load_explicit(&q->tail, memory_order_relaxed);
        }
    }
}

/**
 * Pop item from queue.
 *
 * Algorithm mirrors push:
 *   diff = sequence - (head + 1)
 *     == 0 → producer has committed this slot, claim it.
 *     <  0 → empty (no producer has filled this slot yet).
 *     >  0 → another consumer beat us (only possible under MPMC); reload.
 *
 * After reading, we mark the slot ready for the next lap by setting
 * sequence = head + capacity. This is what producers wait on when the queue
 * has wrapped around.
 *
 * Although this project uses a single consumer, the MPMC-shaped pop is the
 * same cost and keeps the implementation safe if a second consumer is ever
 * added.
 */
void* mev_queue_pop(mev_queue_t* q) {
    size_t pos = atomic_load_explicit(&q->head, memory_order_relaxed);

    for (;;) {
        queue_slot_t* slot = &q->slots[pos & q->mask];
        size_t seq = atomic_load_explicit(&slot->sequence, memory_order_acquire);
        intptr_t diff = (intptr_t)seq - (intptr_t)(pos + 1);

        if (diff == 0) {
            if (atomic_compare_exchange_weak_explicit(
                    &q->head, &pos, pos + 1,
                    memory_order_relaxed, memory_order_relaxed)) {
                void* item = (void*)atomic_load_explicit(&slot->data, memory_order_relaxed);
                // Release the slot for reuse on the next lap.
                atomic_store_explicit(&slot->sequence, pos + q->capacity, memory_order_release);
                return item;
            }
        } else if (diff < 0) {
            return NULL; // Empty: producer hasn't published this slot yet.
        } else {
            pos = atomic_load_explicit(&q->head, memory_order_relaxed);
        }
    }
}

/**
 * Try to pop without blocking
 */
void* mev_queue_try_pop(mev_queue_t* q) {
    return mev_queue_pop(q);
}

/**
 * Get queue size (approximate snapshot — racy by definition under concurrent producers)
 */
size_t mev_queue_size(mev_queue_t* q) {
    size_t tail = atomic_load_explicit(&q->tail, memory_order_acquire);
    size_t head = atomic_load_explicit(&q->head, memory_order_acquire);
    return tail - head;
}

/**
 * Check if empty
 */
int mev_queue_empty(mev_queue_t* q) {
    return mev_queue_size(q) == 0;
}

/**
 * Batch pop for efficiency
 */
size_t mev_queue_pop_batch(mev_queue_t* q, void** items, size_t max_items) {
    size_t count = 0;

    while (count < max_items) {
        void* item = mev_queue_pop(q);
        if (!item) break;
        items[count++] = item;
    }

    return count;
}
