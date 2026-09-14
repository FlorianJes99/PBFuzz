# syntax=docker/dockerfile:1.6
#
# Multi-solver PB test-harness image.
# Each solver is built (or fetched) in its own stage; the final runtime
# image only carries the compiled binaries + minimal runtime libs.

ARG DEBIAN_VERSION=trixie

########################################
# Common C++ build base
########################################
FROM debian:${DEBIAN_VERSION} AS build-base
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential cmake git ca-certificates curl \
        libboost-all-dev libgmp-dev zlib1g-dev pkg-config libtbb-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src

########################################
# Exact — pinned commit (currently latest on main at time of pinning)
########################################
FROM build-base AS build-exact
ARG EXACT_REF=b921cd1c4e3b6a7b7ba16dfd9c38c419d5e53ee4
RUN git clone https://gitlab.com/nonfiction-software/exact.git . \
    && git checkout ${EXACT_REF} \
    && cd build \
    && cmake .. -DCMAKE_BUILD_TYPE=Release \
    && make -j"$(nproc)" \
    && mkdir -p /out && cp Exact /out/exact \
    && echo ${EXACT_REF} > /out/exact.version
    # verify actual binary name/path against the real build output

########################################
# Scip — installed from .deb file
########################################
FROM debian:${DEBIAN_VERSION}-slim AS fetch-scip
ARG SCIP_VERSION=10.0.3
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && curl -fsSL -o /tmp/scip.deb \
       https://www.scipopt.org/download/release/scipoptsuite_${SCIP_VERSION}-1+trixie_amd64.deb
