#ifndef _simplify_h_INCLUDED
#define _simplify_h_INCLUDED
#include <stdbool.h>
#include "file.h"
#include "cvec.h"
#include "internal.h"
#include <stdarg.h>
typedef struct simplify simplify;
typedef long long LL;
struct simplify {
    int vars;
    int clauses;
    int real_clauses;
    int **clause;
    int *clause_size;
    int *clause_delete;
    int *seen;
    int **occurp;
    int **occurn;
    int *occurp_size;
    int *occurn_size;
    int *queue;
    int *known;
    int known_size;
    int *varval;
    int *resseen;
    int cards;
    int **card_one;
    int *card_one_size;
    int M_card;
    cvec *store_clause;
    kissat *solver;
    longlongs ineq;
    int pbcounter;
    int * buf;
    int buf_siz;
	int buf_len;
	long long proof_len;
};


struct kissat;

bool kissat_simplify(struct kissat *solver, int *maxvar, file *file);
void kissat_complete_val(struct kissat *solver);

void add_lits_tmp(simplify * S, proof * proof, int count, ...);
void del_lits(simplify * S, proof * proof, int count, ...);

#endif
