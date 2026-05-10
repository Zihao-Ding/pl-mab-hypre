#include "tiers.h"
#include "internal.h"
#include "logging.h"
#include "print.h"
#include <math.h>
#include <string.h>

#define CLAMP(VAL, LO, HI) ((VAL) < (LO) ? (LO) : ((VAL) > (HI) ? (HI) : (VAL)))

static void compute_tier_limits (kissat *solver, bool stable,
                                 unsigned *tier1_ptr, unsigned *tier2_ptr) {
  statistics *statistics = &solver->statistics;
  uint64_t *used_stats = statistics->used[stable].glue;
  uint64_t total_used = 0;
  for (unsigned glue = 0; glue <= MAX_GLUE_USED; glue++)
    total_used += used_stats[glue];
  int tier1 = -1, tier2 = -1;
  if (total_used) {
    uint64_t accumulated_tier1_limit = total_used * TIER1RELATIVE;
    uint64_t accumulated_tier2_limit = total_used * TIER2RELATIVE;
    uint64_t accumulated_used = 0;
    unsigned glue;
    for (glue = 0; glue <= MAX_GLUE_USED; glue++) {
      uint64_t glue_used = used_stats[glue];
      accumulated_used += glue_used;
      if (accumulated_used >= accumulated_tier1_limit) {
        tier1 = glue;
        break;
      }
    }
    if (accumulated_used < accumulated_tier2_limit) {
      for (glue = tier1 + 1; glue <= MAX_GLUE_USED; glue++) {
        uint64_t glue_used = used_stats[glue];
        accumulated_used += glue_used;
        if (accumulated_used >= accumulated_tier2_limit) {
          tier2 = glue;
          break;
        }
      }
    }
  }
  if (tier1 < 0) {
    tier1 = GET_OPTION (tier1);
    tier2 = MAX (GET_OPTION (tier2), tier1);
  } else if (tier2 < 0)
    tier2 = tier1;
  assert (0 <= tier1);
  assert (0 <= tier2);
  *tier1_ptr = tier1;
  *tier2_ptr = tier2;
  LOG ("%s tier1 limit %u", stable ? "stable" : "focused", tier1);
  LOG ("%s tier2 limit %u", stable ? "stable" : "focused", tier2);
}
// EVOLVE_START
// VATA-EPA: Variance-Aware Tier Adaptation with Enhanced Problem-Aware Adaptation

#define MAX_GLUE_USED 255

// VATA-EPA: Enhanced distribution parameters structure
typedef struct glue_distribution_params {
  double cv;              // Coefficient of variation (std/mean)
  double normalized_skew; // Normalized skewness
  double kurtosis_excess; // Excess kurtosis
  double entropy;         // Information-theoretic entropy
  double concentration;   // Herfindahl concentration index
} glue_dist_params_t;

// VATA-Plus: Enhanced distribution shape detection with 6 categories
static distribution_shape_t detect_distribution_shape_v2 (double variance, double mean, double skewness) {
  if (mean <= 0) return DIST_NORMAL;
  
  double cv = sqrt (fmax (0, variance)) / mean;
  
  // Check for heavy-tailed distribution (negative skewness indicates left tail)
  if (skewness < -0.5) {
    return DIST_HEAVY_TAILED;
  }
  
  // Enhanced classification with 5 CV-based categories
  if (cv < 0.2) {
    return DIST_VERY_CONCENTRATED;
  } else if (cv < 0.4) {
    return DIST_CONCENTRATED;
  } else if (cv < 0.8) {
    return DIST_NORMAL;
  } else if (cv < 1.2) {
    return DIST_SPARSE;
  } else {
    return DIST_VERY_SPARSE;
  }
}

// VATA-EPA: Compute enhanced distribution parameters
static void compute_glue_distribution_enhanced (kissat *solver, bool stable,
                                                uint64_t *used_stats,
                                                glue_dist_params_t *params) {
  uint64_t total_used = 0;
  double sum = 0, sum_sq = 0, sum_cu = 0, sum_qu = 0;
  double entropy = 0;
  
  (void) solver;
  (void) stable;
  
  for (unsigned glue = 0; glue <= MAX_GLUE_USED; glue++) {
    uint64_t count = used_stats[glue];
    total_used += count;
    double glue_d = (double)glue;
    sum += glue_d * (double)count;
    sum_sq += glue_d * glue_d * (double)count;
    sum_cu += glue_d * glue_d * glue_d * (double)count;
    sum_qu += glue_d * glue_d * glue_d * glue_d * (double)count;
    
    // Compute entropy
    if (count > 0) {
      double p = (double)count / total_used;
      entropy -= p * log (p + 1e-10);
    }
  }
  
  if (total_used == 0) {
    params->cv = 0.5;
    params->normalized_skew = 0;
    params->kurtosis_excess = 0;
    params->entropy = 0;
    params->concentration = 1.0;
    return;
  }
  
  double mean = sum / (double)total_used;
  double mean_sq = sum_sq / (double)total_used;
  double variance = mean_sq - mean * mean;
  double std_dev = sqrt (fmax (0, variance));
  
  params->cv = (mean > 0) ? std_dev / mean : 0.5;
  params->entropy = entropy;
  
  // Concentration index (Herfindahl)
  double concentration = 0;
  for (unsigned glue = 0; glue <= MAX_GLUE_USED; glue++) {
    uint64_t count = used_stats[glue];
    if (count > 0) {
      double p = (double)count / total_used;
      concentration += p * p;
    }
  }
  params->concentration = concentration;
  
  // Skewness and kurtosis
  if (std_dev > 0) {
    double mean_cu = sum_cu / (double)total_used;
    double mean_qu = sum_qu / (double)total_used;
    params->normalized_skew = (mean_cu - 3.0 * mean * mean_sq + 2.0 * mean * mean * mean) /
                              (std_dev * std_dev * std_dev);
    params->kurtosis_excess = (mean_qu / (std_dev * std_dev * std_dev * std_dev)) - 3.0;
  } else {
    params->normalized_skew = 0;
    params->kurtosis_excess = 0;
  }
}

