#include "reduce.h"
#include "allocate.h"
#include "arena.h"
#include "clause.h"
#include "collect.h"
#include "inline.h"
#include "inlineheap.h"
#include "print.h"
#include "rank.h"
#include "reference.h"
#include "report.h"
#include "tiers.h"
#include "trail.h"

#include <inttypes.h>
#include <math.h>

// Forward declarations for PRIMA functions
static void extract_prima_features (kissat *solver, prima_state_t *state);
static double compute_value_estimate (prima_state_t *state);
static void update_value_function (prima_state_t *state, double reward);
static double compute_predictive_reward (kissat *solver, prima_state_t *state,
                                         unsigned conflicts_considered);
static unsigned kissat_compute_prima_interval (kissat *solver, prima_state_t *state);

// EVOLVE_START
// PRIMA: Predictive Reinforced Interval Management for Ageing

// PRIMA: Initialize PRIMA state
void kissat_init_prima (kissat *solver) {
  prima_state_t *prima = &solver->prima_state;
  reduction_utility_state_t *rus = &solver->reduction_utility_state;
  
  // RL state representation
  prima->clause_pool_density = 0.5;
  prima->clause_usefulness_rate = 0.5;
  prima->search_progress_rate = 0.5;
  
  // Temporal difference learning
  prima->value_estimate = 0.5;
  prima->last_value_estimate = 0.5;
  prima->alpha = 0.1;
  prima->gamma = 0.95;
  
  // Feature vector
  for (int i = 0; i < 16; i++) {
    prima->features[i] = 0.0;
  }
  prima->feature_count = 0;
  
  // Action history
  prima->last_reduction_conflicts = 0;
  prima->conflicts_at_last_action = 0;
  prima->last_reward = 0.0;
  
  // Policy parameters
  prima->epsilon = 0.1;
  prima->threshold_bias = 0.0;
  
  // Initialize utility tracking
  for (int i = 0; i < 10; i++) {
    rus->utility_buckets[i] = 0;
  }
  rus->avg_clause_utility = 0.5;
  rus->utility_variance = 0.25;
  rus->utility_samples = 0;
  
  // Initialize tier counts
  rus->tier1_count = 0;
  rus->tier2_count = 0;
  rus->tier3_count = 0;
  
  // Initialize reduction history
  rus->last_reduction_conflicts = 0;
  rus->conflicts_since_last_reduction = 0;
  rus->reduction_count = 0;
  rus->avg_reduction_interval = GET_OPTION (reduceint);
  
  // Initialize last reduction results
  rus->clauses_considered_last_reduction = 0;
  rus->clauses_removed_last_reduction = 0;
  
  // Initialize adaptive parameters
  rus->utility_threshold = 0.5;
  rus->base_interval = GET_OPTION (reduceint);
  rus->min_interval = GET_OPTION (reduceint) / 2;
  rus->max_interval = GET_OPTION (reduceint) * 4;
}

// PRIMA: Extract features from current state
static void extract_prima_features (kissat *solver, prima_state_t *state) {
  reduction_utility_state_t *rus = &solver->reduction_utility_state;
  statistics *stats = &solver->statistics;
  
  unsigned idx = 0;
  
  // Feature 1: Clause pool density
  uint64_t total_clauses = stats->clauses_learned;
  uint64_t max_clauses = solver->vars * 15;
  state->features[idx++] = (double)total_clauses / fmax (1, (double)max_clauses);
  
  // Feature 2: Tier distribution
  unsigned tier_total = rus->tier1_count + rus->tier2_count + rus->tier3_count;
  if (tier_total > 0) {
    state->features[idx++] = (double)rus->tier1_count / tier_total;
    state->features[idx++] = (double)rus->tier2_count / tier_total;
    state->features[idx++] = (double)rus->tier3_count / tier_total;
  } else {
    state->features[idx++] = 0.0;
    state->features[idx++] = 0.0;
    state->features[idx++] = 1.0;
  }
  
  // Feature 3: Utility distribution
  state->features[idx++] = rus->avg_clause_utility;
  state->features[idx++] = sqrt (rus->utility_variance);
  
  // Feature 4: Recent performance
  uint64_t conflicts_since_last = stats->conflicts - rus->last_reduction_conflicts;
  double conflicts_per_decision = (stats->decisions > 0) ?
    (double)stats->conflicts / stats->decisions : 0.0;
  state->features[idx++] = conflicts_per_decision;
  
  // Feature 5: Time since last reduction
  state->features[idx++] = tanh ((double)conflicts_since_last / 10000.0);
  
  // Feature 6: Reduction efficiency history
  if (rus->reduction_count > 0) {
    double recent_efficiency = (double)rus->clauses_removed_last_reduction /
                               fmax (1, (double)rus->clauses_considered_last_reduction);
    state->features[idx++] = recent_efficiency;
  } else {
    state->features[idx++] = 0.5;
  }
  
  // Feature 7: Clause creation rate
  double clause_rate = (conflicts_since_last > 0) ?
    (double)total_clauses / conflicts_since_last : 0.0;
  state->features[idx++] = clause_rate;
  
  // Feature 8: Search stability
  state->features[idx++] = rus->avg_clause_utility * (1.0 - conflicts_per_decision);
  
  state->feature_count = idx;
  
  // Normalize features to [0, 1]
  for (unsigned i = 0; i < idx; i++) {
    state->features[i] = fmax (0.0, fmin (1.0, state->features[i]));
  }
}

