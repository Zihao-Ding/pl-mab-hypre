# Core Architecture and Data Flow Analysis

Created: 2025-04-12
Analysis Task #1 of multiple systematic reviews

## Overview

This document documents the core architectural components and data flows in Painless, the parallel SAT solving framework.

## Solver Interface Layer

### Base Interface: `SolverInterface`

The foundational interface defines required behavior for all SAT solver implementations:

```cpp
class SolverInterface {
    virtual unsigned int getVariablesCount() = 0;
    virtual int getDivisionVariable() = 0;  // For search splitting
    virtual void setSolverInterrupt() = 0;
    virtual void unsetSolverInterrupt() = 0;
    virtual SatResult solve(const std::vector<int>& cube) = 0;
    virtual void addClause(ClauseExchangePtr clause) = 0;
    virtual void addClauses(const std::vector<ClauseExchangePtr>& clauses) = 0;
    virtual void addInitialClauses(const std::vector<simpleClause>& clauses, unsigned int nbVars) = 0;
    virtual void loadFormula(const char* filename) = 0;
    virtual std::vector<int> getModel() = 0;
    virtual void diversify(const SeedGenerator& getSeed) = 0;

    // Type identification
    virtual unsigned int getSolverId() = 0;
    virtual unsigned int getSolverTypeId() = 0;
};
```

### Algorithm Types

```cpp
enum class SolverAlgorithmType {
    CDCL = 0,          // Conflict-Driven Clause Learning
    LOCAL_SEARCH = 1,  // Local Search (YalSat, TaSSAT)
    LOOK_AHEAD = 2,
    OTHER = 3,
    UNKNOWN = 255
};
```

### Key Observations

1. **Interrupt mechanism**: Allows portfolio to terminate individual solvers gracefully
2. **Cube solving**: The `cube` parameter enables clause-based problem reduction
3. **Diversification**: Each solver can have its internal state modified for diversity
4. **Instance counting**: Uses atomic counter pattern with type-specific instance tracking

## Clause Representation: `ClauseExchange`

### Memory-Efficient Design

Uses flexible array member and intrusive reference counting:

```cpp
class ClauseExchange {
    lbd_t lbd;                      // Literal Block Distance
    plid_it from;                   // Source processor/solver ID
    csize_t size;                   // Number of literals in clause
    std::atomic<rcount_t> refCounter;
    lit_t lits[0];                  // Flexible array (must be last)
};
```

### Creation Pattern

```cpp
// Allocates memory for object + literals together
ClauseExchangePtr create(const csize_t size, const lbd_t lbd, const plid_it from) {
    void* memory = std::malloc(sizeof(ClauseExchange) + size * sizeof(lit_t));
    return ClauseExchangePtr(new (memory) ClauseExchange(size, lbd, from));
}
```

### Reference Counting

```cpp
// Increment on pointer creation
inline void intrusive_ptr_add_ref(ClauseExchange* ce) {
    ce->refCounter.fetch_add(1, std::memory_order_relaxed);
}

// Decrement; delete when reaches zero
inline void intrusive_ptr_release(ClauseExchange* ce) {
    if (ce->refCounter.fetch_sub(1, std::memory_order_acq_rel) == 1) {
        ce->~ClauseExchange();
        std::free(ce);
    }
}
```

### Key Design Insights

1. **Memory layout optimization**: Flexible array eliminates padding waste
2. **Thread-safe reference counting**: Atomic operations with relaxed ordering
3. **LBD validation**: Non-unit clauses guaranteed to have LBD ≥ 2
4. **No ownership ambiguity**: Intrusive pointers eliminate shared pointer overhead

## Clause Sharing Manager: `Sharer`

### Thread Architecture

Each `Sharer` manages one thread that rotates through sharing strategies:

```cpp
class Sharer {
    std::vector<std::shared_ptr<SharingStrategy>> sharingStrategies;
    Thread* sharer;  // Managed thread
    unsigned int round = 0;
    double totalSharingTime = 0;

    Sharer(int id, std::vector<std::shared_ptr<SharingStrategy>> strategies);
};
```

### Sharing Loop

```cpp
void* mainThrSharing(void* arg) {
    auto shr = static_cast<Sharer*>(arg);

    // Initial desynchronization
    std::this_thread::sleep_for(std::chrono::microseconds(__globalParameters__.initSleep));

    while (!can_break) {
        // 1. Execute current strategy's sharing
        can_break = shr->sharingStrategies[shr->round % nbStrats]->doSharing();

        // 2. Sleep with timeout notification
        std::unique_lock<std::mutex> lock(mutexGlobalEnd);
        condGlobalEnd.wait_for(lock, sleepTime);

        shr->round++;
    }

    // Finalize remaining strategies
    for (auto& strategy : sharers) {
        while (!strategy->doSharing()) { /* cleanup */ }
    }
}
```

