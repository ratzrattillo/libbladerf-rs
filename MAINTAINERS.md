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
11. Explicit Rust 1.98.1 MSRV, isolated native runtime/expansion feature contracts,
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
`scripts/check.sh` is the local pre-flight check. It pins the stable
toolchain (attempting `rustup update stable` first so new default-warn lints
are caught locally), checks formatting with nightly rustfmt, and runs clippy on
both stable (the CI gate) and nightly (early warning for lints about to land).
There is no `cargo-release` pre-release hook. Run this script before a local
release. It mirrors CI, with the additional hardware suite when `CI` is unset:

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

Generated with [`git-cliff`](https://git-cliff.org). `scripts/changelog.sh` runs
`git-cliff --unreleased --prepend CHANGELOG.md`, prepending entries for
unreleased commits to `CHANGELOG.md`. It mutates `CHANGELOG.md` in place, so run
it on demand — it is not part of `scripts/check.sh`.

```bash
bash scripts/changelog.sh
```

This is a local preview. The release workflow replaces an existing unreleased
preview with the versioned section and amends the release commit/tag.

## Release process

Releases use [`cargo-release`](https://github.com/crate-ci/cargo-release).
Config is in `release.toml`:

| Setting | Value | Meaning |
|---------|-------|---------|
| `publish` | `false` | `cargo-release` does not publish to crates.io itself |
| `pre-release-hook` | absent | CI gates the workflow; run local checks explicitly |
| `tag-prefix` | `""` | Tags are `v0.4.1`, not `libbladerf-rs-v0.4.1` |
| `consolidate-commits` | `true` | Single release commit |
| `dependent-version` | `"upgrade"` | Upgrade dependent version requirements |

### Automated release (recommended)

The release is driven by `.github/workflows/release.yml`, triggered manually via
GitHub's **workflow_dispatch** with a version bump level (`patch`/`minor`/`major`):

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

### Manual / local release

When releasing locally instead of via the workflow:

```bash
# Run the checks before invoking cargo-release.
bash scripts/check.sh

# Preview the coordinated breaking release, then execute the bump locally.
cargo release minor
cargo release minor --execute

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

- [ ] **CI is green** — the reusable workflow passes on `main`.
- [ ] **Local pre-flight passes** — `bash scripts/check.sh` (requires BladeRF1
      hardware for the integration tests).
- [ ] **Commit messages are conventional** — enforced by the `commit-msg` hook
      and re-checked by `scripts/check.sh`.
- [ ] **Changelog entries are accurate** — review the preview; the workflow
      generates and commits the versioned section.
- [ ] **`README.md` is up to date** — feature lists, example code, and test
      commands reflect the current API; the example compiles.
- [ ] **Breaking changes are deliberate** — while major = 0, breaking changes
      bump the minor version; after 1.0, they require a major bump. Keep
      `MIGRATION.md`, `tests/public_api.rs`, examples, and seify integration aligned.
- [ ] **Working tree is clean** — `git status` clean; `cargo-release` refuses
      otherwise.

## Post-release

- Verify the tag appears on GitHub and crates.io shows the new version.
- Verify docs.rs builds successfully (check the build log if it fails).
- If hardware is available, run the hardware benchmarks and compare against the
  previous release for regressions in NIOS packet encode/decode, sample format
  pack/unpack, and metadata header parsing.
