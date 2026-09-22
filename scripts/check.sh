#!/bin/bash

# Bash debug mode
set -xe

# Run from the repository root so relative paths (cliff.toml, CHANGELOG.md)
# resolve regardless of the invocation directory.
cd "$(dirname "$0")/.."

###########################################################
# TOOLCHAINS
###########################################################
# Mirrors CI's dtolnay/rust-toolchain@stable: run everything on the latest
# stable regardless of the local default toolchain, and try to bring it up
# to date first so new default-warn lints are caught here, not in CI.
# Explicit `cargo +nightly` invocations below override this variable.
export RUSTUP_TOOLCHAIN=stable
if ! rustup update stable; then
  echo "WARNING: could not update the stable toolchain; CI may run a newer stable with additional lints." >&2
fi
rustc --version
cargo +nightly --version

# ci.yml env: neutralize the repo's target-cpu=native (see AGENTS.md): cargo
# appends this after the config's rustflags and rustc honors the last
# -C target-cpu.
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C target-cpu=x86-64"
# ci.yml test-* jobs env:
export RUST_BACKTRACE=full

###########################################################
# BUILD (ci.yml: test-linux)
###########################################################
cargo build --features bladerf1
cargo build --no-default-features --features bladerf1,tokio

###########################################################
# TEST (ci.yml: test-linux / test-macos / test-windows)
###########################################################
# Unit tests (no hardware)
cargo test --lib
# Protocol encode/decode tests (no hardware)
cargo test --test unit
# Public API contract (no hardware)
cargo test --test public_api --features bladerf1
# Hardware integration tests (single-threaded, shared device), default
# `smol`. The only addition relative to CI, which runs without a device.
if [ -z "$CI" ]; then
  cargo test --features bladerf1 --tests -- --test-threads=1
  # Same suite plus the async tests with nusb's tokio integration only
  cargo test --no-default-features --features bladerf1,xb100,xb200,xb300,tokio --tests -- --test-threads=1
fi

###########################################################
# CLIPPY (ci.yml: clippy, clippy-nightly)
###########################################################
# Stable is the gate CI enforces.
cargo clippy --features bladerf1 --all-targets -- -D warnings
cargo clippy --no-default-features --features bladerf1,xb100,xb200,xb300,tokio --all-targets -- -D warnings
# Nightly clippy is the early warning (continue-on-error in CI): lints that
# are warn-by-default on nightly today become CI failures on the next
# stable. Fix them now or `#[allow]` them deliberately.
cargo +nightly clippy --features bladerf1 --all-targets -- -D warnings
cargo +nightly clippy --no-default-features --features bladerf1,xb100,xb200,xb300,tokio --all-targets -- -D warnings

###########################################################
# FMT (ci.yml: fmt)
###########################################################
cargo fmt --all --check

###########################################################
# WASM (ci.yml: wasm)
###########################################################
# WebUSB (needs --cfg=web_sys_unstable_apis, supplied by .cargo/config.toml)
rustup target add wasm32-unknown-unknown
cargo check --target wasm32-unknown-unknown --features bladerf1 --lib

###########################################################
# CROSS-COMPILE (ci.yml: cross)
###########################################################
# CI additionally runs: sudo apt-get install -y gcc-aarch64-linux-gnu gcc-mingw-w64-x86-64
rustup target add aarch64-unknown-linux-gnu aarch64-linux-android x86_64-pc-windows-gnu
cargo build --target aarch64-unknown-linux-gnu --features bladerf1 --lib
cargo build --target aarch64-linux-android --features bladerf1 --lib
cargo build --target x86_64-pc-windows-gnu --features bladerf1 --lib

###########################################################
# CONVENTIONAL COMMITS (ci.yml: conventional-commits)
###########################################################
# Validate that all unreleased commits (since the latest tag) conform to the
# Conventional Commits spec. cliff.toml sets require_conventional=true, so
# git-cliff exits non-zero on a non-conventional commit. Read-only.
git-cliff --unreleased --output /dev/null

###########################################################
# DOC (ci.yml: doc)
###########################################################
cargo doc --features bladerf1 --no-deps

###########################################################
# EXAMPLES (ci.yml: build-examples)
###########################################################
for manifest in examples/*/Cargo.toml; do
  cargo build --manifest-path "$manifest"
done

###########################################################
# DENY (ci.yml: deny)
###########################################################
cargo deny check

###########################################################
# AUDIT (ci.yml: audit)
###########################################################
cargo audit
