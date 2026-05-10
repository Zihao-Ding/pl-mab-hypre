#ifndef _internal_h_INCLUDED
#define _internal_h_INCLUDED

#include "arena.h"
#include "array.h"
#include "assign.h"
#include "averages.h"
#include "check.h"
#include "classify.h"
#include "clause.h"
#include "cover.h"
#include "extend.h"
#include "flags.h"
#include "format.h"
#include "frames.h"
#include "heap.h"
#include "kimits.h"
#include "kissat.h"
#include "literal.h"
#include "mode.h"
#include "options.h"
#include "phases.h"
#include "profile.h"
#include "proof.h"
#include "queue.h"
#include "random.h"
#include "reluctant.h"
#include "rephase.h"
#include "smooth.h"
#include "stack.h"
#include "statistics.h"
#include "value.h"
#include "vector.h"
#include "watch.h"

typedef struct datarank datarank;

struct datarank {
  unsigned data;
  unsigned rank;
};

typedef struct import import;

struct import {
  unsigned lit;
  bool extension;
  bool imported;
  bool eliminated;
};

typedef struct termination termination;

struct termination {
#ifdef COVERAGE
  volatile uint64_t flagged;
#else
  volatile bool flagged;
#endif
  volatile void *state;
  int (*volatile terminate) (void *);
};

// clang-format off

typedef STACK (value) eliminated;
typedef STACK (import) imports;
typedef STACK (datarank) dataranks;
typedef STACK (watch) statches;
typedef STACK (watch *) patches;

// clang-format on

// Forward declarations and types for algorithm insights
struct kitten;

#define MAX_VARIABLES_SAMPLING 65536
#define MAX_TIER1 50
#define MAX_TIER2 100
#define MAX_CLAUSES_TRACKING 131072

typedef struct {
  uint64_t select_count;
  uint64_t conflict_count;
  double reward;
} literal_success_t;

typedef struct {
  literal_success_t pos[MAX_VARIABLES_SAMPLING];
  literal_success_t neg[MAX_VARIABLES_SAMPLING];
  uint64_t lookahead_window;
  double exploration_rate;
  bool enabled;
  unsigned recent_selections_lits[32];
  unsigned recent_selections_count;
} cbls_state_t;

typedef struct {
  double alpha;
  double beta;
  double ema_reward;
  unsigned samples;
} sdats_arm_t;

typedef struct {
  sdats_arm_t arms[4];
  double momentum;
  unsigned adaptive_stay;
  unsigned restart_count;
  unsigned current_level;
  unsigned max_level_this_restart;
  double avg_level;
  unsigned level_samples;
  unsigned stay_counter;
} sdats_state_t;

typedef struct {
  unsigned phase;
  uint64_t phase_start_conflicts;
  double clause_satisfaction_rate;
  unsigned avg_trail_size;
} search_phase_t;

typedef struct {
  double Q[4][MAX_TIER1][MAX_TIER2];
  unsigned tier1_action;
  unsigned tier2_action;
  double learning_rate;
  double discount_factor;
  double exploration_rate;
  unsigned observations;
  int problem_class;
  double confidence;
  unsigned learned_glue[101];
} tier_predictor_t;

typedef struct {
  float clause_density;
  float binary_ratio;
  float avg_clause_size;
  float conflict_rate;
  float propagation_rate;
  float trail_reuse;
  float decision_rate;
  float learned_clause_quality;
  float variable_activity_var;
  float search_depth;
} problem_features_t;

typedef struct {
  float input[10];
  float hidden[16];
  float output[2];
  float W1[10 * 16];
  float b1[16];
  float W2[16 * 2];
  float b2[2];
  float hidden_grad[16];
  float output_grad[2];
  float input_grad[10];
  float learning_rate;
  bool trained;
} neural_selector_t;

typedef struct {
  float clause_count;
  float variable_count;
  float binary_ratio;
  float average_clause_size;
  float clause_graph_density;
  float variable_dependency_density;
  float backbone_fraction;
  float clause_hitting_set_size;
  float variable_activity_entropy;
  float clause_size_variance;
  float literal_occurrence_skewness;
  float estimated_difficulty;
  float solution_density;
} problem_feature_vector_t;

