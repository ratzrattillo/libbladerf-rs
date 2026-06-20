# Architecture & API Design

How to structure DSP code so it is reusable, testable, and hard to misuse —
independent of the signal domain (audio, RF/communications, sensor/control,
imaging, etc.).

## A domain-neutral processing vocabulary

Describe processors in domain-neutral terms so the same design applies to any
signal:

- **Sample:** one scalar value of the signal.
- **Block / frame:** a contiguous run of samples processed together.
- **Channel:** one independent stream of samples (e.g. a sensor lane).
- **Processing callback:** the function the runtime calls to process a block.
- **State:** values that persist across calls (filter memory, phase, counters).

Prefer these terms over domain-specific words so processors stay reusable across
applications.

## Separate state from processing

Keep coefficients/state (`struct`s) separate from processing algorithms
(functions/methods). This scales cleanly to multichannel processing and improves
reuse. A processor owns its state; the processing routine reads inputs and updates
that state deterministically.

## Block processing over sample-at-a-time

Process buffers in blocks rather than one sample per call. Blocks amortize
call/branch overhead and are the prerequisite for effective vectorization. Provide
a block-based API as the primary interface; a single-sample helper, if needed, can
be layered on top.

## Encode units and interpretation in the type system

A scalar like `f32` has many possible meanings (Hz, normalized frequency, seconds,
samples, linear gain, decibels). Confusing them is a classic, hard-to-find DSP
bug. Use distinct wrapper types to make interpretation explicit and let the
compiler reject mismatches:

```rust
struct Hertz(f32);
struct Normalized(f32); // cycles/sample in [0, 0.5]
struct Samples(usize);
struct Seconds(f32);

impl Hertz {
    fn to_normalized(self, sample_rate: Hertz) -> Normalized { /* ... */ }
}
```

Conversions then happen once, explicitly, at well-defined boundaries.

## Convey meaning through types, not `bool`/`Option`

Bare booleans and options hide intent at the call site. Prefer enums (or
dedicated types) for mode selection:

```rust
// Unclear: process(buf, true, false)
enum Window { Rectangular, Hann, Blackman }
enum Edge   { Zero, Clamp, Wrap }

fn process(buf: &mut [f32], window: Window, edge: Edge) { /* ... */ }
```

This documents the choices, prevents argument-order mistakes, and makes adding a
new mode a compile-checked change.

## Make invalid configurations unrepresentable (correct by construction)

Prefer designs where an invalid pipeline simply cannot be built, so the hot path
needs no defensive checks:

- **Const generics for fixed sizes:** Use const generics for compile-time buffer
  sizes (e.g. `process<const N: usize>(&mut self, buf: &mut [f32; N])`). This
  enables full unrolling and bounds-check elision for fixed-size kernels.
- **Compile-time validation:** Prefer compile-time validation (types, const
  generics) over runtime checks where possible.
- **Validated constructors:** Validate sizes, sample rates, and coefficient ranges
  once when the processor is constructed, so the per-block routine can assume
  validity and stay branch-free.

## Test algorithms against references

Always unit-test algorithms against known mathematical references to verify
correctness and catch regressions. Where bit-exact behavior is required, assert
it; otherwise assert within a documented numerical tolerance. (The reference-and-
tolerance discipline is described in `00_methodology.md`.)
