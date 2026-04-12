# Clause Sharing Mechanisms Analysis

Created: 2025-04-12
Analysis Task #2 of multiple systematic reviews

## Overview

This document documents the clause sharing architecture, strategies, and data structures used in Painless's parallel SAT solving framework.

## The Sharing Entity Hierarchy

```
SharingEntity (abstract base)
    ├── std::atomic<int> m_sharingId (unique identifier)
    ├── std::vector<weak_ptr<SharingEntity>> m_clients
    ├── mutable std::shared_mutex m_clientsMutex
    │
    └── virtual bool importClause(const ClauseExchangePtr& clause) = 0
        virtual void importClauses(const vector<ClauseExchangePtr>& clauses) = 0

SharingStrategy (abstract, inherits from SharingEntity)
    ├── std::vector<weak_ptr<SharingEntity>> m_producers
    ├── mutable std::shared_mutex m_producersMutex
    ├── std::shared_ptr<ClauseDatabase> m_clauseDB
    ├── SharingStatistics stats
    │
    └── virtual bool doSharing() = 0
        virtual std::chrono::microseconds getSleepingTime()
        virtual void addProducer(std::shared_ptr<SharingEntity>)
        virtual void connectProducer(std::shared_ptr<SharingEntity>)

GlobalSharingStrategy (abstract, inherits from SharingStrategy)
    ├── int mpi_rank, mpi_world_size
    ├── std::vector<std::shared_ptr<Sharer>> sharers
    │
    └── virtual bool initMpiVariables() = 0
        virtual void joinProcess(int winnerRank, SatResult res, const vector<int>& model)

MallobSharing (concrete global strategy)
    ├── Advanced filter system with clause metadata
    ├── Epoch-based sharing coordination
    ├── Volume balancing between MPI processes
    └── Real-time clause exchange optimization
```

## Key Observation: Dual Producer/Consumer Relationship

Each `SharingStrategy` maintains bidirectional relationships:

```cpp
// In SharingStrategy::connectProducer()
void connectProducer(std::shared_ptr<SharingEntity> producer) {
    producer->addClient(shared_from_this());  // This strategy becomes a client
    m_producers.push_back(producer);          // Strategy stored as producer
}

// In exportClauseToClient()
bool exportClauseToClient(const ClauseExchangePtr& clause,
                          std::shared_ptr<SharingEntity> client) {
    // Clauses only sent to clients NOT originating from them
    if (clause->from != client->getSharingId())
        return client->importClause(clause);
    return false;  // Don't send clauses that client already has
}
```

**Meaning**: Every clause exported by this strategy is guaranteed to be needed by every client, and clients never receive duplicate clauses they already imported.

## Local vs Global Sharing Strategies

### Local Sharing Strategies

Run on each process independently; no inter-process communication.

```cpp
SharingStrategyFactory::instantiateLocalStrategies(
    __globalParameters__.sharingStrategy,
    this->localStrategies,
    cdclSolvers)  // producers = clients = these solvers
```

**Character codes for local strategies:**
| Code | Strategy | Description |
|------|---------|-------------|
| `1` | HordeSatWithBuffers | Per-entity buffers, two producer groups |
| `2` | HordeSatSimple | Simplified version |
| `3` | SimpleSharing | Basic clause selection and export |

### Global Sharing Strategies

Use MPI communication; only executed on processes that are both producers AND clients.

```cpp
if (dist) {
    SharingStrategyFactory::instantiateGlobalStrategies(
        __globalParameters__.globalSharingStrategy,
        globalStrategies);
}
```

**Character codes for global strategies:**
| Code | Strategy |
|------|---------|
| `1` | AllGatherSharing |
| `2` | MallobSharing |
| `3` | GenericGlobalSharing |

## AllGatherSharing: Simple Global Strategy

### Design Philosophy

Uses MPI's `MPI_Allgather` to exchange all clauses in a single synchronized broadcast.

