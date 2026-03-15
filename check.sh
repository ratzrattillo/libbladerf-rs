#!/bin/bash

# Bash debug mode
set -xe

find . -name "Cargo.lock" -delete

###########################################################
# TEST
###########################################################
# standard tests
cargo test --examples --all-features --all-targets -- # --no-capture

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
cargo doc --all-features --no-deps --lib --bins --examples

###########################################################
# AUDIT
###########################################################
# Install cargo-audit
# cargo install cargo-audit
# Run security audit
cargo audit