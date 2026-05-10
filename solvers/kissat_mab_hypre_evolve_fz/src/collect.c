#define INLINE_SORT

#include "collect.h"
#include "allocate.h"
#include "colors.h"
#include "compact.h"
#include "inline.h"
#include "print.h"
#include "report.h"
#include "sort.c"
#include "trail.h"

#include <inttypes.h>
#include <string.h>

static void flush_watched_clauses_by_literal (kissat *solver, unsigned lit,
                                              bool compact,
                                              reference start) {
  assert (start != INVALID_REF);

  const value *const values = solver->values;
  const assigned *const all_assigned = solver->assigned;

  const value lit_value = values[lit];
  const assigned *const lit_assigned = all_assigned + IDX (lit);
  const value lit_fixed =
      (lit_value && !lit_assigned->level) ? lit_value : 0;
  const unsigned mlit = kissat_map_literal (solver, lit, true);

  watches *lit_watches = &WATCHES (lit);
  watch *begin = BEGIN_WATCHES (*lit_watches), *q = begin;
  const watch *const end_of_watches = END_WATCHES (*lit_watches), *p = q;

  while (p != end_of_watches) {
    watch head = *p++;
    if (head.type.binary) {
      const unsigned other = head.binary.lit;
      const unsigned other_idx = IDX (other);
      const value other_value = values[other];
      const value other_fixed =
          (other_value && !all_assigned[other_idx].level) ? other_value : 0;
      const unsigned mother = kissat_map_literal (solver, other, compact);
      if (lit_fixed > 0 || other_fixed > 0 || mother == INVALID_LIT) {
        if (lit < other)
          kissat_delete_binary (solver, lit, other);
      } else {
        assert (!lit_fixed);
        assert (!other_fixed);

        {
          head.binary.lit = mother;
          *q++ = head;
#ifdef LOGGING
          if (lit < other) {
            LOGBINARY (lit, other, "SRC");
            LOGBINARY (mlit, mother, "DST");
          }
#endif
        }
      }
    } else {
      assert (solver->watching);
      const watch tail = *p++;
      if (!lit_fixed) {
        const reference ref = tail.large.ref;
        if (ref < start) {
          *q++ = head;
          *q++ = tail;
        }
      }
    }
  }

  assert (!lit_fixed || q == begin);
  SET_END_OF_WATCHES (*lit_watches, q);
#ifdef LOGGING
  const size_t size_lit_watches = SIZE_WATCHES (*lit_watches);
  LOG ("keeping %zu watches[%u]", size_lit_watches, lit);
#endif
  if (!compact)
    return;

  if (mlit == INVALID_LIT)
    return;

  watches *mlit_watches = &WATCHES (mlit);
#if defined(LOGGING) || !defined(NDEBUG)
  const size_t size_mlit_watches = SIZE_WATCHES (*mlit_watches);
#endif
  if (lit_fixed)
    assert (!size_mlit_watches);
  else if (mlit < lit) {
    assert (mlit != INVALID_LIT);
    assert (mlit < lit);
    *mlit_watches = *lit_watches;
    LOG ("copied watches[%u] = watches[%u] (size %zu)", mlit, lit,
         size_mlit_watches);
    memset (lit_watches, 0, sizeof *lit_watches);
  } else
    assert (mlit == lit);
}

static void flush_all_watched_clauses (kissat *solver, bool compact,
                                       reference start) {
  assert (solver->watching);
  LOG ("starting to flush watches at clause[%" REFERENCE_FORMAT "]", start);
  for (all_variables (idx)) {
    const unsigned lit = LIT (idx);
    flush_watched_clauses_by_literal (solver, lit, compact, start);
    const unsigned not_lit = NOT (lit);
    flush_watched_clauses_by_literal (solver, not_lit, compact, start);
  }
}

static void update_large_reason (kissat *solver, assigned *assigned,
                                 unsigned forced, clause *dst) {
  assert (dst->reason);
  assert (forced != INVALID_LIT);
  reference dst_ref = kissat_reference_clause (solver, dst);
  const unsigned forced_idx = IDX (forced);
  struct assigned *a = assigned + forced_idx;
  assert (!a->binary);
  if (a->reason != dst_ref) {
    LOG ("reason reference %u of %s updated to %u", a->reason,
         LOGLIT (forced), dst_ref);
    a->reason = dst_ref;
  }
  dst->reason = false;
}

static unsigned get_forced (const value *values, clause *dst) {
  assert (dst->reason);
  unsigned forced = INVALID_LIT;
  for (all_literals_in_clause (lit, dst)) {
    const value value = values[lit];
    if (value <= 0)
      continue;
    forced = lit;
    break;
  }
  assert (forced != INVALID_LIT);
  return forced;
}

static void get_forced_and_update_large_reason (kissat *solver,
                                                assigned *assigned,
                                                const value *const values,
                                                clause *dst) {
  const unsigned forced = get_forced (values, dst);
  update_large_reason (solver, assigned, forced, dst);
}

static void update_first_reducible (kissat *solver, const clause *end,
                                    clause *first_reducible) {
  if (first_reducible >= end) {
    LOG ("first reducible after end of arena");
    solver->first_reducible = INVALID_REF;
  } else if (first_reducible) {
    LOGCLS (first_reducible, "updating first reducible clause to");
    solver->first_reducible =
        kissat_reference_clause (solver, first_reducible);
  } else {
    LOG ("first reducible clause becomes invalid");
    solver->first_reducible = INVALID_REF;
  }
}

static void update_last_irredundant (kissat *solver, const clause *end,
                                     clause *last_irredundant) {
  if (!last_irredundant) {
    LOG ("no more large irredundant clauses left");
    solver->last_irredundant = INVALID_REF;
  } else if (end <= last_irredundant) {
    LOG ("last irredundant clause after end of arena");
    solver->last_irredundant = INVALID_REF;
  } else {
    LOGCLS (last_irredundant, "updating last irredundant clause to");
    reference ref = kissat_reference_clause (solver, last_irredundant);
    solver->last_irredundant = ref;
  }
}

void kissat_update_first_reducible (kissat *solver, clause *reducible) {
  assert (reducible);
  assert (!reducible->garbage);
  assert (reducible->redundant);
  if (solver->first_reducible != INVALID_REF) {
    reference ref = kissat_reference_clause (solver, reducible);
    if (ref >= solver->first_reducible) {
      LOG ("no need to update larger first reducible");
      return;
    }
  }
  clause *end = (clause *) END_STACK (solver->arena);
  update_first_reducible (solver, end, reducible);
}

