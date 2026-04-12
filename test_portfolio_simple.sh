#!/bin/bash
# Simple PortfolioSimple Parallel Mode Test Script
#
# This script tests the portfolio parallel strategy WITHOUT MPI.
# It runs directly with the Painless executable.

set -e

SCRIPT_DIR="$(dirname "$0")"
PAINLESS_DEBUG="${SCRIPT_DIR}/build/debug/painless_debug"
PAINLESS_RELEASE="${SCRIPT_DIR}/build/release/painless_release"

# Use debug version by default
PAINLESS="${PAINLESS_DEBUG}"
if [ ! -f "$PAINLESS" ]; then
    echo "Error: Debug version not found: $PAINLESS"
    exit 1
fi

echo "Using Painless: $PAINLESS"
echo "================================================"

# Test case function
run_test() {
    local cnf_file=$1
    local timeout=${2:-30}
    local cpus=${3:-8}

    echo "------------------------------------------------"
    echo "Test Case: $cnf_file"
    echo "Timeout: ${timeout}s, Threads: $cpus"
    echo "Commands: $PAINLESS --cpus $cpus --v 1 '$cnf_file'"
    echo "------------------------------------------------"

    timeout "$timeout" "$PAINLESS" \
        --cpus "$cpus" \
        --verbosity 1 \
        "$cnf_file" || true

    local exit_code=$?

    if [ $exit_code -eq 124 ]; then
        echo "→ TIMEOUT after ${timeout}s"
    elif [ $exit_code -ne 0 ] && [ $exit_code -ne 2 ]; then
        echo "→ Exit code: $exit_code (non-standard)"
    fi

    echo ""
}

# Main test suite
echo "========================================"
echo "PortfolioSimple Mode Tests"
echo "========================================"

# Test 1: Very small instance
run_test "/mnt/d/wsl-code/sat_benchmark_local/x9-03065.sat.sanitized.cnf" 10 4

# Test 2: Small instance (6 KB)
run_test "/mnt/d/wsl-code/sat_benchmark_local/unif-c1275-v300-s428434218.cnf" 15 8

# Test 3: Medium instance (10 MB)
run_test "/mnt/d/wsl-code/sat_benchmark_local/42-121369.cnf" 60 8

# Test 4: Another medium instance
run_test "/mnt/d/wsl-code/sat_benchmark_local/vlsat2_16297_1562268.dimacs.cnf" 90 8

echo "========================================"
echo "All tests completed"
echo "========================================"

# Additional strategy variations (can run separately)
test_with_strategy() {
    local strategy=$1
    echo "Testing with sharing strategy $strategy"
    timeout 30 "$PAINLESS" \
        --cpus 8 \
        --shr-strat "$strategy" \
        --v 0 \
        "/mnt/d/wsl-code/sat_benchmark_local/42-121369.cnf" || true
}

# Test different sharing strategies on one instance
echo "Testing strategy variations on unif-c1275-v300-s428434218.cnf:"
for strat in 1 2 3; do
    echo "  Strategy $strat:"
    timeout 15 "$PAINLESS" --cpus 8 --shr-strat "$strat" --v 0 "/mnt/d/wsl-code/sat_benchmark_local/unif-c1275-v300-s428434218.cnf" || true
done

echo "================================================"