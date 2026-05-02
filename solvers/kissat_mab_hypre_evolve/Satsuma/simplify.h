#ifndef _simplify_h_INCLUDED
#define _simplify_h_INCLUDED
#include "cvec.h"
#include <vector>

struct simplify {
    int vars;
    int clauses;
    int real_clauses;
    int nlit;
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
    std::vector<long long> ineq;
    int pbcounter;
    FILE *fp;
};

#endif