void kissat_update_last_irredundant (kissat *solver, clause *irredundant) {
  assert (irredundant);
  assert (!irredundant->garbage);
  assert (!irredundant->redundant);
  if (solver->last_irredundant != INVALID_REF) {
    reference ref = kissat_reference_clause (solver, irredundant);
    if (ref <= solver->last_irredundant) {
      LOG ("no need to update smaller last irredundant");
      return;
    }
  }
  clause *end = (clause *) END_STACK (solver->arena);
  update_last_irredundant (solver, end, irredundant);
}

static void move_redundant_clauses_to_the_end (kissat *solver,
                                               reference ref) {
  INC (moved);
  assert (ref != INVALID_REF);
#ifndef NDEBUG
  const size_t size = SIZE_STACK (solver->arena);
  assert ((size_t) ref <= size);
#endif
  clause *begin = (clause *) (BEGIN_STACK (solver->arena) + ref);
  clause *end = (clause *) END_STACK (solver->arena);
  size_t bytes_redundant = (char *) end - (char *) begin;
  kissat_phase (solver, "move", GET (moved),
                "moving redundant clauses of %s to the end",
                FORMAT_BYTES (bytes_redundant));
  kissat_mark_reason_clauses (solver, ref);
  clause *redundant = (clause *) kissat_malloc (solver, bytes_redundant);
  clause *p = begin, *q = begin, *r = redundant;

  const value *const values = solver->values;
  assigned *assigned = solver->assigned;

  clause *last_irredundant = kissat_last_irredundant_clause (solver);

  while (p != end) {
    assert (!p->shrunken);
    size_t bytes = kissat_bytes_of_clause (p->size);
    if (p->redundant) {
      memcpy (r, p, bytes);
      r = (clause *) (bytes + (char *) r);
    } else {
      LOGCLS (p, "old DST");
      memmove (q, p, bytes);
      LOGCLS (q, "new DST");
      last_irredundant = q;
      if (q->reason)
        get_forced_and_update_large_reason (solver, assigned, values, q);
      q = (clause *) (bytes + (char *) q);
    }
    p = (clause *) (bytes + (char *) p);
  }
  r = redundant;
  clause *first_reducible = 0;
  while (q != end) {
    size_t bytes = kissat_bytes_of_clause (r->size);
    memcpy (q, r, bytes);
    LOGCLS (q, "new DST");
    if (q->reason)
      get_forced_and_update_large_reason (solver, assigned, values, q);
    assert (q->redundant);
    if (!first_reducible)
      first_reducible = q;
    r = (clause *) (bytes + (char *) r);
    q = (clause *) (bytes + (char *) q);
  }
  assert ((char *) r <= (char *) redundant + bytes_redundant);
  kissat_free (solver, redundant, bytes_redundant);

  assert (!first_reducible || first_reducible < q);

  update_first_reducible (solver, q, first_reducible);
  update_last_irredundant (solver, q, last_irredundant);
  kissat_reset_last_learned (solver);
}