### Strategy Selection Pattern

Strategies cycle through the portfolio: `round % nbStrats`

## Configuration System: `Parameters`

### Macro-Based Parameter Definition

```cpp
#define PARAMETERS \
    PARAM(timeout, int, "t", -1, "Timeout in seconds") \
    PARAM(cpus, int, "c", 0, "Number of solver threads") \
    CATEGORY("Portfolio") \
    PARAM(solver, std::string, "solver", "kcl", "Portfolio of solvers") \
    // ... more parameters
```

### Parameter Categories

| Category | Parameters |
|----------|-----------|
| General | timeout, cpus, verbosity, distributed mode |
| Portfolio | solver type, PRS enabled, local searchers after SBVA |
| Solving | glucose split heuristic, clause buffer size, local search flips |
| Preprocessing | PRS limits, SBVA timeout and counts, SUBCAT reorganizations |
| Sharing | clause sizes, strategy selection, sleep times, database types |

### Key Parameters

```cpp
__globalParameters__.timeout          // Seconds or -1 for infinite
__globalParameters__.cpus             // Number of parallel solvers
__globalParameters__.verbosity        // Log verbosity (0-5)
__globalParameters__.enableDistributed // Enables MPI
__globalParameters__.solver           // Portfolio string (e.g., "gkMcy")
__globalParameters__.importDB         // Database type: 'd'=PerSize, 'm'=Mallob,
                                      //               's'=SingleBuffer, 'e'=PerSource
```

## Solver Factory

### Solver Creation Mapping

```cpp
SolverAlgorithmType createSolver(char type, char importDBType,
                                  std::shared_ptr<SolverInterface>& createdSolver) {
    switch (type) {
        case 'g':  // GlucoseSyrup (CDCL)
            createdSolver = std::make_shared<GlucoseSyrup>(id, importDB);
            return SolverAlgorithmType::CDCL;
        case 'l':  // Lingeling (CDCL)
            createdSolver = std::make_shared<Lingeling>(id, importDB);
            return SolverAlgorithmType::CDCL;
        case 'c':  // CaDiCaL (CDCL)
            createdSolver = std::make_shared<Cadical>(id, importDB);
            return SolverAlgorithmType::CDCL;
        case 'k':  // Kissat (CDCL)
            createdSolver = std::make_shared<Kissat>(id, importDB);
            return SolverAlgorithmType::CDCL;
        case 'K':  // Kissat MAB
            createdSolver = std::make_shared<KissatMABSolver>(id, importDB);
            return SolverAlgorithmType::CDCL;
        case 'I':  // Kissat INC
            createdSolver = std::make_shared<KissatINCSolver>(id, importDB);
            return SolverAlgorithmType::CDCL;
        case 'm':  // MiniSat (CDCL)
            createdSolver = std::make_shared<MiniSat>(id, importDB);
            return SolverAlgorithmType::CDCL;
        case 'M':  // MapleCOMSPS (CDCL)
            createdSolver = std::make_shared<MapleCOMSPSSolver>(id, importDB);
            return SolverAlgorithmType::CDCL;
        case 'y':  // YalSat (LOCAL_SEARCH)
            createdSolver = std::make_shared<YalSat>(id, __globalParameters__.localSearchFlips,
                                                     __globalParameters__.maxDivNoise);
            return SolverAlgorithmType::LOCAL_SEARCH;
        case 't':  // TaSSAT (LOCAL_SEARCH)
            createdSolver = std::make_shared<TaSSAT>(id, __globalParameters__.localSearchFlips,
                                                     __globalParameters__.maxDivNoise);
            return SolverAlgorithmType::LOCAL_SEARCH;
        default:
            LOGERROR("Unknown solver type: {}", type);
            exit(PERR_UNKNOWN_SOLVER);
    }
}
```

### Diversification

```cpp
void diversification(const std::vector<std::shared_ptr<SolverCdclInterface>>& cdclSolvers,
                      const std::vector<std::shared_ptr<LocalSearchInterface>>& localSolvers,
                      const IDScaler& gIDScaler,
                      const IDScaler& typeIDScaler) {
    // Set solver IDs based on diversification function
    for (auto cdclSolver : cdclSolvers) {
        cdclSolver->setSolverId(gIDScaler(cdclSolver));
        cdclSolver->setSolverTypeId(typeIDScaler(cdclSolver));
    }
    for (auto localSolver : localSolvers) {
        localSolver->setSolverId(gIDScaler(localSolver));
        localSolver->setSolverTypeId(typeIDScaler(localSolver));
    }

    // Call each solver's diversification
    for (auto cdclSolver : cdclSolvers) cdclSolver->diversify();
    for (auto localSolver : localSolvers) localSolver->diversify();

    LOG0("Diversification done");
}
```

