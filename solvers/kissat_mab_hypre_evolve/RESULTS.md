# SATLUTION_51 Evaluation Results

## Summary
| Metric | SATLUTION_51 | Champion (SATLUTION_42) | Delta |
|--------|--------------|-------------------------|-------|
| SAT Solved | 174 | 170 | +4 |
| UNSAT Solved | 180 | 180 | 0 |
| Total Solved | 354 | 350 | +4 |
| PAR-2 SAT | 618.45 | — | — |
| PAR-2 UNSAT | 768.51 | — | — |
| PAR-2 Overall | 1744.21 | 1805.16 | -60.95 |
| Solved within 300s | 187 | — | — |
| Solved within 1000s | 263 | — | — |
| Solved within 3000s | 337 | — | — |
| Solved within 4500s | 351 | — | — |
| Wall time (s) | 5904.2 | — | — |
| Avg memory (MB) | 713.89 | — | — |
| Max memory (MB) | 16686.38 | — | — |

## Champion Status
- [x] New champion (PAR-2 1744.21 < 1805.16)
- [ ] Did not beat champion

## Acceptance Status
- Accepted as S_curr (seed for next cycle): Yes
- Acceptance α: 0.0 (strict greedy)
- Reason: PAR-2 strictly decreased from 1805.16 to 1744.21 (-60.95); accepted as new champion under α=0.

## Hypothesis Evaluation
- Hypothesis confirmed: Yes (Real win — PAR-2 1744.21 < 1761 threshold)
- Hypothesis: Iterative C42 — adding a second `kissat_transitive_reduction` call inside the C42 conditional block, after the conditional congruence, on the premise that congruence merges produce new equivalence binaries that a follow-up transitive reduction may now find redundant.
- Outcome: The improvement materialized strongly. PAR-2 dropped by ~61 points, well outside the noise band (σ ≈ 22). Total solved increased by 4 instances, all on the SAT side (170 → 174); UNSAT solved stayed flat at 180. This is consistent with the mechanism: a second transitive sweep after congruence-induced binary equivalence merges reduces the binary clause graph the search subsequently navigates, helping units propagate and shaving runtime on hard SAT instances near the timeout, without disrupting UNSAT progress.

## Comparison with Baseline (SATLUTION_42)
- Better:
  - SAT solving: +4 instances solved (174 vs 170).
  - PAR-2 overall: -60.95.
- Worse:
  - None observed at the aggregate metric level (UNSAT solved unchanged, overall PAR-2 strictly improved).

## Conclusions
- The transitive↔congruence junction continues to be a productive site for structural changes; iterating transitive after congruence yields a real and clearly-above-noise improvement.
- The gain is concentrated in SAT (+4 solved). UNSAT count is saturated at 180/199 in this configuration; squeezing more UNSAT likely requires structural changes elsewhere.
- Future cycles can probe deeper at this junction (e.g., further iterating, or interleaving with other simplifiers) but should be wary of diminishing returns and increased simplification cost on instances where transitive/congruence is hot.
- No new forbidden patterns discovered.

