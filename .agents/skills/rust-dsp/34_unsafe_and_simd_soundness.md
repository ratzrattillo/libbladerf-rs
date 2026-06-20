# Unsafe & SIMD Soundness

DSP code is a frequent temptation for `unsafe` (skipping bounds checks, SIMD
intrinsics, DMA/register access). These rules keep that code sound. They are
intentionally concise; the principle is simple: **undefined behavior is never an
acceptable price for speed.**

## When `unsafe` is justified

Use `unsafe` only for:

1. **Performance**, e.g. eliding a bounds check with `get_unchecked` — and only
   after a benchmark proves the win.
2. **FFI / platform access**, e.g. DMA, memory-mapped registers, calling into C or
   the kernel.
3. **Novel sound abstractions**, e.g. a specialized buffer or allocator that
   cannot be expressed safely.

Do **not** use `unsafe` to silence the borrow checker, to bypass `Send`/`Sync`
bounds, or to "simplify" code that has a safe equivalent.

## Prefer the safe path first

- Reach for safe constructs before `unsafe`: `chunks_exact`, iterators, and
  validate-length-once-then-index patterns often produce identical codegen to
  `get_unchecked` without the risk (see `30_memory_and_data.md`).
- Only drop to unchecked indexing when (a) a benchmark shows a real improvement
  and (b) the precondition (length/alignment) is established once *outside* the
  hot loop.

## Document and verify every `unsafe`

- Every `unsafe` block carries a plain-text safety comment stating the
  precondition that makes it sound and why it holds here.
- Public `unsafe` functions document the conditions the caller must uphold.
- Validate `unsafe` code with Miri where applicable, and always compare its output
  against the safe scalar reference within the documented tolerance (see
  `00_methodology.md`).

## SIMD intrinsics

- `core::arch` intrinsics are `unsafe` and target-gated. Guard them behind
  `#[cfg(target_feature = "...")]` and/or runtime feature detection, and always
  keep a safe scalar fallback (see `10_build_and_portability.md`).
- Uphold alignment and lane-count preconditions; reading or writing past a slice
  with a wide load/store is undefined behavior.
- Prefer portable SIMD abstractions over raw intrinsics when they meet the
  performance goal, to reduce the amount of `unsafe` surface.

## Soundness is non-negotiable

A function that is *safe* to call must never be able to cause undefined behavior,
for any input. If you cannot encapsulate an operation soundly, expose it as an
`unsafe` function with documented preconditions instead of hiding the hazard.
