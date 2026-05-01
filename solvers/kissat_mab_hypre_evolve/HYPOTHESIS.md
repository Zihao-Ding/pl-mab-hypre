# Cycle 51 Hypothesis — Iterative C42 (second transitive after the conditional congruence)

## Background
C42 (current champion) added a conditional `kissat_congruence` after `kissat_transitive_reduction` made progress:

```c
bool transitive_reduced = kissat_transitive_reduction (solver);
if (!solver->inconsistent && transitive_reduced)
  kissat_congruence (solver);
```

The structural argument: transitive reduction REMOVES redundant binaries → exposes gate structure that congruence can detect.

If the C42 conditional congruence finds new equivalences/merges, those merges produce NEW binary clauses (equivalence pairs added to the implication graph). Those new binaries may themselves contain transitive redundancies that the just-completed transitive_reduction couldn't have seen. Adding ONE more `kissat_transitive_reduction` call inside the C42 block would clean these up.

This stays at the SAME junction (transitive↔congruence) where C42 won. Different from C43/C45/C46 which transposed the C42 pattern to OTHER junctions and lost.

## Mechanism
Replace the C42 conditional block:
```c
if (!solver->inconsistent && transitive_reduced)
  kissat_congruence (solver);
```

with:
```c
if (!solver->inconsistent && transitive_reduced) {
  kissat_congruence (solver);
  if (!solver->inconsistent)
    kissat_transitive_reduction (solver);   // second round, clean new binaries from congruence's merges
}
```

3-line addition (brace-wrap + 2 new lines).

## Critical files
- `src/probe.c::probe()` — modify the C42 conditional block.

## Correctness
- The first transitive_reduction (capturing `transitive_reduced`) is unchanged.
- The C42 conditional congruence is unchanged.
- The added second transitive_reduction is gated on `!solver->inconsistent`. Bounded by transitive's own internal budget; not a runaway loop.
- Single iteration — no while-loop deepening.

## Why this is structurally different from C43/C45/C46

- **C43** gated 2nd substitute on vivify+sweep progress — substitute consumes equivalences from many upstream sources, gate too narrow. **(LOST.)**
- **C45** gated congruence on factor's variables_extension — wrong structural argument (factor adds vars; congruence detects gates). **(LOST.)**
- **C46** added substitute INSIDE C42's conditional — substitute is implicit in congruence's downstream consumer; redundant. **(LOST.)**
- **C51 (this)** stays at the SAME junction: transitive→congruence→transitive. The argument is "congruence MERGES variables, producing new equivalence binaries; transitive may now find new redundancies." Same structural shape as C42's first half.

## Risks
- 4 prior C42 extensions failed (C43, C45, C46, C48). The streak is real.
- If congruence rarely finds new merges in the C42 conditional path, the second transitive_reduction is mostly wasted work — small noise-band regression.
- If congruence finds many merges, the second transitive could be productive, yielding a small improvement.

## Forbidden-pattern compliance
- No malloc/free/calloc, no statics, no forbidden literals.

## Expected outcome
- Within noise band; best case small gain ~−10 PAR-2.
- Worst case: small regression similar to C46 +50ish PAR-2 was its result.
