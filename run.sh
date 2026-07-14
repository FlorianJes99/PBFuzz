#!/bin/bash

# Convenience script for building the program and then running the binary with some cli args
echo "Building release executable..."
cargo build --release
target/release/fuzzer_orchestrator \
  -i 100 \
  --log info \
  --thread-count 12 \
  --timeout-external-program-secs 10 \
  --settings-file configuration/default_settings.toml \
  --program-config1 external_programs/exact.toml \
  --program-config2 external_programs/roundingsat.toml
