# Memory & Data Management

- **Zero-copy & DMA:** Prefer zero-copy access and Direct Memory Access (DMA)
  where the platform supports it. Avoid copying buffers between processing stages.

- **No dynamic allocation in hot paths:** Pre-allocate all buffers outside the
  processing callback. Never `Box`, `Vec::push`, `String`, or otherwise allocate
  inside the hot path.

- **Cache-line alignment:** Align hot buffers/structs to the cache-line size
  (commonly 64 bytes) to avoid false sharing and maximize fetch efficiency
  (`#[repr(align(64))]`).

- **Struct layout — SoA vs AoS:** Prefer **Structure-of-Arrays (SoA)** over
  Array-of-Structures (AoS) for multichannel/vectorizable data — it makes access
  contiguous and SIMD-friendly. Use `#[repr(C)]` when layout must be stable
  (FFI, DMA, explicit alignment reasoning).

- **Circular buffers:** Use ring buffers where applicable. Choose **power-of-two
  sizes** so the modulo wrap becomes a fast bitwise `& (len - 1)` instead of a
  slow `%`.
  - **Context caveat:** virtual-memory-mapping ring-buffer tricks (e.g. mapping
    the same pages twice for seamless wraparound) are **not usable under
    `#![no_std]`/bare-metal**. Use a plain index-and-mask ring buffer in those
    environments.

- **Slice iterators over indexing:** Prefer `.iter()` / `.iter_mut()` /
  `chunks_exact()` / `windows()` over manual indexing. By *not indexing*, you
  avoid per-access bounds checks, and `chunks_exact` in particular enables better
  autovectorization than `chunks`.
  - Note: this works because iterators don't index — manual indexing with a
    separate counter is **not** reliably optimized the same way.
