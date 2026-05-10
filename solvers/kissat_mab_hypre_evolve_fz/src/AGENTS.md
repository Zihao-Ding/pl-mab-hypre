# src/ — Main Solver Library

**94 .c files, 80+ .h files.**

## WHERE TO LOOK

| Component | Files | Purpose |
|-----------|-------|---------|
| **Core** | `internal.h`, `internal.c` | Solver state aggregation |
| **Entry** | `main.c`, `application.c` | CLI + app loop |
| **Search** | `search.c`, `decide.c`, `deduce.c` | CDCL core |
| **Preprocess** | `preprocess.c`, `probe.c`, `substitute.c` | Simplification |
| **Memory** | `allocate.c`, `arena.c`, `heap.c` | Data structures |
| **Proof** | `proof.c`, `check.c` | DRAT proof generation |
| **Data structures** | `vector.c`, `stack.c`, `queue.c`, `watch.c` | Collections |

## KEY CONCEPTS

- **Internal struct**: `kissat` in `internal.h` — contains all solver state
- **Literals**: Positive = variable*2, Negative = variable*2+1
- **Clauses**: Irredundant (active) + learned (temporary)
- **Phases**: Search + preprocess + solve modes

## STYLE

- Functions use `kissat_` prefix, macros use `KISSAT_` prefix
- `ATTRIBUTE_ALWAYS_INLINE` for hot paths
- No external dependencies (pure C stdlib)