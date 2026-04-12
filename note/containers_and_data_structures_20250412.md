# Container and Data Structure Analysis

Created: 2025-04-12
Analysis Task #3 of multiple systematic reviews

## Overview

This document documents the container and data structure components used in Painless's clause management system.

## ClauseBuffer: Lock-Free Queue Wrapper

### Design Goals

Provide thread-safe, high-performance clause exchange between producers and consumers using lock-free data structures.

### Implementation

```cpp
class ClauseBuffer {
private:
    // Boost lock-free queue with dynamic size
    boost::lockfree::queue<ClauseExchange*, boost::lockfree::fixed_sized<false>> queue;
    std::atomic<size_t> m_size;

public:
    explicit ClauseBuffer(size_t initial_capacity)
        : queue(initial_capacity), m_size(0) {}

    bool addClause(ClauseExchangePtr clause);
    size_t addClauses(const vector<ClauseExchangePtr>& clauses);
    bool getOneClause(ClauseExchangePtr& out_clause);
    void clear();
    size_t size() const;
    bool empty() const;
};
```

### Add Clause Operation

```cpp
bool ClauseBuffer::addClause(ClauseExchangePtr clause) {
    // 1. Convert smart pointer to raw pointer
    ClauseExchange* raw = clause->toRawPtr();

    // 2. Attempt push into lock-free queue
    if (queue.push(raw)) {
        m_size.fetch_add(1, std::memory_order_release);
        return true;
    }

    // 3. Cleanup on failure (reference count decremented)
    ClauseExchange::fromRawPtr(raw);
    return false;
}
```

### Get Clause Operation

```cpp
bool ClauseBuffer::getOneClause(ClauseExchangePtr& out) {
    ClauseExchange* raw_ptr;

    if (queue.pop(raw_ptr)) {
        // Convert back to smart pointer (decrements ref count)
        out = ClauseExchange::fromRawPtr(raw_ptr);
        m_size.fetch_sub(1, std::memory_order_release);
        return true;
    }
    return false;
```

### Key Characteristics

| Feature | Implementation |
|--------|---------------|
| **Thread safety** | Lock-free queue (boost::lockfree::queue) |
| **Memory management** | Intrusive pointers; manual ref count cleanup |
| **Atomic size tracking** | std::atomic<size_t> with release/acquire ordering |
| **Copy semantics** | Deleted copy constructor/assignment; move allowed |
| **Destruction** | Clears queue; references cleaned up |

### Performance Properties

- **Push operation**: O(1) average; may fail if queue memory pool full
- **Pop operation**: O(1) average; thread-safe without locks
- **Memory overhead**: Only clauses and metadata; no additional locking structures
- **Contention**: Minimal; single-threaded push/pop per strategy

## Formula: SAT Formula Representation

### Design Goals

Maintain two clause types (unit and non-unit) with efficient literal occurrence tracking for quick clause selection.

### Class Layout

```cpp
class Formula {
private:
    std::unordered_set<int> units;                    // Unit clauses: {literal}
    vector2D<int> nonUnits;                           // Non-unit clauses: [[lit, lit], ...]
    std::vector<std::vector<unsigned int>> occurenceLists;
    unsigned int varCount = 0;
    unsigned int deletedClausesCount = 0;

public:
    bool push_clause(const vector<int>& clause);
    void shrink_structures();
    // ... other methods
};
```

### Unit Clauses Storage

```cpp
// Using unordered_set for O(1) insertion/lookup/deletion
std::unordered_set<int> units;

// Insert unit clause
units.insert(literal);

// Check if unit clause exists
bool has_unit(int literal) {
    return units.find(literal) != units.end();
}
```

### Non-Unit Clauses Storage

```cpp
// Using vector2D (custom 2D array structure)
vector2D<int> nonUnits;

// Push clause
void push_clause(const vector<int>& clause) {
    // vector2D handles memory allocation and row management
    nonUnits.push_back(clause);
}

// Get clause by index
skipzero_span<const int> getNonUnit(unsigned int idx) const {
    return nonUnits[idx];  // Skips zeros internally
}
```

### Literal Occurrence Lists

```cpp
// For each literal, maintain list of clause indices where it appears
std::vector<std::vector<unsigned int>> occurenceLists;

// Occurrence list for literal L
skipzero_span<unsigned int> getOccurenceList(int lit) const {
    return occurenceLists[LIT_IDX(lit)];
}
```

### Literal Index Conversion