// VATA-EPA: Compute phase-adaptive EMA alpha
static double compute_phase_adaptive_alpha (vata_state_t *state,
                                            unsigned conflicts) {
  // Initial exploration phase: high alpha for fast adaptation
  if (state->search_phase == 0) {
    return 0.2;
  }
  
  // Exploitation phase: lower alpha for stability
  if (state->search_phase == 2) {
    return 0.05;
  }
  
  // Transition phase: medium alpha
  return 0.1;
}

// VATA-EPA: Detect search phase based on conflict count
static unsigned detect_search_phase (unsigned conflicts) {
  if (conflicts < 1000) {
    return 0;  // Initial/Exploration phase
  } else if (conflicts < 10000) {
    return 1;  // Transition phase
  } else {
    return 2;  // Exploitation phase
  }
}

// VATA-EPA: Estimate problem type based on glue distribution
// Returns: 0=industrial, 1=cryptographic, 2=combinatorial
static unsigned estimate_problem_type (glue_dist_params_t *params) {
  // Combinatorial: typically more concentrated (low CV, low entropy)
  if (params->cv < 0.4 && params->entropy < 2.0) {
    return 2;
  }
  // Cryptographic: typically more scattered (higher CV, higher entropy)
  if (params->cv > 0.8 && params->entropy > 3.0) {
    return 1;
  }
  // Default: industrial
  return 0;
}

static void init_vata_state (kissat *solver, bool stable) {
  vata_state_t *vata = &solver->vata_state[stable];
  
  // VATA-Plus: Updated initial parameters
  vata->glue_mean = 8.0;
  vata->glue_variance = 30.0;
  vata->glue_skewness = 0.0;
  
  // VATA-Plus: EMA-based statistics initialization
  vata->glue_ema = 8.0;
  vata->glue_ema2 = 30.0;
  vata->glue_ema3 = 0.0;
  vata->recent_mean = 8.0;
  vata->long_term_mean = 8.0;
  
  // Initialize history with default values
  unsigned default_tier1 = GET_OPTION (tier1);
  unsigned default_tier2 = GET_OPTION (tier2);
  for (int i = 0; i < 8; i++) {
    vata->tier1_history[i] = default_tier1;
    vata->tier2_history[i] = default_tier2;
  }
  vata->history_index = 0;
  vata->history_count = 0;
  
  vata->distribution_shape = DIST_NORMAL;
  
  // VATA-Plus: Phase-aware tracking
  vata->phase_switch_recent = false;
  vata->phase_switch_count = 0;
  
  // VATA-Plus: Updated initial relative thresholds (changed: 0.05, 0.18)
  vata->relative_tier1 = 0.05;  // Changed from 0.04
  vata->relative_tier2 = 0.18;  // Changed from 0.15
  vata->tier1_min = 3;
  vata->tier2_min = 6;
  
  // VATA-Plus: Kurtosis initialization
  vata->glue_kurtosis = 0.0;
  
  // VATA-EPA: Initialize enhanced state
  vata->search_phase = 0;
  vata->phase_adaptive_alpha = 0.15;  // Changed from 0.1
  vata->problem_complexity = 0.5;
  vata->convergence_count = 0;
  
  // VATA-EPA: Initialize enhanced distribution parameters
  vata->glue_cv = 0.5;
  vata->glue_entropy = 0.0;
  vata->glue_concentration = 1.0;
}

static void compute_glue_statistics_v2 (kissat *solver, bool stable,
                                       uint64_t *used_stats,
                                       double *mean, double *variance,
                                       double *skewness, double *kurtosis) {
  uint64_t total_used = 0;
  double sum = 0, sum_sq = 0, sum_cu = 0, sum_qu = 0;
  
  for (unsigned glue = 0; glue <= MAX_GLUE_USED; glue++) {
    uint64_t count = used_stats[glue];
    total_used += count;
    double glue_d = (double)glue;
    sum += glue_d * (double)count;
    sum_sq += glue_d * glue_d * (double)count;
    sum_cu += glue_d * glue_d * glue_d * (double)count;
    sum_qu += glue_d * glue_d * glue_d * glue_d * (double)count;
  }
  
  if (total_used == 0) {
    *mean = 8.0;
    *variance = 30.0;
    *skewness = 0.0;
    *kurtosis = 0.0;
    return;
  }
  
  *mean = sum / (double)total_used;
  
  double mean_sq = sum_sq / (double)total_used;
  *variance = mean_sq - (*mean) * (*mean);
  
  double mean_cu = sum_cu / (double)total_used;
  double mean_qu = sum_qu / (double)total_used;
  double std_dev = sqrt (fmax (0, *variance));
  
  if (std_dev > 0) {
    *skewness = (mean_cu - 3.0 * (*mean) * mean_sq + 2.0 * (*mean) * (*mean) * (*mean)) /
                (std_dev * std_dev * std_dev);
    // Excess kurtosis: kurtosis - 3
    *kurtosis = (mean_qu / (std_dev * std_dev * std_dev * std_dev)) - 3.0;
  } else {
    *skewness = 0.0;
    *kurtosis = 0.0;
  }
}

