#include "bump.h"
#include "analyze.h"
#include "inlineheap.h"
#include "inlinequeue.h"
#include "inlinevector.h"
#include "internal.h"
#include "logging.h"
#include "print.h"
#include "rank.h"
#include "sort.h"

#define RANK(A) ((A).rank)
#define SMALLER(A, B) (RANK (A) < RANK (B))

#define RADIX_SORT_BUMP_LIMIT 32

static void sort_bump (kissat *solver) {
  const size_t size = SIZE_STACK (solver->analyzed);
  if (size < RADIX_SORT_BUMP_LIMIT) {
    LOG ("quick sorting %zu analyzed variables", size);
    SORT_STACK (datarank, solver->ranks, SMALLER);
  } else {
    LOG ("radix sorting %zu analyzed variables", size);
    RADIX_STACK (datarank, unsigned, solver->ranks, RANK);
  }
}

void kissat_rescale_scores (kissat *solver) {
  INC (rescaled);
  heap *scores = &solver->scores;
  const double max_score = kissat_max_score_on_heap (scores);
  kissat_phase (solver, "rescale", GET (rescaled),
                "maximum score %g increment %g", max_score, solver->scinc);
  const double rescale = MAX (max_score, solver->scinc);
  assert (rescale > 0);
  const double factor = 1.0 / rescale;
  kissat_rescale_heap (solver, scores, factor);
  solver->scinc *= factor;
  kissat_phase (solver, "rescale", GET (rescaled), "rescaled by factor %g",
                factor);
}

void kissat_bump_score_increment (kissat *solver) {
  const double old_scinc = solver->scinc;
  const double decay = GET_OPTION (decay) * 1e-3;
  assert (0 <= decay), assert (decay <= 0.5);
  const double factor = 1.0 / (1.0 - decay);
  const double new_scinc = old_scinc * factor;
  LOG ("new score increment %g = %g * %g", new_scinc, factor, old_scinc);
  solver->scinc = new_scinc;
  if (new_scinc > MAX_SCORE)
    kissat_rescale_scores (solver);
}

static inline void bump_analyzed_variable_score (kissat *solver,
                                                 unsigned idx) {
  heap *scores = &solver->scores;
  const double old_score = kissat_get_heap_score (scores, idx);
  const double inc = solver->scinc;
  const double new_score = old_score + inc;
  LOG ("new score[%u] = %g = %g + %g", idx, new_score, old_score, inc);
  kissat_update_heap (solver, scores, idx, new_score);
  if (new_score > MAX_SCORE)
    kissat_rescale_scores (solver);
}

void kissat_bump_variable (kissat *solver, unsigned idx) {
  bump_analyzed_variable_score (solver, idx);
}

static void bump_analyzed_variable_scores (kissat *solver) {
  flags *flags = solver->flags;

  for (all_stack (unsigned, idx, solver->analyzed))
    if (flags[idx].active)
      bump_analyzed_variable_score (solver, idx);

  kissat_bump_score_increment (solver);
}

static void move_analyzed_variables_to_front_of_queue (kissat *solver) {
  assert (EMPTY_STACK (solver->ranks));
  const links *const links = solver->links;
  for (all_stack (unsigned, idx, solver->analyzed)) {
    // clang-format off
    const datarank rank = { .data = idx, .rank = links[idx].stamp };
    // clang-format on
    PUSH_STACK (solver->ranks, rank);
  }

  sort_bump (solver);

  flags *flags = solver->flags;
  unsigned idx;

  for (all_stack (datarank, rank, solver->ranks))
    if (flags[idx = rank.data].active)
      kissat_move_to_front (solver, idx);

  CLEAR_STACK (solver->ranks);
}

void kissat_bump_analyzed (kissat *solver) {
  START (bump);
  const size_t bumped = SIZE_STACK (solver->analyzed);
  if (!solver->stable)
    move_analyzed_variables_to_front_of_queue (solver);
  else
    bump_analyzed_variable_scores (solver);
  ADD (literals_bumped, bumped);
  STOP (bump);
}

void kissat_update_scores (kissat *solver) {
  assert (solver->stable);
  heap *scores = kissat_get_scores (solver);
  for (all_variables (idx))
    if (ACTIVE (idx) && !kissat_heap_contains (scores, idx))
      kissat_push_heap (solver, scores, idx);
}

// CHB

void kissat_bump_chb (kissat *solver, unsigned v, double multiplier) {
  int64_t age =
      solver->statistics.conflicts - solver->conflicted_chb[v] + 1;
  double reward_chb = multiplier / age;
  double old_score = kissat_get_heap_score (&solver->scores_chb, v);
  double new_score =
      solver->step_chb * reward_chb + (1 - solver->step_chb) * old_score;
  LOG ("new score[%u] = %g vs %g", v, new_score, old_score);
  kissat_update_heap (solver, &solver->scores_chb, v, new_score);
}

void kissat_decay_chb (kissat *solver) {
  if (solver->step_chb > solver->step_min_chb)
    solver->step_chb -= solver->step_dec_chb;
}

void kissat_update_conflicted_chb (kissat *solver) {
  flags *flags = solver->flags;

  for (all_stack (unsigned, idx, solver->analyzed))
    if (flags[idx].active)
      solver->conflicted_chb[idx] = solver->statistics.conflicts;
}