// PRIMA: Value function approximation
static double compute_value_estimate (prima_state_t *state) {
  double value = state->threshold_bias;
  
  // Weights for feature-based value estimation
  double weights[11] = {
    0.3,   // pool_density
    -0.2,  // tier1_ratio (negative = less tier1 is concerning)
    0.1,   // tier2_ratio
    0.2,   // tier3_ratio (higher tier3 = more to reduce)
    -0.15, // avg_utility (negative = lower utility is concerning)
    0.1,   // utility_variance
    0.15,  // conflicts_per_decision
    0.2,   // time_since_last (longer = more valuable)
    -0.1,  // recent_efficiency (negative = inefficient = need more)
    0.25,  // clause_rate (high creation rate = need to reduce)
    0.05   // search_stability
  };
  
  for (unsigned i = 0; i < state->feature_count && i < 11; i++) {
    value += weights[i] * state->features[i];
  }
  
  return value;
}

// PRIMA: Temporal difference learning update
static void update_value_function (prima_state_t *state, double reward) {
  double old_value = state->value_estimate;
  
  // TD(0) update: V(s) = V(s) + alpha * (reward + gamma * V(s') - V(s))
  double td_error = reward + state->gamma * state->value_estimate - old_value;
  
  // Update value estimate
  state->value_estimate += state->alpha * td_error;
  
  // Update threshold bias based on TD error
  state->threshold_bias += state->alpha * td_error * 0.1;
  
  // Adaptive learning rate based on recent TD errors
  state->alpha = fmax (0.01, fmin (0.3, state->alpha * (1.0 + td_error * 0.01)));
}

// PRIMA: Compute predictive reward
static double compute_predictive_reward (kissat *solver, prima_state_t *state,
                                         unsigned conflicts_considered) {
  reduction_utility_state_t *rus = &solver->reduction_utility_state;
  
  // Reward components:
  // 1. Memory freed (normalized)
  double memory_reward = (double)conflicts_considered / 10000.0;
  
  // 2. Improvement in clause quality
  double quality_improvement = 0.0;
  if (rus->reduction_count > 0) {
    double prev_efficiency = (double)rus->clauses_removed_last_reduction /
                            fmax (1, (double)rus->clauses_considered_last_reduction);
    quality_improvement = prev_efficiency * 0.5;
  }
  
  // 3. Search progress rate improvement
  uint64_t conflicts = solver->statistics.conflicts;
  double progress_rate = (conflicts - state->conflicts_at_last_action) /
                         fmax (1, (double)conflicts);
  double progress_reward = progress_rate * 0.3;
  
  // 4. Penalty for excessive reduction
  double over_reduction_penalty = 0.0;
  if (conflicts_considered > 5000) {
    over_reduction_penalty = -0.2;
  }
  
  // Combined reward with weighting
  double reward = memory_reward + quality_improvement +
                  progress_reward + over_reduction_penalty;
  
  // Normalize to [-1, 1] range
  return tanh (reward);
}