typedef struct {
  problem_feature_vector_t support_vectors[50];
  float support_weights[50];
  unsigned support_vector_count;
  float kernel_gamma;
  float bias;
  unsigned problem_class;
  float decision_value;
  bool trained;
} kernel_classifier_t;

typedef struct {
  unsigned problem_id;
  problem_feature_vector_t features;
  unsigned actual_performance;
} problem_signature_t;

typedef struct {
  uint64_t created_at_conflict;
  uint64_t last_used_conflict;
  unsigned collection_count;
  bool marked_for_keep;
} clause_age_t;

// Insight 0: Temporal-Weighted Adaptive Centrality (TWAC)
typedef struct {
  unsigned *adjacency;           // Compressed adjacency list
  unsigned *adjacency_offsets;   // Start indices for each literal
  unsigned *out_degree;          // Out-degree for each literal
  double *centrality;            // Pre-computed centrality scores
  double *temporal_centrality;   // Time-decayed centrality scores (TWAC)
  unsigned max_lit;              // Maximum literal index
  bool computed;                 // Whether centrality is current
  
  // Incremental tracking for AGC-IU
  unsigned *incremental_delta;   // Changes since last computation
  uint64_t last_update_conflict; // When graph was last updated
  unsigned pending_additions;    // Binary clauses added since update
  unsigned pending_deletions;    // Binary clauses deleted since update
  
  // Adaptive convergence
  double last_pr_delta;          // Last PageRank convergence delta
  unsigned actual_iterations;    // Actual iterations used
  
  // TWAC: temporal tracking and adaptive damping
  double graph_density;          // Computed density for adaptive damping
  double adaptive_damping;       // Computed adaptive damping factor
  double threshold_velocity;     // Rate of threshold change
  double threshold_momentum;     // Momentum for smoothing
} implication_graph_t;

// Insight 0: Decision-Pattern-Aware Literal Selection (DPALS)
typedef struct {
  uint64_t decision_count;      // Times variable was decision
  uint64_t propagation_count;   // Times variable propagated
  uint64_t conflict_count;      // Times variable in conflict clause
  double decision_rate;         // decision_count / total_decisions
  double propagation_rate;      // propagation_count / total_propagations
  unsigned last_decision_level; // Decision level of last decision
  double recent_success;        // Recent decision success score
} decision_pattern_t;

typedef struct {
  decision_pattern_t *patterns;
  uint64_t total_decisions;
  uint64_t total_propagations;
  unsigned window_size;         // Sliding window for recent decisions
  unsigned *recent_decisions;  // Circular buffer of recent decisions
  unsigned decision_index;
  double pattern_decay;         // Exponential decay factor
} dpals_state_t;

// Insight 0: Adaptive Score Normalization with Exponential Recency (ASN-ER)
typedef struct {
  decision_pattern_t *patterns;
  uint64_t total_decisions;
  uint64_t total_propagations;
  unsigned window_size;
  unsigned *recent_decisions;
  unsigned decision_index;
  
  // Adaptive parameters
  double decision_weight;
  double propagation_weight;
  double conflict_weight;
  
  // Score normalization state
  double score_min;
  double score_max;
  double score_variance;
  unsigned score_samples;
  
  // Recency parameters
  double recency_decay;
  unsigned recency_window;
} asner_state_t;

// Insight 1: Conflict-Pattern-Driven Adaptive Restart (CPDAR)
typedef struct {
  uint64_t glue_sum;           // Sum of glue values
  uint64_t glue_samples;       // Number of glue samples
  double avg_glue;             // Average glue (lower is better)
  double glue_variance;        // Variance in glue
  double glue_trend;           // Glue trend (improving/deteriorating)
  
  uint64_t size_sum;           // Sum of clause sizes
  double avg_size;             // Average clause size
  
  uint64_t last_update_conflicts;      // When last updated
} conflict_pattern_t;

typedef enum {
  CPDAR_PHASE_INITIAL,      // Early search
  CPDAR_PHASE_EXPLORATION,  // Broad search  
  CPDAR_PHASE_DEEP,         // Deep search
  CPDAR_PHASE_INTENSIFICATION, // Focused search
  CPDAR_PHASE_EXHAUSTION    // Near solution
} cpdar_phase_t;

