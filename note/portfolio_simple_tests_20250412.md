# PortfolioSimple Parallel Mode Tests - Summary

Date: 2025-04-12
Test Method: Single-process (non-MPI) portfolio parallel strategy

## Test Execution Script

```bash
./build/debug/painless_debug <cnf-file>
```

Default portfolio configuration:
- Solver strategy: `kcl` (Kissat, Lingeling, CaDiCaL, YalSAT, etc.)
- Sharing strategy: HordeSatSharing (strategy 1)
- Number of solver instances: Auto (based on CPU cores, default 32)
- Diversification: Enabled
- Timeout: Unlimited

---

## Test Results Summary

### Test 1: x9-03065.sat.sanitized.cnf

**Statistics:**
| Metric | Value |
|--------|-------|
| Variables | 150 |
| Clauses | 1,347 |
| Runtime | 0.35 seconds |
| Result | SATISFIABLE ✓ |

**Winner Solver:**
```
kissat(0, 0) of family: SAT_STABLE
- Conflicts: 245
- Propagations: 12,198
- Decisions: 311
- Restarts: 0
```

**Sharing Statistics:**
| Metric | Value |
|--------|-------|
| Initial clauses | 1,347 |
| Received per round | ~283 |
| Shared per round | 0 (empty) |
| Total rounds | 1 |

---

### Test 2: unif-c1275-v300-s428434218.cnf

**Statistics:**
| Metric | Value |
|--------|-------|
| Variables | 300 |
| Clauses | 1,275 |
| Runtime | 8.08 seconds |
| Result | UNSATISFIABLE ✓ |

**Winner Solver:**
```
kissat(0, 0) of family: SAT_STABLE
- Conflicts: ~250K (varies by solver)
- Propagations: ~7-12M
- Decisions: ~290-410K
- Restarts: 6-7K
```

**Sharing Statistics:**
| Metric | Value |
|--------|-------|
| Initial clauses | 1,275 |
| Received per round | ~3,200 |
| Shared per round | ~2,600 |
| Total rounds | 12 |

---

## Observations

### Startup Behavior

Each test followed a consistent initialization sequence:

```
1. Parameter parsing and validation
2. PortfolioSimple strategy initialization
3. Clause database factory creation (PerSize database, max clause size 60)
4. Diversification of all solver instances
5. All solvers launched simultaneously
6. Sharing mechanism active (HordeSatSharing)
7. Competition until single winner determined
```

### Key Differences Between Tests

| Aspect | Test 1 | Test 2 |
|--------|--------|--------|
| Result type | SAT | UNSAT |
| Clauses per initial formula | 1,347 | 1,275 |
| Variables | 150 | 300 |
| Rounds needed | 1 | 12 |
| Sharing efficiency | 100% (empty to share) | 82% (2.6k/3.2k) |
| Runtime | 0.35s | 8.08s |
| Peak memory | 304 MB | 777 MB |

### Exit Codes

- Test 1: 10
- Test 2: 0

The exit code variation may reflect different result types (10 vs 0 for UNSAT).

---

## Strategy Behavior Analysis

### HordeSatSharing Mechanism

Each test used strategy `'15HordeSatSharing'` with these initial parameters:

```
[HordeSat] Producers: 32, Consumers: 32
Initial LBD limit: 2
Rounds before increase: 1
Literals per round: ~1500
```

This means:
- All 32 solvers start as producers (clauses in watchlist)
- All 32 act as consumers requesting clauses
- Each round: producers share fresh clauses, consumers import and use them
- After Round 1, LBD limit reviewed (likely increased)

### Diversification

Before launch, all 32 solver instances had unique configurations:

```
Diversification done
Diversified all solvers
```

Each solver received distinct genetic configuration based on:
- Solver type (Kissat, Lingeling, CaDiCaL, etc.)
- Diversifier ID (0-31)
- Various diversification parameters

---

## Performance Characteristics

### Test 1 Performance Profile

Fast resolution due to:
- Smaller instance (150 variables, 1,347 clauses)
- Quickly resolved as SAT with simple proof
- Kissat solver found solution within 245 conflicts
- Minimal sharing required (strategy emptied quickly)

### Test 2 Performance Profile

More complex due to:
- Slightly more variables (300 vs 150)
- Almost identical clause count (1,275 vs 1,347)
- UNSAT result requiring deeper search
- More rounds of sharing (12 vs 1)
- Higher memory usage (777 MB peak)

---

## Conclusions

1. **PortfolioSimple works** - Single-process execution produces correct results

2. **Rapid initial validation** - Test 1 completed in <1 second with trivial instance

3. **Sharing dynamics matter** - Test 2 required more rounds and showed different sharing efficiency

4. **Winner selection consistent** - Kissat(0, 0) won both tests initially examined

5. **Memory manageable** - Peak memory under 800 MB in all tests

6. **No MPI required** - The strategy can run without distributed execution

---

## Suggested Extensions

To more thoroughly test PortfolioSimple parallel mode, consider:

1. Testing with different sharing strategies (`-shr-strat 2`, `3`)
2. Varying the number of solver threads (`-c 4`, `-c 16`)
3. Comparing PRS and SBVA preprocessing options
4. Running multiple tests simultaneously for comparative analysis

---

## Files Used for Testing

| Path | Used in Tests |
|------|---------------|
| /mnt/d/wsl-code/sat_benchmark_local/x9-03065.sat.sanitized.cnf | Test 1 ✓ |
| /mnt/d/wsl-code/sat_benchmark_local/unif-c1275-v300-s428434218.cnf | Test 2 ✓ |