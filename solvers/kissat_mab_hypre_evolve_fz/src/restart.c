#include "restart.h"
#include "backtrack.h"
#include "bump.h"
#include "decide.h"
#include "internal.h"
#include "kimits.h"
#include "logging.h"
#include "print.h"
#include "reluctant.h"
#include "report.h"

#include <inttypes.h>
#include <math.h>

// Forward declaration for CPDAR
bool kissat_qsar_should_restart (kissat *solver);

bool kissat_restarting (kissat *solver) {
  assert (solver->unassigned);
  if (!GET_OPTION (restart))
    return false;
  if (!solver->level)
    return false;
  if (CONFLICTS < solver->limits.restart.conflicts)
    return false;
  
  if (solver->stable) {
    // Use CPDAR in stable mode if enabled
    if (GET_OPTION (qsar)) {
      return kissat_qsar_should_restart (solver);
    }
    // Fall back to reluctant triggering
    return kissat_reluctant_triggered (&solver->reluctant);
  }
  
  const double fast = AVERAGE (fast_glue);
  const double slow = AVERAGE (slow_glue);
  const double margin = (100.0 + GET_OPTION (restartmargin)) / 100.0;
  const double limit = margin * slow;
  kissat_extremely_verbose (solver,
                            "restart glue limit %g = "
                            "%.02f * %g (slow glue) %c %g (fast glue)",
                            limit, margin, slow,
                            (limit > fast    ? '>'
                             : limit == fast ? '='
                                             : '<'),
                            fast);
  return (limit <= fast);
}

void kissat_update_focused_restart_limit (kissat *solver) {
  assert (!solver->stable);
  limits *limits = &solver->limits;
  uint64_t restarts = solver->statistics.restarts;
  uint64_t delta = GET_OPTION (restartint);
  if (restarts)
    delta += kissat_logn (restarts) - 1;
  limits->restart.conflicts = CONFLICTS + delta;
  kissat_extremely_verbose (solver,
                            "focused restart limit at %" PRIu64
                            " after %" PRIu64 " conflicts ",
                            limits->restart.conflicts, delta);
}

static unsigned reuse_stable_trail (kissat *solver) {
  const heap *const scores = kissat_get_scores (solver);
  const unsigned next_idx = kissat_next_decision_variable (solver);
  const double limit = kissat_get_heap_score (scores, next_idx);
  unsigned level = solver->level, res = 0;
  while (res < level) {
    frame *f = &FRAME (res + 1);
    const unsigned idx = IDX (f->decision);
    const double score = kissat_get_heap_score (scores, idx);
    if (score <= limit)
      break;
    res++;
  }
  return res;
}

static unsigned reuse_focused_trail (kissat *solver) {
  const links *const links = solver->links;
  const unsigned next_idx = kissat_next_decision_variable (solver);
  const unsigned limit = links[next_idx].stamp;
  LOG ("next decision variable stamp %u", limit);
  unsigned level = solver->level, res = 0;
  while (res < level) {
    frame *f = &FRAME (res + 1);
    const unsigned idx = IDX (f->decision);
    const unsigned score = links[idx].stamp;
    if (score <= limit)
      break;
    res++;
  }
  return res;
}

static unsigned reuse_trail (kissat *solver) {
  assert (solver->level);
  assert (!EMPTY_STACK (solver->trail));

  if (!GET_OPTION (restartreusetrail))
    return 0;

  unsigned res;

  if (solver->stable)
    res = reuse_stable_trail (solver);
  else
    res = reuse_focused_trail (solver);

  LOG ("matching trail level %u", res);

  if (res) {
    INC (restarts_reused_trails);
    ADD (restarts_reused_levels, res);
    LOG ("restart reuses trail at decision level %u", res);
  } else
    LOG ("restarts does not reuse the trail");

  return res;
}
// EVOLVE_START
// CPDAR: Conflict-Pattern-Driven Adaptive Restart
// Enhanced version using momentum-based tracking and phase detection

// Forward declarations for CPDAR static functions
static cpdar_phase_t detect_cpdar_phase (cpdar_state_t *state);
static double compute_cpdar_threshold (cpdar_state_t *state);
static double compute_cpdar_stability (cpdar_state_t *state);

