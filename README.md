# PBFuzz - A fuzz-testing framework for Pseudo-Boolean solvers

This project presents a fast and easy to use framework for testing Pseudo-Boolean Solvers.
It includes a fuzzer generating PB problems.
The fuzzer can be used standalone or integrated into the orchestrator.
The orchestrator tests generated instances on the provided solvers.

## Fuzzer for Pseudo Boolean solvers
The fuzzer generates valid pseudo boolean problems in `.opb` format.
An example file for configuration can be found [here](configuration/default_settings.toml).
Currently generation supports two different modes.
Instances are either generated random or based on a planted solution.

## Fuzzing Orchestrator
The orchestrator is used to fuzz test different solvers.

### Usage
The orchestrator can be used in the following way:
```bash
fuzzer_orchestrator \
  -t 600 \ # fuzz time in minuntes  
  --thread-count 8 \ # worker threads used
  --log warn \ # enable logs of level warn and upwards
  --proofs one \ # only first supported solver is used for proof generation and validation
  --timeout-solver-secs 30 \ # terminates a solver after 30 seconds
  --option-fuzzing-perc 0.1 \ # option fuzzing in a iteration with 10 % chance
  --settings-file configuration/default_settings.toml \ # settings for the fuzzer
  --solver-configs solver_configs/exact.toml  solver_configs/hitpbo.toml solver_configs/roundingsat.toml solver_configs/naps.toml solver_configs/sat4j.toml solver_configs/scip.toml # all solvers used for this run
```
Each solver used, needs to be configured in a seperate file.
Predefined configuration files for solvers are stored [here](solver_configs/).
```toml
binary = "path/to/binary"
args = ["--argument1", "value1", "--argument2=valu2"]
name = "exact"
parser_kind = "Default" # or SCIP
# optional
proof_arg = "--proof-log="

[optional_args]
args = [
    { arg_name = "--flag" },
    { arg_name = "--set-of-values", arg_type = { Values = [
        "value1",
        "value2",
        "value3"
    ]} },
    { arg_name = "--value-range-int", arg_type = { IntRange = [ 1, 50 ] } },
    { arg_name = "--value-range-float", arg_type = { FloatRange = [0.0, 1.0] } },
]
space_separated = true
```
### PB Solver Compatability
This gives an overview about the predefined configurations in the [config folder](solver_configs/)

| Solver      | Program                                                                 | Proof | Option Fuzzing |
|-------------|-------------------------------------------------------------------------|-------|----------------|
| Exact       | compiled [source](https://gitlab.com/nonfiction-software/exact)         | Yes   | Yes            |
| Roundingsat | compiled [source](https://gitlab.com/MIAOresearch/software/roundingsat) | Yes   | Yes            |
| Hitpbo      | compiled [source](https://bitbucket.org/coreo-group/hitpbo/src/main/)   | Yes   | Yes            |
| Sat4J       | jar      [source](https://gitlab.ow2.org/sat4j/sat4j/-/releases/)       | No    | No             |
| Naps        | compiled [source](https://gitlab.com/naps2040334/naps/-/tree/master)    | No    | Yes            |
| Scip        | compiled [source](https://github.com/scipopt/scip)                      | No    | No             |

### Docker

Use the [Dockerfile](Dockerfile) to create a reproducible environment for running the orchestrator.

```bash
docker build -t pbfuzz:final .
```

When starting a container, [bind mount](https://docs.docker.com/engine/storage/bind-mounts/) a directory to it.
The output of the fuzzing run should be placed in this directory.

```bash
docker run -it -v "$(pwd)/results:/work" pbfuzz:final
```

Inside the docker container all solvers binaries are accessible. Also the fuzzing orchestrator (fuzzer_orchestrator binary)
and the fuzzer binary itself. Also the settings files and the solver configurations stored in this directory are copied
into the container at `/app/`.
Normally you would want to start a run like this:

```bash
fuzzing_orchestrator \
  -t 600 \
  --output-dir /work/out \
  --settings-file /app/configuration/<my_settings> \
  --program-config1 /app/solver_configs/<my_config> \
  --program-config2 /app/solver_configs/<my_other_config>
  # ...<other_args>
```