static reference sparse_sweep_garbage_clauses (kissat *solver, bool compact,
                                               reference start) {
  assert (solver->watching);
  LOG ("sparse garbage collection starting at clause[%" REFERENCE_FORMAT
       "]",
       start);
#ifdef CHECKING_OR_PROVING
  const bool checking_or_proving = kissat_checking_or_proving (solver);
#endif
  assert (EMPTY_STACK (solver->added));
  assert (EMPTY_STACK (solver->removed));

  const value *const values = solver->values;
  assigned *assigned = solver->assigned;

#ifndef QUIET
  size_t flushed_garbage_clauses = 0;
  size_t flushed_satisfied_clauses = 0;
#endif
  size_t flushed = 0;

  clause *begin = (clause *) BEGIN_STACK (solver->arena);
  const clause *const end = (clause *) END_STACK (solver->arena);

  clause *first, *src, *dst;
  if (start)
    first = kissat_dereference_clause (solver, start);
  else
    first = begin;
  src = dst = first;

  clause *first_redundant = 0;
  clause *first_reducible = 0;
  clause *last_irredundant;

  if (start)
    last_irredundant = kissat_last_irredundant_clause (solver);
  else
    last_irredundant = 0;
#ifdef LOGGING
  size_t redundant_bytes = 0;
#endif
  for (clause *next; src != end; src = next) {
    if (src->garbage) {
      next = kissat_delete_clause (solver, src);
#ifndef QUIET
      flushed_garbage_clauses++;
#endif
      if (last_irredundant == src) {
        if (first == begin)
          last_irredundant = 0;
        else
          last_irredundant = first;
      }
      continue;
    }

    assert (src->size > 1);
    LOGCLS (src, "SRC");
    next = kissat_next_clause (src);
#if !defined(NDEBUG) || defined(CHECKING_OR_PROVING)
    const unsigned old_size = src->size;
#endif
    assert (SIZE_OF_CLAUSE_HEADER == sizeof (unsigned));
    *(unsigned *) dst = *(unsigned *) src;

    unsigned *q = dst->lits;

    unsigned mfirst = INVALID_LIT;
    unsigned msecond = INVALID_LIT;
    unsigned forced = INVALID_LIT;
    unsigned other = INVALID_LIT;
    unsigned non_false = 0;

    bool satisfied = false;

    for (all_literals_in_clause (lit, src)) {
#ifdef CHECKING_OR_PROVING
      if (checking_or_proving)
        PUSH_STACK (solver->removed, lit);
#endif
      if (satisfied)
        continue;

      const value tmp = values[lit];
      const unsigned idx = IDX (lit);
      const unsigned level = tmp ? assigned[idx].level : INVALID_LEVEL;

      if (tmp < 0 && !level)
        flushed++;
      else if (tmp > 0 && !level) {
        assert (!satisfied);
        assert (!dst->reason);
        LOG ("SRC satisfied by %s", LOGLIT (lit));
        satisfied = true;
      } else {
        const unsigned mlit = kissat_map_literal (solver, lit, compact);

        if (tmp > 0) {
          assert (level);
          forced = non_false++ ? INVALID_LIT : lit;
        } else if (tmp < 0)
          other = lit;

        if (mfirst == INVALID_LIT)
          mfirst = mlit;
        else if (msecond == INVALID_LIT)
          msecond = mlit;

        *q++ = mlit;

#ifdef CHECKING_OR_PROVING
        if (checking_or_proving)
          PUSH_STACK (solver->added, lit);
#endif
      }
    }

    if (satisfied) {
      if (dst->redundant)
        DEC (clauses_redundant);
      else
        DEC (clauses_irredundant);
#ifndef QUIET
      flushed_satisfied_clauses++;
#endif
#ifdef CHECKING_OR_PROVING
      if (checking_or_proving) {
        REMOVE_CHECKER_STACK (solver->removed);
        DELETE_STACK_FROM_PROOF (solver->removed);
        CLEAR_STACK (solver->added);
        CLEAR_STACK (solver->removed);
      }
#endif
      if (last_irredundant == src) {
        if (first == begin)
          last_irredundant = 0;
        else
          last_irredundant = first;
      }
      continue;
    }

    const unsigned new_size = q - dst->lits;
    assert (new_size <= old_size);
    assert (1 < new_size);

    if (new_size == 2) {
      assert (mfirst != INVALID_LIT);
      assert (msecond != INVALID_LIT);

      statistics *statistics = &solver->statistics;
      assert (statistics->clauses_binary < UINT64_MAX);
      statistics->clauses_binary++;
      bool redundant = dst->redundant;
      if (redundant) {
        assert (statistics->clauses_redundant > 0);
        statistics->clauses_redundant--;
        redundant = false;
      } else {
        assert (statistics->clauses_irredundant > 0);
        statistics->clauses_irredundant--;
      }
      LOGBINARY (mfirst, msecond, "DST");
      kissat_watch_binary (solver, mfirst, msecond);

      if (dst->reason) {
        assert (non_false == 1);
        assert (other != INVALID_LIT);
        assert (forced != INVALID_LIT);

        const unsigned forced_idx = IDX (forced);
        struct assigned *a = assigned + forced_idx;
        assert (!a->binary);

        LOGBINARY (mfirst, msecond,
                   "reason clause[%u] of %s updated to binary reason",
                   a->reason, LOGLIT (forced));

        a->binary = true;
        a->reason = other;
      }

      if (!redundant && last_irredundant == src) {
        if (first == begin)
          last_irredundant = 0;
        else
          last_irredundant = first;
      }
    } else {
      assert (2 < new_size);

      dst->size = new_size;
      dst->shrunken = false;
      dst->searched = 2;

      LOGCLS (dst, "DST");
      if (dst->reason)
        update_large_reason (solver, assigned, forced, dst);

      clause *next_dst = kissat_next_clause (dst);

      if (dst->redundant) {
        if (!first_reducible)
          first_reducible = dst;
#ifdef LOGGING
        redundant_bytes += (char *) next_dst - (char *) dst;
#endif
        if (!first_redundant)
          first_redundant = dst;
      } else
        last_irredundant = dst;

      dst = next_dst;
    }

#ifdef CHECKING_OR_PROVING
    if (!checking_or_proving)
      continue;

    if (new_size != old_size) {
      assert (1 < new_size);
      assert (new_size < old_size);

      CHECK_AND_ADD_STACK (solver->added);
      ADD_STACK_TO_PROOF (solver->added);

      REMOVE_CHECKER_STACK (solver->removed);
      DELETE_STACK_FROM_PROOF (solver->removed);
    }
    CLEAR_STACK (solver->added);
    CLEAR_STACK (solver->removed);
#endif
  }

  update_first_reducible (solver, dst, first_reducible);
  update_last_irredundant (solver, dst, last_irredundant);
  kissat_reset_last_learned (solver);

  if (first_redundant)
    LOGCLS (first_redundant, "determined first redundant clause as");

#if !defined(QUIET) || defined(METRICS)
  size_t bytes = (char *) END_STACK (solver->arena) - (char *) dst;
#endif
#ifndef QUIET
  if (flushed)
    kissat_phase (solver, "collect", GET (garbage_collections),
                  "flushed %zu falsified literals in large clauses",
                  flushed);
  size_t flushed_clauses =
      flushed_satisfied_clauses + flushed_garbage_clauses;
  if (flushed_satisfied_clauses)
    kissat_phase (
        solver, "collect", GET (garbage_collections),
        "flushed %zu satisfied large clauses %.0f%%",
        flushed_satisfied_clauses,
        kissat_percent (flushed_satisfied_clauses, flushed_clauses));
  if (flushed_garbage_clauses)
    kissat_phase (
        solver, "collect", GET (garbage_collections),
        "flushed %zu large garbage clauses %.0f%%", flushed_garbage_clauses,
        kissat_percent (flushed_garbage_clauses, flushed_clauses));
  kissat_phase (solver, "collect", GET (garbage_collections),
                "collected %s in total", FORMAT_BYTES (bytes));
#endif
  ADD (flushed, flushed);
#ifdef METRICS
  ADD (allocated_collected, bytes);
#endif

  reference res = INVALID_REF;

  if (first_redundant && last_irredundant &&
      first_redundant < last_irredundant) {
#ifdef LOGGING
    size_t move_bytes = (char *) dst - (char *) first_redundant;
    LOG ("redundant bytes %s (%.0f%%) out of %s moving bytes",
         FORMAT_BYTES (redundant_bytes),
         kissat_percent (redundant_bytes, move_bytes),
         FORMAT_BYTES (move_bytes));
#endif
    assert (first_redundant < dst);
    res = kissat_reference_clause (solver, first_redundant);
    assert (res != INVALID_REF);
  }

  SET_END_OF_STACK (solver->arena, (ward *) dst);
  kissat_shrink_arena (solver);

#ifdef METRICS
  if (solver->statistics.arena_garbage)
    kissat_very_verbose (solver, "still %s garbage left in arena",
                         FORMAT_BYTES (solver->statistics.arena_garbage));
  else
    kissat_very_verbose (solver, "all garbage clauses in arena collected");
#endif

  return res;
}

