#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

make clean 2>/dev/null || true

./configure
make -j$(nproc)

# Static link for portability on GCP Batch VMs
gcc -static -o build/kissat-static build/main.o build/application.o build/handle.o     build/parse.o build/witness.o build/libkissat.a -lm 2>/dev/null     && cp build/kissat-static bin/solver_binary     || cp build/kissat bin/solver_binary

chmod +x bin/solver_binary