// VATA-EPA: Get enhanced thresholds with problem-aware adaptation
static void get_enhanced_thresholds (vata_state_t *vata, bool stable,
                                     double *rel_tier1, double *rel_tier2,
                                     glue_dist_params_t *params,
                                     unsigned problem_type) {
  // Base thresholds from VATA-Plus
  double base_tier1 = vata->relative_tier1;
  double base_tier2 = vata->relative_tier2;
  
  // Problem-type specific adjustments
  // 0=industrial, 1=cryptographic, 2=combinatorial
  double problem_multipliers[3][2] = {
    {1.0, 1.0},    // Industrial: default
    {1.3, 1.2},    // Cryptographic: higher tiers (tougher clauses)
    {0.7, 0.8}     // Combinatorial: lower tiers (more aggressive)
  };
  
  base_tier1 *= problem_multipliers[problem_type][0];
  base_tier2 *= problem_multipliers[problem_type][1];
  
  // CV-based continuous adjustment (smoothed)
  double cv = params->cv;
  double cv_adjustment = 1.0;
  
  if (cv < 0.2) {
    // Very concentrated: reduce tier1 significantly
    cv_adjustment = 0.5 + cv * 2.5;
  } else if (cv > 1.0) {
    // Very sparse: increase tier1
    cv_adjustment = 1.0 + (cv - 1.0) * 0.5;
  } else {
    // Normal range: smooth interpolation
    cv_adjustment = 0.7 + cv * 0.4;
  }
  
  // Entropy-based adjustment (higher entropy = more diverse = higher tiers)
  double entropy_factor = fmin (1.5, 0.5 + params->entropy * 0.3);
  cv_adjustment *= entropy_factor;
  
  *rel_tier1 = base_tier1 * cv_adjustment;
  *rel_tier2 = base_tier2 * cv_adjustment * (1.0 + 0.2 * (cv - 0.5));
  
  // VATA-EPA: Stricter bounds than VATA-Plus
  *rel_tier1 = fmax (0.02, fmin (0.15, *rel_tier1));  // Changed: 0.02-0.15
  *rel_tier2 = fmax (0.08, fmin (0.40, *rel_tier2));  // Changed: 0.08-0.40
  
  (void) stable;  // Keep for future extension
}

static void get_adaptive_thresholds_v2 (vata_state_t *vata, bool stable,
                                       double *rel_tier1, double *rel_tier2) {
  // VATA-Plus: Phase-aware threshold adjustment
  double phase_context = stable ? 1.0 : 0.8;
  
  // If recently switched phases, apply smoothing
  if (vata->phase_switch_recent) {
    phase_context = 0.9;
  }
  
  double base_tier1 = vata->relative_tier1;
  double base_tier2 = vata->relative_tier2;
  
  // VATA-Plus: Enhanced distribution shape detection with multipliers
  double distribution_multiplier = 1.0;
  switch (vata->distribution_shape) {
    case DIST_VERY_CONCENTRATED:
      base_tier1 *= 0.6;
      base_tier2 *= 0.7;
      distribution_multiplier = 0.6;
      break;
      
    case DIST_CONCENTRATED:
      base_tier1 *= 0.7;
      base_tier2 *= 0.8;
      distribution_multiplier = 0.75;
      break;
      
    case DIST_SPARSE:
      base_tier1 *= 1.4;
      base_tier2 *= 1.3;
      distribution_multiplier = 1.35;
      break;
      
    case DIST_VERY_SPARSE:
      base_tier1 *= 1.6;
      base_tier2 *= 1.5;
      distribution_multiplier = 1.55;
      break;
      
    case DIST_HEAVY_TAILED:
      // Heavy-tailed: be more conservative with tier1
      base_tier1 *= 1.2;
      base_tier2 *= 1.1;
      distribution_multiplier = 1.15;
      break;
      
    case DIST_NORMAL:
    default:
      distribution_multiplier = 1.0;
      break;
  }
  
  // Apply phase-aware adjustment
  *rel_tier1 = base_tier1 * phase_context;
  *rel_tier2 = base_tier2 * phase_context;
  
  // VATA-Plus: Relaxed bounds with problem-aware clamping
  // Use more relaxed bounds for problems with varied distributions
  double tier1_lower = 0.01;
  double tier1_upper = 0.20;  // Increased from 0.15
  double tier2_lower = 0.05;
  double tier2_upper = 0.50;  // Increased from 0.40
  
  *rel_tier1 = fmax (tier1_lower, fmin (tier1_upper, *rel_tier1));
  *rel_tier2 = fmax (tier2_lower, fmin (tier2_upper, *rel_tier2));
}

// VATA-Plus: Cross-mode state transfer
static void vata_transfer_state (kissat *solver, bool from_stable, bool to_stable) {
  vata_state_t *from = &solver->vata_state[from_stable];
  vata_state_t *to = &solver->vata_state[to_stable];
  
  // Transfer EMA state with blending
  to->glue_ema = 0.7 * to->glue_ema + 0.3 * from->glue_ema;
  to->glue_ema2 = 0.7 * to->glue_ema2 + 0.3 * from->glue_ema2;
  to->long_term_mean = 0.9 * to->long_term_mean + 0.1 * from->long_term_mean;
  
  // Reset recent state to avoid stale data
  to->phase_switch_recent = true;
  to->phase_switch_count++;
}

