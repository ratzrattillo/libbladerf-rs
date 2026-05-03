#!/bin/bash

# Bash debug mode
set -xe

find . -name "Cargo.lock" -delete

###########################################################
# TEST
###########################################################
# standard tests
# Use --jobs=1 because integration tests all share the same BladeRF hardware device
cargo test --features bladerf1 --examples --lib --tests --jobs=1 -- --test-threads=1

###########################################################
# CLIPPY
###########################################################
cargo clippy --all-targets -- -D warnings

###########################################################
# FMT
###########################################################
cargo fmt --all --check

###########################################################
# DOC
###########################################################
cargo doc --features bladerf1 --no-deps --lib --bins --examples

###########################################################
# BENCH
###########################################################
cargo bench --features bladerf1 --bench nios_packet_bench --bench sample_format_bench --bench metadata_header_bench

###########################################################
# AUDIT
###########################################################
# Install cargo-audit
# cargo install cargo-audit
# Run security audit
cargo audit