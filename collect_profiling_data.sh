timestamp=$(date +%Y_%m_%d_%H_%M_%S)
binary=software/roundingsat_debug_cov
merged_data=snapshots/${timestamp}_merged.profdata

echo "Deleting old report..."
rm -r coverage_report/*

# Merge all profraw files
echo "Merging raw profiling data..."
llvm-profdata merge -sparse profiles/*.profraw -o $merged_data

echo "Export data to json"
llvm-cov export $binary \
    -instr-profile=$merged_data \
    -format=text > snapshots/${timestamp}_snapshot.json

# Generate HTML report
echo "Generating a HTML report about profiled data..."
llvm-cov show $binary \
    -instr-profile=$merged_data \
    -format=html \
    -output-dir=coverage_report

# Or just a summary in the terminal
llvm-cov report $binary -instr-profile=$merged_data

echo "Deleting raw profile data"
rm profiles/*

firefox coverage_report/index.html &
