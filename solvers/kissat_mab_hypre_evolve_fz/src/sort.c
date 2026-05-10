#include "internal.h"
#include "inlineheap.h"
#include "inlinevector.h"
#include "logging.h"
#include "vector.h"
#include <math.h>
#include <string.h>

// EVOLVE_START
// PRECISE: Priority-based REward-driven Clause Improvement with State Estimation
// Bandit-based propagation scheduling

// Initialize PRECISE state (made static to avoid multiple definition via dense.c include)
static void init_precise (kissat *solver) {
  if (!GET_OPTION (caps))
    return;
  
  precise_state_t *state = &solver->precise_state;
  
  // Initialize bandit state
  for (int i = 0; i < 8; i++) {
    state->arm_rewards[i] = 0.5;  // Initialize to neutral reward
    state->arm_selections[i] = 0;
  }
  state->ucb_exploration = 2.0;
  
  // Initialize state tracking
  state->last_update_conflicts = 0;
  state->recent_success_rate = 0.5;
  for (unsigned i = 0; i < 32; i++) {
    state->success_window[i] = 0;
  }
  state->success_idx = 0;
  state->success_count = 0;
  
  // Initialize adaptive parameters
  state->priority_bias = 0.5;
  state->search_phase = 0;
  
  // Initialize performance metrics
  state->total_propagations = 0;
  state->successful_propagations = 0;
  state->avg_propagation_depth = 0.0;
  
  (void) solver;
}

// Classify clause into category based on size and glue
// Category: 0-7 (size * 3 + glue_category)
// Size: 0=small(<=3), 1=medium(4-8), 2=large(>8)
// Glue: 0=low(<=5), 1=medium(6-15), 2=high(>15)
static unsigned classify_clause_category (clause *c) {
  unsigned size = c->size;
  unsigned glue = c->glue;
  
  // Determine size category
  unsigned size_cat;
  if (size <= 3) size_cat = 0;       // Small
  else if (size <= 8) size_cat = 1;  // Medium
  else size_cat = 2;                  // Large
  
  // Determine glue category
  unsigned glue_cat;
  if (glue <= 5) glue_cat = 0;        // Low
  else if (glue <= 15) glue_cat = 1;  // Medium
  else glue_cat = 2;                   // High
  
  // Map to arm index
  return size_cat * 3 + glue_cat;
}

// UCB-based category selection for exploration
static unsigned precise_select_category (precise_state_t *state) {
  // Check if any arm has been selected
  unsigned total_selections = 0;
  for (int i = 0; i < 8; i++) {
    total_selections += state->arm_selections[i];
  }
  
  // If no data, use default priority (small, low glue = 0)
  if (total_selections == 0) return 0;
  
  // UCB selection
  double best_value = -1.0;
  unsigned best_arm = 0;
  
  for (int i = 0; i < 8; i++) {
    double exploitation = state->arm_rewards[i];
    double exploration = state->ucb_exploration * 
                        sqrt (log ((double) total_selections + 1) / 
                             (state->arm_selections[i] + 1));
    double ucb_value = exploitation + exploration;
    
    if (ucb_value > best_value) {
      best_value = ucb_value;
      best_arm = i;
    }
  }
  
  return best_arm;
}

