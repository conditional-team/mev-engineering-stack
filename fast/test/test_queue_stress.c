/**
 * Lock-free queue stress test — N producers, 1 consumer.
 *
 * Invariants checked:
 *   1. Push count == pop count (no lost or duplicated items).
 *   2. Per-producer FIFO ordering (sequence numbers tagged with producer ID
 *      come back in monotonically increasing order for that producer).
 *   3. No payload corruption (pointer round-trips intact).
 *   4. No deadlock / no spurious "queue full" beyond legitimate backpressure.
 *
 * Build (MSYS2 mingw64):
 *   gcc -O2 -pthread -Wall -Wextra -I../include \
 *       test_queue_stress.c ../src/lockfree_queue.c \
 *       -o test_queue_stress.exe
 *
 * Run:
 *   ./test_queue_stress.exe        # default: 4 producers x 250000 items
 *   ./test_queue_stress.exe 8 1000000
 */

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <pthread.h>
#include <time.h>

#include "../include/lockfree_queue.h"

/* Encode (producer_id, sequence) into a single 64-bit pointer payload.
 * High 16 bits = producer id, low 48 bits = sequence. */
#define ENCODE(pid, seq) ((void*)(((uint64_t)(pid) << 48) | ((uint64_t)(seq) & 0xFFFFFFFFFFFFULL)))
#define DECODE_PID(p)    ((uint16_t)((uintptr_t)(p) >> 48))
#define DECODE_SEQ(p)    ((uint64_t)((uintptr_t)(p) & 0xFFFFFFFFFFFFULL))

typedef struct {
    mev_queue_t* q;
    uint16_t     pid;
    uint64_t     n_items;
    uint64_t     pushed;     /* out */
    uint64_t     full_retries; /* out */
} producer_args_t;

static void* producer_thread(void* arg) {
    producer_args_t* a = (producer_args_t*)arg;
    for (uint64_t seq = 1; seq <= a->n_items; seq++) {
        void* item = ENCODE(a->pid, seq);
        /* Spin on full queue (legitimate backpressure). */
        while (mev_queue_push(a->q, item) != 0) {
            a->full_retries++;
            /* Yield to consumer. */
            struct timespec ts = {0, 1000}; /* 1us */
            nanosleep(&ts, NULL);
        }
        a->pushed++;
    }
    return NULL;
}

