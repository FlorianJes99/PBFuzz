#!/bin/bash

# cleanup on ctrl+c
trap handle_sigint SIGINT

# TODO: move ctrl+c handling to program with ctrlc crate
function handle_sigint() {
    echo "Caught Ctrl+C! Exiting..."
    exit 1
}

# Convenience script for building the program and then running the binary with some cli args
echo "Building release executable..."
cargo build --release
target/release/fuzzer_orchestrator \
  -i 100 \
  --settings-file configuration/default_settings.toml \
  --program-config1 external_programs/roundingsat.toml \
  # --program-config2 external_programs/scip.toml
