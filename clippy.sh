#!/usr/bin/env bash
set -euo pipefail

# Compatibility wrapper. Main logic lives in Rust xtask.
exec cargo xtask ci "$@"
