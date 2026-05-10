#include "classify.h"
#include "internal.h"
#include "print.h"
#include <math.h>

static void collect_formula_features (kissat *solver, formula_features_t *ff) {
  statistics *s = &solver->statistics;
  
  ff->clauses = s->clauses_binary + s->clauses_irredundant;
  ff->variables = solver->vars;
  ff->binary_clauses = s->clauses_binary;
  ff->irredundant_clauses = s->clauses_irredundant;
  ff->literals = 0;
  
  ff->avg_clause_size = 3.0;
  
  ff->binary_ratio = (ff->clauses > 0) ?
      (double)ff->binary_clauses / ff->clauses : 0;
  
  ff->clause_density = (ff->variables > 0) ?
      (double)ff->clauses / ff->variables : 0;
  
  ff->units = 0;
  ff->binaries_from_learned = 0;
}

static double compute_classification_confidence (formula_features_t *ff,
                                                temporal_classifier_t *tc) {
  double confidence = 0.5;
  
  if (ff->clauses > 10000) {
    confidence += 0.2;
  } else if (ff->clauses > 1000) {
    confidence += 0.1;
  }
  
  if (tc->snapshot_count >= 5) {
    confidence += 0.2;
  } else if (tc->snapshot_count >= 2) {
    confidence += 0.1;
  }
  
  confidence += tc->stability_score * 0.1;
  
  return fmin (confidence, 1.0);
}

static void classify_at_point (kissat *solver, formula_features_t *ff,
                              classification_snapshot_t *snap) {
  unsigned small_clauses_limit = GET_OPTION (smallclauses);
  
  snap->small = (ff->clauses <= small_clauses_limit);
  
  unsigned bigbigfraction = GET_OPTION (bigbigfraction);
  double percent = bigbigfraction / 1000.0;
  snap->bigbig = (ff->binary_ratio >= percent);
  
  double industrial_score = 0;
  double crypto_score = 0;
  double combo_score = 0;
  
  if (ff->clause_density > 5.0) {
    industrial_score += 0.3;
  } else if (ff->clause_density < 1.0) {
    crypto_score += 0.3;
  }
  
  if (ff->binary_ratio > 0.5) {
    industrial_score += 0.3;
  } else if (ff->binary_ratio < 0.1) {
    combo_score += 0.3;
  }
  
  if (ff->avg_clause_size > 4.0) {
    combo_score += 0.2;
  } else if (ff->avg_clause_size < 3.0) {
    crypto_score += 0.2;
  }
  
  if (ff->units > 0) {
    industrial_score += 0.2 * fmin (1.0, ff->units / 100.0);
  }
  
  if (industrial_score >= crypto_score && industrial_score >= combo_score) {
    snap->industrial = true;
    snap->crypto = false;
    snap->combinatorial = false;
  } else if (crypto_score >= industrial_score && crypto_score >= combo_score) {
    snap->industrial = false;
    snap->crypto = true;
    snap->combinatorial = false;
  } else {
    snap->industrial = false;
    snap->crypto = false;
    snap->combinatorial = true;
  }
  
  snap->timestamp = CONFLICTS;
  snap->confidence = 0.5;
}

static void update_temporal_classifier (kissat *solver,
                                       temporal_classifier_t *tc,
                                       formula_features_t *ff) {
  classification_snapshot_t *snap = &tc->snapshots[tc->snapshot_index];
  classify_at_point (solver, ff, snap);
  
  snap->confidence = compute_classification_confidence (ff, tc);
  
  tc->snapshot_index = (tc->snapshot_index + 1) % 10;
  if (tc->snapshot_count < 10) {
    tc->snapshot_count++;
  }
  
  if (tc->snapshot_count >= 2) {
    unsigned matches = 0;
    unsigned total = 0;
    
    for (unsigned i = 0; i < tc->snapshot_count; i++) {
      for (unsigned j = i + 1; j < tc->snapshot_count; j++) {
        if (tc->snapshots[i].small == tc->snapshots[j].small) matches++;
        if (tc->snapshots[i].bigbig == tc->snapshots[j].bigbig) matches++;
        total += 2;
      }
    }
    
    tc->stability_score = (double)matches / total;
  }
  
  tc->last_conflict_count = CONFLICTS;
}

static void aggregate_classifications (temporal_classifier_t *tc) {
  tc->final_small_votes = 0;
  tc->final_bigbig_votes = 0;
  tc->final_industrial_votes = 0;
  tc->final_crypto_votes = 0;
  tc->final_combinatorial_votes = 0;
  
  double total_weight = 0;
  
  for (unsigned i = 0; i < tc->snapshot_count; i++) {
    classification_snapshot_t *snap = &tc->snapshots[i];
    
    unsigned age = tc->snapshot_index - i;
    if (tc->snapshot_index < i) {
      age = 10 - i + tc->snapshot_index;
    }
    double recency_weight = 1.0 / (1.0 + age * 0.1);
    
    double weight = snap->confidence * recency_weight;
    
    if (snap->small) tc->final_small_votes++;
    if (snap->bigbig) tc->final_bigbig_votes++;
    if (snap->industrial) tc->final_industrial_votes++;
    if (snap->crypto) tc->final_crypto_votes++;
    if (snap->combinatorial) tc->final_combinatorial_votes++;
    
    total_weight += weight;
  }
  
  if (tc->snapshot_count > 0) {
    tc->final_small_votes = 
        (unsigned)((tc->final_small_votes / tc->snapshot_count) * 100);
    tc->final_bigbig_votes = 
        (unsigned)((tc->final_bigbig_votes / tc->snapshot_count) * 100);
  }
}

// EVOLVE_START
void kissat_classify (kissat *solver) {
  temporal_classifier_t *tc = &solver->temporal_classifier;
  formula_features_t ff;
  
  collect_formula_features (solver, &ff);
  
  uint64_t conflicts_since_last = CONFLICTS - tc->last_conflict_count;
  bool should_classify = false;
  
  if (tc->snapshot_count == 0) {
    should_classify = true;
  }
  else if (conflicts_since_last > 10000) {
    should_classify = true;
  }
  else if (tc->snapshot_count < 10 && CONFLICTS > tc->last_conflict_count + 5000) {
    should_classify = true;
  }
  
  if (should_classify) {
    update_temporal_classifier (solver, tc, &ff);
  }
  
  aggregate_classifications (tc);
  
  solver->classification.small = (tc->final_small_votes > 50);
  solver->classification.bigbig = (tc->final_bigbig_votes > 50);
  
  int problem_type = 0;
  if (tc->final_industrial_votes > tc->final_crypto_votes &&
      tc->final_industrial_votes > tc->final_combinatorial_votes) {
    problem_type = 0;
  } else if (tc->final_crypto_votes > tc->final_industrial_votes &&
             tc->final_crypto_votes > tc->final_combinatorial_votes) {
    problem_type = 1;
  } else {
    problem_type = 2;
  }
  
  kissat_very_verbose (
      solver, "temporal classification: small=%s bigbig=%s type=%d "
      "stability=%.2f snapshots=%u",
      solver->classification.small ? "true" : "false",
      solver->classification.bigbig ? "true" : "false",
      problem_type,
      tc->stability_score,
      tc->snapshot_count);
}
// EVOLVE_END