// PRIMA: Sample clause utility
void kissat_sample_clause_utility (kissat *solver) {
  reduction_utility_state_t *rus = &solver->reduction_utility_state;
  
  // Sample every 100 conflicts
  if (solver->statistics.conflicts % 100 != 0)
    return;
  
  // Use heuristic-based utility estimation
  rus->tier1_count = solver->tier1[0];
  rus->tier2_count = solver->tier2[1];
  rus->tier3_count = solver->statistics.clauses_learned - rus->tier1_count - rus->tier2_count;
  
  // Estimate utility based on tier distribution
  unsigned total = rus->tier1_count + rus->tier2_count + rus->tier3_count;
  if (total > 0) {
    double tier1_ratio = (double) rus->tier1_count / total;
    double tier2_ratio = (double) rus->tier2_count / total;
    double estimated_utility = tier1_ratio * 1.0 + tier2_ratio * 0.7 + (1.0 - tier1_ratio - tier2_ratio) * 0.3;
    rus->avg_clause_utility = (rus->avg_clause_utility * rus->utility_samples + estimated_utility) /
                              (rus->utility_samples + 1);
    rus->utility_samples++;
  }
}

// PRIMA: Compute adaptive interval
static unsigned kissat_compute_prima_interval (kissat *solver, prima_state_t *state) {
  reduction_utility_state_t *rus = &solver->reduction_utility_state;
  
  // Base interval from configuration
  unsigned interval = rus->base_interval;
  
  // Adjust based on utility variance
  if (rus->avg_clause_utility > 0) {
    double cv = sqrt (rus->utility_variance) / rus->avg_clause_utility;
    if (cv > 0.5) {
      interval = (unsigned) (interval * 0.8);
    } else if (cv < 0.2) {
      interval = (unsigned) (interval * 1.2);
    }
  }
  
  // Adjust based on tier distribution
  unsigned total_clauses = rus->tier1_count + rus->tier2_count + rus->tier3_count;
  if (total_clauses > 0) {
    double tier1_ratio = (double) rus->tier1_count / total_clauses;
    if (tier1_ratio < 0.1) {
      interval = (unsigned) (interval * 0.9);
    } else if (tier1_ratio > 0.4) {
      interval = (unsigned) (interval * 1.1);
    }
  }
  
  // Adjust based on recent performance
  if (rus->reduction_count > 0 && rus->clauses_considered_last_reduction > 0) {
    double recent_efficiency = (double) rus->clauses_removed_last_reduction /
                               rus->clauses_considered_last_reduction;
    if (recent_efficiency < 0.3) {
      interval = (unsigned) (interval * 0.85);
    } else if (recent_efficiency > 0.7) {
      interval = (unsigned) (interval * 1.15);
    }
  }
  
  // Clamp to bounds
  interval = MAX (interval, rus->min_interval);
  interval = MIN (interval, rus->max_interval);
  
  return interval;
}

// PRIMA: Determine if reduction should be triggered
bool kissat_prima_should_reduce (kissat *solver) {
  if (!GET_OPTION (reduce))
    return false;
  
  if (!solver->statistics.clauses_redundant)
    return false;
  
  prima_state_t *state = &solver->prima_state;
  reduction_utility_state_t *rus = &solver->reduction_utility_state;
  
  // Extract current state features
  extract_prima_features (solver, state);
  
  // Compute value estimate
  state->value_estimate = compute_value_estimate (state);
  
  // Check minimum interval
  unsigned interval = kissat_compute_prima_interval (solver, state);
  uint64_t conflicts = solver->statistics.conflicts;
  
  if (conflicts - rus->last_reduction_conflicts < interval)
    return false;
  
  // Epsilon-greedy policy with learned value
  double action_value = state->value_estimate + state->threshold_bias;
  bool should_reduce = false;
  
  // Exploration: random action with probability epsilon
  if (rand () / (double) RAND_MAX < state->epsilon) {
    should_reduce = (rand () / (double) RAND_MAX < 0.5);
  } else {
    // Exploitation: use learned value estimate
    should_reduce = (action_value > 0.5);
  }
  
  // Store state for TD learning after reduction
  state->conflicts_at_last_action = conflicts;
  state->last_value_estimate = state->value_estimate;
  
  return should_reduce;
}

