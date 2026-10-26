fuzzer_orchestrator \
  -t 720 \
  --thread-count 8 \
  --output-dir /work/out \
  --log warn \
  --proofs one \
  --timeout-solver-secs 5 \
  --settings-file /app/configuration/default_settings.toml \
  --solver-configs /app/solver_configs/exact.toml /app/solver_configs/hitpbo.toml /app/solver_configs/roundingsat.toml /app/solver_configs/sat4j.toml /app/solver_configs/naps.toml /app/solver_configs/scip.toml