typedef struct {
  conflict_pattern_t pattern;
  cpdar_phase_t phase;
  uint64_t phase_start_conflicts;
  uint64_t phase_changes;
  
  // Performance tracking
  double performance_history[16];
  unsigned perf_index;
  unsigned perf_count;
  
  // Adaptive parameters
  unsigned restart_heuristic;
  unsigned stay_count;
  unsigned stay_limit;
  
  // Quality thresholds
  double good_glue_threshold;
  double bad_glue_threshold;
} cpdar_state_t;

// Insight 1: PRECISE - Priority-based REward-driven Clause Improvement with State Estimation
typedef struct {
  // Bandit state for clause categories
  double arm_rewards[8];           // Reward estimates for each category
  unsigned arm_selections[8];      // Times each arm was selected
  double ucb_exploration;          // Exploration constant for UCB
  
  // State tracking
  uint64_t last_update_conflicts;
  double recent_success_rate;      // Recent propagation success
  unsigned success_window[32];     // Circular buffer for success tracking
  unsigned success_idx;
  unsigned success_count;
  
  // Adaptive parameters
  double priority_bias;            // Learned bias for priority computation
  unsigned search_phase;           // 0=initial, 1=stable, 2=deep
  
  // Performance metrics
  unsigned total_propagations;
  unsigned successful_propagations;
  double avg_propagation_depth;
} precise_state_t;

// Insight 1: Glue-Variance-Aware Restart with Adaptive Momentum (GVAR-AM)
typedef struct {
  uint64_t glue_sum;
  uint64_t glue_samples;
  double avg_glue;
  double glue_variance;
  double glue_trend;
  double glue_momentum;
  double prev_glue_trend;
  
  uint64_t size_sum;
  double avg_size;
  
  uint64_t last_update_conflicts;
  
  double glue_history[32];
  unsigned glue_history_idx;
  unsigned glue_history_count;
} enhanced_conflict_pattern_t;

typedef struct {
  enhanced_conflict_pattern_t pattern;
  cpdar_phase_t phase;
  uint64_t phase_start_conflicts;
  uint64_t phase_changes;
  
  double performance_history[32];
  unsigned perf_index;
  unsigned perf_count;
  
  double performance_momentum;
  double performance_acceleration;
  
  unsigned restart_heuristic;
  unsigned stay_count;
  unsigned stay_limit;
  unsigned stay_limit_momentum;
  
  double good_glue_threshold;
  double bad_glue_threshold;
  
  double threshold_adjustment;
  unsigned good_samples;
} gvar_am_state_t;

// Centrality learning state with oscillation damping
typedef struct {
  double learned_threshold;
  uint64_t samples_collected;
  double avg_propagation_centrality;
  double threshold_velocity;
  double target_centrality;
} centrality_learner_t;

typedef struct {
  uint64_t out_degree_pos;       // Positive literal out-degree
  uint64_t out_degree_neg;       // Negative literal out-degree
  double pagerank;               // PageRank score
  double centrality;             // Combined centrality score
} literal_centrality_t;

// Insight 1: Distribution-Aware Tier Adaptation (DATA)
typedef struct {
  uint64_t glue_histogram[101];  // Count of clauses at each glue (MAX_GLUE=100)
  uint64_t total_learned;        // Total clauses learned
  double mean;                   // Mean glue
  double variance;               // Variance of glue
  double skewness;               // Skewness of distribution
  double kurtosis;               // Kurtosis (peakedness)
  unsigned mode;                 // Most common glue value
  unsigned median;               // Median glue value
} glue_distribution_t;

typedef enum {
  PROBLEM_INDUSTRIAL,
  PROBLEM_CRYPTO,
  PROBLEM_COMBINATORIAL
} problem_class_t;

// Insight 2: Usage-Aware Predictive Tier Adaptation (UAPTA)
typedef struct {
  uint64_t learned_count;
  uint64_t used_count;
  uint64_t propagated_count;
  double usage_ratio;
} glue_usage_t;

typedef struct {
  glue_usage_t glue_stats[101];
  uint64_t total_learned;
  uint64_t total_used;
  
  double tier1_ema;
  double tier2_ema;
  double tier1_trend;
  double tier2_trend;
  
  double hardness_score;
  double structure_score;
  
  unsigned observations;
} tier_usage_state_t;
typedef struct {
  double ewma_reward;            // Exponentially weighted moving average
  double ewma_variance;          // EWMA of squared reward (for variance)
  double change_threshold;       // Threshold for detecting change
  unsigned stability_counter;    // How long we've been stable
  uint64_t change_detected_at;   // Conflict count when change detected
  bool in_transition;            // Currently in phase transition
} change_detector_t;

