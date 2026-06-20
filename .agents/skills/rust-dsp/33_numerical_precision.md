# Numerical Precision & Math Optimization

- **`f32` vs `f64`:** Default to `f32` in sample/block hot paths — it doubles SIMD
  lane count and halves memory bandwidth. Use `f64` selectively where accumulation
  error matters (recursive filter state, long summations, coefficient
  computation).

- **Fused multiply-add (FMA):** Prefer `f32::mul_add` / `f64::mul_add` for
  `a * b + c` — it is faster *and* more accurate (single rounding).
  - **Caveat:** `mul_add` is only fast when the target has the `fma` feature;
    without it, `mul_add` is **emulated in software and slower** than separate
    `*` and `+`. Gate on `target-feature = "fma"` or feature-detect (see
    `10_build_and_portability.md`).

- **Autovectorization first:** Prefer enabling autovectorization (contiguous
  data, `chunks_exact`, no aliasing, simple loop bodies) over hand-written SIMD.
  - **Rust advantage:** `&mut` no-aliasing guarantees often let the optimizer
    vectorize loops that C cannot. Lean on this.

- **Custom SIMD when needed:** Use `core::arch` / portable SIMD only when
  autovectorization provably falls short. Keep it feature-gated and benchmarked.
  SIMD intrinsics are `unsafe` and require care — follow the soundness rules in
  `34_unsafe_and_simd_soundness.md`.

- **Strength reduction:** Replace expensive operations with cheaper equivalents
  in hot paths (e.g. precompute reciprocals, replace repeated `pow` with
  incremental multiply, hoist invariants out of loops).

- **Division & bit shifting:** Division is expensive; the compiler already turns
  *constant* power-of-two `*`/`/` into shifts, so hand-written shifts on integer
  constants add no speed and hurt readability. Focus instead on eliminating
  **runtime division** (e.g. precompute `1.0 / x` and multiply, or multiply by a
  reciprocal). Bit shifts don't apply to floats.

- **Fixed-point saturation:** When doing fixed-point math, implement saturation
  (or guard bits) to prevent overflow from wrapping to opposite extremes, which
  produces severe distortion. Use `saturating_*` ops.

- **Recursive-filter numerical stability:** For recursive (IIR-type) filters:
  - Keep filter state in `f64` even when I/O is `f32` to limit error accumulation.
  - Prefer cascaded second-order sections (biquads / SOS) over high-order
    direct-form for coefficient-quantization robustness.
  - Watch for denormal accumulation in feedback paths (see
    `10_build_and_portability.md`).