## Data Flow in PortfolioSimple

### Initialization Sequence

```cpp
void PortfolioSimple::solve(const std::vector<int>& cube) {
    // 1. Load and possibly preprocess formula
    if (__globalParameters__.prs && mpi_rank <= 0) {
        for (auto& preproc : preprocessors) {
            SatResult res = preproc->solve({});
            if (res == SAT || res = UNSAT) return; // Early termination
        }
        initClauses = preprocessors.back()->getSimplifiedFormula();
    } else {
        Parsers::parseCNF(__globalParameters__.filename, initClauses, &varCount);
    }

    // 2. Broadcast to MPI processes if distributed
    if (dist) {
        MPI_Bcast(&receivedFinalResultBcast, ..., MPI_COMM_WORLD);
        if (receivedFinalResultBcast != 0) return;
        mpiutils::sendFormula(initClauses, varCount, 0);
    }

    // 3. Create solver portfolio
    SolverFactory::createSolvers(__globalParameters__.cpus,
                                  __globalParameters__.importDB.c_str()[0],
                                  __globalParameters__.solver,
                                  cdclSolvers,
                                  localSolvers);

    // 4. Diversify solver configurations
    if (dist) {
        globalIDScaler = [rank=mpi_rank, size=__globalParameters__.cpus](
            const std::shared_ptr<SolverInterface>& solver) {
            return rank * __globalParameters__.cpus + solver->getSolverId();
        };
    }
    SolverFactory::diversification(cdclSolvers, localSolvers,
                                     globalIDScaler, typeIDScaler);

    // 5. Setup clause sharing
    SharingStrategyFactory::instantiateLocalStrategies(...);
    if (dist) SharingStrategyFactory::instantiateGlobalStrategies(...);

    // 6. Launch threads
    for (auto& cdcl : cdclSolvers) {
        auto* worker = new SequentialWorker(cdcl);
        addSlave(worker);
        solverInitializers.emplace_back([&, &cdcl]() {
            cdcl->addInitialClauses(initClauses, varCount);
            myworker->solve(cube);
        });
    }
    for (auto& local : localSolvers) { /* similar pattern */ }

    // 7. Start sharing threads
    SharingStrategyFactory::launchSharers(sharingStrategiesConcat, sharers);

    // 8. Optional: Genetic Algorithm for initial phase initialization
    if (__globalParameters__.gaInitPeriod) {
        saga::GeneticAlgorithm gaInitializer(...);
        gaInitializer.solve();
        // Assign initialized phases to solvers
    }

    // 9. Main portfolio execution begins
}
```

### Clause Sharing Execution

Each `Sharer` thread runs independently:

1. Every ~500ms, selects active strategy
2. Executes `doSharing()` - exchanges clauses
3. Waits for notification or timeout
4. Repeats until `globalEnding` signal

## Key Interactions

```
PortfolioSimple (WorkingStrategy)
    │
    ├───> slaves/strategies/
    │     ├── SequentialWorker 1 (CDCL solver)
    │     ├── SequentialWorker 2 (Local search solver)
    │     └── ...
    │
    ├───> sharers/
    │     ├── Sharer 0
    │     │   └── [sharing_strategies]/
    │     │       ├── LocalStrategy 0
    │     │       └── GlobalStrategy 0
    │     ├── Sharer 1
    │     └── ...
    │
    └───> preprocessors/
          ├── PreProcessor 0 (PRS)
          └── ...

Each thread operates independently:
- Slavers manage solver execution lifecycle
-Sharers exchange clauses via sharing strategies
-Preprocessors can simplify formula and extract initial clauses

Termination propagates upward through join() calls.
```

## Important Observations for Code Modifications

1. **Thread synchronization**: Uses condition variables with `mutexGlobalEnd`
2. **Interrupt propagation**: Portfolio calls setSolverInterrupt() on all slaves
3. **Memory management**: Clauses use intrusive pointers; no shared ownership conflicts
4. **MPI communication**: Winner-takes-all; only mpi_rank <= 0 returns results
5. **Strategy cycle**: Prevents identical solvers from working synchronously through diversification

## Files to Understand for Changes

- `src/painless.cpp` - Entry point and main portfolio launch
- `src/working/PortfolioSimple.cpp` - Full initialization sequence
- `src/solvers/SolverFactory.cpp` - Solver creation and diversification
- `src/sharing/Sharer.cpp` - Sharing thread management
- `src/utils/Parameters.hpp` - Configuration system

## Potential Extension Points

1. New solver types in `SolverFactory::createSolver()`
2. New sharing strategies in `SharingStrategyFactory`
3. New preprocessing techniques (PRS, SBVA)
4. New working strategies (PortfolioPRS, test)
5. Custom diversification functions