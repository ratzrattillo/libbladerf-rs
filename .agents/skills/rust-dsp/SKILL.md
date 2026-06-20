---
name: rust-dsp
description: Invoke this skill BEFORE writing or modifying Rust code that performs Digital Signal Processing (DSP) on any signal domain such as audio, RF/communications, sensor/control, or imaging. Covers sample/block processing pipelines, filters, transforms (FFT/DFT/wavelet), modulation/demodulation, resampling, SIMD numeric kernels, and other latency-sensitive hot-path math. Enforces real-time-safe, high-performance, numerically correct Rust DSP practices. Every optimization must be measured and verified.
---

# Rust DSP Development Skill

This skill enforces high-performance, real-time-safe, numerically correct Rust for
Digital Signal Processing code, independent of the signal domain. It is
self-contained: all guidance lives in this skill's files, and references between
those files are intra-skill only.

> **Golden rule:** Every optimization is a *hypothesis*, not a fact. Measure before
> and after. If an optimization shows no measurable benefit but harms readability,
> **omit it**. See `00_methodology.md`.

## When to invoke this skill

Invoke before writing or modifying Rust that involves any of:

- Sample-by-sample or block-based signal processing (any domain)
- Filters (FIR, IIR, biquads, cascaded second-order sections)
- Transforms and spectral processing (FFT/DFT/wavelet), modulation/demodulation
- Resampling, interpolation, and rate conversion
- SIMD or autovectorized numeric kernels and other latency-sensitive hot paths
- Real-time callbacks, embedded/`no_std` DSP, ring buffers, DMA

## Guideline files, grouped by type of decision

The file hierarchy reflects the *kind* of decision each file governs. Load only the
files relevant to the task, using segmented reading (`offset` / `limit`) for large
files.

### 00s — Process & methodology

- **`00_methodology.md`** — Use in ALL DSP tasks. How to establish a correctness
  reference and tolerance, inspect generated code, benchmark and profile, and omit
  optimizations that don't pay off.

### 10s — Build & environment

- **`10_build_and_portability.md`** — Use when configuring build flags
  (`target-cpu`, fast-math), handling subnormal flushing, designing a portable
  baseline plus optimized/feature-gated paths, or targeting generic architectures
  (WebAssembly, embedded, baseline x86-64).

### 20s — Architecture & design

- **`20_architecture_and_api.md`** — Use when structuring DSP code: domain-neutral
  processing vocabulary, separating state from processing, block processing,
  units-as-types, enums over `bool`/`Option`, const generics, and correct-by-
  construction APIs.

### 30s — Runtime engineering

- **`30_memory_and_data.md`** — Buffers and memory layout: zero-copy/DMA, avoiding
  allocation in hot paths, cache-line alignment, SoA vs AoS, ring buffers, slice
  iterators.
- **`31_realtime_determinism.md`** — Hot-path/ISR/embedded code: eliminating panics
  and blocking calls, `no_std`, branchless techniques, loop unrolling, inlining,
  LUTs vs polynomials.
- **`32_concurrency_and_control.md`** — Connecting the real-time thread to the rest
  of the program: lock-free data/parameter handoff, artifact-free parameter
  changes (smoothing/crossfade), control-rate vs sample-rate, the latency/block-size
  tradeoff, and reproducibility.
- **`33_numerical_precision.md`** — Numeric correctness and math optimization:
  `f32` vs `f64`, FMA (`mul_add`), autovectorization, custom SIMD, strength
  reduction, division/reciprocals, fixed-point saturation, recursive-filter
  stability.
- **`34_unsafe_and_simd_soundness.md`** — Use whenever `unsafe` or SIMD intrinsics
  appear: when `unsafe` is justified, preferring the safe path, documenting and
  verifying safety, and SIMD soundness preconditions.

## Coding Rules

1. **Load the relevant guideline files BEFORE generating or modifying DSP code.**
2. Apply the rules from the relevant files to the hot path first.
3. Never add an optimization without a plan to measure it; remove unmeasured or
   non-beneficial optimizations that hurt readability.
4. Comments must be written in American English unless the user requests otherwise.
5. Verify the resulting code compiles and that any claimed optimization is confirmed
   via the verification workflow in `00_methodology.md`.
