This README documents the Docker setup for the artifact review.
For a general documentation, refer to the README.md file.

PBFuzz runs on multiple platforms and was tested on amd64 and arm64 architecture using the steps outlined in README.md.
However, the Docker container was only successfully tested on Linux and an amd64 architecture.
Specifically, the Docker container does not run on Apple arm64 devices.

### Docker

After installing and starting docker, the container can be built using the following command from PBFuzz's main directory.

```bash
docker build -t pbfuzz:final .
```

Building the container will take around half an hour. Afterwards, the container can be run using the following command from PBFuzz's main directory.

```bash
docker run -it -v "$(pwd)/results:/work" pbfuzz:final
```

This binds the "results" directory in PBFuzz's main directory to the "/work" dirctory inside the container. 
All results from PBFuzz runs in the provided configurations are placed in the "/work/out", i.e., "results/out" directory.

### Running Experiments

The Docker container provides scripts to reproduce the results in the folder "/app/run_scripts/".
We provide two versions for each experiment:
- The default version (<name>.sh) runs the configuration for 4 hours using 8 threads and is designed to allow faster evaluation.
- The full version (<name>_full.sh) uses the original configuration from the experiments with 12 hours and 64 threads, exact reproducibility would require a similar machine setup (32 physical, 64 virtual cores and a distributed file system in case of proofs).

The following configurations are provided:
/app/run_scripts/pbo_30s_no_proof.sh (PBO 30 s N)
/app/run_scripts/pbo_5s_all_proof.sh (PBO 5 s A)
/app/run_scripts/pbo_5s_first_proof.sh (PBO 5 s F)
/app/run_scripts/pbo_5s_no_proof.sh (PBO 5 s N)
/app/run_scripts/pbs_10s_first_proof.sh (PBS 10 s F)

### General Docker Container Environment

The container provides a fully functioning version of PBFuzz, as well as the following PB solvers:

| Solver      | Program                                                                 | Proof | Option Fuzzing |
|-------------|-------------------------------------------------------------------------|-------|----------------|
| Exact       | compiled [source](https://gitlab.com/nonfiction-software/exact)         | Yes   | Yes            |
| Roundingsat | compiled [source](https://gitlab.com/MIAOresearch/software/roundingsat) | Yes   | Yes            |
| Hitpbo      | compiled [source](https://bitbucket.org/coreo-group/hitpbo/src/main/)   | Yes   | Yes            |
| Sat4J       | jar      [source](https://gitlab.ow2.org/sat4j/sat4j/-/releases/)       | No    | No             |
| Naps        | compiled [source](https://gitlab.com/naps2040334/naps/-/tree/master)    | No    | Yes            |
| Scip        | compiled [source](https://github.com/scipopt/scip)                      | No    | No             |

The binaries for PBFuzz can be found in "/app/fuzzer", the binaries for the solvers are in "/opt/solvers/".
PBFuzz configurations are in "/app/configuration and solver configurations are in "/app/solver_configs"

We refer to the configurations mentioned above for example calls.
Generally the fuzzer is called using:

```bash
fuzzing_orchestrator \
  -t <time_limit> \
  --output-dir /work/out \
  --settings-file /app/configuration/<my_settings> \
  --program-config1 /app/solver_configs/<my_config> \
  --program-config2 /app/solver_configs/<my_other_config>
  # ...<other_args>
```