// Initialize CPDAR state in solver
void kissat_init_qsar (kissat *solver) {
  if (!GET_OPTION (qsar))
    return;
  
  cpdar_state_t *state = &solver->cpdar_state;
  
  // Reset pattern tracking
  state->pattern.glue_sum = 0;
  state->pattern.glue_samples = 0;
  state->pattern.avg_glue = 0;
  state->pattern.glue_variance = 0;
  state->pattern.glue_trend = 0;
  state->pattern.size_sum = 0;
  state->pattern.avg_size = 0;
  state->pattern.last_update_conflicts = 0;
  
  // Reset phase detection
  state->phase = CPDAR_PHASE_INITIAL;
  state->phase_start_conflicts = 0;
  state->phase_changes = 0;
  
  // Reset performance tracking
  for (unsigned i = 0; i < 16; i++) {
    state->performance_history[i] = 0.5;
  }
  state->perf_index = 0;
  state->perf_count = 0;
  
  // Reset adaptive parameters
  state->restart_heuristic = 0;
  state->stay_count = 0;
  state->stay_limit = 10;
  
  // Reset quality thresholds
  state->good_glue_threshold = 5.0;
  state->bad_glue_threshold = 15.0;
  
  (void) solver;
}

// Detect current search phase based on glue patterns
static cpdar_phase_t detect_cpdar_phase (cpdar_state_t *state) {
  conflict_pattern_t *pattern = &state->pattern;
  
  // Need minimum samples for phase detection
  if (pattern->glue_samples < 10)
    return CPDAR_PHASE_INITIAL;
  
  double avg_glue = pattern->avg_glue;
  double variance = pattern->glue_variance;
  double trend = pattern->glue_trend;
  
  // Exhaustion: consistently low glue with negative trend
  if (avg_glue < 5.0 && trend < -0.1 && variance < 10.0) {
    return CPDAR_PHASE_EXHAUSTION;
  }
  
  // Intensification: low glue with stable trend
  if (avg_glue < 8.0 && fabs (trend) < 0.05) {
    return CPDAR_PHASE_INTENSIFICATION;
  }
  
  // Deep: elevated glue but trending down
  if (avg_glue > 10.0 && trend < -0.2) {
    return CPDAR_PHASE_DEEP;
  }
  
  // Exploration: high variance or increasing glue
  if (variance > 50.0 || trend > 0.2) {
    return CPDAR_PHASE_EXPLORATION;
  }
  
  return CPDAR_PHASE_INITIAL;
}

// Compute momentum-aware restart threshold
static double compute_cpdar_threshold (cpdar_state_t *state) {
  double base_threshold = 0.25;
  
  // Adjust based on phase
  switch (state->phase) {
    case CPDAR_PHASE_INITIAL:
      base_threshold = 0.15;  // Aggressive restarts early
      break;
    case CPDAR_PHASE_EXPLORATION:
      base_threshold = 0.20;  // Moderate restarts
      break;
    case CPDAR_PHASE_DEEP:
      base_threshold = 0.30;  // Conservative restarts
      break;
    case CPDAR_PHASE_INTENSIFICATION:
      base_threshold = 0.35;  // Very conservative
      break;
    case CPDAR_PHASE_EXHAUSTION:
      base_threshold = 0.40;  // Strategic restarts only
      break;
  }
  
  // Clamp to reasonable bounds
  return fmax (0.10, fmin (0.50, base_threshold));
}

// Update glue pattern with trend and variance tracking
void kissat_update_qsar_metrics (kissat *solver) {
  if (!GET_OPTION (qsar))
    return;
  if (!solver->stable)
    return;
  
  cpdar_state_t *state = &solver->cpdar_state;
  conflict_pattern_t *pattern = &state->pattern;
  
  // Update trail statistics
  unsigned current_trail_size = SIZE_STACK (solver->trail);
  if (pattern->glue_samples > 0) {
    double delta = (double) current_trail_size - pattern->avg_size;
    pattern->avg_size += delta / pattern->glue_samples;
  } else {
    pattern->avg_size = (double) current_trail_size;
  }
  pattern->size_sum += current_trail_size;
}

// Update after decision
void kissat_qsar_update_after_decision (kissat *solver) {
  if (!GET_OPTION (qsar))
    return;
  if (!solver->stable)
    return;
  
  cpdar_state_t *state = &solver->cpdar_state;
  state->pattern.glue_samples++;
  kissat_update_qsar_metrics (solver);
}

// Update after conflict
void kissat_qsar_update_after_conflict (kissat *solver) {
  if (!GET_OPTION (qsar))
    return;
  if (!solver->stable)
    return;
  
  cpdar_state_t *state = &solver->cpdar_state;
  conflict_pattern_t *pattern = &state->pattern;
  
  // Update performance history with conflict rate
  double conflict_rate = (pattern->glue_samples > 0) ?
    1.0 / (1.0 + (double) pattern->glue_samples) : 0.5;
  
  state->performance_history[state->perf_index] = conflict_rate;
  state->perf_index = (state->perf_index + 1) % 16;
  if (state->perf_count < 16) state->perf_count++;
  
  kissat_update_qsar_metrics (solver);
}

