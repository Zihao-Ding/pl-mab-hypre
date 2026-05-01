# SATLUTION_51 Changelog

## Changes from SATLUTION_42 (current champion)

### Source Files Modified

- **src/probe.c**:
  - Function `probe()`: Brace-wrapped the C42 conditional block (gated on
    `!solver->inconsistent && transitive_reduced`) and added a second
    `kissat_transitive_reduction(solver)` call inside, gated on
    `!solver->inconsistent`. Rationale: when C42's conditional
    `kissat_congruence` finds new equivalences/merges, those merges add
    fresh equivalence binaries to the implication graph that the just-
    completed transitive reduction could not have seen. A single follow-up
    transitive_reduction at the same junction cleans those new binaries.
    Same junction (transitive↔congruence) where C42 won, so distinct from
    C43/C45/C46/C48 which transposed the C42 pattern to other junctions.

### Parameters Changed

| Parameter | Old Value | New Value | Rationale |
|-----------|-----------|-----------|-----------|
| (none) | — | — | Pure structural change in `probe.c::probe()`; no parameter tuning. |

### Net diff in `src/probe.c::probe()`

```
   bool transitive_reduced = kissat_transitive_reduction (solver);
-  if (!solver->inconsistent && transitive_reduced)
-    kissat_congruence (solver);
+  if (!solver->inconsistent && transitive_reduced) {
+    kissat_congruence (solver);
+    if (!solver->inconsistent)
+      kissat_transitive_reduction (solver);
+  }
   kissat_binary_clauses_backbone (solver);
```

3-line addition (open brace, new transitive call with its inconsistency guard, close brace).

### Forbidden-pattern compliance

- No `malloc`/`free`/`calloc`, no `static` pointer/array additions.
- No forbidden literals (`early_termination`, `predictive_sat`,
  `heuristic_result`).
- Existing functions called only — no new APIs introduced.
