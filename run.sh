#!/bin/bash

# cleanup on ctrl+c
trap handle_sigint SIGINT

function handle_sigint() {
    echo "Caught Ctrl+C! Cleaning up..."
    mv $file "out/not_terminating_${seed}${file_ext}"
    echo "Saved file as: out/not_terminating_${seed}${file_ext}"
    echo "Not terminating with seed: $seed"
    exit 1
}

# clean up dir
echo "Cleaning up output directory..."
rm -r out/*
mkdir out/sat
mkdir out/unsat

echo "Building release executable..."
cargo build --release
target/release/fuzzer_orchestrator -o out/
