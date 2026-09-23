#!/bin/bash

# Convenience script for building the program and then running the binary with some cli args
echo "Building release executable..."
cargo build --release
echo "Running program..."
target/release/fuzzer_orchestrator \
  -i 200 \
  --thread-count 10 \
  --log warn \
  --timeout-external-program-secs 5 \
  --settings-file configuration/default_settings.toml \
  --solver-configs external_programs/exact.toml external_programs/hitpbo.toml  external_programs/roundingsat.toml external_programs/sat4j.toml external_programs/naps.toml external_programs/scip.toml
  # --save-all-files \
  # --proofs none \
