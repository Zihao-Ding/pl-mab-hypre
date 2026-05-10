#ifndef _bump_h_INCLUDED
#define _bump_h_INCLUDED

#include <stdbool.h>

struct kissat;

void kissat_bump_analyzed (struct kissat *);
void kissat_update_scores (struct kissat *);
void kissat_rescale_scores (struct kissat *);
void kissat_bump_variable (struct kissat *, unsigned idx);
void kissat_bump_score_increment (struct kissat *);

// CHB
void kissat_bump_chb (struct kissat *, unsigned idx, double multiplier);
void kissat_decay_chb (struct kissat *);
void kissat_update_conflicted_chb (struct kissat *);

// AW-VSIDS
void kissat_init_aw_vsids (struct kissat *);
void kissat_destroy_aw_vsids (struct kissat *);
void kissat_update_aw_vsids (struct kissat *, unsigned *lits, unsigned size, unsigned glue);
double kissat_compute_aw_vsids_score (struct kissat *, unsigned idx, unsigned clause_lbd);
unsigned kissat_select_variable_aw_vsids (struct kissat *);

#define MAX_SCORE 1e150

#endif
