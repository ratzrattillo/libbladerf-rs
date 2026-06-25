# Maintainer Guide

This guide documents the CI pipelines, commit conventions, changelog flow, and
release process for `libbladerf-rs`. Paths are relative to the repository root.

## Continuous integration

CI is defined in `.github/workflows/ci.yml` and runs on every push and pull
request to `main` (plus manual `workflow_dispatch`). It delegates to the
composite action `.github/actions/check`, which runs:

1. Build — `cargo build --features bladerf1`
2. Unit tests — `cargo test --lib`
3. Protocol tests — `cargo test --test unit`
4. Clippy — `cargo clippy --features bladerf1 --all-targets -- -D warnings`
5. Format check — `cargo fmt --all --check`
6. Security audit — `cargo install cargo-audit && cargo audit`
7. License/deny check — `cargo install cargo-deny && cargo deny check`
8. Documentation — `cargo doc --features bladerf1 --no-deps`
9. Build all examples — every `examples/*/Cargo.toml`

CI does **not** run hardware integration tests (no BladeRF1 attached to the
runner) and does **not** run the local-only conventional-commits validation.

## Local pre-flight

`scripts/check.sh` is the local pre-flight check. It is also the
`cargo-release` pre-release hook (see `release.toml`). It runs, in order:

- Build (`--features bladerf1`)
- Unit tests (`--lib`), protocol tests (`--test unit`), and hardware
  integration tests (`--features bladerf1 --tests -- --test-threads=1`)
- Clippy, format check
- **Conventional-commits validation** — `git-cliff --unreleased --output
  /dev/null` (fails on any non-conventional unreleased commit; local-only, not
  in CI)
- Docs, build all examples
- `cargo deny check`, `cargo audit`

`cargo clean` and the benchmark step are commented out in `scripts/check.sh`.
Run benchmarks manually when needed:

```bash
# No hardware required:
cargo bench --bench nios_packet_bench --bench sample_format_bench --bench metadata_header_bench
# Requires hardware (run one at a time — they share one physical device):
cargo bench --features bladerf1 --bench hardware_gpio_bench
cargo bench --features bladerf1 --bench hardware_tuning_bench
cargo bench --features bladerf1 --bench hardware_stream_latency_bench
cargo bench --features bladerf1 --bench hardware_stream_build_teardown_bench
cargo bench --features bladerf1 --bench hardware_gain_bench
cargo bench --features bladerf1 --bench hardware_calibration_bench
```

## Commit messages

Commits must follow [Conventional Commits](https://www.conventionalcommits.org/).
This is required because the changelog is generated from commit messages with
`git-cliff` (`cliff.toml` sets `require_conventional = true`).

Enforcement:

- A husky-rs `commit-msg` hook (`.husky/commit-msg`) validates each message via
  `git-cliff` at commit time. Install git-cliff with `cargo install git-cliff`.
- The hook installs automatically on `cargo build` / `cargo test` (husky-rs sets
  `core.hooksPath` to `.husky`). Set `NO_HUSKY_HOOKS=1` to skip installation.
- `scripts/check.sh` re-validates all unreleased commits as a backstop.

## Changelog

Generated with [`git-cliff`](https://git-cliff.org). `scripts/changelog.sh` runs
`git-cliff --unreleased --prepend CHANGELOG.md`, prepending entries for
unreleased commits to `CHANGELOG.md`. It mutates `CHANGELOG.md` in place, so run
it on demand — it is not part of `scripts/check.sh`.

```bash
bash scripts/changelog.sh
```

Review the generated entries, move them under a versioned header during release
(see below), and commit `CHANGELOG.md` separately.

## Release process

Releases use [`cargo-release`](https://github.com/crate-ci/cargo-release).
Config is in `release.toml`:

| Setting | Value | Meaning |
|---------|-------|---------|
| `publish` | `false` | `cargo-release` does not publish to crates.io itself |
| `pre-release-hook` | `["bash", "scripts/check.sh"]` | Full pre-flight before bump |
| `tag-prefix` | `""` | Tags are `v0.4.1`, not `libbladerf-rs-v0.4.1` |
| `consolidate-commits` | `true` | Single release commit |
| `dependent-version` | `"upgrade"` | Upgrade dependent version requirements |

### Automated release (recommended)

The release is driven by `.github/workflows/release.yml`, triggered manually via
GitHub's **workflow_dispatch** with a version bump level (`patch`/`minor`/`major`):

1. `check` — runs the composite `.github/actions/check` action.
2. `bump_and_tag` — `cargo release <level> --execute --no-verify --no-publish`,
   which bumps the version in `Cargo.toml`, commits, and creates the git tag.
3. `github_release` — `gh release create <tag> --generate-notes`.
4. `publish` — checks out the tag and runs `cargo publish` using the
   `CARGO_REGISTRY_TOKEN` secret.

To cut a release: prepare the changelog (below), then run the **Release**
workflow from the GitHub Actions tab and pick the bump level.

### Manual / local release

When releasing locally instead of via the workflow:

```bash
# Dry run — review version bump, pre-release checks, commit, tag. Writes nothing.
cargo release patch --dry-run

# Execute — runs scripts/check.sh, bumps Cargo.toml, commits, tags. No publish.
cargo release patch

# Publish manually (publish = false in release.toml):
cargo publish -p libbladerf-rs --dry-run   # verify tarball contents
cargo publish -p libbladerf-rs             # actual publish

# Push the release commit and tag:
git push --follow-tags
```

### Version bump levels

| Command | Effect | When to use |
|---------|--------|-------------|
| `cargo release patch` | 0.4.1 → 0.4.2 | Bug fixes, minor additions |
| `cargo release minor` | 0.4.1 → 0.5.0 | New features, **and** breaking changes while major = 0 |
| `cargo release major` | 0.4.1 → 1.0.0 | Breaking changes after 1.0 |

While the major version is `0`, breaking changes bump the **minor** version per
SemVer (0.y.z), not the major.

## Pre-release checklist

- [ ] **CI is green** — the composite check action passes on `main`.
- [ ] **Local pre-flight passes** — `bash scripts/check.sh` (requires BladeRF1
      hardware for the integration tests).
- [ ] **Commit messages are conventional** — enforced by the `commit-msg` hook
      and re-checked by `scripts/check.sh`.
- [ ] **`CHANGELOG.md` is up to date** — run `bash scripts/changelog.sh`, review,
      and move the `[unreleased]` entries under the new version header.
- [ ] **`README.md` is up to date** — feature lists, example code, and test
      commands reflect the current API; the example compiles.
- [ ] **Breaking changes are deliberate** — while major = 0, breaking changes
      bump the minor version; after 1.0, they require a major bump.
- [ ] **Working tree is clean** — `git status` clean; `cargo-release` refuses
      otherwise.

## Post-release

- Verify the tag appears on GitHub and crates.io shows the new version.
- Verify docs.rs builds successfully (check the build log if it fails).
- If hardware is available, run the hardware benchmarks and compare against the
  previous release for regressions in NIOS packet encode/decode, sample format
  pack/unpack, and metadata header parsing.
