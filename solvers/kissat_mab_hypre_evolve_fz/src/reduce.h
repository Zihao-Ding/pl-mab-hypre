#ifndef _reduce_h_INCLUDED
#define _reduce_h_INCLUDED

#include <stdbool.h>

struct kissat;

bool kissat_reducing (struct kissat *);
int kissat_reduce (struct kissat *);

// U-ARS
void kissat_init_uars (struct kissat *);
bool kissat_reducing_uars (struct kissat *);
bool kissat_should_reduce_uars (struct kissat *);
void kissat_post_reduction_analysis (struct kissat *, unsigned clauses_considered, unsigned clauses_removed);

#endif