// PRIMA: Post-reduction TD learning
void kissat_prima_post_reduction (kissat *solver, unsigned clauses_considered,
                                  unsigned clauses_removed) {
  prima_state_t *state = &solver->prima_state;
  reduction_utility_state_t *rus = &solver->reduction_utility_state;
  
  // Compute reward based on reduction outcome
  double reward = compute_predictive_reward (solver, state, clauses_considered);
  
  // TD update
  update_value_function (state, reward);
  
  // Update exploration rate (decay epsilon)
  state->epsilon = fmax (0.05, state->epsilon * 0.98);
  
  // Update threshold bias based on outcome
  double efficiency = (clauses_considered > 0) ?
    (double)clauses_removed / clauses_considered : 0.0;
  
  if (efficiency > 0.5) {
    state->threshold_bias += 0.01;
  } else if (efficiency < 0.2) {
    state->threshold_bias -= 0.01;
  }
  
  // Update reduction history
  rus->reduction_count++;
  rus->clauses_considered_last_reduction = clauses_considered;
  rus->clauses_removed_last_reduction = clauses_removed;
  rus->last_reduction_conflicts = solver->statistics.conflicts;
  
  LOG ("PRIMA: value=%.3f bias=%.3f epsilon=%.3f removed=%u/%u eff=%.2f",
       state->value_estimate, state->threshold_bias, state->epsilon,
       clauses_removed, clauses_considered, efficiency);
}

// U-ARS: Wrapper functions for compatibility
void kissat_sample_clause_utility_uars (kissat *solver) {
  kissat_sample_clause_utility (solver);
}

void kissat_post_reduction_analysis (kissat *solver, unsigned clauses_considered,
                                     unsigned clauses_removed) {
  // Always use PRIMA post-reduction
  kissat_prima_post_reduction (solver, clauses_considered, clauses_removed);
}

// Initialize U-ARS state (for compatibility)
void kissat_init_uars (kissat *solver) {
  // Always use PRIMA initialization
  kissat_init_prima (solver);
}

bool kissat_reducing (kissat *solver) {
  if (!GET_OPTION (reduce))
    return false;
  if (!solver->statistics.clauses_redundant)
    return false;
  if (CONFLICTS < solver->limits.reduce.conflicts)
    return false;
  return true;
}

// U-ARS enabled reducing check
bool kissat_reducing_uars (kissat *solver) {
  if (!GET_OPTION (reduce))
    return false;
  if (!solver->statistics.clauses_redundant)
    return false;
  
  // Always use PRIMA logic
  return kissat_prima_should_reduce (solver);
}
// EVOLVE_END

typedef struct reducible reducible;

struct reducible {
  uint64_t rank;
  unsigned ref;
};

#define RANK_REDUCIBLE(RED) (RED).rank

// clang-format off
typedef STACK (reducible) reducibles;
// clang-format on

static bool collect_reducibles (kissat *solver, reducibles *reds,
                                reference start_ref) {
  assert (start_ref != INVALID_REF);
  assert (start_ref <= SIZE_STACK (solver->arena));
  ward *const arena = BEGIN_STACK (solver->arena);
  clause *start = (clause *) (arena + start_ref);
  const clause *const end = (clause *) END_STACK (solver->arena);
  assert (start < end);
  while (start != end && !start->redundant)
    start = kissat_next_clause (start);
  if (start == end) {
    solver->first_reducible = INVALID_REF;
    LOG ("no reducible clause candidate left");
    return false;
  }
  const reference redundant = (ward *) start - arena;
#ifdef LOGGING
  if (redundant < solver->first_reducible)
    LOG ("updating start of redundant clauses from %zu to %zu",
         (size_t) solver->first_reducible, (size_t) redundant);
  else
    LOG ("no update to start of redundant clauses %zu",
         (size_t) solver->first_reducible);
#endif
  solver->first_reducible = redundant;
  const unsigned tier1 = TIER1;
  const unsigned tier2 = MAX (tier1, TIER2);
  assert (tier1 <= tier2);
  for (clause *c = start; c != end; c = kissat_next_clause (c)) {
    if (!c->redundant)
      continue;
    if (c->garbage)
      continue;
    const unsigned used = c->used;
    if (used)
      c->used = used - 1;
    if (c->reason)
      continue;
    const unsigned glue = c->glue;
    if (glue <= tier1 && used)
      continue;
    if (glue <= tier2 && used >= MAX_USED - 1)
      continue;
    assert (kissat_clause_in_arena (solver, c));
    reducible red;
    const uint64_t negative_size = ~c->size;
    const uint64_t negative_glue = ~c->glue;
    red.rank = negative_size | (negative_glue << 32);
    red.ref = (ward *) c - arena;
    PUSH_STACK (*reds, red);
  }
  if (EMPTY_STACK (*reds)) {
    kissat_phase (solver, "reduce", GET (reductions),
                  "did not find any reducible redundant clause");
    return false;
  }
  return true;
}