// Compute adaptive priority using PRECISE
static double
compute_precise_priority (kissat *solver, clause *c,
                         const assigned *assigned,
                         precise_state_t *state) {
  if (!GET_OPTION (caps))
    return 0.0;
  
  // Base priority from clause properties
  double priority = 1.0;
  
  // Size factor: inverse logarithmic (smaller = higher priority)
  if (c->size > 0) {
    priority *= 1.0 / (1.0 + log ((double) c->size));
  }
  
  // Glue factor: inverse logarithmic with adaptation
  double glue_factor = 1.0 / (1.0 + log1p ((double) c->glue));
  
  // Adjust glue factor based on category performance
  unsigned category = classify_clause_category (c);
  if (state->arm_selections[category] > 10) {
    // Scale by learned reward (0.5 = neutral, >0.5 = good, <0.5 = bad)
    glue_factor *= (0.5 + state->arm_rewards[category]);
  }
  priority *= glue_factor;
  
  // Level-based bonus (same as original CAPS)
  if (c->size > 0 && solver->level > 0) {
    unsigned first_literal = c->lits[0];
    unsigned var = IDX (first_literal);
    if (assigned[var].level == solver->level) {
      priority *= 1.5;
    }
    if (c->size > 1) {
      unsigned second_literal = c->lits[1];
      unsigned var2 = IDX (second_literal);
      if (assigned[var2].level == solver->level) {
        priority *= 1.5;
      }
    }
  }
  
  // Redundant clause penalty with learning
  if (c->redundant) {
    priority *= 0.8 * (0.5 + state->priority_bias);
  }
  
  // Phase-based adjustment
  if (state->search_phase == 2) {
    // Deep search: prefer low-glue clauses more
    priority *= (1.0 + (1.0 - glue_factor) * 0.5);
  }
  
  return priority;
}

// Update reward after clause propagation
static void precise_update_reward (precise_state_t *state,
                                   unsigned category,
                                   bool propagation_success,
                                   unsigned propagation_depth) {
  // Update selection count
  state->arm_selections[category]++;
  
  // Compute reward (1.0 for success, 0.0 for failure, with depth bonus)
  double reward = propagation_success ? 1.0 : 0.0;
  
  // Add depth bonus (deeper propagation = more valuable)
  if (propagation_success) {
    reward += tanh ((double) propagation_depth / 10.0) * 0.5;
  }
  
  // Update arm reward using incremental mean
  double old_reward = state->arm_rewards[category];
  double n = (double) state->arm_selections[category];
  state->arm_rewards[category] = old_reward + (reward - old_reward) / n;
  
  // Update success tracking
  state->success_window[state->success_idx] = propagation_success ? 1 : 0;
  state->success_idx = (state->success_idx + 1) % 32;
  if (state->success_count < 32) state->success_count++;
  
  // Compute recent success rate
  unsigned success_sum = 0;
  for (unsigned i = 0; i < state->success_count; i++) {
    success_sum += state->success_window[i];
  }
  state->recent_success_rate = (double) success_sum / state->success_count;
  
  // Update overall metrics
  state->total_propagations++;
  if (propagation_success) {
    state->successful_propagations++;
  }
}

// Detect search phase
static void detect_precise_phase (precise_state_t *state, uint64_t conflicts) {
  if (conflicts < 1000) {
    state->search_phase = 0;  // Initial: high exploration
  } else if (state->recent_success_rate > 0.7) {
    state->search_phase = 1;  // Stable: exploitation
  } else {
    state->search_phase = 2;  // Deep: focused exploitation
  }
}

// Adapt PRECISE parameters based on performance
static void adapt_precise_parameters (precise_state_t *state) {
  // Adapt exploration constant based on phase
  switch (state->search_phase) {
    case 0: // Initial
      state->ucb_exploration = 2.0;  // High exploration
      break;
    case 1: // Stable
      state->ucb_exploration = 1.0;  // Balanced
      break;
    case 2: // Deep
      state->ucb_exploration = 0.5;  // Low exploration
      break;
  }
  
  // Adapt priority bias based on recent performance
  if (state->total_propagations > 100) {
    double success_rate = (double) state->successful_propagations /
                          state->total_propagations;
    
    if (success_rate > 0.6) {
      // Good performance: increase bias toward redundant clauses
      state->priority_bias = fmin (1.0, state->priority_bias * 1.1);
    } else if (success_rate < 0.3) {
      // Poor performance: decrease bias toward redundant clauses
      state->priority_bias = fmax (0.2, state->priority_bias * 0.9);
    }
  }
  
  // Reset periodic counters
  state->total_propagations = 0;
  state->successful_propagations = 0;
}

