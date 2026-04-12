#!/bin/bash
# Single-process portfolio mode test script

PORTABLE_HOME="/mnt/d/wsl-code/my-painless/painless"

echo "========================================"
echo "Testing PortfolioSimple Single-Process Mode"
echo "========================================"

# Test 1: Small case - 42-121369.cnf (10 MB)
echo ""
echo "Test 1: Running with 42-121369.cnf (10MB)"
echo "Timeout: 30 seconds"
timeout 30 "${PORTABLE_HOME}/build/release/painless_release" \
    --cpus 8 \
    --shr-strat 3 \
    --gstrat -1 \
    "/mnt/d/wsl-code/sat_benchmark_local/42-121369.cnf" || true

echo "Test 1 completed with exit code: $?"

# Test 2: Small case - x9-03065.sat.sanitized.cnf (18 KB)
echo ""
echo "Test 2: Running with x9-03065.sat.sanitized.cnf (18KB)"
timeout 10 "${PORTABLE_HOME}/build/release/painless_release" \
    --cpus 8 \
    --shr-strat 3 \
    --gstrat -1 \
    "/mnt/d/wsl-code/sat_benchmark_local/x9-03065.sat.sanitized.cnf" || true

echo "Test 2 completed with exit code: $?"

# Test 3: Small case - unif-c1275-v300-s428434218.cnf (6 KB)
echo ""
echo "Test 3: Running with unif-c1275-v300-s428434218.cnf (6KB)"
timeout 10 "${PORTABLE_HOME}/build/release/painless_release" \
    --cpus 8 \
    --shr-strat 3 \
    --gstrat -1 \
    "/mnt/d/wsl-code/sat_benchmark_local/unif-c1275-v300-s428434218.cnf" || true

echo "Test 3 completed with exit code: $?"