static void rewatch_clauses (kissat *solver, reference start) {
  LOG ("rewatching clause[%" REFERENCE_FORMAT "] and following clauses",
       start);
  assert (solver->watching);

  const value *const values = solver->values;
  const assigned *const assigned = solver->assigned;
  watches *watches = solver->watches;
  ward *const arena = BEGIN_STACK (solver->arena);

  clause *end = (clause *) END_STACK (solver->arena);
  clause *c = (clause *) (BEGIN_STACK (solver->arena) + start);
  assert (c <= end);

  for (clause *next; c != end; c = next) {
    next = kissat_next_clause (c);

    unsigned *lits = c->lits;
    kissat_sort_literals (solver, values, assigned, c->size, lits);
    c->searched = 2;

    const reference ref = (ward *) c - arena;
    const unsigned l0 = lits[0];
    const unsigned l1 = lits[1];

    kissat_push_blocking_watch (solver, watches + l0, l1, ref);
    kissat_push_blocking_watch (solver, watches + l1, l0, ref);
  }
}



static double compute_utility_score (kissat *solver, clause_utility_t *util) {
  uint64_t age = CONFLICTS - util->created_at_conflict;
  if (age == 0) age = 1;
  
  double conflict_rate = (double)util->conflict_used / age;
  double propagation_rate = (double)util->propagation_checked / age;
  
  uint64_t recency = CONFLICTS - util->last_used_conflict;
  double recency_bonus = (recency < 1000) ? 1.0 : 
                        (recency < 10000) ? 0.5 : 0.1;
  
  double survival_bonus = 1.0 + 0.1 * util->gc_survived;
  
  double score = (conflict_rate * 10.0 + 
                 propagation_rate * 0.5 + 
                 recency_bonus) * survival_bonus;
  
  return score;
}

static double build_utility_histogram (kissat *solver, reference start, 
                                    reference end,
                                    utility_histogram_t *hist) {
  memset (hist, 0, sizeof (utility_histogram_t));
  
  hist->bucket_threshold[0] = 0.1;
  hist->bucket_threshold[1] = 0.5;
  hist->bucket_threshold[2] = 1.0;
  hist->bucket_threshold[3] = 2.0;
  hist->bucket_threshold[4] = 5.0;
  hist->bucket_threshold[5] = 10.0;
  hist->bucket_threshold[6] = 20.0;
  hist->bucket_threshold[7] = 50.0;
  hist->bucket_threshold[8] = 100.0;
  hist->bucket_threshold[9] = 1e100;
  
  double total_utility = 0;
  unsigned count = 0;
  
  for (reference ref = start; ref < end; ref++) {
    clause *c = (clause *)kissat_dereference_clause (solver, ref);
    if (c->garbage) continue;
    
    clause_utility_t *util = &solver->clause_utilities[ref];
    double score = compute_utility_score (solver, util);
    util->utility_score = score;
    
    total_utility += score;
    count++;
    
    for (int i = 0; i < 10; i++) {
      if (score < hist->bucket_threshold[i]) {
        hist->histogram[i]++;
        break;
      }
    }
  }
  
  if (count > 0) {
    return total_utility / count;
  }
  return 0.0;
}

static double select_utility_threshold (utility_histogram_t *hist,
                                       double target_keep_ratio) {
  unsigned total = 0;
  for (int i = 0; i < 10; i++) {
    total += hist->histogram[i];
  }
  
  if (total == 0) return 0.1;
  
  unsigned target_count = (unsigned)(total * target_keep_ratio);
  unsigned cumulative = 0;
  
  for (int i = 0; i < 10; i++) {
    cumulative += hist->histogram[i];
    if (cumulative >= target_count) {
      return hist->bucket_threshold[i];
    }
  }
  
  return hist->bucket_threshold[9];
}
// EVOLVE_START
// DUCB-GC: Dynamic Upper Confidence Bound Garbage Collection

// DUCB-GC: Retention decision structure
typedef struct retention_decision {
  double lower_bound;     // Pessimistic estimate
  double expected;        // Expected utility
  double upper_bound;     // Optimistic estimate
  bool keep_confident;    // High confidence decision
  bool discard_confident; // High confidence discard
} retention_decision_t;

// DUCB-GC: Pool health assessment
typedef enum {
  POOL_SCARCE,
  POOL_HEALTHY,
  POOL_ABUNDANT
} pool_health_t;

static pool_health_t assess_pool_health_ducb (kissat *solver) {
  uint64_t clause_count = IRREDUNDANT_CLAUSES + REDUNDANT_CLAUSES;
  uint64_t vars = solver->vars;
  
  uint64_t max_clauses = vars * 10;
  if (max_clauses == 0) return POOL_HEALTHY;
  
  double ratio = (double)clause_count / (double)max_clauses;
  
  if (ratio < 0.3) return POOL_SCARCE;
  if (ratio > 0.8) return POOL_ABUNDANT;
  return POOL_HEALTHY;
}

static void init_ducb_gc_state (kissat *solver) {
  ducb_gc_state_t *ducb = &solver->ducb_gc_state;
  
  ducb->ucb_exploration_constant = 1.0;
  ducb->confidence_multiplier = 1.5;
  
  ducb->clause_set_count = 0;
  ducb->mean_utility = 0.5;
  ducb->utility_variance = 0.25;
  ducb->sum_squared_rewards = 0.0;
  
  ducb->search_phase = 0;
  ducb->phase_confidence = 0.5;
  
  for (int i = 0; i < 256; i++) {
    ducb->ucb_scores[i] = 0.5;
  }
  ducb->ucb_history_idx = 0;
  
  ducb->total_collected = 0;
  ducb->total_kept = 0;
  ducb->collection_efficiency = 0.5;
  
  ducb->utility_threshold = 0.35;
  ducb->threshold_adjustment = 1.0;
}

// DUCB-GC: Compute clause utility (exploitation component)
static double compute_clause_utility_ducb (kissat *solver, clause_utility_t *util,
                                           unsigned clause_size, uint64_t age) {
  // Base utility from utility score
  double base_score = tanh (util->utility_score / 10.0);
  
  // Age factor: medium age is best
  double log_age = log1p ((double)age);
  double age_score = exp (-pow (log_age - 3.0, 2) / 4.0);
  
  // Survival bonus
  double survival_bonus = 1.0 + 0.15 * tanh ((double)util->gc_survived / 5.0);
  
  // Size efficiency
  double size_eff = 1.0 / (1.0 + log ((double)clause_size + 1.0));
  
  // Weighted combination
  double score = 0.4 * base_score + 0.2 * age_score * survival_bonus + 0.3 * size_eff + 0.1;
  
  return score;
}

