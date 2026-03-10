#!/bin/bash

# cleanup on ctrl+c
trap handle_sigint SIGINT

file_ext=".opb"
file="out/test${file_ext}"
function handle_sigint() {
    echo "Caught Ctrl+C! Cleaning up..."
    mv $file "out/error/not_terminating${file_ext}"
    echo "Saved file as: out/error/not_terminating${file_ext}"
    exit 1
}

echo "Building release executable..."
cargo build --release
target/release/fuzzer_orchestrator -l info -o out/ -t 1 --settings-file configuration/settings.ron
