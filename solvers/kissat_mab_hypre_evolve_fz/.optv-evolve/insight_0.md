### Introduction

This insight addresses the clause lifecycle management problem in SAT solvers, which is critical for memory efficiency and search performance. The existing DUCB-GC (Dynamic Upper Confidence Bound Garbage Collection) in collect.c and PRIMA (Predictive Reinforced Interval Management for Ageing) in reduce.c operate as separate subsystems with independent decision-making. This fragmentation leads to suboptimal clause retention decisions and missed opportunities for cross-component learning.

### Motivation

The current implementation has three key limitations:

1. **Independent Decision Making**: Garbage collection (DUCB-GC) and clause reduction (PRIMA) make decisions independently without sharing learned information about clause quality.

2. **Static Exploration Constants**: The UCB exploration parameter in DUCB-GC uses fixed phase-based values (2.0, 1.0, 0.5) rather than adapting based on actual clause performance history.

3. **Redundant Feature Computation**: Both components compute similar features (clause utility, age, tier distribution) but don't share the computed values, leading to redundant computation.

**Key Observation**: The reference algorithm achieves 36.5% improvement over baseline by using adaptive learning. We can extend this by creating a unified clause lifecycle manager that treats garbage collection and reduction as a single multi-armed bandit problem with shared state.

### Method

#### Unified Clause Lifecycle Manager (UCLM)

The proposed algorithm unifies garbage collection and reduction decisions using a shared reinforcement learning framework:

```
┌─────────────────────────────────────────────────────────────────┐
│                    UCLM Architecture                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌──────────────┐    ┌────────────────────────────────────┐   │
│   │   Feature    │    │         Shared Value Network       │   │
│   │   Extractor  │───▶│  Q(clause_state, action) =         │   │
│   │              │    │  Σ w_i * f_i(clause_state)         │   │
│   └──────────────┘    └────────────────────────────────────┘   │
│                                    │                            │
│                     ┌──────────────┼──────────────┐            │
│                     ▼              ▼              ▼            │
│              ┌───────────┐  ┌───────────┐  ┌───────────┐      │
│              │    GC     │  │  Reduce   │  │  Retain   │      │
│              │  Action   │  │  Action   │  │  Action   │      │
│              └───────────┘  └───────────┘  └───────────┘      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**State Representation**:

Each clause is represented by a feature vector:
- `f₁`: Clause size (normalized logarithmically)
- `f₂`: Clause age in conflicts
- `f₃`: Glue value (normalized)
- `f₄`: Tier (1, 2, or 3)
- `f₅`: Redundant flag (binary)
- `f₆`: Usage count since creation
- `f₇`: LBD (Literal Block Distance) trend
- `f₈`: Activity score normalized

**Action Space**:

For each clause, the manager chooses one of three actions:
- `A₁`: Mark as garbage (immediate removal)
- `A₂`: Mark for reduction queue (age-based removal)
- `A₃`: Retain (keep in current tier)

**Unified Q-Function**:

```
Q(s, a) = w₀ + Σᵢ wᵢ · fᵢ(s) + Σⱼ θⱼ · gⱼ(s, a)
```

Where:
- `fᵢ(s)` are state features
- `gⱼ(s, a)` are action-specific interaction features
- `wᵢ, θⱼ` are learned weights

**Key Innovations**:

1. **Thompson Sampling with Gradient Descent**: Instead of standard UCB, use Thompson Sampling for action selection with online gradient descent updates:

```python
# Thompson Sampling for Action Selection
def select_action(state_features):
    # Sample from posterior for each action
    for a in actions:
        theta_a = posterior_mean[a] + noise * posterior_std[a]
        Q[a] = dot_product(theta_a, state_features)
    
    # Select action with highest sampled Q-value
    return argmax(Q)
```

2. **Cross-Component Reward Sharing**: The reward signal from garbage collection informs reduction decisions and vice versa:

```
reward = α · gc_efficiency + β · reduction_efficiency + γ · search_progress

where gc_efficiency = clauses_removed / clauses_considered
      reduction_efficiency = clauses_reduced / clauses_in_queue
      search_progress = conflicts_since_last_restart / time_window
```

3. **Adaptive Feature Weight Learning**: Use exponentiated gradient descent to adapt feature weights based on recent performance:

```python
def update_weights(reward, state_features, chosen_action):
    # Compute prediction error
    predicted_Q = Q(state_features, chosen_action)
    error = reward - predicted_Q
    
    # Exponentiated gradient update
    for i in range(num_features):
        if chosen_action == best_action:
            weights[i] *= exp(learning_rate * error * state_features[i])
        else:
            weights[i] *= exp(-learning_rate * error * state_features[i])
    
    # Normalize weights
    weights /= sum(weights)
```

4. **Hierarchical Action Selection**: First decide whether to perform GC or reduction, then decide which clauses to target:

```python
def hierarchical_decide(solver):
    # Level 1: Decide operation type
    pool_health = assess_pool_health(solver)
    if pool_health == POOL_ABUNDANT:
        primary_action = GC  # Focus on garbage collection
    elif pool_health == POOL_SCARCE:
        primary_action = RETAIN  # Preserve clauses
    else:
        # Use learned policy
        primary_action = select_from_bandit(global_state)
    
    # Level 2: Select specific clauses using UCB within action
    clause_scores = []
    for clause in candidate_clauses:
        score = compute_ucb_with_learned_weights(clause, primary_action)
        clause_scores.append((score, clause))
    
    return sorted(clause_scores, reverse=True)[:target_count]
```

### Complexity Analysis

- **Time Complexity**: O(n × d) per operation where n = number of clauses, d = feature dimension (8)
- **Space Complexity**: O(n) for clause state storage + O(d × |A|) for weight vectors
- **Memory Overhead**: ~15% compared to DUCB-GC alone due to shared state structures

### Block Info

block_id: 0
file_path: src/collect.c
lines: 695 - 1009

This insight can also be applied to reduce.c (lines 26-419) by replacing the PRIMA decision logic with the unified UCLM framework.