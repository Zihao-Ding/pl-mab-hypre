# Painless SAT Solver Parameters Documentation

This document provides comprehensive documentation for all command-line parameters supported by the Painless SAT solver.

## Table of Contents
1. [General Options](#general-options)
2. [Portfolio Options](#portfolio-options)
3. [Solving Options](#solving-options)
4. [Preprocessing Options](#preprocessing-options)
5. [Sharing Options](#sharing-options)

---

## General Options

| Parameter | Flag | Type | Default | Description |
|-----------|------|------|--------|-------------|
| `help` | `-help` | bool | false | Prints this help message and exits |
| `details` | `-details=<string>` | string | "" | Gets detailed information about a specific category; use "-details=\*" for all |
| `filename` | `<input.cnf>` | string | "" | Input CNF formula file to solve |
| `cpus` | `-c=<int>` | int | Auto (hardware concurrency) | Number of solver threads to launch |
| `timeout` | `-t=<int>` | int | -1 | Timeout in seconds; -1 means no timeout |
| `verbosity` | `-v=<int>` | int | 0 | Verbosity level (0-5) |
| `test` | `-test` | bool | false | Use Test working strategy instead of default portfolio |
| `noModel` | `-no-model` | bool | false | Disable model output; only report satisfiability |
| `enableDistributed` | `-dist` | bool | false | Enable distributed solving via MPI |

### Verbosity Levels

The `-v` parameter controls output verbosity:

- **Level 0**: Minimal output (default)
- **Level 1**: Basic progress information
- **Level 2**: More detailed solver activity
- **Level 3**: Verbose clause sharing logs
- **Level 4**: Very verbose debugging output
- **Level 5**: Maximum verbosity with extensive logging

---

## Portfolio Options

| Parameter | Flag | Type | Default | Description |
|-----------|------|------|--------|-------------|
| `solver` | `-solver=<string>` | string | "kcl" | Portfolio solver combination strategy |
| `prs` | `-prs` | bool | false | Enable PortfolioPRS strategy |
| `enableMallob` | `-mallob` | bool | false | Emulate Mallob's sharing strategy in PortfolioSimple |
| `sbvaPostLocalSearchers` | `-ls-after-sbva=<int>` | int | 2 | Number of local search solvers to place after SBVA preprocessing (PortfolioSBVA) |
| `maxDivNoise` | `-max-div-noise=<int>` | int | 1000 | Maximum noise value for random diversification engine in solver configuration |
| `gaInitPeriod` | `-ga-init=<int>` | int | 0 | Genetic algorithm phase initialization; 0=disabled, n=period for ID where (ID % n) == 0 |
| `gaSeed` | `-ga-seed=<int>` | int | 0 | Random number generator seed for genetic algorithm diversification |
| `gaPopSize` | `-ga-pop-size=<int>` | int | 50 | Number of candidate solver configurations per generation in GA |
| `gaMaxGen` | `-ga-max-gen=<int>` | int | 100 | Maximum number of generations (iterations) in genetic algorithm |
| `gaMutRate` | `-ga-mut-rate=<float>` | float | 0.88 | Mutation rate; probability of randomly assigning a new genome value |
| `gaCrossRate` | `-ga-cross-rate=<float>` | float | 0.5 | Crossover rate; probability of creating crossover point in genome |

### Portfolio Solver Codes

The `-solver=<code>` parameter accepts single characters representing different solver combinations:

| Code | Includes |
|------|----------|
| `g` | Glucose Syrup |
| `k` | Kissat |
| `l` | Lingeling |
| `c` | CaDiCaL |
| `m` | MiniSat |
| `M` | MapleCOMSPS |
| `I` | Kissat INC (incremental) |
| `K` | Kissat MAB (Multi-Armed Bandit) |
| `y` | YalSAT (local search) |
| `t` | TaSSAT (local search) |

**kcl** = k,i,s,a,t,c,y (all 7 main types)

### Genetic Algorithm Parameters

The GA-based diversification features allow you to customize the solver portfolio creation process:

- **Population Size**: How many candidate solvers compete simultaneously
- **Generation Count**: Maximum iteration limit for the evolution process
- **Mutation Rate**: Probability of randomly changing a solver's genetic configuration
- **Crossover Rate**: Probability of combining configurations from two parents
- **Seed**: Reproducible randomization for testing
- **Initialization Period**: When to reset/refresh the genetic algorithm cycle

---

## Solving Options

| Parameter | Flag | Type | Default | Description |
|-----------|------|------|--------|-------------|
| `glucoseSplitHeuristic` | `-glc-split-heur=<int>` | int | 1 | Split heuristic for Glucose solver |
| `defaultClauseBufferSize` | `-default-clsbuff-size=<int>` | int | 1000 | Default clause buffer size for clause management |
| `localSearchFlips` | `-ls-flips=<int>` | int | -1 | Number of local search flip attempts; -1 uses default |

### Glucose Split Heuristics

The `-glc-split-heur` parameter controls how the Glucose solver selects clauses to split:

| Value | Name | Description |
|-------|------|-------------|
| 1 | None | No splitting performed |
| 2 | Random | Select clause randomly |
| 3 | Activity | Select clause with highest activity score |
| 4 | Phase | Select clause based on variable phase preferences |

### Local Search Flips

Controls how many flip operations the local search component attempts:

- **Default (-1)**: Uses internal default value
- **Positive values**: Exact number of flips to attempt


- Zero or negative values may trigger alternative behavior patterns

---

## Preprocessing Options

Two preprocessing strategies are supported:

### PRS Options

| Parameter | Flag | Type | Default | Description |
|-----------|------|------|--------|-------------|
| `prsCircuitVar` | `-prs-circuit-var=<int>` | int | 100,000 | Variable threshold for circuit preprocessing |
| `prsGaussVar` | `-prs-gauss-var=<int>` | int | 100,000 | Variable threshold for Gaussian elimination |
| `prsCardVar` | `-prs-card-var=<int>` | int | 100,000 | Variable threshold for cardinality constraints |
| `prsCircuitCls` | `-prs-circuit-cls=<int>` | int | 1,000,000 | Clause threshold for circuit technique |
| `prsGaussClsSize` | `-prs-gauss-cls-size=<int>` | int | 6 | Maximum clause size for Gaussian elimination clauses |
| `prsGaussCls` | `-prs-gauss-cls=<int>` | int | 1,000,000 | Clause threshold for Gaussian elimination |
| `prsBinCls` | `-prs-bin-cls=<int>` | int | 10,000,000 | Threshold for binary clauses |
| `prsCardCls` | `-prs-card-cls=<int>` | int | 1,000,000 | Threshold for cardinality constraint clauses |

### SBVA Options

| Parameter | Flag | Type | Default | Description |
|-----------|------|------|--------|-------------|
| `sbvaTimeout` | `-sbva-timeout=<int>` | int | 500 | Maximum SBVA processing time in seconds |
| `sbvaCount` | `-sbva-count=<int>` | int | 12 | Number of parallel SBVA threads |
| `sbvaMaxClause` | `-sbva-max-clause=<int>` | int | 10,000,000 | Maximum clauses SBVA will process |
| `sbvaMaxAdd` | `-sbva-max-add=<int>` | int | 0 | Maximum variable additions; 0 means unlimited |
| `sbvaNoShuffle` | `-no-sbva-shuffle=<bool>` | bool | false | Disables random shuffling in SBVA |

---

## Sharing Options

| Parameter | Flag | Type | Default | Description |
|-----------|------|------|--------|-------------|
| `maxClauseSize` | `-max-cls-size=<int>` | int | 60 | Maximum clause literal count for insertion into clause database |
| `initSleep` | `-init-sleep=<int>` | int | 10,000 | Initial sleep duration (microseconds) for sharer initialization |
| `sharingStrategy` | `-shr-strat=<int>` | int | 1 | Local sharing strategy selector |
| `globalSharingStrategy` | `-gshr-strat=<int>` | int | -1 | Global sharing strategy selection |
| `sharingSleep` | `-shr-sleep=<int>` | int | 500,000 | Sleep duration after each sharing round (microseconds) |
| `globalSharingSleep` | `-gshr-sleep=<int>` | int | 600,000 | Duration between global sharing rounds (microseconds) |
| `oneSharer` | `-one-sharer=<bool>` | bool | false | Enables single sharer instead of per-solver sharing |
| `globalSharedLiterals` | `-gshr-lit=<int>` | int | 2,000 | Quantity of literals shared across all processes |
| `sharedLiteralsPerProducer` | `-shr-lit-per-prod=<int>` | int | 1,500 | Literals contributed by each producer in local sharing |
| `simpleShareLimit` | `-simple-limit=<int>` | int | 10 | Size boundary for simple share clauses |

### Sharing Strategy Codes

#### Local Sharing Strategy (`-shr-strat`)

| Code | Strategy Name |
|------|---------------|
| 1 | HordeSat with per-entity buffer (default) |
| 2 | HordeSat with two producer groups |
| 3 | Simple sharing limited by clause size |

#### Global Sharing Strategy (`-gshr-strat`)

| Code | Strategy Name |
|------|---------------|
| 1 | AllGatherSharing |
| 2 | MallobSharing |
| 3 | GenericGlobalSharing (ring topology) |

### Clause Database Types

The `-importDB=<type>` and `-localSharingDB=<type>` parameters specify which clause database implementation to use:

| Type | Name | Description |
|------|------|-------------|
| `s` | SingleBuffer | Simple buffer with single storage |
| `m` | Mallob | Memory-efficient with compensation |
| `d` | PerSize | Clauses stored by literal size |
| `e` | PerSource | Buffer allocated per clause source |

### Sharing Sleep Durations

- **Local sharing sleep**: 500,000 microseconds (0.5 seconds)
- **Global sharing sleep**: 600,000 microseconds (0.6 seconds)

These intervals prevent excessive communication overhead during clause exchange.

---

## Parameter Parsing Behavior

Command-line arguments are processed in the following order:

1. Positional arguments treated as input file paths when not starting with `-`
2. Long-form arguments with `=` separator support both names and values
3. Boolean parameters default to `true` when present without value
4. Invalid parameters trigger error exit

### Argument Format Examples

```
# Short flags without values
./painless -c 8 -t 300 -v 2 input.cnf

# Flags with values
./painless "-solver=gkMcy" "-ga-pop-size=75" input.cnf

# Combination
./painless -prs -mallob -sbva-count=24 input.cnf
```

---

## Default Behavior Summary

When no parameters are provided beyond the input file:

- **Threads**: Auto-detected from hardware concurrency (typically 8-16)
- **Timeout**: Unlimited (-1 seconds)
- **Verbosity**: Level 0 (minimal output)
- **Strategy**: Standard Portfolio with Kissat-Lingeling-CaDiCaL-YalSat diversification
- **Distributed**: Disabled (single process)
- **Model Output**: Enabled
- **Preprocessing**: None
- **Sharing**: Default local/global sharing with per-solver buffers

---

## Related Documentation

Detailed explanations of portfolio strategies, preprocessing techniques (PRS/SBVA), and clause sharing mechanisms are available in the project's documentation.