static void compute_tier_limits_vata (kissat *solver, bool stable,
                                      unsigned *tier1_ptr, unsigned *tier2_ptr) {
  vata_state_t *vata = &solver->vata_state[stable];
  statistics *statistics = &solver->statistics;
  uint64_t *used_stats = statistics->used[stable].glue;
  
  // VATA-Plus: Check for phase switch and transfer state
  static bool last_stable = false;
  if (solver->vata_state[0].history_count > 0 || solver->vata_state[1].history_count > 0) {
    if (last_stable != stable) {
      vata_transfer_state (solver, last_stable, stable);
      last_stable = stable;
    }
  } else {
    last_stable = stable;
  }
  
  // VATA-Plus: Compute distribution statistics with kurtosis
  double mean, variance, skewness, kurtosis;
  compute_glue_statistics_v2 (solver, stable, used_stats, &mean, &variance, &skewness, &kurtosis);
  
  // VATA-EPA: Compute enhanced distribution parameters
  glue_dist_params_t dist_params;
  compute_glue_distribution_enhanced (solver, stable, used_stats, &dist_params);
  
  // Update enhanced distribution parameters in state
  vata->glue_cv = dist_params.cv;
  vata->glue_entropy = dist_params.entropy;
  vata->glue_concentration = dist_params.concentration;
  
  // VATA-EPA: Update search phase based on conflicts
  uint64_t conflicts = statistics->conflicts;
  vata->search_phase = detect_search_phase ((unsigned) conflicts);
  
  // VATA-EPA: Compute phase-adaptive alpha
  vata->phase_adaptive_alpha = 0.15;  // Base value (changed from 0.1)
  if (vata->search_phase == 0) {
    vata->phase_adaptive_alpha = 0.2;  // Exploration: fast adaptation
  } else if (vata->search_phase == 2) {
    vata->phase_adaptive_alpha = 0.05;  // Exploitation: stable
  }
  
  // VATA-EPA: Estimate problem type
  unsigned problem_type = estimate_problem_type (&dist_params);
  
  // VATA-EPA: Estimate problem complexity
  vata->problem_complexity = fmin (1.0, dist_params.cv * 0.8 + (1.0 - dist_params.concentration) * 0.2);
  
  // VATA-Plus: Update EMA-based statistics with phase-adaptive alpha
  double alpha = vata->phase_adaptive_alpha;
  vata->glue_ema = alpha * mean + (1.0 - alpha) * vata->glue_ema;
  vata->glue_ema2 = alpha * variance + (1.0 - alpha) * vata->glue_ema2;
  
  // Update short-term and long-term means
  vata->recent_mean = 0.7 * vata->recent_mean + 0.3 * mean;
  vata->long_term_mean = 0.95 * vata->long_term_mean + 0.05 * mean;
  
  // Update state with exponential decay
  vata->glue_mean = 0.9 * vata->glue_mean + 0.1 * mean;
  vata->glue_variance = 0.9 * vata->glue_variance + 0.1 * variance;
  vata->glue_skewness = 0.9 * vata->glue_skewness + 0.1 * skewness;
  vata->glue_kurtosis = 0.9 * vata->glue_kurtosis + 0.1 * kurtosis;
  
  // VATA-Plus: Use enhanced distribution shape detection
  vata->distribution_shape = detect_distribution_shape_v2 (vata->glue_variance, 
                                                            vata->glue_mean,
                                                            vata->glue_skewness);
  
  // VATA-EPA: Use enhanced threshold computation with problem-aware adaptation
  double rel_tier1, rel_tier2;
  get_enhanced_thresholds (vata, stable, &rel_tier1, &rel_tier2, &dist_params, problem_type);
  
  // Compute total used
  uint64_t total_used = 0;
  for (unsigned glue = 0; glue <= MAX_GLUE_USED; glue++)
    total_used += used_stats[glue];
  
  int tier1 = -1, tier2 = -1;
  if (total_used) {
    uint64_t accumulated_tier1_limit = (uint64_t)(total_used * rel_tier1);
    uint64_t accumulated_tier2_limit = (uint64_t)(total_used * rel_tier2);
    uint64_t accumulated_used = 0;
    unsigned glue;
    
    for (glue = 0; glue <= MAX_GLUE_USED; glue++) {
      uint64_t glue_used = used_stats[glue];
      accumulated_used += glue_used;
      if (accumulated_used >= accumulated_tier1_limit) {
        tier1 = glue;
        break;
      }
    }
    
    if (accumulated_used < accumulated_tier2_limit) {
      for (glue = tier1 + 1; glue <= MAX_GLUE_USED; glue++) {
        uint64_t glue_used = used_stats[glue];
        accumulated_used += glue_used;
        if (accumulated_used >= accumulated_tier2_limit) {
          tier2 = glue;
          break;
        }
      }
    }
  }
  
  unsigned computed_tier1 = (tier1 >= 0) ? tier1 : GET_OPTION (tier1);
  unsigned computed_tier2 = (tier2 >= 0) ? tier2 : MAX (GET_OPTION (tier2), computed_tier1);
  
  computed_tier1 = MAX (computed_tier1, vata->tier1_min);
  computed_tier2 = MAX (computed_tier2, MAX (computed_tier1, vata->tier2_min));
  
  // Smooth with history (temporal smoothing) - VATA-EPA: changed decay from 0.8 to 0.75
  vata->tier1_history[vata->history_index] = computed_tier1;
  vata->tier2_history[vata->history_index] = computed_tier2;
  vata->history_index = (vata->history_index + 1) % 8;
  if (vata->history_count < 8) vata->history_count++;
  
  double weight = 1.0;
  double sum1 = 0, sum2 = 0, weight_sum = 0;
  
  for (int i = 0; i < vata->history_count; i++) {
    int idx = (vata->history_index - 1 - i + 8) % 8;
    sum1 += weight * (double)vata->tier1_history[idx];
    sum2 += weight * (double)vata->tier2_history[idx];
    weight_sum += weight;
    weight *= 0.75;  // Changed from 0.8 (more weight on recent history)
  }
  
  unsigned smoothed_tier1 = (unsigned)round (sum1 / weight_sum);
  unsigned smoothed_tier2 = (unsigned)round (sum2 / weight_sum);
  
  // Clear phase switch flag after first computation
  vata->phase_switch_recent = false;
  
  *tier1_ptr = smoothed_tier1;
  *tier2_ptr = smoothed_tier2;
  
  LOG ("VATA-EPA %s: mean=%.1f var=%.1f shape=%d tier1=%u tier2=%u phase=%u",
      stable ? "stable" : "focused",
      vata->glue_mean, vata->glue_variance, 
      vata->distribution_shape,
      smoothed_tier1, smoothed_tier2,
      vata->search_phase);
}

