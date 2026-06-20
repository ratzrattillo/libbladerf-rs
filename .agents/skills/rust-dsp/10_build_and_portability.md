# Build Configuration & Portability

Build-time and target-level decisions that affect performance and correctness of
signal-processing code.

## Compiler flags

- **CPU-native instructions:** Build for the target architecture to unlock its
  full instruction set, e.g. `-C target-cpu=native`.
  - **Caveat:** `target-cpu=native` produces **non-portable binaries** that will
    fault (`SIGILL`) on older CPUs. Use it only for binaries built and run on the
    same machine class. For distributed libraries/binaries, prefer **runtime
    feature detection** combined with function multiversioning, or target a
    conservative baseline (e.g. `x86-64-v2`). See "Portable baseline + optimized
    paths" below.

- **Fast-math — prefer targeted, opt-in usage:** Rust strictly adheres to
  IEEE-754, which prevents some reorderings (e.g. it cannot freely reassociate
  sums). **Do not enable global fast-math.**
  - There is **no stable global `-ffast-math`** in Rust. `-C llvm-args=-ffast-math`
    is unsupported, unstable, and applies to *all* code including dependencies.
  - Global fast-math is **unsound**: it changes float semantics program-wide and
    can silently break correctness-sensitive algorithms (e.g. compensated/Kahan
    summation, NaN/Inf sentinels). With it enabled, the compiler assumes values
    are never `NaN`/`Inf`, so explicit `is_nan()` checks may be optimized away to
    `false`.
  - **Preferred approach:** apply fast-math semantics *per operation*:
    - Use `f32::mul_add` for fused multiply-add (see `33_numerical_precision.md`).
    - Structure algorithms so the optimizer can vectorize without relaxed math.
    - For relaxed per-op intrinsics (`fadd_fast`, `fmul_fast`, ...) note these
      live in `core::intrinsics` and are **nightly-only**.

- **Subnormal (denormal) flushing:** Subnormals can cause severe CPU spikes
  (10-100x slowdowns) in feedback/recursive signal paths and slowly-decaying
  state. Flushing them to zero is a **runtime CPU register setting**, *not* a
  compile-time feature.
  - **There is no `target-feature=+fast-math`** — that target feature does not
    exist and will be ignored/rejected. Do not use it.
  - On x86 set **FTZ/DAZ** via the `MXCSR` register (e.g. via a crate or a small
    `_mm_setcsr` / inline-asm shim); on ARM use the `FPCR` `FZ` bit. No portable
    stable Rust API exists.
  - **Portable alternative (preferred for determinism):** prevent denormals
    numerically — add a tiny constant (e.g. `1e-20`) into feedback paths, or
    periodically zero out very small state values. This is cross-platform and
    independent of CPU flags.

## Portable baseline + optimized paths

A single kernel often cannot be both maximally fast *and* runnable everywhere.
Structure the build so portability and performance coexist:

- **Always ship a safe scalar baseline** that compiles and runs on every target
  (including `no_std`, embedded, and WebAssembly). It is the correctness anchor
  and the fallback when no optimized path applies.
- **Gate optimized kernels** behind `#[cfg(target_feature = "...")]` and/or
  **runtime feature detection** with function multiversioning, so a binary can
  pick the best implementation available on the actual host.
- **Keep the default build portable.** Do not let an optimization make the crate
  fail to build on a generic target; gate it with `#[cfg(...)]` where it does not
  apply.
- **All paths must agree:** every optimized path must produce results that match
  the scalar baseline within the documented tolerance (see `00_methodology.md`).

## Target-dependent optimizations

For generic targets (e.g. WebAssembly, `x86-64` baseline, embedded), many
optimizations do not apply or are unavailable (no `target-cpu=native`,
limited/absent SIMD, no `MXCSR`). Drop or gate them (`#[cfg(...)]`) where they do
not make sense.
