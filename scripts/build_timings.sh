#!/usr/bin/env bash
set -euo pipefail

export CARGO_BUILD_TIMINGS=html
cargo build --workspace

TIMINGS_DIR="target/cargo-timings"
echo "Build timings HTML: ${TIMINGS_DIR}"