void kissat_compute_and_set_tier_limits_vata (struct kissat *solver) {
  bool stable = solver->stable;
  unsigned tier1, tier2;
  unsigned old_tier1 = solver->tier1[stable];
  unsigned old_tier2 = solver->tier2[stable];
  
  // Initialize state if needed
  if (solver->vata_state[stable].history_count == 0) {
    init_vata_state (solver, stable);
  }
  
  // Compute with VATA-EPA
  compute_tier_limits_vata (solver, stable, &tier1, &tier2);
  
  solver->tier1[stable] = tier1;
  solver->tier2[stable] = tier2;
  
  // Adaptive decay for statistics if limits changed significantly
  if (tier1 > 1.5 * old_tier1 || tier2 > 1.5 * old_tier2) {
    statistics *statistics = &solver->statistics;
    uint64_t *used_stats = statistics->used[stable].glue;
    for (unsigned glue = 0; glue <= MAX_GLUE_USED; glue++) {
      used_stats[glue] = 1 + round ((double)used_stats[glue] * 0.8);
    }
  }
  
  kissat_phase (solver, "retiered", GET (retiered),
              "recomputed %s tier1 limit %u and tier2 limit %u "
              "(VATA-EPA: mean=%.1f var=%.1f phase=%u) after %" PRIu64 " conflicts",
              stable ? "stable" : "focused", tier1, tier2,
              solver->vata_state[stable].glue_mean,
              solver->vata_state[stable].glue_variance,
              solver->vata_state[stable].search_phase,
              CONFLICTS);
}

// CATRC: Context-Aware Tier-Restart Coordinator
// EVOLVE: CATRC - Joint Tier-Restart Optimization with Problem Profile

// CATRC: Problem profile classes
typedef enum {
  PROFILE_SOFT,
  PROFILE_MIXED,
  PROFILE_HARD
} catrc_profile_t;

// CATRC: Conflict pattern for restart coordination (already defined in internal.h)

// CATRC: Joint optimization state
typedef struct {
  // Problem profile
  catrc_profile_t profile;
  double hardness;
  double entropy;
  double concentration;
  double cv;
  double mean;
  double skewness;
  
  // Dynamic bounds (adapted)
  double tier1_min, tier1_max;
  double tier2_min, tier2_max;
  
  // Activity correlation
  double activity_correlation;
  
  // Joint optimization weights
  double lambda_tier;
  double lambda_restart;
  double lambda_correlation;
  
  // Restart coordination
  unsigned current_restart_interval;
  unsigned joint_decision_count;
  
  // Statistics
  double tier_score;
  double restart_score;
  double correlation_bonus;
} catrc_state_t;

// CATRC: Compute problem profile based on glue distribution
static void catrc_compute_problem_profile (kissat *solver, glue_dist_params_t *params,
                                            catrc_state_t *catrc) {
  // Extract from distribution parameters
  catrc->cv = params->cv;
  catrc->entropy = params->entropy;
  catrc->concentration = params->concentration;
  
  // Estimate hardness based on distribution shape
  // Higher CV and lower concentration = harder problem
  catrc->hardness = fmin (1.0, catrc->cv * 0.7 + (1.0 - catrc->concentration) * 0.3);
  
  // Determine profile class
  if (catrc->hardness < 0.3) {
    catrc->profile = PROFILE_SOFT;
  } else if (catrc->hardness < 0.7) {
    catrc->profile = PROFILE_MIXED;
  } else {
    catrc->profile = PROFILE_HARD;
  }
  
  LOG ("CATRC: profile=%d hardness=%.2f cv=%.2f concentration=%.2f",
       catrc->profile, catrc->hardness, catrc->cv, catrc->concentration);
}