int main(int argc, char** argv) {
    int n_producers = (argc > 1) ? atoi(argv[1]) : 4;
    uint64_t n_per_producer = (argc > 2) ? strtoull(argv[2], NULL, 10) : 250000ULL;

    if (n_producers < 1 || n_producers > 64) {
        fprintf(stderr, "n_producers must be in [1, 64]\n");
        return 1;
    }
    if (n_producers > (1 << 16)) {
        fprintf(stderr, "n_producers exceeds 16-bit pid encoding\n");
        return 1;
    }

    printf("Stress test: %d producers x %llu items each (total %llu)\n",
           n_producers,
           (unsigned long long)n_per_producer,
           (unsigned long long)(n_producers * n_per_producer));

    /* Capacity intentionally small relative to throughput to exercise the
     * "queue full" branch and force producer/consumer interleaving. */
    mev_queue_t* q = mev_queue_create(1024);
    if (!q) {
        fprintf(stderr, "queue_create failed\n");
        return 1;
    }

    producer_args_t* args = calloc(n_producers, sizeof(*args));
    pthread_t* threads = calloc(n_producers, sizeof(*threads));
    if (!args || !threads) {
        fprintf(stderr, "alloc failed\n");
        return 1;
    }

    /* Per-producer last-seen sequence (must be monotonically increasing). */
    uint64_t* last_seq = calloc(n_producers, sizeof(*last_seq));
    /* Per-producer total received (must equal n_per_producer at end). */
    uint64_t* received = calloc(n_producers, sizeof(*received));
    if (!last_seq || !received) {
        fprintf(stderr, "alloc failed\n");
        return 1;
    }

    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);

    /* Spawn producers. */
    for (int i = 0; i < n_producers; i++) {
        args[i].q = q;
        args[i].pid = (uint16_t)(i + 1);  /* avoid pid=0 (matches NULL/empty) */
        args[i].n_items = n_per_producer;
        if (pthread_create(&threads[i], NULL, producer_thread, &args[i]) != 0) {
            fprintf(stderr, "pthread_create failed\n");
            return 1;
        }
    }

    /* Single consumer (this thread). */
    uint64_t total_expected = (uint64_t)n_producers * n_per_producer;
    uint64_t total_popped = 0;
    uint64_t empty_spins = 0;
    int producers_done = 0;

    while (total_popped < total_expected) {
        void* item = mev_queue_try_pop(q);
        if (!item) {
            /* Check whether all producers have finished pushing. */
            if (!producers_done) {
                int still_running = 0;
                for (int i = 0; i < n_producers; i++) {
                    if (args[i].pushed < args[i].n_items) { still_running = 1; break; }
                }
                if (!still_running) producers_done = 1;
            }
            if (producers_done && mev_queue_empty(q)) {
                break;
            }
            empty_spins++;
            continue;
        }
        uint16_t pid = DECODE_PID(item);
        uint64_t seq = DECODE_SEQ(item);

        if (pid == 0 || pid > n_producers) {
            fprintf(stderr, "FAIL: corrupted pid=%u seq=%llu (item=%p)\n",
                    pid, (unsigned long long)seq, item);
            return 1;
        }
        int idx = pid - 1;
        if (seq != last_seq[idx] + 1) {
            fprintf(stderr,
                    "FAIL: producer %u FIFO violation: expected seq %llu, got %llu\n",
                    pid,
                    (unsigned long long)(last_seq[idx] + 1),
                    (unsigned long long)seq);
            return 1;
        }
        last_seq[idx] = seq;
        received[idx]++;
        total_popped++;
    }

    /* Join producers. */
    for (int i = 0; i < n_producers; i++) {
        pthread_join(threads[i], NULL);
    }

    /* Drain anything that arrived after the producers_done check. */
    void* leftover;
    while ((leftover = mev_queue_try_pop(q)) != NULL) {
        uint16_t pid = DECODE_PID(leftover);
        uint64_t seq = DECODE_SEQ(leftover);
        if (pid == 0 || pid > n_producers) {
            fprintf(stderr, "FAIL: corrupted leftover pid=%u\n", pid);
            return 1;
        }
        int idx = pid - 1;
        if (seq != last_seq[idx] + 1) {
            fprintf(stderr, "FAIL: leftover FIFO violation pid=%u\n", pid);
            return 1;
        }
        last_seq[idx] = seq;
        received[idx]++;
        total_popped++;
    }

    clock_gettime(CLOCK_MONOTONIC, &t1);
    double elapsed = (t1.tv_sec - t0.tv_sec) + (t1.tv_nsec - t0.tv_nsec) / 1e9;

    /* Final assertions. */
    int ok = 1;

    if (total_popped != total_expected) {
        fprintf(stderr, "FAIL: popped %llu != expected %llu\n",
                (unsigned long long)total_popped,
                (unsigned long long)total_expected);
        ok = 0;
    }

    uint64_t total_pushed = 0;
    uint64_t total_full_retries = 0;
    for (int i = 0; i < n_producers; i++) {
        total_pushed += args[i].pushed;
        total_full_retries += args[i].full_retries;
        if (received[i] != n_per_producer) {
            fprintf(stderr, "FAIL: producer %d pushed %llu, consumer received %llu\n",
                    i + 1,
                    (unsigned long long)args[i].pushed,
                    (unsigned long long)received[i]);
            ok = 0;
        }
        if (last_seq[i] != n_per_producer) {
            fprintf(stderr, "FAIL: producer %d last_seq=%llu, expected %llu\n",
                    i + 1,
                    (unsigned long long)last_seq[i],
                    (unsigned long long)n_per_producer);
            ok = 0;
        }
    }

    if (total_pushed != total_expected) {
        fprintf(stderr, "FAIL: total pushed %llu != expected %llu\n",
                (unsigned long long)total_pushed,
                (unsigned long long)total_expected);
        ok = 0;
    }

    if (!mev_queue_empty(q)) {
        fprintf(stderr, "FAIL: queue not empty at end (size=%zu)\n", mev_queue_size(q));
        ok = 0;
    }

    mev_queue_destroy(q);
    free(args);
    free(threads);
    free(last_seq);
    free(received);

    if (!ok) {
        printf("\n=== STRESS TEST FAILED ===\n");
        return 1;
    }

    double mops = (double)total_popped / elapsed / 1e6;
    printf("\n=== STRESS TEST PASSED ===\n");
    printf("  total items   : %llu\n", (unsigned long long)total_popped);
    printf("  elapsed       : %.3f s\n", elapsed);
    printf("  throughput    : %.2f Mops/s\n", mops);
    printf("  full retries  : %llu (producer backpressure)\n",
           (unsigned long long)total_full_retries);
    printf("  empty spins   : %llu (consumer waiting)\n",
           (unsigned long long)empty_spins);
    return 0;
}