// AW-VSIDS: Activity-Weighted Variable Ordering with Locality
// Compute locality score for a variable based on temporal locality
static double kissat_compute_locality_score (kissat *solver, unsigned idx) {
  variable_activity_extended_t *va = &solver->variable_activities_ext[idx];
  
  // Time since last conflict involvement
  unsigned current_level = solver->level;
  unsigned level_diff = (va->last_conflict_level < current_level) ? 
                        (current_level - va->last_conflict_level) : 0;
  
  // Exponential locality decay (stronger for recent conflicts)
  double locality = exp (-level_diff * 0.15);
  
  // Recent conflict density (last 100 decisions)
  double recent_density = (double) va->recent_conflicts / 100.0;
  
  // Combined locality score [0, 1]
  double locality_score = locality * 0.7 + recent_density * 0.3;
  
  return locality_score;
}

// Compute LBD-weighted boost for a variable
static double kissat_compute_lbd_boost (kissat *solver, unsigned idx, 
                                        unsigned clause_lbd) {
  variable_activity_extended_t *va = &solver->variable_activities_ext[idx];
  
  // Update LBD statistics with exponential moving average
  double alpha = 0.1;
  if (va->lbd_count > 0) {
    double avg_lbd = (double) va->lbd_sum / va->lbd_count;
    // Lower average LBD = higher boost
    double quality_factor = 1.0 / (1.0 + log1p (avg_lbd));
    va->lbd_boost = alpha * quality_factor + (1.0 - alpha) * va->lbd_boost;
  }
  
  // Additional boost for current clause if low LBD (high quality)
  double current_boost = (clause_lbd <= 5) ? 0.2 : 0.0;
  
  return va->lbd_boost + current_boost;
}

// Compute the combined AW-VSIDS score for a variable
double kissat_compute_aw_vsids_score (kissat *solver, unsigned idx, 
                                      unsigned clause_lbd) {
  variable_activity_extended_t *va = &solver->variable_activities_ext[idx];
  
  // Base VSIDS score (from standard heap)
  heap *scores = kissat_get_scores (solver);
  double base_score = kissat_get_heap_score (scores, idx);
  
  // Compute locality component
  double locality = kissat_compute_locality_score (solver, idx);
  
  // Compute LBD-weighted boost
  double lbd_boost = kissat_compute_lbd_boost (solver, idx, clause_lbd);
  
  // Activity-based decay adjustment (adaptive decay)
  double total_conflicts = fmax (1.0, (double) solver->statistics.conflicts);
  double activity_ratio = va->conflict_count / total_conflicts;
  double adaptive_decay = solver->vsids_decay * (1.0 - 0.1 * activity_ratio);
  (void) adaptive_decay;  // Used for potential future decay adjustment
  
  // Combined score formula
  double combined = base_score * (1.0 + locality * 0.3 + lbd_boost * 0.2);
  
  return combined;
}

// Update AW-VSIDS variable activity on conflict
void kissat_update_aw_vsids (kissat *solver, unsigned *lits, unsigned size, 
                             unsigned glue) {
  if (!solver->variable_activities_ext)
    return;
  
  // Update all literals in the learned clause
  for (unsigned i = 0; i < size; i++) {
    unsigned idx = IDX (lits[i]);
    variable_activity_extended_t *va = &solver->variable_activities_ext[idx];
    
    // Update locality tracking
    va->last_conflict_level = solver->level;
    va->conflict_count++;
    if (va->recent_conflicts < 100)
      va->recent_conflicts++;
    
    // Update LBD statistics
    va->lbd_sum += glue;
    va->lbd_count++;
  }
  
  // Periodic decay of recent conflict counters
  if (solver->statistics.conflicts % 1000 == 0) {
    for (all_variables (idx)) {
      solver->variable_activities_ext[idx].recent_conflicts /= 2;
    }
  }
}

// Initialize AW-VSIDS data structures
void kissat_init_aw_vsids (kissat *solver) {
  if (solver->vars == 0)
    return;
  
  // Allocate extended activity tracking
  solver->variable_activities_ext = 
    (variable_activity_extended_t *) calloc (solver->vars, 
                                             sizeof (variable_activity_extended_t));
  
  // Initialize VSIDS decay from options
  solver->vsids_decay = GET_OPTION (decay) * 1e-3;
  solver->current_learned_glue = 0;
  solver->in_conflict = false;
}

// Get the best variable using AW-VSIDS scoring
// Returns the index of the best variable, or INVALID_IDX if none available
unsigned kissat_select_variable_aw_vsids (kissat *solver) {
  if (!solver->variable_activities_ext)
    return INVALID_IDX;
  
  heap *scores = kissat_get_scores (solver);
  unsigned best = INVALID_IDX;
  double best_score = -1.0;
  
  // Iterate through heap to find best AW-VSIDS score
  if (!kissat_empty_heap (scores)) {
    // Make a copy of heap elements since we need to check all
    unsigned vars_count = solver->vars;
    double *temp_scores = (double *) calloc (vars_count, sizeof (double));
    
    // Get current scores from heap
    for (unsigned idx = 0; idx < vars_count; idx++) {
      if (kissat_heap_contains (scores, idx)) {
        temp_scores[idx] = kissat_compute_aw_vsids_score (solver, idx, 
                                                          solver->current_learned_glue);
      }
    }
    
    // Find max
    for (unsigned idx = 0; idx < vars_count; idx++) {
      if (kissat_heap_contains (scores, idx)) {
        if (temp_scores[idx] > best_score) {
          best_score = temp_scores[idx];
          best = idx;
        }
      }
    }
    
    free (temp_scores);
  }
  
  return best;
}

// Cleanup AW-VSIDS data structures
void kissat_destroy_aw_vsids (kissat *solver) {
  if (solver->variable_activities_ext) {
    free (solver->variable_activities_ext);
    solver->variable_activities_ext = 0;
  }
}
