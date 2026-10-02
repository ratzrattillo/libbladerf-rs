# Maintainer Guide

This guide documents the CI pipelines, commit conventions, changelog flow, and
release process for `libbladerf-rs`. Paths are relative to the repository root.

## Continuous integration

CI is defined in `.github/workflows/ci.yml` and runs on every push and pull
request to `main` (plus manual `workflow_dispatch`). It is also a reusable
`workflow_call` gate for releases. Individual jobs run:

1. Build — `cargo build --features bladerf1`
2. Unit tests — `cargo test --lib`
3. Protocol tests — `cargo test --test unit`; public API contract —
   `cargo test --test public_api --features bladerf1`
4. Stable and nightly Clippy — workspace/default integration and root tokio-only
   (`--no-default-features --features bladerf1,xb100,xb200,xb300,tokio`);
   nightly is advisory in CI and required locally
5. Format check — `cargo +nightly fmt --all --check`
6. Security audit — `cargo install cargo-audit && cargo audit`
7. License/deny check — `cargo install cargo-deny && cargo deny check`
8. Documentation — `cargo doc --features bladerf1 --no-deps --lib --bins --examples`
9. Build all examples — every `examples/*/Cargo.toml`
10. WebUSB — `cargo check --target wasm32-unknown-unknown --features bladerf1 --lib`
    plus runtime-free public API checks (`.cargo/config.toml` supplies
    `--cfg=web_sys_unstable_apis`)
11. Explicit Rust 1.88.0 MSRV, isolated native runtime/expansion feature contracts,
    Android API checks with both runtime integrations, and aarch64/Android/Windows
    cross-builds; Windows also gets a cross-target API check
12. Conventional Commits — `git-cliff --unreleased --output /dev/null`

Unit/protocol/API tests run on Linux, macOS, and Windows; Linux additionally
runs the tokio-only test configuration. Keep the workflow-level
`CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C target-cpu=x86-64"` override.

CI does not run USB hardware integration tests. Cross-compilation checks API and
compiler compatibility; platform execution requires that platform's USB host.

## Local pre-flight

`cargo install git-cliff`
`scripts/check.sh` is the local pre-flight check. It refreshes the stable and
nightly toolchains first (nightly from `static.rust-lang.org`, matching CI's
floating `dtolnay/rust-toolchain@nightly`, so newly-added nightly lints fail
locally before CI), checks formatting with nightly rustfmt, and runs clippy on
both stable (the CI gate) and nightly (early warning for lints about to land).
There is no `cargo-release` pre-release hook. Run this script before triggering
a release. It mirrors CI, with the additional hardware suite when `CI` is unset:

- MSRV, both runtime builds, and the feature matrix
- Unit/protocol/public API tests with default and tokio-only integrations
- Hardware integration tests with default and tokio-only integrations,
  sequentially (`-- --test-threads=1`)
- Clippy, format check
- WebUSB, Android, Windows, and aarch64 cross/API checks
- **Conventional-commits validation** — `git-cliff --unreleased --output
  /dev/null` (fails on any non-conventional unreleased commit)
- Docs, build all examples
- `cargo deny check`, `cargo audit`

The script retains `Cargo.lock` and build artifacts. Run benchmarks separately:

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
- `scripts/check.sh` and the CI conventional-commits job re-validate unreleased commits.

## Changelog

Generated with [`git-cliff`](https://git-cliff.org). The release workflow
replaces any `## [unreleased]` section with the versioned section for the new
tag and amends it into the release commit/tag. There is no local changelog
generation step.

## Release process

Releases are GitHub Actions only. They are driven by
`.github/workflows/release.yml`, triggered manually via GitHub's
**workflow_dispatch** with a version bump level (`patch`/`minor`/`major`). There
is no local release path — do not run `cargo release` by hand.

The workflow uses [`cargo-release`](https://github.com/crate-ci/cargo-release)
with its stock defaults (root `tag-prefix = ""`, `consolidate-commits = true`,
`dependent-version = "upgrade"`); the pipeline passes `--no-publish`/`--no-push`
and performs the tagging, pushing, and publishing itself. There is no
`release.toml`.

### Release workflow

The pipeline runs:

1. `ci` — calls the reusable `.github/workflows/ci.yml` workflow.
2. `bump_and_tag` — `cargo release <level> --execute --no-verify --no-publish --no-push --no-confirm`,
   then updates the versioned changelog, amends the commit and annotated tag,
   and explicitly pushes both with `git push origin HEAD <tag>`.
3. `github_release` — `gh release create <tag> --generate-notes`.
4. `publish` — checks out the tag and runs `cargo publish` using the
   `CARGO_REGISTRY_TOKEN` secret.

To cut a release, run the **Release** workflow and pick the bump level. The
concurrency group serializes release triggers. `--no-confirm` is mandatory for
noninteractive cargo-release; otherwise it can exit successfully without a bump.
Release/publish jobs check out the new branch tip, then resolve the new tag.

### Version bump levels

The workflow's `workflow_dispatch` input picks one of:

| Level | Effect | When to use |
|-------|--------|-------------|
| `patch` | 0.4.1 → 0.4.2 | Bug fixes, minor additions |
| `minor` | 0.4.1 → 0.5.0 | New features, **and** breaking changes while major = 0 |
| `major` | 0.4.1 → 1.0.0 | Breaking changes after 1.0 |

While the major version is `0`, breaking changes bump the **minor** version per
SemVer (0.y.z), not the major.

## Pre-release checklist

- [ ] **CI is green** — the reusable workflow passes on `main`.
- [ ] **Local pre-flight passes** — `bash scripts/check.sh` (requires BladeRF1
      hardware for the integration tests).
- [ ] **Commit messages are conventional** — enforced by the `commit-msg` hook
      and re-checked by `scripts/check.sh`.
- [ ] **Commit messages are changelog-ready** — the workflow generates and
      commits the versioned section from the Conventional Commit history.
- [ ] **`README.md` is up to date** — feature lists, example code, and test
      commands reflect the current API; the example compiles.
- [ ] **Breaking changes are deliberate** — while major = 0, breaking changes
      bump the minor version; after 1.0, they require a major bump. Keep
      `MIGRATION.md`, `tests/public_api.rs`, examples, and seify integration aligned.
- [ ] **Working tree is clean** — `git status` clean before triggering the
      workflow.

## Post-release

- Verify the tag appears on GitHub and crates.io shows the new version.
- Verify docs.rs builds successfully (check the build log if it fails).
- If hardware is available, run the hardware benchmarks and compare against the
  previous release for regressions in NIOS packet encode/decode, sample format
  pack/unpack, and metadata header parsing.
