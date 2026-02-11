#!/bin/bash

# cleanup on ctrl+c
trap handle_sigint SIGINT

file_ext=".opb"
file="out/test${file_ext}"
function handle_sigint() {
    echo "Caught Ctrl+C! Cleaning up..."
    mv $file "out/not_terminating${file_ext}"
    echo "Saved file as: out/not_terminating${file_ext}"
    exit 1
}

# clean up dir
echo "Cleaning up output directory..."
rm -r out/*
mkdir out/sat
mkdir out/unsat

echo "Building release executable..."
cargo build --release
target/release/fuzzer_orchestrator -o out/ -i 100 --settings-file configuration/settings.ron
