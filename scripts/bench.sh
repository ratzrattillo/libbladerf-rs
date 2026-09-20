#!/bin/bash

# Bash debug mode
set -xe

# Run from the repository root so relative paths resolve regardless of the
# invocation directory.
cd "$(dirname "$0")/.."

# Run on the latest stable, like check.sh.
export RUSTUP_TOOLCHAIN=stable
rustc --version

# Extra criterion args, e.g. BENCH_ARGS=--test runs each benchmark once
# without measuring (useful as a smoke test).
BENCH_ARGS="${BENCH_ARGS:-}"

###########################################################
# PURE COMPUTATION (no hardware required)
###########################################################
cargo bench --bench nios_packet_bench --bench sample_format_bench --bench metadata_header_bench -- $BENCH_ARGS

###########################################################
# HARDWARE (requires a connected BladeRF1; run one at a time
# — they share one physical device)
###########################################################
# Skipped in CI, where no device is attached.
if [ -z "$CI" ]; then
  cargo bench --features bladerf1 --bench hardware_gpio_bench -- $BENCH_ARGS
  cargo bench --features bladerf1 --bench hardware_tuning_bench -- $BENCH_ARGS
  cargo bench --features bladerf1 --bench hardware_stream_latency_bench -- $BENCH_ARGS
  cargo bench --features bladerf1 --bench hardware_stream_build_teardown_bench -- $BENCH_ARGS
  cargo bench --features bladerf1 --bench hardware_gain_bench -- $BENCH_ARGS
  cargo bench --features bladerf1 --bench hardware_calibration_bench -- $BENCH_ARGS
fi