// Update after learning clause - track glue pattern
void kissat_qsar_update_after_learn (kissat *solver, unsigned glue) {
  if (!GET_OPTION (qsar))
    return;
  if (!solver->stable)
    return;
  
  cpdar_state_t *state = &solver->cpdar_state;
  conflict_pattern_t *pattern = &state->pattern;
  
  // Update running statistics for glue
  double old_mean = pattern->avg_glue;
  pattern->glue_sum += glue;
  pattern->glue_samples++;
  pattern->avg_glue = (double) pattern->glue_sum / pattern->glue_samples;
  
  // Update trend (exponential moving average of deviation)
  double deviation = (double) glue - old_mean;
  pattern->glue_trend = 0.7 * pattern->glue_trend + 0.3 * deviation;
  
  // Update variance (Welford's algorithm)
  if (pattern->glue_samples > 1) {
    double delta = (double) glue - pattern->avg_glue;
    pattern->glue_variance = ((pattern->glue_samples - 1) * pattern->glue_variance + 
                               delta * ((double) glue - pattern->avg_glue)) / 
                             pattern->glue_samples;
  }
  
  // Detect phase after sufficient samples
  if (pattern->glue_samples >= 10) {
    cpdar_phase_t new_phase = detect_cpdar_phase (state);
    if (new_phase != state->phase) {
      state->phase = new_phase;
      state->phase_start_conflicts = CONFLICTS;
      state->phase_changes++;
    }
  }
  
  (void) solver;
}

// Compute stability score for CPDAR restart decision
static double compute_cpdar_stability (cpdar_state_t *state) {
  conflict_pattern_t *pattern = &state->pattern;
  
  if (pattern->glue_samples < 10)
    return 1.0;  // No decision yet
  
  double score = 1.0;
  
  // Glue stability: lower average glue = higher stability
  score *= exp (-pattern->avg_glue * 0.05);
  
  // Trend stability: negative trend (improving) = higher stability
  score *= exp (pattern->glue_trend * 0.1);
  
  // Variance stability: low variance = higher stability
  score *= exp (-sqrt (pattern->glue_variance) * 0.02);
  
  // Performance stability
  if (state->perf_count > 0) {
    double avg_perf = 0;
    for (unsigned i = 0; i < state->perf_count; i++) {
      avg_perf += state->performance_history[i];
    }
    avg_perf /= state->perf_count;
    score *= (0.5 + 0.5 * avg_perf);
  }
  
  return fmax (0.0, fmin (1.0, score));
}

// Determine if CPDAR should trigger a restart
bool kissat_qsar_should_restart (kissat *solver) {
  if (!GET_OPTION (qsar))
    return false;
  if (!solver->stable)
    return false;
  
  cpdar_state_t *state = &solver->cpdar_state;
  
  // Minimum samples before considering restart
  if (state->pattern.glue_samples < 80)
    return false;
  
  // Compute stability score
  double stability = compute_cpdar_stability (state);
  
  // Get phase-aware threshold
  double threshold = compute_cpdar_threshold (state);
  
  // Decision: restart if stability below threshold
  return stability < threshold;
}

// CPDAR post-restart handler - learn from restart outcomes
void restart_qsar (kissat *solver) {
  cpdar_state_t *state = &solver->cpdar_state;
  
  // Update stay limit based on phase
  switch (state->phase) {
    case CPDAR_PHASE_INITIAL:
      state->stay_limit = 5;
      break;
    case CPDAR_PHASE_EXPLORATION:
      state->stay_limit = 8;
      break;
    case CPDAR_PHASE_DEEP:
      state->stay_limit = 15;
      break;
    case CPDAR_PHASE_INTENSIFICATION:
      state->stay_limit = 20;
      break;
    case CPDAR_PHASE_EXHAUSTION:
      state->stay_limit = 25;
      break;
  }
  
  // Update stay count
  if (state->stay_count < state->stay_limit) {
    state->stay_count++;
  } else {
    state->stay_count = 0;
    // Reset for next search phase - keep phase detection
  }
  
  // Reset per-restart counters
  state->pattern.glue_sum = 0;
  state->pattern.glue_samples = 0;
  state->pattern.avg_glue = 0;
  state->pattern.glue_variance = 0;
  state->pattern.glue_trend = 0;
  state->pattern.size_sum = 0;
  state->pattern.avg_size = 0;
  
  // Reset performance tracking
  for (unsigned i = 0; i < 16; i++) {
    state->performance_history[i] = 0.5;
  }
  state->perf_index = 0;
  state->perf_count = 0;
  
  // Adjust thresholds based on phase changes
  if (state->phase_changes > 0) {
    if (state->phase == CPDAR_PHASE_EXHAUSTION) {
      // Near solution - be more conservative
      state->good_glue_threshold = fmax (3.0, state->good_glue_threshold - 0.5);
    } else if (state->phase == CPDAR_PHASE_EXPLORATION) {
      // Wide search - be more aggressive
      state->good_glue_threshold = fmin (8.0, state->good_glue_threshold + 0.5);
    }
  }
  
  (void) solver;
}