RUN apt-get update && apt-get install -y --no-install-recommends /tmp/scip.deb \
    && rm -rf /var/lib/apt/lists/* \
    && echo ${SCIP_VERSION} > /scip.version

########################################
# RoundingSat — pinned commit (currently latest on master at time of pinning)
########################################
FROM build-base AS build-roundingsat
ARG ROUNDINGSAT_REF=d4edbf7908a9bb951fd181940919e0f3ac7ab1ee
RUN git clone https://gitlab.com/MIAOresearch/software/roundingsat.git . \
    && git checkout ${ROUNDINGSAT_REF} \
    && cd build \
    && cmake .. -DCMAKE_BUILD_TYPE=Release \
    && make -j"$(nproc)" \
    && mkdir -p /out && cp roundingsat /out/roundingsat \
    && echo ${ROUNDINGSAT_REF} > /out/roundingsat.version

########################################
# Hitpbo — pinned commit (currently latest on master at time of pinning)
########################################
FROM build-base AS build-hitpbo
COPY --from=fetch-scip /tmp/scip.deb /tmp/scip.deb
RUN apt-get update && apt-get install -y --no-install-recommends /tmp/scip.deb \
    && rm -rf /var/lib/apt/lists/* /tmp/scip.deb
ARG HITPBO_REF=e615c28a617147e772b3494ced9bf6a71111842b
RUN git clone https://coreo-group@bitbucket.org/coreo-group/hitpbo.git . \
    && git checkout ${HITPBO_REF} \
    # Force Git to use HTTPS for submodules
    && git config --global url."https://bitbucket.org/".insteadOf "git@bitbucket.org:" \
    && git config --global url."https://github.com/".insteadOf "git@github.com:" \
    && ./init_modules.sh \
    && make -j"$(nproc)" WITH_GUROBI=0 WITH_CPLEX=0 WITH_SCIP=1 WITH_PAPILO=0 \
    && mkdir -p /out && cp hitpbo /out/hitpbo \
    && echo ${HITPBO_REF} > /out/hitpbo.version

########################################
# Naps — pinned to a tag
########################################
FROM build-base AS build-naps
COPY --from=fetch-scip /tmp/scip.deb /tmp/scip.deb
RUN apt-get update && apt-get install -y --no-install-recommends /tmp/scip.deb \
    && rm -rf /var/lib/apt/lists/* /tmp/scip.deb
ARG NAPS_TAG=1.4.1
RUN git clone --depth 1 --branch ${NAPS_TAG} \
        https://gitlab.com/naps2040334/naps.git . \
    && make r -j"$(nproc)" \
    && mkdir -p /out && cp naps_release-${NAPS_TAG} /out/naps \
    && echo ${NAPS_TAG} > /out/naps.version

########################################
# PrinTeMPS — pinned commit, Rust toolchain
########################################
FROM rust:1-${DEBIAN_VERSION} AS build-printemps
RUN apt-get update && apt-get install -y --no-install-recommends \
        git ca-certificates cmake build-essential libboost-all-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
ARG PRINTEMPS_REF=567fc31c02591a1292e2015cb9c69627a7bfead0
RUN git clone https://github.com/msakai/printemps-pb26.git . \
    && git checkout ${PRINTEMPS_REF} \
    && git submodule update --init \
    && BUILD_SCIP_PRINTEMPS=OFF ./build.sh \
    && mkdir -p /out \
    && cp bin/pb_competition_2025_solver /out/printemps \
    && echo ${PRINTEMPS_REF} > /out/printemps.version

########################################
# Sat4J — prebuilt jar
########################################
FROM debian:${DEBIAN_VERSION}-slim AS fetch-sat4j
ARG SAT4J_VERSION=2.3.6
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl unzip \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /out \
    && curl -fsSL -o sat4j-pb.zip \
        https://release.ow2.org/sat4j/sat4j-pb-v20201214.zip \
    && unzip sat4j-pb.zip \
    && cp sat4j-pb.jar /out/sat4j.jar \
    && echo ${SAT4J_VERSION} > /out/sat4j.version

########################################
# Fuzzing harness
########################################
FROM rust:1-${DEBIAN_VERSION} AS build-fuzzer
WORKDIR /fuzz_pb
COPY ./src ./src
COPY ./build.rs ./build.rs
COPY ./Cargo.toml ./Cargo.lock ./
RUN mkdir -p /out \
    && cargo build --release \
    && cp target/release/fuzzer_orchestrator target/release/fuzzer /out/

########################################
# Final runtime image
########################################
FROM debian:${DEBIAN_VERSION}-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        libboost-program-options-dev libgmp10 \
        libboost-all-dev \
        default-jre-headless \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=fetch-scip /tmp/scip.deb /tmp/scip.deb
RUN apt-get update && apt-get install -y --no-install-recommends /tmp/scip.deb \
    && rm -rf /var/lib/apt/lists/* /tmp/scip.deb

COPY --from=build-exact       /out/ /opt/solvers/exact/
COPY --from=build-roundingsat /out/ /opt/solvers/roundingsat/
COPY --from=build-hitpbo      /out/ /opt/solvers/hitpbo/
COPY --from=build-naps        /out/ /opt/solvers/naps/
COPY --from=build-printemps   /out/ /opt/solvers/printemps/
COPY --from=fetch-sat4j       /out/ /opt/solvers/sat4j/
COPY --from=build-fuzzer      /out/ /opt/harness/

RUN chmod +x /opt/solvers/exact/exact \
             /opt/solvers/roundingsat/roundingsat \
             /opt/solvers/hitpbo/hitpbo \
             /opt/solvers/naps/naps \
             /opt/solvers/printemps/printemps \
             /opt/harness/fuzzer \
             /opt/harness/fuzzer_orchestrator \
     && ln -s /opt/solvers/exact/exact             /usr/local/bin/exact \
     && ln -s /opt/solvers/roundingsat/roundingsat /usr/local/bin/roundingsat \
     && ln -s /opt/solvers/hitpbo/hitpbo           /usr/local/bin/hitpbo \
     && ln -s /opt/solvers/naps/naps               /usr/local/bin/naps \
     && ln -s /opt/solvers/printemps/printemps     /usr/local/bin/printemps \
     && ln -s /opt/harness/fuzzer_orchestrator     /usr/local/bin/fuzzer_orchestrator \
     && ln -s /opt/harness/fuzzer                  /usr/local/bin/fuzzer

RUN printf '#!/bin/sh\nexec java -jar /opt/solvers/sat4j/sat4j.jar "$@"\n' \
        > /usr/local/bin/sat4j && chmod +x /usr/local/bin/sat4j

# Copy config files needed for the solvers
COPY ./configuration /app/configuration

# Copy solver configs
COPY ./external_programs /app/external_programs

WORKDIR /work

ENTRYPOINT ["/bin/bash"]