typedef struct {
  double ucb_values[4];          // UCB values for each (strategy, heuristic) pair
  unsigned select_count[4];      // Selection counts
  double reward_history[20];     // Recent reward history
  unsigned history_index;        // Current position in history
  unsigned phase;                // 0=initial, 1=exploration, 2=deep, 3=refinement
  unsigned phase_conflicts;      // Conflicts in current phase
} phase_mab_t;

// Insight 1: Enhanced Context-Aware MAB (ECA-MAB)
typedef struct {
  double decision_efficiency;    // log2(decisions) / log2(conflicts)
  double clause_utility;         // propagations / learned_clause
  double trail_efficiency;       // average trail reuse
  double conflict_complexity;    // average glue in conflicts
  double composite;              // weighted combination
  uint64_t conflicts;            // when computed
} reward_signal_t;

typedef struct {
  unsigned problem_type;         // 0=industrial, 1=crypto, 2=combo
  uint64_t clause_db_size;       // Current clause count
  double clause_quality;         // Average glue of tier1 clauses
  unsigned search_depth;         // Average decision level
  unsigned restart_count;        // Restarts in current phase
  bool phase_stable;             // Whether phase is stable
} search_context_t;

typedef struct {
  double ucb_values[4];          // UCB values for each arm
  unsigned select_count[4];      // Selection counts
  double reward_history[20];     // Recent reward history
  unsigned history_index;        // Current position in history
  unsigned phase;                // Current detected phase
  
  double value_decay_factor;     // How much to decay on phase change
  unsigned promising_arms;       // Arms with high historical reward
  
  double adaptive_c;             // Context-adaptive exploration constant
} enhanced_mab_t;

typedef struct {
  unsigned problem_type;
  uint64_t clause_count;
  double tier1_glue_avg;
  unsigned avg_search_depth;
  uint64_t last_context_update;
} context_state_t;

typedef struct {
  unsigned phase;                // Current detected phase
  uint64_t phase_start;          // When phase started (conflicts)
  double avg_conflict_rate;      // Conflicts per second in this phase
  double avg_propagation_rate;   // Propagations per conflict
  unsigned consecutive_restarts; // Restarts without progress
} search_phase_info_t;

// Insight 3: Temporal Multi-Phase Classifier (TMPC)
typedef struct {
  unsigned small:1;              // Small formula flag
  unsigned bigbig:1;             // High binary ratio flag
  unsigned industrial:1;         // Industrial problem flag
  unsigned crypto:1;             // Cryptographic problem flag
  unsigned combinatorial:1;      // Combinatorial problem flag
  uint64_t timestamp;            // When this classification was made
  double confidence;             // Confidence in this classification
} classification_snapshot_t;

typedef struct {
  classification_snapshot_t snapshots[10];  // Recent snapshots
  unsigned snapshot_index;                   // Current position
  unsigned snapshot_count;                   // Number of snapshots taken
  
  // Aggregated classification
  unsigned final_small_votes;
  unsigned final_bigbig_votes;
  unsigned final_industrial_votes;
  unsigned final_crypto_votes;
  unsigned final_combinatorial_votes;
  
  // Stability metrics
  double stability_score;
  uint64_t last_conflict_count;
} temporal_classifier_t;

// Insight 3: Context-Aware Variable Bumping (CAVB)
typedef struct {
  unsigned is_decision:1;
  unsigned is_1uip:1;
  unsigned is_reason:1;
  unsigned level;
  unsigned chain_position;
} variable_role_t;

typedef struct {
  double decision_weight;
  double reason_weight;
  double chain_weight;
  double uip_weight;
  double decay_base;
  double decay_min;
  double decay_max;
} bump_config_t;

typedef struct {
  uint64_t bump_count;
  uint64_t last_bump_conflict;
  double activity_score;
  unsigned consecutive_bumps;
} variable_activity_t;

typedef struct {
  unsigned decision_count;
  unsigned propagation_count;
  unsigned chain_length;
  double complexity;
  unsigned uip_position;
} conflict_context_t;

