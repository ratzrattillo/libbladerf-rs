#!/bin/bash

set -xe

SCRIPT=$(readlink -f $0)
SCRIPTPATH=`dirname $SCRIPT`

cd ${SCRIPTPATH} && find . -name "Cargo.lock" -delete

CARGO_FMT="cargo fmt"

###########################################################
# FMT
###########################################################
cd ${SCRIPTPATH} && ${CARGO_FMT} --check

###########################################################
# CLIPPY
###########################################################
cd ${SCRIPTPATH} && cargo clippy --all-targets -- -D warnings
# examples
cd ${SCRIPTPATH}/examples && cargo clippy --all-targets -- -D warnings

###########################################################
# Test
###########################################################
cd ${SCRIPTPATH} && cargo test --all-targets
# examples
cd ${SCRIPTPATH}/examples && cargo test --all-targets