#define USEFULNESS RANK_REDUCIBLE

static void sort_reducibles (kissat *solver, reducibles *reds) {
  RADIX_STACK (reducible, uint64_t, *reds, USEFULNESS);
}

static void mark_less_useful_clauses_as_garbage (kissat *solver,
                                                 reducibles *reds) {
  statistics *statistics = &solver->statistics;
  const double high = GET_OPTION (reducehigh) * 0.1;
  const double low = GET_OPTION (reducelow) * 0.1;
  double percent;
  if (low < high) {
    const double delta = high - low;
    percent = high - delta / log10 (statistics->reductions + 9);
  } else
    percent = low;
  const double fraction = percent / 100.0;
  const size_t size = SIZE_STACK (*reds);
  size_t target = size * fraction;
#ifndef QUIET
  const size_t clauses =
      statistics->clauses_irredundant + statistics->clauses_redundant;
  kissat_phase (solver, "reduce", GET (reductions),
                "reducing %zu (%.0f%%) out of %zu (%.0f%%) "
                "reducible clauses",
                target, kissat_percent (target, size), size,
                kissat_percent (size, clauses));
#endif
  unsigned reduced = 0, reduced1 = 0, reduced2 = 0, reduced3 = 0;
  ward *arena = BEGIN_STACK (solver->arena);
  const reducible *const begin = BEGIN_STACK (*reds);
  const reducible *const end = END_STACK (*reds);
  const unsigned tier1 = TIER1;
  const unsigned tier2 = TIER2;
  for (const reducible *p = begin; p != end && target--; p++) {
    clause *c = (clause *) (arena + p->ref);
    assert (kissat_clause_in_arena (solver, c));
    assert (!c->garbage);
    assert (!c->reason);
    assert (c->redundant);
    LOGCLS (c, "reducing");
    kissat_mark_clause_as_garbage (solver, c);
    reduced++;
    if (c->glue <= tier1)
      reduced1++;
    else if (c->glue <= tier2)
      reduced2++;
    else
      reduced3++;
  }
  ADD (clauses_reduced_tier1, reduced1);
  ADD (clauses_reduced_tier2, reduced2);
  ADD (clauses_reduced_tier3, reduced3);
  ADD (clauses_reduced, reduced);
}

int kissat_reduce (kissat *solver) {
  START (reduce);
  INC (reductions);
  kissat_phase (solver, "reduce", GET (reductions),
                "reduce limit %" PRIu64 " hit after %" PRIu64 " conflicts",
                solver->limits.reduce.conflicts, CONFLICTS);
  kissat_compute_and_set_tier_limits (solver);
  bool compact = kissat_compacting (solver);
  reference start = compact ? 0 : solver->first_reducible;
  if (start != INVALID_REF) {
#ifndef QUIET
    size_t arena_size = SIZE_STACK (solver->arena);
    size_t words_to_sweep = arena_size - start;
    size_t bytes_to_sweep = sizeof (word) * words_to_sweep;
    kissat_phase (solver, "reduce", GET (reductions),
                  "reducing clauses after offset %" REFERENCE_FORMAT
                  " in arena",
                  start);
    kissat_phase (solver, "reduce", GET (reductions),
                  "reducing %zu words %s %.0f%%", words_to_sweep,
                  FORMAT_BYTES (bytes_to_sweep),
                  kissat_percent (words_to_sweep, arena_size));
#endif
    if (kissat_flush_and_mark_reason_clauses (solver, start)) {
      reducibles reds;
      INIT_STACK (reds);
      if (collect_reducibles (solver, &reds, start)) {
        sort_reducibles (solver, &reds);
        mark_less_useful_clauses_as_garbage (solver, &reds);
        RELEASE_STACK (reds);
        kissat_sparse_collect (solver, compact, start);
      } else if (compact)
        kissat_sparse_collect (solver, compact, start);
      else
        kissat_unmark_reason_clauses (solver, start);
    } else
      assert (solver->inconsistent);
  } else
    kissat_phase (solver, "reduce", GET (reductions), "nothing to reduce");
  kissat_classify (solver);
  UPDATE_CONFLICT_LIMIT (reduce, reductions, SQRT, false);
  solver->last.conflicts.reduce = CONFLICTS;
  REPORT (0, '-');
  STOP (reduce);
  return solver->inconsistent ? 20 : 0;
}
