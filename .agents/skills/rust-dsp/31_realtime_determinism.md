# Real-Time Determinism

Rules for hot-path / interrupt / embedded code where execution time must be
bounded and predictable. This file covers what the hot path must **not** do and
how to keep it tight; for *how to move data and parameters in and out* of the hot
path safely, see `32_concurrency_and_control.md`.

- **Eliminate panics in the hot path:** A panic (e.g. out-of-bounds access) costs
  unpredictable time. Validate slice lengths *once before* the loop, or use
  `.get(...)` with a fallback, so the hot loop is branch- and panic-free.

- **No blocking calls:** Never lock a mutex, perform I/O, sleep, or otherwise
  block in the hot path. When the hot path needs data or parameters from another
  thread, use a non-blocking handoff (see `32_concurrency_and_control.md`).

- **Keep ISRs brief:** Minimize work in Interrupt Service Routines — flag data as
  ready and offload heavy computation to a background processing loop.

- **`#![no_std]` where appropriate:** For core processing that must avoid the
  allocator and OS overhead, develop against `core`/`alloc` (or fully `no_std`).

- **Deterministic integer behavior:** Release builds disable overflow checks. Be
  explicit about intent with `wrapping_*`, `saturating_*`, or `checked_*` instead
  of relying on the build profile.

- **Branchless techniques (situational):** Where it helps, replace branches with
  arithmetic/masks, `f32::clamp`, `bool as` arithmetic, or select patterns.
  Modern branch predictors handle predictable branches well, so **profile first**
  — branchless code is not automatically faster.

- **Loop unrolling — prefer the compiler:** Let the compiler unroll and
  autovectorize. Manual unrolling often *pessimizes* by inhibiting vectorization.
  Hand-unroll **only** when disassembly/benchmarks prove it helps.

- **Inlining — deliberate, not blanket:** Inline short, hot functions to remove
  call overhead. But `#[inline(always)]` is **not free**: over-inlining bloats
  instruction cache and can *hurt* performance. Use `#[inline]` (a hint) by
  default and reserve `#[inline(always)]` for verified cases.

- **Lookup tables (LUTs) — measure vs. polynomials:** Precomputing non-linear
  functions (`sin`, `cos`, `log`, ...) with LUT + interpolation can help, but on
  modern hardware a **minimax polynomial approximation** is often faster (it
  vectorizes and avoids cache pressure). Choose based on benchmarks; apply only
  in the hot path.