void restart_mab (kissat *solver) {
  // Reset MAB tracking variables
  unsigned stable_restarts = 0;
  if (0 == solver->strategy)
    solver->mab_reward[solver->heuristic] +=
        log2 (solver->mab_decisions) / log2 (solver->mab_conflicts);
  else
    solver->mab_reward[2 + solver->heuristic] +=
        log2 (solver->mab_conflicts) / log2 (solver->mab_decisions);

  // Clear per-variable MAB data
  for (all_variables (idx)) {
    solver->mab_chosen[idx] = 0;
  }
  solver->mab_chosen_tot = 0;
  solver->mab_decisions = 0;
  solver->mab_conflicts = 0;

  // Count stable restarts across all heuristics
  for (unsigned i = 0; i < solver->mab_heuristics; i++) {
    stable_restarts += solver->mab_select[solver->strategy * 2 + i];
  }

  // Track recent gains with momentum
  static double recent_gains[20] = {0};
  static int gain_index[2] = {0};
  static double momentum = 1.0;
  bool switch_strategy = true;
  static int stay_count = 0;

  double current_gain =
      solver->mab_reward[solver->strategy * 2 + solver->heuristic] /
      solver->mab_select[solver->strategy * 2 + solver->heuristic];
  recent_gains[solver->strategy * 10 + gain_index[solver->strategy]] =
      current_gain;
  gain_index[solver->strategy] = (gain_index[solver->strategy] + 1) % 10;

  // Compute average gain over recent window
  double avg_gain = 0;
  for (int i = 0; i < 10; i++) {
    avg_gain += recent_gains[solver->strategy * 10 + i];
  }
  avg_gain /= 10;

  // Update momentum based on performance
  if (current_gain > avg_gain) {
    momentum *= 1.1;
  } else {
    momentum *= 0.9;
  }

  // Compute adaptive exploration parameter
  double adaptive_c = solver->mabc / (momentum * (stable_restarts + 1));

  // Select next heuristic
  if (stable_restarts < solver->mab_heuristics) {
    // Exploration phase: alternate between first two heuristics
    solver->next_heuristic[solver->strategy] =
        solver->heuristic == 0 ? 1 : 0;
  } else {
    // UCB-based selection
    double ucb[2];
    solver->next_heuristic[solver->strategy] = 0;
    for (unsigned i = 0; i < solver->mab_heuristics; i++) {
      ucb[i] = solver->mab_reward[solver->strategy * 2 + i] /
                   solver->mab_select[solver->strategy * 2 + i] +
               sqrt (adaptive_c * log (stable_restarts + 1) /
                     solver->mab_select[solver->strategy * 2 + i]);
      if (i != 0 && ucb[i] > ucb[solver->heuristic]) {
        solver->next_heuristic[solver->strategy] = i;
      }
    }
  }
  if (stay_count < 10) {
    switch_strategy = false;
    stay_count++;
  }
  if (switch_strategy) {
    solver->strategy = 1 - solver->strategy;
    stay_count = 0;
  }
  solver->heuristic = solver->next_heuristic[solver->strategy];

  // Update selection count for chosen heuristic
  solver->mab_select[solver->strategy * 2 + solver->heuristic]++;
}

void kissat_restart (kissat *solver) {
  START (restart);
  INC (restarts);
  ADD (restarts_levels, solver->level);
  if (solver->stable)
    INC (stable_restarts);
  else
    INC (focused_restarts);

  unsigned old_heuristic = solver->heuristic;
  if (solver->stable && solver->mab)
    restart_mab (solver);
  else if (solver->stable && GET_OPTION (qsar))
    restart_qsar (solver);
  unsigned new_heuristic = solver->heuristic;

  unsigned level =
      old_heuristic == new_heuristic ? reuse_trail (solver) : 0;

  kissat_extremely_verbose (solver,
                            "restarting after %" PRIu64 " conflicts"
                            " (limit %" PRIu64 ")",
                            CONFLICTS, solver->limits.restart.conflicts);
  LOG ("restarting to level %u", level);
  if (solver->stable && solver->mab)
    solver->heuristic = old_heuristic;
  kissat_backtrack_in_consistent_state (solver, level);
  if (solver->stable && solver->mab)
    solver->heuristic = new_heuristic;
  if (!solver->stable)
    kissat_update_focused_restart_limit (solver);

  if (solver->stable && solver->mab && old_heuristic != new_heuristic)
    kissat_update_scores (solver);

  REPORT (1, 'R');
  STOP (restart);
}
// EVOLVE_END