typedef struct {
  uint64_t clauses;
  uint64_t variables;
  uint64_t binary_clauses;
  uint64_t irredundant_clauses;
  uint64_t literals;
  double avg_clause_size;
  double binary_ratio;
  double clause_density;
  uint64_t units;
  uint64_t binaries_from_learned;
} formula_features_t;

// Insight 4: Utility-Aware Garbage Collection (UAGC)
typedef struct {
  uint64_t conflict_used;
  uint64_t propagation_checked;
  uint64_t last_used_conflict;
  uint64_t created_at_conflict;
  double utility_score;
  unsigned gc_survived;
  uint64_t propagation_count;
} clause_utility_t;

typedef struct {
  unsigned garbage_count;        // Current garbage clauses
  unsigned total_clauses;        // Total clauses
  double utility_threshold;      // Minimum utility to keep
  double avg_utility;            // Average clause utility
  unsigned gc_cycles;            // Total GC cycles executed
  uint64_t last_gc_conflicts;    // Conflicts at last GC
} gc_utility_stats_t;

typedef struct {
  unsigned histogram[10];        // Buckets for utility distribution
  double bucket_threshold[10];   // Threshold for each bucket
} utility_histogram_t;

typedef struct {
  reference *heap;
  unsigned size;
  unsigned capacity;
} age_priority_queue_t;

typedef struct {
  uint64_t total_clauses_collected;
  uint64_t young_clauses_collected;
  uint64_t old_clauses_collected;
  double avg_survival_rate;
  unsigned age_threshold;
} gc_age_stats_t;

// Insight 4: Adaptive Utility-Aware Reduction (AUAR)
typedef struct {
  uint64_t last_reduce_conflicts;
  uint64_t clauses_reduced_total;
  double reduction_rate;
  double avg_clause_lifetime;
  unsigned problem_type;
  bool in_high_activity_phase;
} reduction_state_t;

// Insight 1: Predictive Utility-Based Garbage Collection (PUGC)
typedef struct {
  // Historical collection statistics
  uint64_t total_collected;
  uint64_t total_kept;
  double collection_efficiency;
  
  // Adaptive thresholds
  double utility_threshold;
  double threshold_adjustment;
  
  // Decay rate state
  double current_decay_rate;
  double decay_accumulator;
  
  // Prediction model parameters
  double activity_rate;
  double activity_variance;
  uint64_t last_update_conflicts;
  
  // Clause lifecycle tracking
  uint64_t clauses_aged_out;
  uint64_t clauses_reused;
  
  // PUGC-Adaptive: Weights for linear combination
  double base_weight;
  double age_weight;
  double activity_weight;
  double size_weight;
  double redundant_weight;
  
  // PUGC-Adaptive: Dynamic threshold parameters
  double recent_usage_threshold;
  unsigned survivor_threshold;
  double marginal_ratio;
  
  // PUGC-Adaptive: Pool health tracking
  double pool_health_ratio;
  unsigned pool_check_count;
} pugc_state_t;

// DUCB-GC: Dynamic Upper Confidence Bound Garbage Collection
typedef struct {
  // UCB parameters
  double ucb_exploration_constant;    // Exploration vs exploitation trade-off
  double confidence_multiplier;       // For uncertainty bounds
  
  // Clause set statistics
  unsigned clause_set_count;          // Total clauses evaluated
  double mean_utility;                // Running mean utility
  double utility_variance;            // Running utility variance
  double sum_squared_rewards;         // For variance computation
  
  // Phase detection
  unsigned search_phase;              // 0=exploration, 1=exploitation, 2=refinement
  double phase_confidence;            // Confidence in current phase
  
  // UCB history
  double ucb_scores[256];             // Recent UCB scores for analysis
  unsigned ucb_history_idx;
  
  // Collection statistics
  uint64_t total_collected;
  uint64_t total_kept;
  double collection_efficiency;
  
  // Adaptive threshold
  double utility_threshold;
  double threshold_adjustment;
} ducb_gc_state_t;

// UCLM: Unified Clause Lifecycle Manager State
#define UCLM_FEATURE_DIM 8
#define UCLM_NUM_ACTIONS 3