### Key Components

```cpp
class AllGatherSharing : public GlobalSharingStrategy {
    int totalSize;                    // Total buffer size for serialization
    int color;                        // MPI communicator split color
    vector<int> clausesToSendSerialized;
    vector<int> receivedClauses;
    BloomFilter b_filter;             // Duplicate clause detection

    bool doSharing() override;
    bool initMpiVariables() override;
    void joinProcess(int winnerRank, SatResult res, const vector<int>& model) override;

protected:
    int serializeClauses(vector<int>& serialized_v_cls);
    void deserializeClauses(const vector<int>& serialized_v_cls, int num_buffers);
};
```

### Serialization Pattern

```cpp
int AllGatherSharing::serializeClauses(vector<int>& serialized_v_cls) {
    // Each clause serialized as: [literals...][size][LBD][from]
    for (const auto& clause : clauses_to_send) {
        // Append literals
        for (lit_t lit : *clause) {
            serialized_v_cls.push_back(lit);
        }
        // Append clause metadata
        serialized_v_cls.push_back(clause->size);
        serialized_v_cls.push_back((int)clause->lbd);
        serialized_v_cls.push_back(clause->from);
    }
    return serialized_v_cls.size();
}
```

### doSharing() Sequence

```cpp
bool AllGatherSharing::doSharing() {
    // 1. Select clauses to share
    vecot<ClauseExchangePtr> clauses_to_send;
    for (auto& producer : m_producers) {
        if (auto prod = producer.lock()) {
            while (m_clauseDB->getOneClause(clause)) {
                if (should_share(clause)) {
                    clauses_to_send.push_back(clause);
                    m_clauseDB->shrinkDatabase();
                }
            }
        }
    }

    // 2. Serialize all clauses
    serializeClauses(clauses_to_send);

    // 3. MPI Allgather - every process receives everything
    vector<int> recv_buf(total_size * mpi_world_size);
    MPI_Allgather(clausesToSendSerialized.data(), totalSize, MPI_INT,
                  recv_buf.data(), totalSize, MPI_INT, MPI_COMM_WORLD);

    // 4. Deserialize and import
    int num_buffers = mpi_world_size;
    deserializeClauses(recv_buf, num_buffers);

    return false;  // Continue sharing
}
```

### Advantages

- Simple implementation with clear correctness guarantees
- Each process receives complete clause view after each round
- Easy to debug and analyze

### Disadvantages

- Communication overhead: O(n²) in number of processes
- Bandwidth-intensive: duplicate data sent to every process
- Doesn't adapt to process differences in clause needs

## MallobSharing: Advanced Global Strategy

### Design Philosophy

Inspired by the Mallob algorithm for distributed SAT solving, with epoch-based coordination and intelligent volume balancing.

### Key Innovations

1. **Epoch-Based Sharing**: Cycles through discrete sharing epochs
2. **Clause Metadata Tracking**: Maintains detailed state for each clause
3. **Filter-Based Selection**: Prevents duplicate work and repeated sharing
4. **Volume Balancing**: Compensates processes that receive fewer clauses

### Clause Metadata Structure

```cpp
struct ClauseMeta {
    int32_t productionEpoch;   // When clause was first produced
    int32_t sharedEpoch;       // Last epoch clause was shared
    uint64_t sources;          // Bitmask of producer processors
};
```

### Class Layout