// DUCB-GC: Compute UCB score for a clause
static double compute_ucb_score (kissat *solver, clause_utility_t *util,
                                unsigned clause_size, uint64_t age,
                                ducb_gc_state_t *state) {
  // Exploitation: expected utility
  double exploitation = compute_clause_utility_ducb (solver, util, clause_size, age);
  
  // Exploration: uncertainty bonus based on age and usage history
  double uncertainty = 0.0;
  
  // Older clauses have higher uncertainty (haven't been tested)
  if (util->last_used_conflict > 0) {
    double time_since_use = (double)age;
    uncertainty = log1p (time_since_use / 1000.0);
  } else {
    // Never used = maximum uncertainty
    uncertainty = 2.0;
  }
  
  // UCB formula: exploitation + exploration_constant * sqrt(uncertainty)
  double ucb = exploitation + state->ucb_exploration_constant * sqrt (uncertainty);
  
  return ucb;
}

// DUCB-GC: Compute adaptive exploration constant based on phase
static double compute_adaptive_exploration (ducb_gc_state_t *state, unsigned conflicts) {
  // Phase 0 (exploration): High exploration to discover good clauses
  if (state->search_phase == 0) {
    return 2.0;
  }
  
  // Phase 1 (exploitation): Moderate exploration
  if (state->search_phase == 1) {
    return 1.0;
  }
  
  // Phase 2 (refinement): Low exploration, focus on best clauses
  return 0.5;
}

// DUCB-GC: Detect current search phase
static void detect_search_phase (ducb_gc_state_t *state, unsigned conflicts,
                                 double collection_efficiency) {
  if (conflicts < 5000) {
    state->search_phase = 0;  // Exploration
  } else if (collection_efficiency > 0.6) {
    state->search_phase = 1;  // Exploitation
  } else if (collection_efficiency > 0.3) {
    state->search_phase = 2;  // Refinement
  } else {
    state->search_phase = 0;  // Back to exploration if too aggressive
  }
}

// DUCB-GC: Evaluate clause using confidence intervals
static retention_decision_t evaluate_clause_ucb (kissat *solver, clause *c,
                                                 clause_utility_t *util,
                                                 ducb_gc_state_t *state,
                                                 uint64_t age) {
  retention_decision_t decision;
  
  double base_utility = compute_clause_utility_ducb (solver, util, c->size, age);
  double uncertainty = state->confidence_multiplier * sqrt (log ((double)age + 1.0) / ((double)age + 1.0));
  
  decision.expected = base_utility;
  decision.lower_bound = base_utility - uncertainty;
  decision.upper_bound = base_utility + uncertainty;
  
  // High confidence decisions
  decision.keep_confident = (decision.upper_bound > 0.7);
  decision.discard_confident = (decision.lower_bound < 0.3);
  
  return decision;
}

// DUCB-GC: Get adaptive threshold
static double get_adaptive_threshold_ducb (ducb_gc_state_t *state) {
  double base_threshold;
  
  if (state->collection_efficiency > 0.7) {
    base_threshold = 0.38;
  } else if (state->collection_efficiency < 0.3) {
    base_threshold = 0.28;
  } else {
    base_threshold = 0.35;
  }
  
  // Adjust based on search phase
  if (state->search_phase == 0) {
    base_threshold *= 0.9;  // More aggressive in exploration
  } else if (state->search_phase == 2) {
    base_threshold *= 1.1;  // More selective in refinement
  }
  
  return base_threshold * state->threshold_adjustment;
}

// DUCB-GC: Update statistics after collection
static void update_ducb_statistics (ducb_gc_state_t *state, unsigned marked_garbage,
                                    unsigned considered_clauses) {
  if (considered_clauses == 0) return;
  
  double efficiency = (double)marked_garbage / considered_clauses;
  
  // Update running statistics
  state->clause_set_count += considered_clauses;
  
  // Update mean and variance
  double delta = efficiency - state->mean_utility;
  state->mean_utility += delta / state->clause_set_count;
  state->sum_squared_rewards += delta * (efficiency - state->mean_utility);
  
  if (state->clause_set_count > 1) {
    state->utility_variance = state->sum_squared_rewards / (state->clause_set_count - 1);
  }
  
  // Update collection efficiency
  state->collection_efficiency = efficiency;
  
  // Update UCB history
  state->ucb_scores[state->ucb_history_idx] = efficiency;
  state->ucb_history_idx = (state->ucb_history_idx + 1) % 256;
}

void kissat_ducb_collect (kissat *solver, bool compact, reference start) {
  assert (solver->watching);
  START (collect);
  INC (garbage_collections);
  INC (sparse_gcs);
  REPORT (1, 'G');
  
  unsigned vars, mfixed;
  if (compact)
    vars = kissat_compact_literals (solver, &mfixed);
  else {
    vars = solver->vars;
    mfixed = INVALID_LIT;
  }
  flush_all_watched_clauses (solver, compact, start);
  
  reference end_of_irredundant = solver->last_irredundant;
  
  unsigned marked_garbage = 0;
  unsigned considered_clauses = 0;
  
  if (solver->clause_utilities) {
    // Initialize DUCB-GC state if needed
    ducb_gc_state_t *ducb = &solver->ducb_gc_state;
    if (ducb->total_collected == 0 && ducb->total_kept == 0 &&
        ducb->mean_utility == 0.5) {
      init_ducb_gc_state (solver);
    }
    
    // Update phase detection
    detect_search_phase (ducb, CONFLICTS, ducb->collection_efficiency);
    
    // Update exploration constant
    ducb->ucb_exploration_constant = compute_adaptive_exploration (ducb, CONFLICTS);
    
    // Get base threshold
    double base_threshold = get_adaptive_threshold_ducb (ducb);
    
    gc_utility_stats_t *gc_stats = &solver->gc_utility_stats;
    gc_stats->utility_threshold = base_threshold;
    gc_stats->gc_cycles++;
    gc_stats->last_gc_conflicts = CONFLICTS;
    
    for (reference ref = start; ref < end_of_irredundant; ref++) {
      clause *c = (clause *)kissat_dereference_clause (solver, ref);
      if (c->garbage) continue;
      
      clause_utility_t *util = &solver->clause_utilities[ref];
      considered_clauses++;
      
      uint64_t age = CONFLICTS - util->last_used_conflict;
      
      // Compute UCB score
      double ucb_score = compute_ucb_score (solver, util, c->size, age, ducb);
      
      // Get confidence interval decision
      retention_decision_t decision = evaluate_clause_ucb (solver, c, util, ducb, age);
      
      bool keep = false;
      
      // Immediate decisions based on confidence
      if (decision.keep_confident) {
        keep = true;
      } else if (decision.discard_confident) {
        keep = false;
      } else {
        // Marginal case: use UCB score
        keep = (ucb_score > base_threshold);
      }
      
      if (!keep) {
        c->garbage = true;
        marked_garbage++;
        ducb->total_collected++;
      } else {
        util->gc_survived++;
        ducb->total_kept++;
      }
    }
    
    // Update statistics
    update_ducb_statistics (ducb, marked_garbage, considered_clauses);
  }
  
  reference move = sparse_sweep_garbage_clauses (solver, compact, start);
  if (compact)
    kissat_finalize_compacting (solver, vars, mfixed);
  if (move != INVALID_REF)
    move_redundant_clauses_to_the_end (solver, move);
  rewatch_clauses (solver, start);
  REPORT (1, 'C');
  kissat_check_statistics (solver);
  STOP (collect);
  
  if (solver->clause_utilities) {
    kissat_very_verbose (solver,
        "DUCB-GC: marked %u/%u garbage (efficiency %.2f phase %u)", 
        marked_garbage, considered_clauses, 
        solver->ducb_gc_state.collection_efficiency,
        solver->ducb_gc_state.search_phase);
  }
}