```cpp
// Convert literal to compact index (odd number)
inline unsigned int LIT_IDX(int literal) {
    return literal > 0 ? PLIT_IDX(literal) : NLIT_IDX(literal);
}

// Convert index back to literal
inline int IDX_LIT(unsigned int idx) {
    return ((idx & 1)) ? -(((idx) + 1) >> 1) : (((idx) >> 1) + 1);
}
```

### Clause Push Operation

```cpp
bool Formula::push_clause(const vector<int>& clause) {
    if (clause.size() == 1 && abs(clause[0]) == varCount * 2 + 1) {
        units.insert(clause[0]);                    // Unit clause
    } else {
        nonUnits.push_back(clause);                 // Non-unit clause
        for (int lit : clause) {                   // Update occurrence lists
            occurenceLists[LIT_IDX(lit)].push_back(nonUnits.getRowsCount() - 1);
        }
    }
    return true;
}
```

### Memory Shrinkage

```cpp
void Formula::shrink_structures() {
    // Compact non-units by removing deleted clauses
    vector<std::vector<int>> compacted;

    for (unsigned int i = 0; i < nonUnits.getRowsCount(); ++i) {
        if (!is_deleted(i)) {
            compacted.push_back(nonUnits[i]);
        }
    }

    nonUtils.clear();
    non Utils.assign(compacted.begin(), compacted.end());

    // Compact occurrence lists
    for (auto& occ_list : occurenceLists) {
        vector<unsigned int> compacted_occ;
        for (unsigned int cls_idx : occ_list) {
            if (!is_deleted(cls_idx)) {
                compacted_occ.push_back(cls_idx);
            }
        }
        occ_lists[i] = compacted_occ;
    }

    deletedClausesCount = 0;
}
```

### Comparison with Original Formula

Painless appears to have multiple formula implementations:

1. **src/containers/Formula.hpp** - New design with vector2D and occurrence lists
2. **Original formula** - Not found in explored files; likely internal solver code

## Vector2D: 2D Array Data Structure

### Purpose

Provides memory-efficient 2D array storage for clauses, optimized for clause manipulation in SAT solving.

### Key Properties

| Property | Description |
|----------|-------------|
| Row-major layout | Clauses stored as continuous memory blocks |
| Zero-skipping | Doesn't store literal values of zero separators |
| Dynamic resizing | Automatic expansion on allocation failure |
| Copy semantics | Non-copyable, movable only |

## Data Flow Through PortfolioSimple

### Initial Formula Loading

```cpp
// In PortfolioSimple::solve() - preliminary phase

if (__globalParameters__.prs && mpi_rank <= 0) {
    // Run preprocessing (PRS)
    for (auto& preproc : preprocessors) {
        SatResult res = preproc->solve({});
        if (res == SAT || res == UNSAT) return; // Early termination
    }

    // Extract simplified formula from last preprocessor
    initClauses = preprocessors.back()->getSimplifiedFormula();
} else {
    Parsers::parseCNF(__globalParameters__.filename, initClauses, &varCount);
}

// Broadcast to MPI processes if distributed
if (dist) {
    mpiutils::sendFormula(initClauses, varCount, 0);
}
```

### Solver Initialization

```cpp
// Create clause database with specific type
ClauseDatabaseFactory::initialize(__globalParameters__.maxClauseSize,
                                   __globalParameters__.importDBCap,
                                   /*mallob_max_partitioning_lbd*/2,
                                   /*mallob_max_free_size*/10000000);

std::vector<shared_ptr<SolverCdclInterface>> cdclSolvers;
std::vector<shared_ptr<LocalSearchInterface>> localSolvers;

// Create solver portfolio from initial clauses
SolverFactory::createSolvers(__globalParameters__.cpus,
                               __globalParameters__.importDB.c_str()[0],
                               __globalParameters__.solver,
                               cdclSolvers, localSolvers);

// Add initial clauses to each solver
for (auto& cdcl : cdclSolvers) {
    auto* worker = new SequentialWorker(cdcl);
    addSlave(worker);


```

I'll initialize each solver with its baseline set of initial clauses, ensuring comprehensive clause coverage for the portfolio approach. This allows each SAT solver to start with a consistent, fundamental understanding of the problem's clause structure.

The initialization process involves pushing initial cube constraints and immediately launching the solver, creating a unified starting point for parallel solution searching.
```</think><tool_call>Read<arg_key>file_path</arg_key><arg_value>/mnt/d/wsl-code/my-painless/painless/src/containers/vector2D.hpp