typedef struct {
  // Feature weights for each action (learned)
  double weights[UCLM_NUM_ACTIONS][UCLM_FEATURE_DIM];
  
  // Thompson Sampling posterior parameters
  double posterior_mean[UCLM_NUM_ACTIONS][UCLM_FEATURE_DIM];
  double posterior_std[UCLM_FEATURE_DIM];
  
  // Cross-component reward signals
  double gc_efficiency;
  double reduction_efficiency;
  double search_progress;
  
  // Learning rate and adaptation
  double learning_rate;
  double exploration_noise;
  
  // Hierarchical decision state
  unsigned primary_action;
  unsigned pool_health;
  
  // Statistics
  uint64_t total_decisions;
  double cumulative_reward;
  unsigned update_count;
} uclm_state_t;

// Insight 2: Variance-Aware Tier Adaptation (VATA)
typedef enum {
  DIST_VERY_CONCENTRATED,
  DIST_CONCENTRATED,
  DIST_NORMAL,
  DIST_SPARSE,
  DIST_VERY_SPARSE,
  DIST_HEAVY_TAILED
} distribution_shape_t;

typedef struct {
  // Distribution statistics
  double glue_mean;
  double glue_variance;
  double glue_skewness;
  
  // VATA-Plus: EMA-based statistics (replaces history array)
  double glue_ema;
  double glue_ema2;
  double glue_ema3;
  double recent_mean;
  double long_term_mean;
  
  // Historical tier limits for smoothing
  unsigned tier1_history[8];
  unsigned tier2_history[8];
  unsigned history_index;
  unsigned history_count;
  
  // Distribution shape detection
  distribution_shape_t distribution_shape;
  
  // VATA-Plus: Phase-aware tracking
  bool phase_switch_recent;
  unsigned phase_switch_count;
  
  // Adaptation parameters
  double relative_tier1;
  double relative_tier2;
  unsigned tier1_min;
  unsigned tier2_min;
  
  // VATA-Plus: Kurtosis for heavy-tailed detection
  double glue_kurtosis;
  
  // VATA-EPA: Enhanced state for Problem-Aware Adaptation
  unsigned search_phase;        // 0=initial, 1=exploration, 2=exploitation
  double phase_adaptive_alpha;  // Dynamically computed EMA alpha
  double problem_complexity;    // Estimated from glue distribution
  unsigned convergence_count;   // Times distribution stayed similar
  
  // VATA-EPA: Enhanced distribution parameters
  double glue_cv;               // Coefficient of variation
  double glue_entropy;          // Information-theoretic entropy
  double glue_concentration;    // Herfindahl concentration index
} vata_state_t;

typedef struct {
  uint64_t clause_count;
  uint64_t total_usage;
  double avg_utility;
  uint64_t high_utility_count;
  uint64_t low_utility_count;
} tier_stats_t;

struct kitten;

// Insight 0: AW-VSIDS - Activity-Weighted Variable Ordering with Locality
typedef struct {
  double score;                  // Standard VSIDS score
  double locality_score;         // Temporal locality component
  double lbd_boost;              // LBD-correlated boost
  unsigned last_conflict_level;  // Last decision level with conflict
  unsigned conflict_count;       // Total conflicts involving this var
  unsigned recent_conflicts;     // Conflicts in last N decisions
  unsigned lbd_sum;              // Sum of LBDs from learned clauses
  unsigned lbd_count;            // Number of learned clauses
  double decay_rate;             // Individual decay rate
} variable_activity_extended_t;

// Insight 1: U-ARS - Utility-Based Adaptive Reduction Scheduling
typedef struct {
  // Utility histogram for reduction decisions
  unsigned utility_buckets[10];     // 10 buckets for utility distribution
  double avg_clause_utility;        // Average utility across all clauses
  double utility_variance;          // Variance in clause utilities
  unsigned utility_samples;         // Number of samples collected
  
  // Tier distribution tracking
  unsigned tier1_count;             // Number of tier-1 clauses
  unsigned tier2_count;             // Number of tier-2 clauses  
  unsigned tier3_count;             // Number of tier-3 (learned) clauses
  
  // Reduction history
  uint64_t last_reduction_conflicts;
  uint64_t conflicts_since_last_reduction;
  unsigned reduction_count;
  double avg_reduction_interval;
  
  // Last reduction results
  unsigned clauses_considered_last_reduction;
  unsigned clauses_removed_last_reduction;
  
  // Adaptive parameters
  double utility_threshold;         // Trigger reduction if avg < this
  unsigned base_interval;           // Base conflict interval
  unsigned min_interval;            // Minimum interval between reductions
  unsigned max_interval;            // Maximum interval between reductions
} reduction_utility_state_t;

