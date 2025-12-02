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

FUZZER="./target/release/fuzzer"
ROUNDING_SAT="roundingsat"

file_ext=".opb"
file="out/test${file_ext}"
amount_vars=100

counter=1
max_iters=1000

threshold_ms=500

long_running_instances=0
sat_instances=0
unsat_instances=0

# clean up dir
echo "Cleaning up output directory..."
rm -r out/*
mkdir out/sat
mkdir out/unsat

echo "Building release executable..."
cargo build --release
while true; do
    # 30 bit large seeds
    seed=$(( (RANDOM << 15) | RANDOM ))
    echo "========================================"
    echo "Iteration $counter"
    echo "========================================"
    echo ""
    echo "Running Fuzzer with seed $seed..."
    $FUZZER --seed $seed --output-path  $file --amount-vars $amount_vars

    echo "Running PB Solver..."

    start_time=$(date +%s%3N)          # milliseconds since epoch
    output=$($ROUNDING_SAT $file)
    end_time=$(date +%s%3N)            # milliseconds since epoch
    runtime_ms=$((end_time - start_time))

    if ((runtime_ms > threshold_ms)); then
        echo "PB Solver needed more than $threshold_ms ms to solve formula!"
        echo "Saving generated file..."
        mv $file "out/long_running_${runtime_ms}ms_${seed}${file_ext}"
        ((long_running_instances++))
    elif echo "$output" | grep -q "UNSATISFIABLE"; then
        echo "PB Solver says UNSATISFIABLE! Saving file with specified seed."
        mv $file "out/unsat/sat_${seed}_${counter}${file_ext}"
        ((unsat_instances++))

    elif echo "$output" | grep -q "SATISFIABLE"; then
        echo "PB Solver says SATISFIABLE! Saving file with specified seed."
        mv $file "out/sat/sat_${seed}_${counter}${file_ext}"
        ((sat_instances++))
        
    else
        echo "Output is neither SATISFIABLE nor UNSATISFIABLE. Saving to file."
        mv $file "out/crash_${seed}${file_ext}"
      
    fi
    if ((counter >= max_iters)); then
      echo "Max iterations ($counter) reached"
      break
    fi

    ((counter++))
    echo ""
done

echo "===============Statistics==============="
echo "Total amount of instances generated: $counter"
echo "Gerated instances that took longer than $threshold_ms ms: $long_running_instances"
echo "Generated amount of SAT instances: $sat_instances"
echo "Generated amount of UNSAT instances: $unsat_instances"
percentage_sat=$(awk "BEGIN {printf \"%.2f\", $sat_instances / $counter * 100}")
echo "Percentage SAT: $percentage_sat%"
echo "========================================"