// CATRC: Compute dynamic bounds based on problem profile
static void catrc_compute_dynamic_bounds (catrc_state_t *catrc) {
  // Base bounds from VATA-EPA
  double base_tier1_min = 0.02;
  double base_tier1_max = 0.15;
  double base_tier2_min = 0.08;
  double base_tier2_max = 0.40;
  
  // Adjust based on hardness
  double hardness = catrc->hardness;
  double concentration = catrc->concentration;
  
  // More aggressive tier1 for hard problems
  catrc->tier1_min = base_tier1_min * (1.0 - 0.3 * hardness);
  catrc->tier1_max = base_tier1_max * (1.0 + 0.2 * hardness);
  
  // More aggressive tier2 for concentrated distributions
  catrc->tier2_min = base_tier2_min * (1.0 - 0.2 * concentration);
  catrc->tier2_max = base_tier2_max * (1.0 + 0.3 * concentration);
  
  LOG ("CATRC: dynamic tier1=[%.3f, %.3f] tier2=[%.3f, %.3f]",
       catrc->tier1_min, catrc->tier1_max, catrc->tier2_min, catrc->tier2_max);
}

// CATRC: Compute activity correlation for tier refinement
static double catrc_compute_activity_correlation (kissat *solver, unsigned tier1_limit) {
  // Simplified activity correlation calculation
  // In practice, this would track clause activity patterns
  
  (void)solver;
  (void)tier1_limit;
  
  // Return default correlation (can be enhanced with actual tracking)
  return 0.0;
}

// CATRC: Joint optimization of tier limits and restart interval
static void catrc_joint_optimization (kissat *solver, vata_state_t *vata,
                                       catrc_state_t *catrc,
                                       double *out_tier1, double *out_tier2,
                                       unsigned *out_restart_interval) {
  // Get problem profile
  glue_dist_params_t params;
  params.cv = vata->glue_cv;
  params.entropy = vata->glue_entropy;
  params.concentration = vata->glue_concentration;
  
  catrc_compute_problem_profile (solver, &params, catrc);
  catrc_compute_dynamic_bounds (catrc);
  
  // Compute activity correlation
  double activity_corr = catrc_compute_activity_correlation (solver, (unsigned)*out_tier1);
  catrc->activity_correlation = activity_corr;
  
  // Adjust tier1 based on correlation (negative correlation = more restrictive)
  double tier1_adjustment = 1.0 - 0.2 * activity_corr;
  
  // Base tier limits from VATA-EPA
  double tier1 = vata->relative_tier1 * tier1_adjustment;
  double tier2 = vata->relative_tier2;
  
  // Apply dynamic bounds
  tier1 = fmax (catrc->tier1_min, fmin (catrc->tier1_max, tier1));
  tier2 = fmax (catrc->tier2_min, fmin (catrc->tier2_max, tier2));
  
  // Compute tier score (clause survival rate weighted by quality)
  catrc->tier_score = tier1 * 0.5 + tier2 * 0.5;
  
  // Estimate restart interval based on profile
  // Harder problems need more frequent restarts
  unsigned base_restart = 500;
  if (catrc->profile == PROFILE_HARD) {
    *out_restart_interval = (unsigned)(base_restart * 0.7);
  } else if (catrc->profile == PROFILE_SOFT) {
    *out_restart_interval = (unsigned)(base_restart * 1.5);
  } else {
    *out_restart_interval = base_restart;
  }
  
  // Compute restart score
  catrc->restart_score = 1.0 / (1.0 + log ((double)*out_restart_interval));
  
  // Compute correlation bonus (alignment between tier and restart decisions)
  catrc->correlation_bonus = 0.1 * (1.0 - fabs (tier1 - tier2 / 2));
  
  catrc->joint_decision_count++;
  
  *out_tier1 = tier1;
  *out_tier2 = tier2;
  
  LOG ("CATRC: joint opt tier1=%.3f tier2=%.3f restart=%u score=%.3f+%.3f+%.3f",
       tier1, tier2, *out_restart_interval,
       catrc->tier_score, catrc->restart_score, catrc->correlation_bonus);
}

// CATRC: Initialize state
static void init_catrc_state (catrc_state_t *catrc) {
  catrc->profile = PROFILE_MIXED;
  catrc->hardness = 0.5;
  catrc->entropy = 0.0;
  catrc->concentration = 1.0;
  catrc->cv = 0.5;
  catrc->mean = 8.0;
  catrc->skewness = 0.0;
  
  catrc->tier1_min = 0.02;
  catrc->tier1_max = 0.15;
  catrc->tier2_min = 0.08;
  catrc->tier2_max = 0.40;
  
  catrc->activity_correlation = 0.0;
  
  catrc->lambda_tier = 0.4;
  catrc->lambda_restart = 0.3;
  catrc->lambda_correlation = 0.3;
  
  catrc->current_restart_interval = 500;
  catrc->joint_decision_count = 0;
  
  catrc->tier_score = 0.0;
  catrc->restart_score = 0.0;
  catrc->correlation_bonus = 0.0;
}

