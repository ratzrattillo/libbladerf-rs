# Methodology: Measure, Verify, Omit

This file governs *how you work* on every DSP task. Apply it before, during, and
after any optimization.

> **Golden rule:** Every optimization is a *hypothesis*, not a fact. Measure
> before and after. If an optimization shows no measurable benefit but harms
> readability, **omit it**.

## Establish correctness before performance

- **Define a reference (oracle) first:** Before optimizing, implement or obtain a
  simple, obviously-correct version of the algorithm (a "golden" reference). Every
  optimized variant is validated against it.
- **Fix a tolerance up front:** Decide whether the result must be bit-exact or
  correct within a documented numerical tolerance, and write that decision down.
  Floating-point reordering, fused operations, and SIMD can all change low bits.
- **Test against the reference continuously:** Keep the reference comparison as a
  unit/property test so optimizations cannot silently introduce regressions.

## Order of work

1. Make it correct (validated against the reference).
2. Identify the hot path; optimize the hot path first, not speculative code.
3. Form one optimization hypothesis at a time and measure it.

## Inspect the generated code

- Use a disassembler / Compiler Explorer (godbolt) to confirm an optimization
  (vectorization, bounds-check elision, FMA emission, branch removal) actually
  occurred.
- **Note:** assembly inspection shows *what* the compiler did; it does **not**
  measure performance. Pair it with a benchmark.

## Benchmark with real measurements

- Use a statistically sound microbenchmark harness (e.g. `criterion`) for
  isolated kernels.
- Use a sampling profiler / flamegraph (e.g. `perf`, `cargo-flamegraph`) on a
  realistic workload to find the true hot path before committing to a kernel-level
  benchmark.
- **Benchmark realistic and worst-case inputs**, not just convenient ones.
  Pathological inputs matter in DSP: subnormal-producing decays, silence/zeros,
  steady-state vs. transients, and the block sizes you will actually use.

## Omit what doesn't pay off

- Every optimization MUST be verified to provide a measurable benefit.
- If it shows no improvement but degrades readability or portability, **remove
  it.** Unmeasured complexity is technical debt, not performance.