```cpp
class MallobSharing : public GlobalSharingStrategy {
public:
    MallobSharing(const shared_ptr<ClauseDatabase>& clauseDB,
                  unsigned long baseBufferSize,
                  unsigned long maxBufferSize,
                  unsigned int lbdLimitAtImport,
                  unsigned int sizeLimitAtImport,
                  unsigned int roundsPerSecond,
                  float maxCompensation);

    bool doSharing() override;
    bool initMpiVariables() override;
    void joinProcess(int winnerRank, SatResult res, const vector<int>& model) override;

protected:
    // Clause database with metadata
    unordered_map<ClauseExchangePtr, ClauseMeta,
                  ClauseUtils::ClauseExchangePtrHash,
                  ClauseUtils::ClauseExchangePtrEqual> m_clauseMetaMap;

    // Communication topology
    int father, left_child, right_child, nb_children;

    // Volume balancing
    float compensationFactor;
    float accumulatedAdmittedLiterals;
    float estimatedIncomingLiterals;

private:
    bool importClause(const ClauseExchangePtr& cls) override;
    bool exportClauseToClient(const ClauseExchangePtr& clause,
                              shared_ptr<SharingEntity> client) override;

    int serializeClauses(vector<int>& serialized_v_cls);
    void deserializeClauses(const vector<int>& serialized_v_cls, int num_buffers);
    size_t shrinkFilter();
    void computeCompensation();

    // Filter system
    unsigned m_sharingPerSecond;
    unsigned m_resharingPeriodInEpochs;
    unordered_map<ClauseExchangePtr, ClauseMeta, ...> m_clauseMetaMap;
};
```

### The Filter System

MallobSharing maintains a in-memory filter for clause tracking:

```cpp
// Check if clause can be exported
bool canExport(const ClauseExchangePtr& cls) const {
    // Must be in metadata map (already tracked)
    auto it = m_clauseMetaMap.find(cls);
    if (it == m_clauseMetaMap.end()) return false;

    // Not recently shared (resharing period)
    if (!isClauseShared(cls)) return false;

    return true;
}

// Update clause metadata after sharing
void updateClause(const ClauseExchangePtr& cls) {
    auto& meta = m_clauseMetaMap[cls];
    meta.sharedEpoch = m_currentEpoch;
    // Shift sources bitmap
    meta.sources |= (1ULL << cls->from);
}
```

### doSharing() Sequence for MallobSharing

```cpp
bool MallobSharing::doSharing() {
    const auto start_time = now();

    // 0. Periodic synchronization and compensation calculation
    if ((now() - last_sharing_time) >= 1s) {
        computeCompensation();
        resetForNewEpoch();
    }

    // 1. Export phase: extract clauses from database
    vector<ClauseExchangePtr> exported_clauses;
    while (m_clauseDB->getOneClause(clause)) {
        if (should_export(clause)) {           // Filter check
            if (!is_in_map(clause)) {          // New clause
                insert_into_map(clause);        // Add to metadata
                exported_clauses.push_back(clause);
            } else {                           // Existing: update metadata
                update_clause(clause);
            }
        }
    }

    // 2. Serialize exported clauses
    serialize(exported_clauses, serialized_buf);

    // 3. MPI communication with volume balancing
    if (mpi_rank == father) {
        // Send to children with compensation for smaller buffers
        send_to_child(left_child, serialized_buf, compensationFactor);
        send_to_child(right_child, serialized_buf, compensationFactor * 0.9f); // 10% more
    }

    // 4. Receive from parent/children
    receive_from(father, received_left, &buf, &compensationFactor);
    receive_from(right_child, received_right, &buf, &compensationFactor);

    {   scope_lock lock(mutex);
        // Merge received clauses into database
        deserialize(received_left, &clauses_imported);

        // Apply filter: only keep clauses that passed import clause
        vector<ClauseExchangePtr> filtered;
        for (auto& cls : clauses_imported) {
            if (import_clause(cls)) {          // Actual database insertion
                filtered.push_back(cls);
            }
        }
    }

    last_sharing_time = now();
    return false;  // Continue

} // End doSharing
```

### Volume Balancing

```cpp
void MallobSharing::computeCompensation() {
    // Estimate literals this process will receive vs sends
    estimatedSharedLiterals = estimate_literals_sent();
    estimatedIncomingLiterals = estimate_literals_received();

    if (estimatedIncomingLiterals > 0) {
        compensationFactor = estimatedSharedLiterals /
                             max(estimatedIncomingLiterals, 1UL);

        // Cap at maximum compensation
        compensationFactor = min(compensationFactor,
                                 __globalParameters__.maxCompensation);
    }
}
```