// CATRC: Enhanced tier limit computation with joint optimization
static void compute_tier_limits_catrc (kissat *solver, bool stable,
                                        unsigned *tier1_ptr, unsigned *tier2_ptr) {
  vata_state_t *vata = &solver->vata_state[stable];
  statistics *statistics = &solver->statistics;
  uint64_t *used_stats = statistics->used[stable].glue;
  
  // Initialize CATRC state if needed
  static catrc_state_t catrc_global;
  static bool catrc_initialized = false;
  if (!catrc_initialized) {
    init_catrc_state (&catrc_global);
    catrc_initialized = true;
  }
  catrc_state_t *catrc = &catrc_global;
  
  // Compute distribution statistics
  double mean, variance, skewness, kurtosis;
  compute_glue_statistics_v2 (solver, stable, used_stats, &mean, &variance, &skewness, &kurtosis);
  
  // Compute enhanced distribution parameters
  glue_dist_params_t dist_params;
  compute_glue_distribution_enhanced (solver, stable, used_stats, &dist_params);
  
  // Update VATA state
  vata->glue_cv = dist_params.cv;
  vata->glue_entropy = dist_params.entropy;
  vata->glue_concentration = dist_params.concentration;
  
  // VATA-EPA: Update search phase
  uint64_t conflicts = statistics->conflicts;
  vata->search_phase = detect_search_phase ((unsigned)conflicts);
  
  // VATA-EPA: Phase-adaptive alpha
  vata->phase_adaptive_alpha = 0.15;
  if (vata->search_phase == 0) {
    vata->phase_adaptive_alpha = 0.2;
  } else if (vata->search_phase == 2) {
    vata->phase_adaptive_alpha = 0.05;
  }
  
  // Estimate problem type
  unsigned problem_type = estimate_problem_type (&dist_params);
  
  // Estimate complexity
  vata->problem_complexity = fmin (1.0, dist_params.cv * 0.8 + (1.0 - dist_params.concentration) * 0.2);
  
  // Update EMA statistics
  double alpha = vata->phase_adaptive_alpha;
  vata->glue_ema = alpha * mean + (1.0 - alpha) * vata->glue_ema;
  vata->glue_ema2 = alpha * variance + (1.0 - alpha) * vata->glue_ema2;
  vata->recent_mean = 0.7 * vata->recent_mean + 0.3 * mean;
  vata->long_term_mean = 0.95 * vata->long_term_mean + 0.05 * mean;
  vata->glue_mean = 0.9 * vata->glue_mean + 0.1 * mean;
  vata->glue_variance = 0.9 * vata->glue_variance + 0.1 * variance;
  vata->glue_skewness = 0.9 * vata->glue_skewness + 0.1 * skewness;
  vata->glue_kurtosis = 0.9 * vata->glue_kurtosis + 0.1 * kurtosis;
  
  // Distribution shape
  vata->distribution_shape = detect_distribution_shape_v2 (vata->glue_variance, 
                                                            vata->glue_mean,
                                                            vata->glue_skewness);
  
  // Get base thresholds from VATA-EPA
  double rel_tier1, rel_tier2;
  get_enhanced_thresholds (vata, stable, &rel_tier1, &rel_tier2, &dist_params, problem_type);
  
  // CATRC: Apply joint optimization
  unsigned restart_interval;
  catrc_joint_optimization (solver, vata, catrc, &rel_tier1, &rel_tier2, &restart_interval);
  
  // Update global restart interval if needed
  (void)restart_interval;  // Could be used to coordinate with restart.c
  
  // Compute total used
  uint64_t total_used = 0;
  for (unsigned glue = 0; glue <= MAX_GLUE_USED; glue++)
    total_used += used_stats[glue];
  
  int tier1 = -1, tier2 = -1;
  if (total_used) {
    uint64_t accumulated_tier1_limit = (uint64_t)(total_used * rel_tier1);
    uint64_t accumulated_tier2_limit = (uint64_t)(total_used * rel_tier2);
    uint64_t accumulated_used = 0;
    unsigned glue;
    
    for (glue = 0; glue <= MAX_GLUE_USED; glue++) {
      uint64_t glue_used = used_stats[glue];
      accumulated_used += glue_used;
      if (accumulated_used >= accumulated_tier1_limit) {
        tier1 = glue;
        break;
      }
    }
    
    if (accumulated_used < accumulated_tier2_limit) {
      for (glue = tier1 + 1; glue <= MAX_GLUE_USED; glue++) {
        uint64_t glue_used = used_stats[glue];
        accumulated_used += glue_used;
        if (accumulated_used >= accumulated_tier2_limit) {
          tier2 = glue;
          break;
        }
      }
    }
  }
  
  unsigned computed_tier1 = (tier1 >= 0) ? tier1 : GET_OPTION (tier1);
  unsigned computed_tier2 = (tier2 >= 0) ? tier2 : MAX (GET_OPTION (tier2), computed_tier1);
  
  computed_tier1 = MAX (computed_tier1, vata->tier1_min);
  computed_tier2 = MAX (computed_tier2, MAX (computed_tier1, vata->tier2_min));
  
  // Temporal smoothing
  vata->tier1_history[vata->history_index] = computed_tier1;
  vata->tier2_history[vata->history_index] = computed_tier2;
  vata->history_index = (vata->history_index + 1) % 8;
  if (vata->history_count < 8) vata->history_count++;
  
  double weight = 1.0;
  double sum1 = 0, sum2 = 0, weight_sum = 0;
  
  for (int i = 0; i < vata->history_count; i++) {
    int idx = (vata->history_index - 1 - i + 8) % 8;
    sum1 += weight * (double)vata->tier1_history[idx];
    sum2 += weight * (double)vata->tier2_history[idx];
    weight_sum += weight;
    weight *= 0.75;
  }
  
  unsigned smoothed_tier1 = (unsigned)round (sum1 / weight_sum);
  unsigned smoothed_tier2 = (unsigned)round (sum2 / weight_sum);
  
  vata->phase_switch_recent = false;
  
  *tier1_ptr = smoothed_tier1;
  *tier2_ptr = smoothed_tier2;
  
  LOG ("CATRC %s: mean=%.1f var=%.1f shape=%d tier1=%u tier2=%u phase=%u profile=%d",
      stable ? "stable" : "focused",
      vata->glue_mean, vata->glue_variance, 
      vata->distribution_shape,
      smoothed_tier1, smoothed_tier2,
      vata->search_phase,
      catrc->profile);
}

