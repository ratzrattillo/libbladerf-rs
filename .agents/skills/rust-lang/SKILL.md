---
name: rustlang-api
description: ALWAYS invoke this skill BEFORE designing or modifying Rust APIs. Enforces idiomatic Rust API conventions from the Rust library team and requires consulting the appropriate guideline files before any coding activity. This skill is MANDATORY for all Rust API design. Based on the Rust API Guidelines.
---

**Current compliance date: 2026-06-08**

# Rust Development Skill

This skill enforces structured, guideline-driven Rust development. It ensures all Rust code strictly follows the Rust API Guidelines rules, documentation formats, and quality constraints.

## Mandatory Workflow

**This skill MUST be invoked for ANY Rust action**, including:
- Creating new `.rs` files (even minimal examples like Hello World)
- Modifying existing `.rs` files (any change, no matter how small)
- Reviewing, refactoring, or rewriting Rust code

## Which guideline to read and when

Before writing or modifying Rust code, **the agent must load ONLY the guideline files that apply to the requested task**, using segmented reading (`offset` and `limit`) when needed.

### Guidelines and when they apply


#### 1. `01_naming.md`
Use when naming types, traits, functions, modules, macros, or features. Conforms to casing, getter, converter, and iterator naming conventions.


#### 2. `02_interoperability.md`
Use when implementing common traits (Copy, Clone, Debug, etc.), designing conversions with From/AsRef, error types, or ensuring Send/Sync compatibility.


#### 3. `03_macros.md`
Use when writing declarative or procedural macros. Covers evocative syntax, attribute composition, visibility specifiers, and flexible fragments.


#### 4. `04_documentation.md`
Use when writing rustdoc examples, module-level docs, error/panic/panic notes, release notes, or ensuring proper metadata in Cargo.toml.


#### 5. `05_predictability.md`
Use when designing smart pointers, operators, constructors, method vs function placement, or conversion placement on specific types.


#### 6. `06_flexibility.md`
Use when exposing intermediate results, delegating copy/placement to caller, using generics, or designing object-safe traits.


#### 7. `07_type_safety.md`
Use when designing newtypes for static distinctions, replacing bool/Option with meaningful types, bitflags, or builders.


#### 8. `08_dependability.md`
Use when validating function arguments, ensuring destructors never fail, and providing alternatives for blocking destructors.


#### 9. `09_debuggability.md`
Use when implementing Debug for public types, ensuring debug representations are non-empty and informative.


#### 10. `10_future_proofing.md`
Use when applying sealed traits, keeping struct fields private, using newtypes to hide impl details, or avoiding duplicated derived trait bounds.


#### 11. `11_necessities.md`
Use when managing stable dependencies for stable crates or ensuring permissive licensing.


## Coding Rules

1. **Load the necessary guideline files BEFORE ANY RUST CODE GENERATION.**
2. Apply the required rules from the relevant guidelines.
3. Apply the C-CRATE-DOC / C-EXAMPLE / C-FAILURE documentation format (summary sentence < 15 words, then extended docs, Examples, Errors, Panics, Safety, Abort sections as applicable).
4. Comments must ALWAYS be written in American English, unless the user explicitly requests a different language.
5. If the file is fully compliant, add a comment: `// rustlang-api guideline compliant 2026-06-08`