### Advantages over AllGatherSharing

1. **Adaptive Communication**: Only sends useful clauses
2. **Duplicate Prevention**: Filter prevents re-sharing old clauses
3. **Volume Balancing**: Processes that receive more than they send accept smaller compensation penalties
4. **Performance Monitoring**: Tracks rounds per second, adapts sleep times

### Disadvantages

1. More complex implementation
2. Requires more memory for metadata and filters
3. Initialization more involved (initializeFilter)
4. Dependencies on MPI communication patterns

## Clause Database Interface

### Abstract Interface

```cpp
class ClauseDatabase {
    virtual bool addClause(ClauseExchangePtr clause) = 0;
    virtual size_t giveSelection(vector<ClauseExchangePtr>& selectedCls,
                                 unsigned int literalCountLimit) = 0;
    virtual void getClauses(vector<ClauseExchangePtr>& v_cls) = 0;
    virtual bool getOneClause(ClauseExchangePtr& cls) = 0;
    virtual size_t getSize() const = 0;
    virtual size_t shrinkDatabase() = 0;
    virtual void clearDatabase() = 0;
};
```

### Database Types

| Type | Character | Name | Description |
|------|-----------|------|-------------|
| `d` | default | DefaultBuffer | Simple circular buffer |

## Sharing Strategy Factory

### Creation Entry Points

```cpp
// For local strategies (no MPI)
SharingStrategyFactory::instantiateLocalStrategies(
    __globalParameters__.sharingStrategy,
    this->localStrategies,  // output: vector<shared_ptr<SharingStrategy>>
    cdclSolvers);           // producers/clients: vector<shared_ptr<SolverCdclInterface>>

// For global strategies (with MPI)
if (__globalParameters__.globalSharingStrategy >= 0) {
    SharingStrategyFactory::instantiateGlobalStrategies(
        __globalParameters__.globalSharingStrategy,
        globalStrategies);  // output
}
```

### Thread Launch

```cpp
// After strategy creation, launch sharing threads
vector<shared_ptr<SharingStrategy>> sharing_strategies_concat;
for (auto& lstrat : local_strategies) {
    sharing_strategies_concat.push_back(lstrat);
}
for (auto& gstrat : global_strategies) {
    sharing_strategies_concat.push_back(gstrat);
}

SharingStrategyFactory::launchSharers(sharing_strategies_concat,
                                       this->sharers);  // output
```

## Key Insights for Code Modifications

1. **Thread synchronization**: Uses `condition_variable` with `mutexGlobalEnd`
2. **Duplicate clause prevention**: Filters and metadata tracking essential
3. **MPI communication patterns**: Allgather vs parent-child tree vs mixed
4. **Producer/client relationships**: Bidirectional; derived from `shared_from_this()`
5. **Memory management**: Clauses use intrusive pointers; no shared ownership conflicts

## Files to Understand for Changes

- `src/sharing/SharingStrategy.hpp/cpp` - Base strategy interface
- `src/sharing/GlobalStrategies/AllGatherSharing.hpp/cpp` - Simple global strategy
- `src/sharing/GlobalStrategies/MallobSharing.hpp/cpp` - Advanced global strategy
- `src/containers/ClauseDatabase.hpp`, `ClauseDatabases/ClauseDatabaseFactory.hpp`
- `src/sharing/SharingEntity.hpp`

## Potential Extension Points

1. New local strategies in `SharingStrategyFactory::instantiateLocalStrategies()`
2. New global strategies with custom MPI communication patterns
3. New database types in `ClauseDatabaseFactory::createDatabase()`
4. Custom filter systems for new sharing strategies
5. Additional diversification and initialization phases