// UCLM: Unified Clause Lifecycle Manager - Enhanced with Thompson Sampling and Cross-Component Reward Sharing
// EVOLVE: UCLM - Thompson Sampling with Gradient Descent

// UCLM: Feature weights for Q-function (8 features)
#define UCLM_FEATURE_DIM 8
#define UCLM_ACTION_GC 0
#define UCLM_ACTION_REDUCE 1
#define UCLM_ACTION_RETAIN 2
#define UCLM_NUM_ACTIONS 3

// UCLM: State structure is defined in internal.h

// UCLM: Initialize UCLM state
static void init_uclm_state (kissat *solver) {
  uclm_state_t *uclm = &solver->uclm_state;
  
  // Initialize weights to uniform values
  for (int a = 0; a < UCLM_NUM_ACTIONS; a++) {
    for (int i = 0; i < UCLM_FEATURE_DIM; i++) {
      uclm->weights[a][i] = 1.0 / UCLM_FEATURE_DIM;
      uclm->posterior_mean[a][i] = 0.0;
      uclm->posterior_std[i] = 1.0;
    }
  }
  
  // Initialize cross-component rewards
  uclm->gc_efficiency = 0.5;
  uclm->reduction_efficiency = 0.5;
  uclm->search_progress = 0.0;
  
  // Learning parameters
  uclm->learning_rate = 0.01;
  uclm->exploration_noise = 0.1;
  
  // Initial primary action
  uclm->primary_action = UCLM_ACTION_RETAIN;
  uclm->pool_health = POOL_HEALTHY;
  
  // Statistics
  uclm->total_decisions = 0;
  uclm->cumulative_reward = 0.0;
  uclm->update_count = 0;
}

// UCLM: Extract features from clause for Q-function
// Features: f1=clause_size (log), f2=age, f3=glue normalized, f4=tier, f5=redundant, f6=usage_count, f7=LBD_trend, f8=activity
static void extract_clause_features (kissat *solver, clause *c, clause_utility_t *util,
                                      uint64_t age, double *features) {
  // f1: Clause size (normalized logarithmically)
  features[0] = log ((double)c->size + 1.0) / 5.0;
  
  // f2: Clause age in conflicts (normalized)
  features[1] = tanh ((double)age / 10000.0);
  
  // f3: Glue value (normalized, 0-1)
  features[2] = (double)c->glue / 255.0;
  
  // f4: Tier (1, 2, or 3 -> 0.33, 0.66, 1.0)
  if (c->glue <= solver->tier1[solver->stable]) {
    features[3] = 0.33;
  } else if (c->glue <= solver->tier2[solver->stable]) {
    features[3] = 0.66;
  } else {
    features[3] = 1.0;
  }
  
  // f5: Redundant flag (binary)
  features[4] = c->redundant ? 1.0 : 0.0;
  
  // f6: Usage count since creation (normalized) - use propagation_count
  features[5] = tanh ((double)util->propagation_count + 1.0) / 10.0;
  
  // f7: LBD trend (from utility) - use conflict_used as proxy
  features[6] = tanh ((double)util->conflict_used / 1000.0);
  
  // f8: Activity score normalized
  features[7] = tanh (util->utility_score / 100.0);
}

// UCLM: Compute Q-value for a state-action pair
static double compute_uclm_q_value (uclm_state_t *uclm, double *features, int action) {
  double q = 0.0;
  for (int i = 0; i < UCLM_FEATURE_DIM; i++) {
    q += uclm->weights[action][i] * features[i];
  }
  return q;
}

// UCLM: Thompson Sampling action selection
static int uclm_select_action_thompson (kissat *solver, double *features, pool_health_t pool_health) {
  uclm_state_t *uclm = &solver->uclm_state;
  
  // Level 1: Use pool health for primary action decision
  if (pool_health == POOL_ABUNDANT) {
    uclm->primary_action = UCLM_ACTION_GC;
  } else if (pool_health == POOL_SCARCE) {
    uclm->primary_action = UCLM_ACTION_RETAIN;
  } else {
    // Thompson Sampling for learned policy
    double sampled_q[UCLM_NUM_ACTIONS];
    double noise = uclm->exploration_noise;
    
    for (int a = 0; a < UCLM_NUM_ACTIONS; a++) {
      // Sample from posterior
      double theta_a = 0.0;
      for (int i = 0; i < UCLM_FEATURE_DIM; i++) {
        // Simplified Thompson Sampling: mean + noise * std
        double noise_sample = (((double)rand () / RAND_MAX) - 0.5) * 2.0 * noise;
        theta_a += (uclm->posterior_mean[a][i] + noise_sample * uclm->posterior_std[i]) * features[i];
      }
      sampled_q[a] = theta_a;
    }
    
    // Select action with highest sampled Q-value
    int best_action = 0;
    double best_q = sampled_q[0];
    for (int a = 1; a < UCLM_NUM_ACTIONS; a++) {
      if (sampled_q[a] > best_q) {
        best_q = sampled_q[a];
        best_action = a;
      }
    }
    uclm->primary_action = best_action;
  }
  
  return uclm->primary_action;
}