// CATRC: Wrapper function
void kissat_compute_and_set_tier_limits_catrc (struct kissat *solver) {
  bool stable = solver->stable;
  unsigned tier1, tier2;
  unsigned old_tier1 = solver->tier1[stable];
  unsigned old_tier2 = solver->tier2[stable];
  
  if (solver->vata_state[stable].history_count == 0) {
    init_vata_state (solver, stable);
  }
  
  compute_tier_limits_catrc (solver, stable, &tier1, &tier2);
  
  solver->tier1[stable] = tier1;
  solver->tier2[stable] = tier2;
  
  if (tier1 > 1.5 * old_tier1 || tier2 > 1.5 * old_tier2) {
    statistics *statistics = &solver->statistics;
    uint64_t *used_stats = statistics->used[stable].glue;
    for (unsigned glue = 0; glue <= MAX_GLUE_USED; glue++) {
      used_stats[glue] = 1 + round ((double)used_stats[glue] * 0.8);
    }
  }
  
  kissat_phase (solver, "retiered", GET (retiered),
              "recomputed %s tier1 limit %u and tier2 limit %u "
              "(CATRC: profile=%d hardness=%.2f) after %" PRIu64 " conflicts",
              stable ? "stable" : "focused", tier1, tier2,
              solver->vata_state[stable].search_phase,
              CONFLICTS);
}
// EVOLVE_END

// Wrapper function to use VATA by default
void kissat_compute_and_set_tier_limits (struct kissat *solver) {
  kissat_compute_and_set_tier_limits_vata (solver);
}

static unsigned decimal_digits (uint64_t i) {
  unsigned res = 1;
  uint64_t limit = 10;
  for (;;) {
    if (i < limit)
      return res;
    limit *= 10;
    res++;
  }
}

void kissat_print_tier_usage_statistics (kissat *solver, bool stable) {
  unsigned tier1, tier2;
  compute_tier_limits (solver, stable, &tier1, &tier2);
  statistics *statistics = &solver->statistics;
  uint64_t *used_stats = statistics->used[stable].glue;
  uint64_t total_used = 0;
  for (unsigned glue = 0; glue <= MAX_GLUE_USED; glue++)
    total_used += used_stats[glue];
  const char *mode = stable ? "stable" : "focused";
  assert (tier1 <= tier2);
  unsigned span = tier2 - tier1 + 1;
  const unsigned max_printed = 5;
  assert (max_printed & 1), assert (max_printed / 2 > 0);
  unsigned prefix, suffix;
  if (span > max_printed) {
    prefix = tier1 + max_printed / 2 - 1;
    suffix = tier2 - max_printed / 2 + 1;
  } else
    prefix = UINT_MAX, suffix = 0;
  uint64_t accumulated_middle = 0;
  int glue_digits = 1, clauses_digits = 1;
  for (unsigned glue = 0; glue <= MAX_GLUE_USED; glue++) {
    if (glue < tier1)
      continue;
    uint64_t used = used_stats[glue];
    int tmp_glue = 0, tmp_clauses = 0;
    if (glue <= prefix || suffix <= glue) {
      tmp_glue = decimal_digits (glue);
      tmp_clauses = decimal_digits (used);
    } else {
      accumulated_middle += used;
      if (glue + 1 == suffix) {
        tmp_glue = decimal_digits (prefix + 1) + decimal_digits (glue) + 1;
        tmp_clauses = decimal_digits (accumulated_middle);
      }
    }
    if (tmp_glue > glue_digits)
      glue_digits = tmp_glue;
    if (tmp_clauses > clauses_digits)
      clauses_digits = tmp_clauses;
    if (glue == tier2)
      break;
  }
  char fmt[32];
  sprintf (fmt, "%%%d" PRIu64, clauses_digits);
  accumulated_middle = 0;
  uint64_t accumulated = 0;
  for (unsigned glue = 0; glue <= MAX_GLUE_USED; glue++) {
    uint64_t used = used_stats[glue];
    accumulated += used;
    if (glue < tier1)
      continue;
    if (glue <= prefix || suffix <= glue + 1) {
      fputs (solver->prefix, stdout);
      fputs (mode, stdout);
      fputs (" glue ", stdout);
    }
    if (glue <= prefix || suffix <= glue) {
      int len = printf ("%u", glue);
      while (len > 0 && len < glue_digits)
        fputc (' ', stdout), len++;
      fputs (" used ", stdout);
      printf (fmt, used);
      printf (" clauses %5.2f%% accumulated %5.2f%%",
              kissat_percent (used, total_used),
              kissat_percent (accumulated, total_used));
      if (glue == tier1)
        fputs (" tier1", stdout);
      if (glue == tier2)
        fputs (" tier2", stdout);
      fputc ('\n', stdout);
    } else {
      accumulated_middle += used;
      if (glue + 1 == suffix) {
        int len = printf ("%u-%u", prefix + 1, suffix - 1);
        while (len > 0 && len < glue_digits)
          fputc (' ', stdout), len++;
        fputs (" used ", stdout);
        printf (fmt, accumulated_middle);
        printf (" clauses %5.2f%% accumulated %5.2f%%\n",
                kissat_percent (accumulated_middle, total_used),
                kissat_percent (accumulated, total_used));
      }
    }
    if (glue == tier2)
      break;
  }
}