// Update clause propagation score after conflict (CAPS legacy compatibility)
static void update_clause_prop_score (kissat *solver, clause *c,
                                      bool caused_conflict) {
  if (!GET_OPTION (caps))
    return;
  
  precise_state_t *state = &solver->precise_state;
  unsigned category = classify_clause_category (c);
  
  // Update reward based on whether clause was involved in conflict
  precise_update_reward (state, category, caused_conflict, c->size);
  
  // Detect phase
  detect_precise_phase (state, CONFLICTS);
  
  // Adapt parameters periodically
  if (state->total_propagations > 0 && state->total_propagations % 100 == 0) {
    adapt_precise_parameters (state);
  }
}

// Data structures for propagation priority queue (legacy CAPS compatibility)
typedef struct prop_priority {
  double priority;
  clause *clause;
  unsigned blocking_literal;
} prop_priority;

// Compute propagation priority for a clause using PRECISE
static double
compute_clause_prop_priority (kissat *solver, clause *c,
                              const assigned *assigned) {
  if (!GET_OPTION (caps))
    return 0.0;
  
  precise_state_t *state = &solver->precise_state;
  
  // Use PRECISE adaptive priority computation
  return compute_precise_priority (solver, c, assigned, state);
}

// Sort prop_priority by priority (descending)
static int compare_prop_priority (const void *a, const void *b) {
  const prop_priority *pa = (const prop_priority *) a;
  const prop_priority *pb = (const prop_priority *) b;
  if (pa->priority > pb->priority)
    return -1;  // a comes first (higher priority)
  if (pa->priority < pb->priority)
    return 1;   // b comes first
  return 0;
}

// Decay all clause propagation scores (called periodically)
static void decay_clause_prop_scores (kissat *solver) {
  if (!GET_OPTION (caps))
    return;
  (void) solver;
  // Score decay is handled incrementally through EMA updates
  // This function can be called to apply periodic decay
}

static inline value
move_smallest_literal_to_front (kissat *solver, const value *const values,
                                const assigned *const assigned,
                                bool satisfied_is_enough, unsigned start,
                                unsigned size, unsigned *lits) {
  assert (1 < size);
  assert (start < size);

  unsigned a = lits[start];

  value u = values[a];
  if (!u || (u > 0 && satisfied_is_enough))
    return u;

  unsigned pos = 0, best = a;

  const unsigned i = IDX (a);
  unsigned k = (u ? assigned[i].level : UINT_MAX);

  assert (start < UINT_MAX);
  for (unsigned i = start + 1; i < size; i++) {
    const unsigned b = lits[i];
    const value v = values[b];

    if (!v || (v > 0 && satisfied_is_enough)) {
      best = b;
      pos = i;
      u = v;
      break;
    }

    const unsigned j = IDX (b);
    const unsigned l = (v ? assigned[j].level : UINT_MAX);

    bool better;

    if (u < 0 && v > 0)
      better = true;
    else if (u > 0 && v < 0)
      better = false;
    else if (u < 0) {
      assert (v < 0);
      better = (k < l);
    } else {
      assert (u > 0);
      assert (v > 0);
      assert (!satisfied_is_enough);
      better = (k > l);
    }

    if (!better)
      continue;

    best = b;
    pos = i;
    u = v;
    k = l;
  }

  if (!pos)
    return u;

  lits[start] = best;
  lits[pos] = a;

  LOG ("new smallest literal %s at %u swapped with %s at %u", LOGLIT (best),
       pos, LOGLIT (a), start);
#ifndef LOGGING
  (void) solver;
#endif
  return u;
}

#ifdef INLINE_SORT
static inline
#endif
    void
    kissat_sort_literals (kissat *solver,
#ifdef INLINE_SORT
                          const value *const values,
                          const assigned *assigned,
#endif
                          unsigned size, unsigned *lits) {
#ifndef INLINE_SORT
  const value *const values = solver->values;
  const assigned *const assigned = solver->assigned;
#endif
  value u = move_smallest_literal_to_front (solver, values, assigned, false,
                                             0, size, lits);
  if (size > 2)
    move_smallest_literal_to_front (solver, values, assigned, (u >= 0), 1,
                                     size, lits);
}
// EVOLVE_END