// PRIMA: Predictive Reinforced Interval Management for Ageing
typedef struct {
  // RL state representation
  double clause_pool_density;        // Current clause pool fill level
  double clause_usefulness_rate;     // Rate of clause utility generation
  double search_progress_rate;       // Decisions/conflicts per time unit
  
  // Temporal difference learning
  double value_estimate;             // V(s): estimated value of current state
  double last_value_estimate;        // V(s-1): previous value estimate
  double alpha;                      // Learning rate
  double gamma;                      // Discount factor
  
  // Feature vector for state representation
  double features[16];               // Normalized feature vector
  unsigned feature_count;
  
  // Action history
  unsigned last_reduction_conflicts;
  unsigned conflicts_at_last_action;
  double last_reward;
  
  // Policy parameters
  double epsilon;                    // Exploration rate
  double threshold_bias;             // Learned bias toward triggering
} prima_state_t;

struct kissat {
  int *last_val;
#if !defined(NDEBUG) || defined(METRICS)
  bool backbone_computing;
#endif
#ifdef LOGGING
  bool compacting;
#endif
  bool extended;
  bool inconsistent;
  bool iterating;
  bool preprocessing;
  bool probing;
#ifndef QUIET
  bool sectioned;
#endif
  bool stable;
#if !defined(NDEBUG) || defined(METRICS)
  bool transitive_reducing;
  bool vivifying;
#endif
  bool warming;
  bool watching;

  bool large_clauses_watched_after_binary_clauses;

  termination termination;

  unsigned vars;
  unsigned size;
  unsigned active;
  unsigned randec;

  ints export;
  ints units;
  imports import;
  extensions extend;
  unsigneds witness;

  assigned *assigned;
  flags *flags;

  mark *marks;

  value *values;
  phases phases;

  eliminated eliminated;
  unsigneds etrail;

  links *links;
  queue queue;

  heap scores;
  double scinc;

  // CHB
  heap scores_chb;
  unsigned *conflicted_chb;
  double step_chb;
  double step_dec_chb;
  double step_min_chb;

  // MAB
  unsigned strategy;
  unsigned heuristic;
  unsigned next_heuristic[2];
  bool mab;
  double mabc;
  double mab_reward[4];
  unsigned mab_select[4];
  unsigned mab_heuristics;
  double mab_decisions;
  unsigned *mab_chosen;
  unsigned mab_chosen_tot;
  unsigned mab_conflicts;

  sdats_state_t sdats_state;

  dpals_state_t dpals_state;
  asner_state_t asner_state;
  cpdar_state_t cpdar_state;
  precise_state_t precise_state;
  gvar_am_state_t gvar_am_state;

  heap schedule;
  double scoreshift;

  unsigned level;
  frames frames;

  unsigned_array trail;
  unsigned *propagate;

  unsigned best_assigned;
  unsigned target_assigned;
  unsigned unflushed;
  unsigned unassigned;

  unsigneds delayed;

#if defined(LOGGING) || !defined(NDEBUG)
  unsigneds resolvent;
#endif
  unsigned resolvent_size;
  unsigned antecedent_size;

  dataranks ranks;

  unsigneds analyzed;
  unsigneds levels;
  unsigneds minimize;
  unsigneds poisoned;
  unsigneds promote;
  unsigneds removable;
  unsigneds shrinkable;

  clause conflict;

  bool clause_satisfied;
  bool clause_shrink;
  bool clause_trivial;

  unsigneds clause;
  unsigneds shadow;

  arena arena;
  vectors vectors;
  reference first_reducible;
  reference last_irredundant;
  watches *watches;

  reference last_learned[4];

  sizes sorter;

  generator random;
  averages averages[2];
  unsigned tier1[2], tier2[2];
  reluctant reluctant;

  bounds bounds;
  classification classification;
  delays delays;
  enabled enabled;
  limited limited;
  limits limits;
  remember last;
  unsigned walked;

  mode mode;

  uint64_t ticks;

  format format;
  char *prefix;

