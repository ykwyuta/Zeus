# Container environment for running Hercules-based validation suites:
#   - ZHVS (HLASM Validation Suite) on bare-metal Hercules (z/Arch)
#   - ZPVS (PL/I Validation Suite) on MVS 3.8j (OS/360 PL/I (F) V5.5 PGM=IEMAA)
#   - zeus-hercdiff packed-decimal instruction oracle
#
# Build and run via:
#   tools/hercules-Suites.sh build
#   tools/hercules-suites.sh zhvs
#   tools/hercules-suites.sh zpvs --mvs-dasd /path/to/MVSR38/Dasd

FROM rust:1-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
        hercules \
        python3 \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
