# Fuzing Framework for Pseudo Boolean Solvers

Simple fuzzer to generate random inputs for solvers.
Use the [shell script](run.sh) to start generating input.
Ath the moment only the roundingsat solver is tested.

## Fuzzer for Pseudo Boolean solvers

The fuzzer generates valid pseudo boolean problems in `.opb` format.
Currently generation supports two different modes.

### Random Generation

### From Solution Generation

## Fuzzing Harness

The harness is used to fuzz test different solvers. It support two different validation modes.

### Modes

#### Comparison

In `Comparison Mode` to solvers are run and there output is compared. This can detect some faulty behavior, especially
when testing one "battle-tested" solver against a fairly new one. But there is no guarantee that same output means correctness.

#### Proof

In `Proof Mode` the solver generates a proof and this is checked by [veripb](https://gitlab.com/MIAOresearch/software/VeriPB).
In order to use this mode, the solver needs to support the generation of a proof file.

### PB Solver Compatability

| Solver      | Binary                                                                  | Supported | Proof | Option Fuzzing | Version | Notes              |
|-------------|-------------------------------------------------------------------------|-----------|-------|----------------|---------|--------------------|
| Exact       | compiled [source](https://gitlab.com/nonfiction-software/exact)         | Yes       | Yes   | Yes            | ?       |                    |
| Roundingsat | compiled [source](https://gitlab.com/MIAOresearch/software/roundingsat) | Yes       | Yes   | Yes            | ?       |                    |
| Hitpbo      | compiled [source](https://bitbucket.org/coreo-group/hitpbo/src/main/)   | Yes       | Yes   | Yes            | ?       |                    |
| Naps        | compiled [source](https://gitlab.com/naps2040334/naps/-/tree/master)    | Yes       | No    | Yes            | ?       |                    |
| Sat4J       | jar      [source](https://gitlab.ow2.org/sat4j/sat4j/-/releases/)       | Yes       | No    | No             | ?       |                    |
| Scip        | compiled [source](https://github.com/scipopt/scip)                      | Yes       | No    | No             | ?       |                    |
| Printtemps  | compiled [source](https://github.com/msakai/printemps-pb26)             | Yes       | No    | No             | ?       | No UNSAT answer    |
| Toulbar     | compiled [source](https://github.com/toulbar2/toulbar2)                 | No        | No    | No             | ?       | Need custom parser |
| Picat       | ?                                                                       | No        | ?     | ?              | ?       |                    |
| Mixed-bag   | ?                                                                       | No        | ?     | ?              | ?       |                    |

#### Printtemps

This solver can only detect SAT/Optimum, therefore it is adviced to use problem generation mode 'FromSolution'.
This only generates satisfiable problems.

### Docker

Use the [Dockerfile](Dockerfile) to create a reproducible environment for running the harness.

```bash
docker build -t pb-harness:final-harness .
```

When starting a container, [bind mount](https://docs.docker.com/engine/storage/bind-mounts/) a directory to it.
The output of the fuzzing run should be placed in this directory.

```bash
docker run -it -v "$(pwd)/results:/work" pb-harness:final-harness
```

Inside the docker container all solvers binaries are accessible. Also the fuzzing harness (fuzzer_orchestrator binary)
and the fuzzer binary itself. Also the settings files and the solver configurations stored in this directory are copied
into the container at `/app/`.
Normally you would want to start a run like this:

```bash
fuzzing_orchestrator \
  --output-dir /work/out \
  --settings-file /app/configuration/<my_settings> \
  --program-config1 /app/external_programs/<my_config> \
  --program-config2 /app/external_programs/<my_other_config>
  # ...<other_args>
```

**Currently work in progress...** 
