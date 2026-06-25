#!/bin/bash

# Bash debug mode
set -xe

# Run from the repository root so relative paths (cliff.toml, CHANGELOG.md)
# resolve regardless of the invocation directory.
cd "$(dirname "$0")/.."

# cargo clean

###########################################################
# BUILD
###########################################################
cargo build --features bladerf1

###########################################################
# TEST
###########################################################
# Unit tests (no hardware)
cargo test --lib
# Protocol encode/decode tests (no hardware)
cargo test --test unit
# Hardware integration tests (single-threaded, shared device)
cargo test --features bladerf1 --tests -- --test-threads=1

###########################################################
# CLIPPY
###########################################################
cargo clippy --features bladerf1 --all-targets -- -D warnings

###########################################################
# FMT
###########################################################
cargo fmt --all --check

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
