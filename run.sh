#!/bin/bash

# cleanup on ctrl+c
trap handle_sigint SIGINT

function handle_sigint() {
    echo "Caught Ctrl+C! Exiting..."
    exit 1
}

echo "Building release executable..."
cargo build --release
target/release/fuzzer_orchestrator -o out/ -t 1 --settings-file configuration/default_settings.ron --scip-bin software/scip