// UCLM: Update weights using exponentiated gradient descent
static void uclm_update_weights (uclm_state_t *uclm, double reward, double *features, int chosen_action) {
  // Compute prediction error
  double predicted_q = compute_uclm_q_value (uclm, features, chosen_action);
  double error = reward - predicted_q;
  
  // Find best action for update direction
  int best_action = 0;
  double best_q = compute_uclm_q_value (uclm, features, 0);
  for (int a = 1; a < UCLM_NUM_ACTIONS; a++) {
    double q = compute_uclm_q_value (uclm, features, a);
    if (q > best_q) {
      best_q = q;
      best_action = a;
    }
  }
  
  // Exponentiated gradient update
  double lr = uclm->learning_rate;
  for (int i = 0; i < UCLM_FEATURE_DIM; i++) {
    if (chosen_action == best_action) {
      uclm->weights[chosen_action][i] *= exp (lr * error * features[i]);
    } else {
      uclm->weights[chosen_action][i] *= exp (-lr * error * features[i]);
    }
  }
  
  // Normalize weights
  double sum = 0.0;
  for (int a = 0; a < UCLM_NUM_ACTIONS; a++) {
    for (int i = 0; i < UCLM_FEATURE_DIM; i++) {
      sum += uclm->weights[a][i];
    }
  }
  if (sum > 0) {
    for (int a = 0; a < UCLM_NUM_ACTIONS; a++) {
      for (int i = 0; i < UCLM_FEATURE_DIM; i++) {
        uclm->weights[a][i] /= sum;
      }
    }
  }
  
  // Update posterior mean
  for (int i = 0; i < UCLM_FEATURE_DIM; i++) {
    uclm->posterior_mean[chosen_action][i] += lr * error * features[i];
  }
  
  uclm->update_count++;
}

// UCLM: Compute cross-component reward
static double compute_uclm_reward (uclm_state_t *uclm, unsigned gc_decisions, 
                                    unsigned reduce_decisions, unsigned conflicts_delta) {
  // Weighted combination of components
  double alpha = 0.4;  // GC weight
  double beta = 0.3;   // Reduction weight  
  double gamma = 0.3;  // Search progress weight
  
  double gc_eff = gc_decisions > 0 ? (double)gc_decisions / 10.0 : 0.5;
  double red_eff = reduce_decisions > 0 ? (double)reduce_decisions / 10.0 : 0.5;
  double search_prog = tanh ((double)conflicts_delta / 1000.0);
  
  // Update running averages
  uclm->gc_efficiency = 0.9 * uclm->gc_efficiency + 0.1 * gc_eff;
  uclm->reduction_efficiency = 0.9 * uclm->reduction_efficiency + 0.1 * red_eff;
  uclm->search_progress = 0.9 * uclm->search_progress + 0.1 * search_prog;
  
  return alpha * uclm->gc_efficiency + beta * uclm->reduction_efficiency + gamma * uclm->search_progress;
}

// UCLM: Hierarchical decision with UCB for specific clauses
static double uclm_compute_clause_score (kissat *solver, clause *c, clause_utility_t *util,
                                          uint64_t age, uclm_state_t *uclm, int primary_action) {
  double features[UCLM_FEATURE_DIM];
  extract_clause_features (solver, c, util, age, features);
  
  // Compute UCB-style score based on primary action
  double exploitation = compute_uclm_q_value (uclm, features, primary_action);
  
  // Exploration bonus based on age uncertainty
  double uncertainty = log1p ((double)age / 1000.0);
  double exploration_bonus = uclm->exploration_noise * sqrt (uncertainty);
  
  return exploitation + exploration_bonus;
}

// UCLM: Enhanced collect function with unified lifecycle management
void kissat_uclm_collect (kissat *solver, bool compact, reference start) {
  assert (solver->watching);
  START (collect);
  INC (garbage_collections);
  INC (sparse_gcs);
  REPORT (1, 'G');
  
  unsigned vars, mfixed;
  if (compact)
    vars = kissat_compact_literals (solver, &mfixed);
  else {
    vars = solver->vars;
    mfixed = INVALID_LIT;
  }
  flush_all_watched_clauses (solver, compact, start);
  
  reference end_of_irredundant = solver->last_irredundant;
  
  unsigned marked_garbage = 0;
  unsigned considered_clauses = 0;
  
  if (solver->clause_utilities) {
    // Initialize UCLM state if needed
    uclm_state_t *uclm = &solver->uclm_state;
    if (uclm->total_decisions == 0 && uclm->cumulative_reward == 0.0) {
      init_uclm_state (solver);
    }
    
    // Assess pool health for hierarchical decision
    pool_health_t pool_health = assess_pool_health_ducb (solver);
    uclm->pool_health = pool_health;
    
    // Get base threshold from DUCB-GC state
    ducb_gc_state_t *ducb = &solver->ducb_gc_state;
    detect_search_phase (ducb, CONFLICTS, ducb->collection_efficiency);
    ducb->ucb_exploration_constant = compute_adaptive_exploration (ducb, CONFLICTS);
    double base_threshold = get_adaptive_threshold_ducb (ducb);
    
    gc_utility_stats_t *gc_stats = &solver->gc_utility_stats;
    gc_stats->utility_threshold = base_threshold;
    gc_stats->gc_cycles++;
    gc_stats->last_gc_conflicts = CONFLICTS;
    
    unsigned last_conflicts = CONFLICTS;
    
    for (reference ref = start; ref < end_of_irredundant; ref++) {
      clause *c = (clause *)kissat_dereference_clause (solver, ref);
      if (c->garbage) continue;
      
      clause_utility_t *util = &solver->clause_utilities[ref];
      considered_clauses++;
      
      uint64_t age = CONFLICTS - util->last_used_conflict;
      
      // UCLM: Extract features and select action
      double features[UCLM_FEATURE_DIM];
      extract_clause_features (solver, c, util, age, features);
      
      int action = uclm_select_action_thompson (solver, features, pool_health);
      
      // Compute UCB score for the selected action
      double ucb_score = uclm_compute_clause_score (solver, c, util, age, uclm, action);
      
      // Get confidence interval decision from DUCB-GC
      retention_decision_t decision = evaluate_clause_ucb (solver, c, util, ducb, age);
      
      bool keep = false;
      
      // Use UCLM action decision
      if (action == UCLM_ACTION_GC) {
        // For GC action, use UCB score + confidence
        if (decision.discard_confident) {
          keep = false;
        } else {
          keep = (ucb_score > base_threshold * 0.9);
        }
      } else if (action == UCLM_ACTION_REDUCE) {
        // For reduce action, keep in tier but mark for reduction queue
        keep = true;
        // Could add to reduction queue here if needed
      } else {
        // For retain action, use confidence intervals
        if (decision.keep_confident) {
          keep = true;
        } else if (decision.discard_confident) {
          keep = false;
        } else {
          keep = (ucb_score > base_threshold);
        }
      }
      
      if (!keep) {
        c->garbage = true;
        marked_garbage++;
        ducb->total_collected++;
      } else {
        util->gc_survived++;
        ducb->total_kept++;
      }
      
      uclm->total_decisions++;
    }
    
    // Compute reward and update weights
    unsigned conflicts_delta = (unsigned)(CONFLICTS - last_conflicts);
    double reward = compute_uclm_reward (uclm, marked_garbage, 0, conflicts_delta);
    uclm->cumulative_reward += reward;
    
    // Update statistics in DUCB-GC state
    update_ducb_statistics (ducb, marked_garbage, considered_clauses);
  }
  
  reference move = sparse_sweep_garbage_clauses (solver, compact, start);
  if (compact)
    kissat_finalize_compacting (solver, vars, mfixed);
  if (move != INVALID_REF)
    move_redundant_clauses_to_the_end (solver, move);
  rewatch_clauses (solver, start);
  REPORT (1, 'C');
  kissat_check_statistics (solver);
  STOP (collect);
  
  if (solver->clause_utilities) {
    uclm_state_t *uclm = &solver->uclm_state;
    kissat_very_verbose (solver,
        "UCLM: marked %u/%u garbage (efficiency %.2f action=%u reward=%.3f)", 
        marked_garbage, considered_clauses, 
        uclm->gc_efficiency,
        uclm->primary_action,
        uclm->cumulative_reward / (uclm->total_decisions + 1));
  }
}

