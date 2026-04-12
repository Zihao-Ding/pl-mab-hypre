# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Building and Testing

### Build System
- Use Make for building: `make` (defaults to building all targets)
- Debug build: `make debug` (produces `build/debug/painless_debug`, symlinked as `painlessd`)
- Release build: `make release` (produces `build/release/painless_release`, symlinked as `painless`)
- Only solvers: `make solvers`
- Clean builds: `make cleanpainless`, `make cleansolvers`, or `make cleanall`

### Compiler Requirements
- C++20 standard (GCC 10+ with `-std=c++20` or earlier with `-std=c++2a`)
- Requires MPI compiler (`mpic++`)
- Includes pthread, zlib, and libm

### Testing
This codebase doesn't have a unit test framework. For testing solver changes:
1. Build the debug version
2. Use `scripts/launch.sh` for experimental runs with timeout and analysis

## Core Architecture Patterns

### Portfolio Parallel Strategy (Default)
The main execution mode runs multiple SAT solvers simultaneously:

```cpp
// In src/painless.cpp, the default working strategy is PortfolioSimple
working = new PortfolioSimple();
std::thread mainWorker(&WorkingStrategy::solve, working, std::ref(cube));
```

**PortfolioSimple maintains:**
- `cdclSolvers`: Vector of CDCL-based solvers (Glucose, Lingeling, CaDiCaL, etc.)
- `localSolvers`: Vector of local search solvers
- `preprocessors`: Preprocessing components for model restoration
- `localStrategies`, `globalStrategies`: Clause sharing management
- `sharers`: Actual clause exchange entities

### Distributed Strategy
Enabled with `--distributed` parameter, uses MPI:

```cpp
int provided;
MPI_Init_thread(NULL, NULL, MPI_THREAD_SERIALIZED, &provided);
// ... later cleanup with MPI_Finalize()
```

- Single node: 1 process
- Multiple processes: each manages one solver in the portfolio
- Winner-takes-all: only the winning process returns the final result

### Working Strategy Hierarchy

```cpp
class WorkingStrategy {
    virtual void solve(const std::vector<int>& cube) = 0;
    virtual void join(WorkingStrategy* winner, SatResult res, const std::vector<int>& model) = 0;
    virtual void setSolverInterrupt() = 0;
    virtual void unsetSolverInterrupt() = 0;
    virtual void waitInterrupt() = 0;
    virtual void addSlave(WorkingStrategy* slave);

protected:
    WorkingStrategy* parent;
    std::vector<WorkingStrategy*> slaves;
};
```

PortfolioSimple and PortfolioPRS inherit from this and manage their own solver vectors.

### Clause Sharing Mechanisms

Clauses are exchanged using smart pointers:

```cpp
using ClauseExchangePtr = boost::intrusive_ptr<ClauseExchange>;

// In PortfolioSimple
std::vector<std::shared_ptr<SolverCdclInterface>> cdclSolvers;
std::vector<std::unique_ptr<Sharer>> sharers;  // Manages clause import/export
```

**Key sharing strategies:**
- `LocalStrategies`: Per-solver clause queues (lock-free)
- `GlobalStrategies`: MPI-based distributed clause sharing:
  - `AllGatherSharing`: Every process broadcasts clauses to all others
  - `MallobSharing`: Memory-efficient distributed clause sharing

### Solver Interface Requirements

Any new solver must implement:

```cpp
class SolverInterface {
    virtual unsigned int getVariablesCount() = 0;
    virtual int getDivisionVariable() = 0;
    virtual void setSolverInterrupt() = 0;
    virtual void unsetSolverInterrupt() = 0;
    virtual SatResult solve(const std::vector<int>& cube) = 0;
    virtual void addClause(ClauseExchangePtr clause) = 0;
    virtual void addClauses(const std::vector<ClauseExchangePtr>& clauses) = 0;
    virtual void addInitialClauses(const std::vector<simpleClause>& clauses, unsigned int nbVars) = 0;
    virtual void loadFormula(const char* filename) = 0;
    virtual std::vector<int> getModel() = 0;
    virtual void diversify(const SeedGenerator& getSeed) = 0;
    // ... more
};
```

For CDCL solvers, also implement `SolverCdclInterface` with additional methods for activity management and final analysis.

## Key Files to Understand Architecture

- `src/painless.cpp` - Main program entry point and working strategy launch
- `src/working/PortfolioSimple.cpp` - Default portfolio implementation
- `src/solvers/SolverFactory.cpp` - Solver creation and diversification
- `src/sharing/SharingStrategy.hpp` - Clause sharing interfaces
- `src/containers/ClauseExchange.hpp` - Memory-efficient clause representation

## Diversification

Solvers are diversified by modifying their internal state based on solver ID and type:

```cpp
// In SolverFactory::diversification()
// Changes solver behavior without changing solver type
```

This prevents identical solvers from doing the work simultaneously.