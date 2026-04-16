# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build System

### Main build commands
```bash
make              # Build everything: m4ri → solvers → painless (debug and release)
make debug        # Build only debug version (outputs to build/debug/painless_debug)
make release      # Build only release version (outputs to build/release/painless_release)
make solvers      # Build only the SAT solver libraries
make cleanpainless# Clean painless builds only
make cleansolvers # Clean solver builds only
make cleanall     # Clean everything including m4ri
```

### Build outputs
- Debug binary: `build/debug/painless_debug` (symlinked as `painlessd`)
- Release binary: `build/release/painless_release` (symlinked as `painless`)
- Solvers and libraries are built in parallel with Make

### Build requirements
- C++20 compatible compiler (GCC 8+, preferentially)
- OpenMPI implementation (`mpic++` for MPI support)
- Boost library headers
- POSIX-compatible environment

## Core Architecture

### Main entry point: src/painless.hpp

The parallel SAT solver framework PaInleSS has a modular architecture centered around `src/painless.hpp`. The framework supports both sequential and distributed execution modes.

### Solver interface hierarchy

```
solvers/SolverInterface.hpp (base)
├── solvers/CDCL/SolverCdclInterface.hpp
│   ├── Kissat, GlucoseSyrup, Cadical, MiniSat, Lingeling
│   ├── MapleCOMSPS, KissatMAB, KissatINCSolver, KissatMABHyPreSolver
│   └── All inherit from both SolverInterface and SharingEntity
└── solvers/LocalSearch/LocalSearchInterface.hpp
    ├── PASSAT, YalSat, TaSSAT (local search variants)
    └── Also inherit from SharingEntity for clause sharing
```

Key differences:
- **CDCL solvers**: Use conflict-driven clause learning, propagations, decisions, restarts
- **Local search solvers**: Use literal flipping heuristic, no explicit CDCL structure

### Clause management core

Clauses are the primary data structure in PaInleSS:

```cpp
// Core clause representation with flexible array
containers/ClauseExchange.hpp
├── ClauseExchange::lits[]           // Literal array (flexible member)
├── ClauseExchange::size            // Clause literal count
├── ClauseExchange::lbd             // Literal Block Distance (glue value)
└── ClauseExchange::from            // Source solver ID

// Clause database interfaces
containers/ClauseDatabase.hpp                      // Abstract interface
containers/ClauseDatabases/ClauseDatabaseMallob.hpp  // Size/LBD partitioned
containers/ClauseDatabases/ClauseDatabasePerSize.hpp // Only by clause size
containers/ClauseDatabases/ClauseDatabaseSingleBuffer.hpp // Simple buffer
```

### Sharing mechanisms

Clauses are shared between parallel solvers through stratified strategies:

```cpp
sharing/SharingEntity.hpp          // Base class with import/export clauses
sharing/SharingStrategy.hpp        // Strategy interface
sharing/GlobalStrategies/
│   ├── AllGatherSharing.hpp       // Broadcast after each shuffle phase
│   ├── MallobSharing.hpp         // Mallob-style streaming sharing
│   └── GenericGlobalSharing.hpp  // Template for global strategies
sharing/LocalStrategies/
│   ├── HordeSatSharing.hpp       // CP-based local clause injection
│   └── SimpleSharing.hpp         // Direct client list iteration
```

### Working strategies (distribution)

Solvers are dynamically assigned instances of the formula:

```cpp
working/WorkingStrategy.hpp        // Base distribution strategy
├── Portfolio strategies:
│   ├── PortfolioSimple.hpp       // Round-robin solver assignment
│   └── PortfolioPRS.hpp          // PRS-preprocessed formula first
└── Sequential strategy:
    └── SequentialWorker.hpp      // Single solver, clones formula state
```

### Key design patterns

1. **Intrusive reference counting**: Clauses use `boost::intrusive_ptr` via `ClauseExchangePtr`
2. **Weak pointer client lists**: `SharingEntity` maintains clients via `std::weak_ptr`
3. **Atomic operations**: Literal counts and critical sections with `std::atomic`
4. **Strategy patterns**: Sharing and working strategies use virtual interfaces

## Running experiments

### Main execution script: scripts/launch.sh

```bash
# Sequential execution (no MPI)
./scripts/launch.sh parameters.sh formulas.txt [experiment_name] [debug]

# Distributed execution (with MPI)
mpirun --hostfile hostfile -bind-to hwthread \
    --map-by ppr:N:NS:pe=$N_PHYSICAL_CORES \
    build/release/painless_release -v=1 -c=N_SOLVERS <args>
```

Key workflow:
1. Copy formulas to each solver process
2. Run with timeout, monitor results
3. Export/import clauses via sharing strategies
4. Winner declaration when a solver proves SAT/UNSAT
5. Cleanup processes on timeout or completion

### Result analysis: scripts/plot.py

Generates statistics and visualizations:
```bash
python scripts/plot.py --base-dir outputs --timeout 5000
```

## Important implementation details

1. **Interrupt handling**: All solvers support `setSolverInterrupt()`/`unsetSolverInterrupt()` for timeout management
2. **Diversification**: Each solver can be diversified with unique seeds to create portfolio diversity
3. **Clause sharing mutex**: `m_clientsMutex` protects client lists; use shared locks for read access
4. **Memory efficiency**: Clauses use flexible array members; avoid copying in hot paths
5. **MPI communication**: Mallob strategy uses point-to-point sends/receives per shuffle phase

## Existing solver integrations

The framework includes standalone SAT solver implementations that were integrated:

- `solvers/kissat/` - Original Kissat implementation
- `solvers/cadical/` - Cadical CDCL solver
- `solvers/glucose/parallel/` - Glucose with parallel optimizations
- `solvers/yalsat/`, `solvers/tassat/` - Local search variants
- `solvers/kissat_mab_hypre/` - Multi-armed bandit enhanced Kissat with hyper-preprocessing

Each solver integration requires:
- Clause import/export callbacks matching `ClauseExchangePtr`
- Literal array access via `lit_t* begin()/end()`
- LBD (Literal Block Distance) computation for learned clauses
- Source solver ID (`from`) tracking in each clause