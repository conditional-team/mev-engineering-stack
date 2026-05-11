# `fast/` — C / C++ Hot Path

SIMD-accelerated primitives and lock-free data structures for the MEV pipeline.
Linked into the Rust core (`mev-core`) via `cc-rs` build script as `libmev_fast.a`.

When the static library is unavailable (no C toolchain), the Rust side
transparently falls back to pure-Rust implementations through the
`ffi::hot_path::safe` module — the public API is identical and produces
byte-for-byte equivalent output.

---

## Modules

| File | Purpose |
|------|---------|
| `src/keccak.c` | Keccak-256 hashing (used for tx hashing, function selectors) |
| `src/rlp.c` | RLP encoding (string, uint256, address) — Ethereum yellow-paper compliant |
| `src/parser.c` | Swap calldata classifier (4-byte selector dispatch) |
| `src/simd_utils.c` | AVX2/SSE4.2 `memcmp`, address equality, batched price impact |
| `src/memory_pool.c` | Arena allocator for tx / calldata / result buffers |
| `src/lockfree_queue.c` | Bounded MPMC queue (Vyukov per-slot sequence) |
| `src/amm_simulator.cpp` | V2/V3 AMM math kernel with `__uint128_t` overflow protection |
| `src/pathfinder.cpp` | BFS multi-hop path optimizer over a SoA pool graph |

---

## Lock-Free Queue (`lockfree_queue.c`)

Implementation: **Dmitry Vyukov's bounded MPMC queue** (per-slot sequence ticket).

```
slot[i].sequence = i      // initialized "ready for producer i"
slot[i].data            // payload (uintptr_t)

push(item):
    pos = tail.load(relaxed)
    loop:
        slot = &slots[pos & mask]
        seq  = slot.sequence.load(acquire)
        diff = seq - pos
        if diff == 0:
            if CAS(tail, pos, pos+1):       // claim slot
                slot.data = item            // write payload
                slot.sequence.store(pos+1, release)   // publish
                return OK
        elif diff < 0: return FULL          // consumer hasn't drained the lapped slot
        else:          pos = tail.load(relaxed)  // someone else advanced

pop():
    pos = head.load(relaxed)
    loop:
        slot = &slots[pos & mask]
        seq  = slot.sequence.load(acquire)
        diff = seq - (pos + 1)
        if diff == 0:
            if CAS(head, pos, pos+1):       // claim
                item = slot.data
                slot.sequence.store(pos + capacity, release)  // open for next lap
                return item
        elif diff < 0: return EMPTY
        else:          pos = head.load(relaxed)
```

**Why Vyukov over a naive CAS-on-tail design:** the old approach (bump tail, then
write payload) leaves a window where the consumer can read a slot whose payload
hasn't been committed yet — producing zero/stale items. The per-slot sequence
ticket eliminates this race by gating consumer reads on the producer's
release-store of `slot.sequence`.

Reference: <http://www.1024cores.net/home/lock-free-algorithms/queues/bounded-mpmc-queue>

### Stress Test

`test/test_queue_stress.c` — N producers, 1 consumer, capacity 1024 (intentionally
small relative to volume to force the "queue full" branch and maximise interleaving).

Build & run (MSYS2 mingw64 / Linux gcc):

```bash
cd fast/test
gcc -O2 -pthread -Wall -Wextra -I../include \
    test_queue_stress.c ../src/lockfree_queue.c \
    -o test_queue_stress.exe

./test_queue_stress.exe 4 250000     # 4 producers x 250k items (1M total)
./test_queue_stress.exe 8 500000     # 8 producers x 500k items (4M total)
```

**Invariants checked on every run:**
1. `pushed == popped == n_producers * n_per_producer` (no loss, no duplicates)
2. Per-producer FIFO: sequence numbers tagged with producer ID arrive in order
3. No payload corruption (pointer tags round-trip intact)
4. No deadlock; only legitimate backpressure (`full_retries` counter)

**Reference results** (Intel i5-8250U @ 1.60GHz, MSYS2 mingw64 gcc 15.2.0 -O2):

| Producers | Items each | Total | Elapsed | Throughput |
|-----------|-----------:|------:|--------:|-----------:|
| 4 | 250,000 | 1M | 0.104 s | 9.64 Mops/s |
| 8 | 500,000 | 4M | 0.756 s | 5.29 Mops/s |

The 8-producer case is contention-bound (8 cores all CASing the same `tail`
counter); the 4-producer case is closer to the queue's intrinsic throughput
ceiling. Both pass all four invariants.

---

## Build

The Rust crate `mev-core` builds this directory automatically via `core/build.rs`
(`cc-rs`). Standalone build for bench/test purposes:

```bash
cd fast
make           # builds lib/libmev_fast.a + test_runner.exe
make test      # runs C unit tests
```

Required compiler features: C11 `<stdatomic.h>`, GCC/Clang `__uint128_t`.
On Windows, MSYS2 mingw64's gcc 15.x is the validated toolchain.
