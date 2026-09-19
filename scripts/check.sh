#!/bin/bash

# Bash debug mode
set -xe

# Run from the repository root so relative paths (cliff.toml, CHANGELOG.md)
# resolve regardless of the invocation directory.
cd "$(dirname "$0")/.."

###########################################################
# TOOLCHAINS
###########################################################
# CI uses the latest stable (dtolnay/rust-toolchain@stable). Run everything
# on stable regardless of the local default toolchain, and try to bring it
# up to date first so new default-warn lints are caught here, not in CI.
# Explicit `cargo +nightly` invocations below override this variable.
export RUSTUP_TOOLCHAIN=stable
if ! rustup update stable; then
  echo "WARNING: could not update the stable toolchain; CI may run a newer stable with additional lints." >&2
fi
rustc --version
cargo +nightly --version

# cargo clean

###########################################################
# BUILD
###########################################################
cargo build --features bladerf1

###########################################################
# CROSS-COMPILE BUILD (verify supported targets)
###########################################################
rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-pc-windows-gnu wasm32-unknown-unknown
cargo build --target x86_64-unknown-linux-gnu --features bladerf1 --lib
cargo build --target aarch64-unknown-linux-gnu --features bladerf1 --lib
cargo build --target x86_64-pc-windows-gnu --features bladerf1 --lib
# WebUSB (needs --cfg=web_sys_unstable_apis, supplied by .cargo/config.toml)
cargo check --target wasm32-unknown-unknown --features bladerf1 --lib

###########################################################
# TEST
###########################################################
# Unit tests (no hardware)
cargo test --lib
# Protocol encode/decode tests (no hardware)
cargo test --test unit
# Public API contract (no hardware)
cargo test --test public_api --features bladerf1
# Hardware integration tests (single-threaded, shared device), default `smol`.
# Skipped in CI, where no device is attached.
if [ -z "$CI" ]; then
  cargo test --features bladerf1 --tests -- --test-threads=1
  # Same suite plus the async tests with nusb's tokio integration only
  cargo test --no-default-features --features bladerf1,xb100,xb200,xb300,tokio --tests -- --test-threads=1
fi

###########################################################
# CLIPPY
###########################################################
# Stable is the gate CI enforces.
cargo clippy --features bladerf1 --all-targets -- -D warnings
cargo clippy --no-default-features --features bladerf1,xb100,xb200,xb300,tokio --all-targets -- -D warnings
# Nightly clippy is the early warning: lints that are warn-by-default on
# nightly today become CI failures on the next stable. Fix them now or
# `#[allow]` them deliberately.
cargo +nightly clippy --features bladerf1 --all-targets -- -D warnings
cargo +nightly clippy --no-default-features --features bladerf1,xb100,xb200,xb300,tokio --all-targets -- -D warnings

###########################################################
# FMT
###########################################################
# rustfmt.toml enables nightly-only options; check with the toolchain that
# honours them.
cargo +nightly fmt --all --check

###########################################################
# CONVENTIONAL COMMITS
###########################################################
# Validate that all unreleased commits (since the latest tag) conform to the
# Conventional Commits spec. cliff.toml sets require_conventional=true, so
# git-cliff exits non-zero on a non-conventional commit. Read-only.
git-cliff --unreleased --output /dev/null

###########################################################
# DOC
###########################################################
cargo doc --features bladerf1 --no-deps

###########################################################
# EXAMPLES
###########################################################
for manifest in examples/*/Cargo.toml; do
  pkg=$(grep '^name = ' "$manifest" | head -1 | sed 's/name = "\(.*\)"/\1/')
  cargo build -p "$pkg" 2>/dev/null || cargo build --manifest-path "$manifest"
done

###########################################################
# BENCH
###########################################################
# cargo bench --features bladerf1 --bench nios_packet_bench --bench sample_format_bench --bench metadata_header_bench

###########################################################
# DENY
###########################################################
cargo deny check

###########################################################
# AUDIT
###########################################################
# Install cargo-audit
# cargo install cargo-audit
# Run security audit
cargo audit
