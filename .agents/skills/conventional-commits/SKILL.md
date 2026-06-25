---
name: conventional-commits
description: Use when writing git commit messages, amending commits, or creating PR titles. Enforces the Conventional Commits 1.0.0 specification (feat/fix/BREAKING CHANGE types, scopes, footers) for an explicit, tooling-friendly commit history.
---

# Conventional Commits 1.0.0

A lightweight convention on top of commit messages that produces an explicit,
machine-readable history. It dovetails with [SemVer](https://semver.org):
`fix` -> PATCH, `feat` -> MINOR, `BREAKING CHANGE` -> MAJOR.

## Before composing a message

Match this repository's existing style. Run `git log --oneline -20` (or read
recent commits) first and mirror the casing, scope vocabulary, and type usage
already in use. The rules below are the floor, not a reason to override an
established local convention.

## Structure

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

## Types

| Type | Meaning | SemVer |
|------|---------|--------|
| `feat` | Adds a new feature | MINOR |
| `fix` | Patches a bug | PATCH |
| `BREAKING CHANGE` (footer) or `!` (after type/scope) | Breaking API change, any type | MAJOR |

Other allowed types (no implicit SemVer effect unless they carry a breaking
change): `build`, `chore`, `ci`, `docs`, `style`, `refactor`, `perf`, `test`,
`revert`. Teams may add their own types.

A scope is an optional noun in parentheses describing the affected section of
the codebase, e.g. `feat(parser): add ability to parse arrays`.

## Rules (normative)

1. Commits MUST be prefixed with a type, followed by an OPTIONAL scope, OPTIONAL
   `!`, and a REQUIRED terminal colon and space (`: `).
2. `feat` MUST be used when adding a feature; `fix` MUST be used for a bug fix.
3. A scope, if present, MUST be a noun in parentheses, e.g. `fix(parser):`.
4. A description MUST immediately follow the colon and space. It is a short
   summary of the change.
5. A longer body MAY follow, beginning **one blank line** after the description.
   The body is free-form and MAY span multiple blank-line-separated paragraphs.
6. One or more footers MAY follow **one blank line** after the body. Each footer
   is a word token, then either `: ` or ` #`, then a value (git-trailer style).
7. Footer tokens MUST use `-` instead of spaces, e.g. `Reviewed-by`. The sole
   exception is `BREAKING CHANGE`, which MAY be used as a token.
8. A footer value MAY contain spaces and newlines; parsing ends at the next
   valid footer token/separator pair.
9. Breaking changes MUST be signaled either by `!` before the `:` in the
   type/scope prefix, or by a `BREAKING CHANGE:` footer (or both).
10. A `BREAKING CHANGE:` footer MUST be uppercase `BREAKING CHANGE`, followed by
    a colon, space, and description. `BREAKING-CHANGE` is a synonym.
11. If `!` is used, the `BREAKING CHANGE:` footer MAY be omitted and the
    description SHALL describe the breaking change.
12. Casing of types/scopes is implementation-defined (be consistent), EXCEPT
    `BREAKING CHANGE`, which MUST be uppercase.

## Examples

Description plus breaking-change footer:

```
feat: allow provided config object to extend other configs

BREAKING CHANGE: `extends` key in config file is now used for extending other config files
```

`!` to flag a breaking change:

```
feat!: send an email to the customer when a product is shipped
```

Scope with `!`:

```
feat(api)!: send an email to the customer when a product is shipped
```

No body:

```
docs: correct spelling of CHANGELOG
```

With scope:

```
feat(lang): add Polish language
```

Multi-paragraph body and multiple footers:

```
fix: prevent racing of requests

Introduce a request id and a reference to latest request. Dismiss
incoming responses other than from latest request.

Remove timeouts which were used to mitigate the racing issue but are
obsolete now.

Reviewed-by: Z
Refs: #123
```

Revert (recommended convention):

```
revert: let us never again speak of the noodle incident

Refs: 676104e, a215868
```

## Guidance

- If a change conforms to more than one type, prefer splitting it into multiple
  commits.
- Treat early development as if already released: callers want to know what
  changed and what breaks.
- PR titles should follow the same `<type>[scope]: <description>` form so squash
  merges produce conforming history.
