# Concurrency & Control Integration

Real-time processing almost always runs alongside other threads (control/UI,
networking, disk, background analysis). The hot path must keep running without
ever blocking, while still receiving new data and parameters. This file covers how
to connect the real-time thread to the rest of the program. The hot-path
prohibitions themselves are in `31_realtime_determinism.md`.

## The anchoring invariant

The real-time thread must never **block, allocate, or lock** (see
`31_realtime_determinism.md`). Every interaction across the boundary must respect
this, so cross-thread communication uses non-blocking mechanisms only.

## Lock-free data and parameter handoff

- **Use lock-free queues for streaming data:** Move sample blocks between threads
  with a wait-free single-producer/single-consumer (SPSC) ring buffer. The
  real-time side performs only non-blocking `try_*` operations and degrades
  gracefully (e.g. outputs silence/zeros or reuses the last block) if data is not
  yet available.
- **Use atomics or buffer-swapping for parameters:** Publish new coefficients or
  configuration from the control thread and have the real-time thread pick them up
  without locking — e.g. an atomic flag plus a double/triple buffer, or a small
  lock-free channel. Choose explicit memory orderings deliberately.
- **Keep allocation on the non-real-time side:** Allocate, resize, and free
  buffers on the control thread, then hand ownership to the real-time thread
  through the lock-free channel. The real-time thread only borrows or swaps
  pre-allocated storage.

## Apply parameter changes without artifacts

Changing a coefficient instantaneously creates a discontinuity in the output
(audible clicks, spurious spectral content, transients in a control loop):

- **Apply changes at block boundaries** rather than mid-block, so each block uses
  a consistent parameter set.
- **Smooth control changes over time:** Ramp the parameter toward its target
  (e.g. linear ramp, one-pole smoothing) or **crossfade** between the old and new
  processing output over a short interval. Smoothing turns a step change into a
  continuous transition.
- **Smooth in the right space:** Interpolate in whatever space is perceptually or
  numerically appropriate (e.g. interpolate gain logarithmically, frequencies
  geometrically) rather than always linearly.

## Separate control rate from sample rate

- **Recompute derived state only when inputs change.** Expensive coefficient
  computation belongs at the (slow) control rate, not per sample.
- **Update at a coarse cadence:** Evaluate control changes once per block (or
  every N samples) and hold them constant in between, combined with smoothing
  above. This keeps the per-sample hot path minimal.

## Latency vs. block-size tradeoff

Block size is a deliberate design decision, not an afterthought:

- **Larger blocks** improve throughput, amortize per-call overhead, and vectorize
  better — but **add latency** proportional to the block length.
- **Smaller blocks** reduce latency but increase relative overhead and may inhibit
  vectorization.
- **For low-latency paths**, consider partitioning work (process a small head
  block immediately and larger blocks behind it) instead of simply shrinking the
  block size everywhere.
- Choose the block size from the latency budget and measured throughput, and
  document the resulting latency.

## Determinism & reproducibility

- Cross-thread interleaving and floating-point reassociation can make results
  non-reproducible run to run. Where reproducibility matters (testing, regression
  comparison), pin block sizes and processing order, and avoid summation orders
  that depend on thread scheduling.
- Validate cross-thread processing against the single-threaded reference within
  the documented tolerance (see `00_methodology.md`).