// UCLM: Wrapper function to replace PUGC-Adaptive
void kissat_sparse_collect_pugc (kissat *solver, bool compact, reference start) {
  kissat_uclm_collect (solver, compact, start);
}
// EVOLVE_END
bool kissat_compacting (kissat *solver) {
  if (!GET_OPTION (compact))
    return false;
  unsigned inactive = solver->vars - solver->active;
  unsigned limit = GET_OPTION (compactlim) / 1e2 * solver->vars;
  bool compact = (inactive > limit);
  LOG ("%u inactive variables %.0f%% <= limit %u %.0f%%", inactive,
       kissat_percent (inactive, solver->vars), limit,
       kissat_percent (limit, solver->vars));
  return compact;
}

void kissat_sparse_collect (kissat *solver, bool compact, reference start) {
  kissat_sparse_collect_pugc (solver, compact, start);
}

void kissat_initial_sparse_collect (kissat *solver) {
  assert (!solver->level);
  assert (!solver->inconsistent);
  assert (solver->watching);
  assert (kissat_trail_flushed (solver));
  if (solver->statistics.units) {
    bool compact = GET_OPTION (compact);
    kissat_sparse_collect (solver, compact, 0);
  }
  REPORT (0, '.');
}

static void dense_sweep_garbage_clauses (kissat *solver) {
  assert (!solver->level);
  assert (!solver->watching);

  LOG ("dense garbage collection");

#ifndef QUIET
  size_t flushed_garbage_clauses = 0;
#endif
  clause *first_reducible = 0;
  clause *last_irredundant = 0;

  clause *begin = (clause *) BEGIN_STACK (solver->arena);
  const clause *const end = (clause *) END_STACK (solver->arena);

  clause *src = begin;
  clause *dst = src;

  for (clause *next; src != end; src = next) {
    if (src->garbage) {
      next = kissat_delete_clause (solver, src);
#ifndef QUIET
      flushed_garbage_clauses++;
#endif
      continue;
    }
    assert (src->size > 1);
    LOGCLS (src, "SRC");
    next = kissat_next_clause (src);
    assert (SIZE_OF_CLAUSE_HEADER == sizeof (unsigned));
    *(unsigned *) dst = *(unsigned *) src;
    dst->searched = src->searched;
    dst->size = src->size;
    dst->shrunken = false;
    memmove (dst->lits, src->lits, src->size * sizeof (unsigned));
    LOGCLS (dst, "DST");
    if (!dst->redundant)
      last_irredundant = dst;
    else if (!first_reducible)
      first_reducible = dst;
    dst = kissat_next_clause (dst);
  }

  update_first_reducible (solver, dst, first_reducible);
  update_last_irredundant (solver, dst, last_irredundant);
  kissat_reset_last_learned (solver);

#if !defined(QUIET) || defined(METRICS)
  size_t bytes = (char *) END_STACK (solver->arena) - (char *) dst;
#endif
  kissat_phase (solver, "collect", GET (garbage_collections),
                "flushed %zu large garbage clauses",
                flushed_garbage_clauses);
  kissat_phase (solver, "collect", GET (garbage_collections),
                "collected %s in total", FORMAT_BYTES (bytes));
#ifdef METRICS
  ADD (allocated_collected, bytes);
#endif

  SET_END_OF_STACK (solver->arena, (ward *) dst);
  kissat_shrink_arena (solver);

#ifdef METRICS
  if (solver->statistics.arena_garbage)
    kissat_very_verbose (solver, "still %s garbage left in arena",
                         FORMAT_BYTES (solver->statistics.arena_garbage));
  else
    kissat_very_verbose (solver, "all garbage clauses in arena collected");
#endif
}

void kissat_dense_collect (kissat *solver) {
  assert (!solver->watching);
  assert (!solver->level);
  START (collect);
  INC (garbage_collections);
  INC (dense_garbage_collections);
  REPORT (1, 'G');
  dense_sweep_garbage_clauses (solver);
  REPORT (1, 'C');
  STOP (collect);
}