  statches antecedents[2];
  statches gates[2];
  patches xorted[2];
  unsigneds resolvents;
  bool resolve_gate;

  struct kitten *kitten;
#ifdef METRICS
  uint64_t *gate_eliminated;
#else
  bool gate_eliminated;
#endif
  bool sweep_incomplete;
  unsigneds sweep_schedule;

#if !defined(NDEBUG) || !defined(NPROOFS)
  unsigneds added;
  unsigneds removed;
#endif

#if !defined(NDEBUG) || !defined(NPROOFS) || defined(LOGGING)
  ints original;
  size_t offset_of_last_original_clause;
#endif

#ifndef QUIET
  profiles profiles;
#endif

#ifndef NOPTIONS
  options options;
#endif

#ifndef NDEBUG
  checker *checker;
#endif

#ifndef NPROOFS
  proof *proof;
#endif

  statistics statistics;

  cbls_state_t cbls_state;
  unsigned last_tier1;
  unsigned last_tier2;
  search_phase_t search_phase;
  tier_predictor_t tier_predictor;
  neural_selector_t neural_selector;
  kernel_classifier_t kernel_classifier;
  clause_age_t *clause_ages;
  gc_age_stats_t gc_age_stats;

  implication_graph_t implication_graph;
  centrality_learner_t centrality_learner;
  change_detector_t change_detector;
  phase_mab_t phase_mab;
  enhanced_mab_t enhanced_mab;
  context_state_t context_state;
  search_phase_info_t search_phase_info;
  temporal_classifier_t temporal_classifier;
  clause_utility_t *clause_utilities;
  gc_utility_stats_t gc_utility_stats;
  tier_usage_state_t tier_usage;
  variable_activity_t *variable_activities;
  reduction_state_t reduction_state;
  pugc_state_t pugc_state;
  ducb_gc_state_t ducb_gc_state;
  uclm_state_t uclm_state;
  vata_state_t vata_state[2];

  // Insight 0: AW-VSIDS - Activity-Weighted Variable Ordering with Locality
  variable_activity_extended_t *variable_activities_ext;
  double vsids_decay;              // Current VSIDS decay rate
  unsigned current_learned_glue;   // Current learned clause LBD
  bool in_conflict;                // Whether we're in conflict analysis

  // Insight 1: U-ARS - Utility-Based Adaptive Reduction Scheduling
  reduction_utility_state_t reduction_utility_state;
  // PRIMA: Predictive Reinforced Interval Management for Ageing
  prima_state_t prima_state;
};

#define VARS (solver->vars)
#define LITS (2 * solver->vars)

#if 0
#define TIEDX (GET_OPTION (focusedtiers) ? 0 : solver->stable)
#define TIER1 (solver->tier1[TIEDX])
#define TIER2 (solver->tier2[TIEDX])
#else
#define TIER1 (solver->tier1[0])
#define TIER2 (solver->tier2[1])
#endif

#define SCORES (&solver->scores)

static inline unsigned kissat_assigned (kissat *solver) {
  assert (VARS >= solver->unassigned);
  return VARS - solver->unassigned;
}

#define all_variables(IDX) \
  unsigned IDX = 0, IDX##_END = solver->vars; \
  IDX != IDX##_END; \
  ++IDX

#define all_literals(LIT) \
  unsigned LIT = 0, LIT##_END = LITS; \
  LIT != LIT##_END; \
  ++LIT

#define all_clauses(C) \
  clause *C = (clause *) BEGIN_STACK (solver->arena), \
         *const C##_END = (clause *) END_STACK (solver->arena), *C##_NEXT; \
  C != C##_END && (C##_NEXT = kissat_next_clause (C), true); \
  C = C##_NEXT

#define capacity_last_learned \
  (sizeof solver->last_learned / sizeof *solver->last_learned)

#define real_end_last_learned (solver->last_learned + capacity_last_learned)

#define really_all_last_learned(REF_PTR) \
  reference *REF_PTR = solver->last_learned, \
            *REF_PTR##_END = real_end_last_learned; \
  REF_PTR != REF_PTR##_END; \
  REF_PTR++

void kissat_reset_last_learned (kissat *solver);

#endif

#if 1
// 获取当前分支启发式策略所对应的分数堆
heap *kissat_get_scores (kissat *solver);
#endif