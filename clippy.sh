#!/bin/bash
cargo clippy --workspace --all-features --bins --lib --tests -- -D warnings && cargo fmt