// Copyright 2025 Markus Anders
// This file is part of satsuma 1.2.
// See LICENSE for extended copyright information.

#ifndef SATSUMA_PARSER_H
#define SATSUMA_PARSER_H
#include "utility.h"
#include "cnf.h"
#include "cnf2wl.h"
#include "simplify.h"
#include "hashmap.h"
#include <string>
#include <charconv>
#include <initializer_list>

typedef long long ll;
const int MAX_CLAUSES = 1000000000;
void print_added_proof_line(simplify *S, std::vector<int>& line){
    if(S->fp){
        fprintf(S->fp, "red ");
        bool has_firstlit = false;
        int firstlit = 0;

        for(int elit: line){
            assert (elit);
            assert (elit != INT_MIN);
            unsigned eidx;
            fprintf(S->fp, "1 ");

            if(!has_firstlit) {
                has_firstlit = true;
                firstlit = elit;
            }

            if (elit < 0) {
                fprintf(S->fp, "~");
                eidx = -elit;
            } else
            eidx = elit;
            fprintf(S->fp, "x%d ", eidx);
        }
        fprintf(S->fp, ">= 1 : ");

        if(has_firstlit) {
            unsigned firstlit_idx;
            if (firstlit < 0)
                firstlit_idx = -firstlit;
            else
                firstlit_idx = firstlit;
            fprintf(S->fp, "x%d -> ", firstlit_idx);

            // to 0 if negative, to 1 if positive
            if (firstlit < 0) // if negative
                fprintf(S->fp, "0");
            else // if positive
                fprintf(S->fp, "1");
        }

        fprintf(S->fp, ";\n");
    }
    line.clear();
}


void add_lits_tmp(simplify* S, std::initializer_list<int> lits) {
    std::vector<int> line(lits);
    print_added_proof_line(S, line);
}

simplify *simplify_init()
{
    simplify *s = (simplify *)malloc(sizeof(simplify));
    return s;
}

bool simplify_store_clause(simplify *S, int v)
{
    if (v == 0)
    {
        S->real_clauses++;
        int sz = S->store_clause->sz;
        S->clause_size[S->real_clauses] = sz;
        S->clause[S->real_clauses] = (int *)malloc(sizeof(int) * sz);
        for (int i = 0; i < sz; i++)
            S->clause[S->real_clauses][i] = cvec_data(S->store_clause, i);
        cvec_clear(S->store_clause);
        if (!sz)
            return false;
    }
    else
        cvec_push(S->store_clause, v);
    return true;
}

void simplify_alloc(simplify *S, int vars, int clauses)
{
    S->vars = vars;
    S->clauses = clauses;

    S->real_clauses = 0;
    S->clause = (int **)malloc(sizeof(int *) * (clauses + 1));
    S->clause_size = (int *)malloc(sizeof(int) * (clauses + 1));
    S->varval = (int *)malloc(sizeof(int) * (vars + 1));
    S->queue = (int *)malloc(sizeof(int) * (vars + 1));
    S->known = (int *)malloc(sizeof(int) * (vars + 1));
    S->known_size = 0;

    S->occurp = (int **)calloc((vars + 1), sizeof(int *));
    S->occurn = (int **)calloc((vars + 1), sizeof(int *));
    S->occurp_size = (int *)malloc(sizeof(int) * (vars + 1));
    S->occurn_size = (int *)malloc(sizeof(int) * (vars + 1));

    S->seen = (int *)malloc(sizeof(int) * (2 * vars + 2));
    S->clause_delete = (int *)malloc(sizeof(int) * (clauses + 1));
    S->resseen = (int *)malloc(sizeof(int) * (2 * vars + 2));

    S->store_clause = cvec_init();
}

void simplify_release(simplify *S)
{
    free(S->varval);
    for (int i = 1; i <= S->vars; i++)
    {
        if (S->occurp[i] != NULL)
            free(S->occurp[i]);
        if (S->occurn[i] != NULL)
            free(S->occurn[i]);
    }
    for (int i = 1; i <= S->clauses; i++)
    {
        free(S->clause[i]);
    }
    free(S->clause);
    S->clause = NULL;
    free(S->clause_size);

    free(S->queue);
    free(S->known);
    free(S->occurp);
    free(S->occurn);
    free(S->occurn_size);
    free(S->occurp_size);
    free(S->seen);
    free(S->clause_delete);
    free(S->resseen);
    cvec_release(S->store_clause);
    if(S->fp) fclose(S->fp);
}

static void simplify_parse(simplify *S, std::string& filename, bool entered_file) {
    FILE* file = nullptr;
    if(entered_file) file = fopen(filename.c_str(), "r");
    else file = stdin;
    if(!file) terminate_with_error("could not open file '" + filename + "'");

    constexpr int line_buf_sz = 1024*8;
    char line_buffer[line_buf_sz];
    setvbuf(file, line_buffer, _IOFBF, line_buf_sz);
    satsuma_flockfile(file);

    bool reserved = false;
    const char* last_conversion = nullptr;

    int nv = 0;
    int nc = 0;

    int line_num = 0;

    char m;
    char*  buffer_pt;
    int    literal;
    char   buffer[24];
    std::vector<int> construct_clause;

    while ((m = satsuma_getc(file)) != EOF) {
        [[likely]]
        ++line_num;
        //const char m = line[0];
        switch (m) {
            // a clause
            [[likely]]
            case '-':
            case '0':
            case '1':
            case '2':
            case '3':
            case '4':
            case '5':
            case '6':
            case '7':
            case '8':
            case '9':
            {
                // not possible to continue without allocating the memory first
                if (!reserved) terminate_with_error("formula must begin with 'p' line");
                construct_clause.clear();

                // we've already read the first digit of the first literal
                buffer_pt = buffer+1;
                buffer[0] = m;
                for(;;) {
                    [[likely]]

                    // read next literal digit-by-digit
                    while ((m = satsuma_getc(file)) >= '-') [[likely]] *(buffer_pt++) = m;

                    // allow to eat additional whitespace
                    //if(buffer_pos == 0 && (m == ' ' || m == '\t')) continue;

                    // the pointer arithmetic to get this going is evil, but this function is amazingly fast
                    literal = 0;
                    last_conversion = std::from_chars(buffer, buffer_pt, literal).ptr;
                    simplify_store_clause(S, literal);
                    if (literal == 0) break; // either the clause ended, or an error in the conversion occurred
                    buffer_pt = buffer;
                }

                // check if error in conversion occurred
                if(last_conversion == buffer)
                    terminate_with_error("invalid conversion occured in line " +
                                          std::to_string(line_num) + ": '" + buffer +"'");
                break;
            }

            // the problem definition
            [[unlikely]]
            case 'p': {
                // eat 5 characters
                m = satsuma_getc(file); // == ' ' // should not be unsafe since getc will keep returning EOF once
                bool valid = (m == ' ' || m == '\t'); // reached
                m = satsuma_getc(file); // == 'c'
                valid = valid && (m == 'c' || m == 'C');
                m = satsuma_getc(file); // == 'n'
                valid = valid && (m == 'n' || m == 'N');
                m = satsuma_getc(file); // == 'f'
                valid = valid && (m == 'f' || m == 'F');
                m = satsuma_getc(file);
                valid = valid && (m == ' ' || m == '\t');

                // could not match up "p cnf "
                if (!valid) terminate_with_error("invalid problem definition not matching 'p cnf '");

                buffer_pt = buffer;
                while ((m = satsuma_getc(file)) >= '-') *(buffer_pt++) = m;
                last_conversion = std::from_chars(buffer, buffer_pt, nv).ptr;
                if(last_conversion == buffer)
                    terminate_with_error("could not parse integer in line " + std::to_string(line_num)
                                                                   + ": '" + std::string(buffer) + "'");

                buffer_pt = buffer;
                while ((m = satsuma_getc(file)) >= '-') *(buffer_pt++) = m;
                last_conversion = std::from_chars(buffer, buffer_pt, nc).ptr;
                if(last_conversion == buffer)
                    terminate_with_error("could not parse integer in line " + std::to_string(line_num)
                                         + ": '" + std::string(buffer) + "'");

                reserved = true;
                simplify_alloc(S, nv, nc);
                break;
            }

            [[unlikely]]
            case 'c': {
                while ((m = satsuma_getc(file)) != '\n' && m != '\r' && m != EOF);
                break;
            }

                // just eat whitespaces, carriage returns, and newlines
            [[unlikely]]
            case '\t':
            case ' ':
            case '\r':
            case '\n':
                break;

                // can not recognize, let's abort
            [[unlikely]]
            default: {
                terminate_with_error("can not parse line " + std::to_string(line_num));
                break;
            }
        }
    }

    satsuma_funlockfile(file);
    if(!reserved) terminate_with_error("file did not contain an instance");
    fclose(file);
}

static inline int pnsign(int x)
{
    return (x > 0 ? 1 : -1);
}
static inline int tolit(int x)
{
    if (x > 0)
        return x * 2;
    else
        return (-x) * 2 + 1;
}
static inline int toidx(int x)
{
    return (x & 1 ? -(x >> 1) : (x >> 1));
}
static inline ll mapv(simplify *S, int a, int b)
{
    return 1ll * a * S->nlit + (ll)b;
}

void update_var_clause_label(simplify *S)
{
    int id = 0;
    for (int i = 1; i <= S->clauses; i++)
    {
        if (S->clause_delete[i])
            continue;
        ++id;
        int l = S->clause_size[i];
        S->clause[id] = (int *)realloc(S->clause[id], sizeof(int) * l);
        S->clause_size[id] = l;
        for (int j = 0; j < l; j++)
            S->clause[id][j] = S->clause[i][j];
    }
    for (int i = id + 1; i <= S->clauses; i++)
        free(S->clause[i]);
    S->clauses = id;
}

bool simplify_resolution(simplify *S)
{
    memset(S->occurn_size + 1, 0, sizeof(int) * S->vars);
    memset(S->occurp_size + 1, 0, sizeof(int) * S->vars);
    memset(S->resseen + 1, 0, sizeof(int) * S->vars * 2);
    memset(S->clause_delete + 1, 0, sizeof(int) * S->clauses);
    for (int i = 1; i <= S->clauses; i++)
    {
        int l = S->clause_size[i];
        for (int j = 0; j < l; j++)
        {
            int x = S->clause[i][j];
            if (x > 0)
            {
                S->occurp_size[x]++;
            }
            else
            {
                S->occurn_size[-x]++;
            }
        }
    }
    for (int i = 1; i <= S->vars; i++)
    {
        if (S->occurp_size[i])
        {
            S->occurp[i] = (int *)realloc(S->occurp[i], sizeof(int) * S->occurp_size[i]);
        }
        if (S->occurn_size[i])
        {
            S->occurn[i] = (int *)realloc(S->occurn[i], sizeof(int) * S->occurn_size[i]);
        }
        S->occurp_size[i] = S->occurn_size[i] = 0;
    }
    for (int i = 1; i <= S->clauses; i++)
    {

        for (int j = 0; j < S->clause_size[i]; j++)
        {
            int v = S->clause[i][j];
            if (v > 0)
                S->occurp[v][S->occurp_size[v]++] = i;
            else
                S->occurn[-v][S->occurn_size[-v]++] = i;
        }
    }
    for (int i = 1; i <= S->vars; i++)
        if (S->occurn_size[i] == 0 && S->occurp_size[i] == 0)
            S->seen[i] = 1;

    return true;
}

bool simplify_easy_clause(simplify *S)
{
    memset(S->occurn_size + 1, 0, sizeof(int) * S->vars);
    memset(S->occurp_size + 1, 0, sizeof(int) * S->vars);
    memset(S->resseen + 1, 0, sizeof(int) * S->vars * 2);
    memset(S->varval + 1, 0, sizeof(int) * S->vars);
    memset(S->clause_delete + 1, 0, sizeof(int) * S->clauses);
    std::vector<int> line;
    for (int i = 1; i <= S->clauses; i++)
        S->clause_delete[i] = 0;
    int head = 1, tail = 0;
    for (int i = 1; i <= S->clauses; i++)
    {
        int l = S->clause_size[i], t = 0;
        for (int j = 0; j < l; j++)
        {
            int lit = 0;
            if (S->clause[i][j] > 0)
                lit = S->clause[i][j];
            else
                lit = S->vars - S->clause[i][j];
            if (S->resseen[lit] == i)
                continue;
            int neg = 0;
            if (lit > S->vars)
                neg = lit - S->vars;
            else
                neg = lit + S->vars;
            if (S->resseen[neg] == i)
            {
                S->clause_delete[i] = 1;
                break;
            }
            S->clause[i][t++] = S->clause[i][j];
            S->resseen[lit] = i;
        }
        if (S->clause_delete[i])
        {
            continue;
        }
        S->clause_size[i] = t;
        for (int j = 0; j < t; j++)
        {
            if (S->clause[i][j] > 0)
                S->occurp_size[S->clause[i][j]]++;
            else
                S->occurn_size[-S->clause[i][j]]++;
            if (t == 0 || t == 1 || t < l)
            {
                int var = S->clause[i][j];
                line.push_back(var);
            }
        }
        if (t == 0 || t == 1 || t < l)
        {
            print_added_proof_line(S, line);
        }
        if (t == 0)
        {
            return false;
        }
        if (t == 1)
        {
            int lit = S->clause[i][0];
            S->clause_delete[i] = 1;
            if (S->varval[abs(lit)])
            {
                if (S->varval[abs(lit)] == pnsign(lit))
                    continue;
                else
                    return false;
            }
            S->varval[abs(lit)] = pnsign(lit);
            S->queue[++tail] = abs(lit);
            S->known[S->known_size++] = lit;
        }
    }
    for (int i = 1; i <= S->vars; i++)
    {
        if (S->occurp_size[i])
        {
            S->occurp[i] = (int *)malloc(sizeof(int) * (S->occurp_size[i]));
        }
        if (S->occurn_size[i])
        {
            int *tmp = (int *)malloc(sizeof(int) * (S->occurn_size[i]));
            S->occurn[i] = tmp;
        }
        S->occurp_size[i] = S->occurn_size[i] = 0;
    }
    for (int i = 1; i <= S->clauses; i++)
    {
        if (S->clause_delete[i])
            continue;
        for (int j = 0; j < S->clause_size[i]; j++)
        {
            int v = S->clause[i][j];
            if (v > 0)
                S->occurp[v][S->occurp_size[v]++] = i;
            else
                S->occurn[-v][S->occurn_size[-v]++] = i;
        }
    }
    memset(S->resseen + 1, 0, sizeof(int) * S->vars * 2);
    while (head <= tail)
    {
        int x = S->queue[head++];
        if (S->varval[x] == 1)
        {
            for (int i = 0; i < S->occurp_size[x]; i++)
                S->clause_delete[S->occurp[x][i]] = 1;
            for (int i = 0; i < S->occurn_size[x]; i++)
            {
                int o = S->occurn[x][i], t = 0;
                if (S->clause_delete[o])
                    continue;
                for (int j = 0; j < S->clause_size[o]; j++)
                {
                    if (S->varval[abs(S->clause[o][j])] == pnsign(S->clause[o][j]))
                    {
                        S->clause_delete[o] = 1;
                        break;
                    }
                    if (S->varval[abs(S->clause[o][j])] == -pnsign(S->clause[o][j]))
                        continue;
                    S->clause[o][t++] = S->clause[o][j];
                }
                if (S->clause_delete[o])
                {
                    continue;
                }
                if (t == 0 || t == 1 || t < S->clause_size[o])
                {
                    for (int j = 0; j < t; j++)
                    {
                        int var = S->clause[o][j];
                        line.push_back(var);
                    }
                    print_added_proof_line(S, line);
                }
                S->clause_size[o] = t;
                if (t == 0)
                {
                    return false;
                }
                if (t == 1)
                {
                    int lit = S->clause[o][0];
                    S->clause_delete[o] = 1;
                    if (S->varval[abs(lit)])
                    {
                        if (S->varval[abs(lit)] == pnsign(lit))
                            continue;
                        else
                            return false;
                    }
                    S->varval[abs(lit)] = pnsign(lit);
                    S->queue[++tail] = abs(lit);
                    S->known[S->known_size++] = lit;
                }
            }
        }
        else
        {
            for (int i = 0; i < S->occurn_size[x]; i++)
                S->clause_delete[S->occurn[x][i]] = 1;
            for (int i = 0; i < S->occurp_size[x]; i++)
            {
                int o = S->occurp[x][i], t = 0;
                if (S->clause_delete[o])
                    continue;
                for (int j = 0; j < S->clause_size[o]; j++)
                {
                    if (S->varval[abs(S->clause[o][j])] == pnsign(S->clause[o][j]))
                    {
                        S->clause_delete[o] = 1;
                        break;
                    }
                    if (S->varval[abs(S->clause[o][j])] == -pnsign(S->clause[o][j]))
                        continue;
                    S->clause[o][t++] = S->clause[o][j];
                }
                if (S->clause_delete[o])
                {
                    continue;
                }
                if (t == 0 || t == 1 || t < S->clause_size[o])
                {
                    for (int j = 0; j < t; j++)
                    {
                        int var = S->clause[o][j];
                        line.push_back(var);    
                    }
                    print_added_proof_line(S, line);
                }
                S->clause_size[o] = t;
                if (t == 0)
                {
                    return false;
                }
                if (t == 1)
                {
                    int lit = S->clause[o][0];
                    S->clause_delete[o] = 1;
                    if (S->varval[abs(lit)])
                    {
                        if (S->varval[abs(lit)] == pnsign(lit))
                            continue;
                        else
                            return false;
                    }
                    S->varval[abs(lit)] = pnsign(lit);
                    S->queue[++tail] = abs(lit);
                    S->known[S->known_size++] = lit;
                }
            }
        }
    }
    update_var_clause_label(S); // keep all variables.
    return true;
}

int search_almost_one(simplify *S)
{
    HashMap *H = map_init(40000003);
    S->nlit = 2 * S->vars + 2;
    int **occur = (int **)calloc((S->nlit), sizeof(int *));
    int *occur_size = (int *)calloc((S->nlit), sizeof(int));
    for (int i = 1; i <= S->clauses; i++)
    {
        S->clause_delete[i] = 0;
        if (S->clause_size[i] != 2)
            continue;
        int x = tolit(S->clause[i][0]);
        int y = tolit(S->clause[i][1]);
        ll id1 = mapv(S, x, y);
        ll id2 = mapv(S, y, x);
        map_insert(H, id1, i);
        map_insert(H, id2, i);
        occur_size[x]++;
        occur_size[y]++;
    }
    for (int i = 2; i < S->nlit; i++)
    {
        if (occur_size[i])
            occur[i] = (int *)malloc(sizeof(int) * (occur_size[i]));
        occur_size[i] = S->seen[i] = 0;
    }
    for (int i = 1; i <= S->clauses; i++)
    {
        if (S->clause_size[i] != 2)
            continue;
        int x = tolit(S->clause[i][0]);
        int y = tolit(S->clause[i][1]);
        occur[x][occur_size[x]++] = y;
        occur[y][occur_size[y]++] = x;
    }
    S->cards = 0;
    cvec *nei = cvec_init();
    cvec *ino = cvec_init();
    long long tot_size = 0;
    for (int i = 2; i <= S->vars * 2 + 1; i++)
    {
        if (S->seen[i] || !occur_size[i])
            continue;
        S->seen[i] = 1;
        cvec_clear(nei);
        for (int j = 0; j < occur_size[i]; j++)
        {
            if (!S->seen[occur[i][j]])
            {
                cvec_push(nei, occur[i][j]);
            }
        }
        do
        {
            cvec_clear(ino);
            cvec_push(ino, i);
            for (int j = 0; j < nei->sz; j++)
            {
                int v = cvec_data(nei, j), flag = 1;
                for (int k = 0; k < ino->sz; k++)
                {
                    ll id = mapv(S, v, cvec_data(ino, k));
                    int d1 = map_get(H, id, 0);
                    if (!d1)
                    {
                        flag = 0;
                        break;
                    }
                    S->queue[k] = d1;
                }
                if (flag)
                {
                    for (int k = 0; k < ino->sz; k++)
                    {
                        S->clause_delete[S->queue[k]] = 1;
                        ll id1 = mapv(S, v, cvec_data(ino, k)), id2 = mapv(S, cvec_data(ino, k), v);
                        map_delete(H, id1);
                        map_delete(H, id2);
                    }
                    cvec_push(ino, v);
                }
            }
            if (ino->sz >= 2)
            {

                S->card_one[S->cards] = (int *)malloc(sizeof(int) * (ino->sz));
                S->card_one_size[S->cards] = 0;
                for (int j = 0; j < ino->sz; j++)
                {
                    S->card_one[S->cards][S->card_one_size[S->cards]++] = -toidx(cvec_data(ino, j));
                }
                S->cards++;
                tot_size += ino->sz;
                if (S->cards >= S->M_card || tot_size >= 10000000)
                {
                    cvec_release(ino);
                    cvec_release(nei);
                    map_free(H);
                    free(occur_size);
                    for (int i = 0; i < S->nlit; i++)
                        if (occur[i] != NULL)
                            free(occur[i]);

                    free(occur);
                    return 0;
                }
            }
        } while (ino->sz != 1);
    }
    cvec_release(ino);
    cvec_release(nei);
    map_free(H);
    free(occur_size);
    for (int i = 0; i < S->nlit; i++)
    {
        if (occur[i] != NULL)
        {
            free(occur[i]);
        }
    }
    free(occur);
    return S->cards;
}

int dfs(simplify *S, int v, int *vst, int *match, int **e)
{
    for (int i = 0; i < S->clause_size[v]; i++)
    {
        int y = e[v][i];
        if (!vst[y])
        {
            vst[y] = 1;
            if (match[y] == -1 || dfs(S, match[y], vst, match, e))
            {
                match[y] = v;
                return 1;
            }
        }
    }
    return 0;
}
void dfs_mark(simplify *S, int v, int *vst, int *match, int **e, std::vector<unsigned>& le, std::vector<unsigned>& ri)
{
    le.push_back(v);
    for (int i = 0; i < S->clause_size[v]; i++)
    {
        int y = e[v][i];
        if (!vst[y])
        {
            ri.push_back(y);
            vst[y] = 1;
            dfs_mark(S, match[y], vst, match, e, le, ri);
        }
    }
}
static inline bool lequ(unsigned a, unsigned b)
{
    return a < b;
}
enum
{
    GE,
    LE
};

void print_inequality(simplify *S, int type, long long b)
{
    std::vector<long long>& l = S->ineq;
    if(S->fp){
        fprintf(S->fp, "rup");
        for (unsigned i = 0; i < l.size(); i += 2)
        {
            fprintf(S->fp, " %lld ", l[i + 1]);
            int lit = l[i];

            if (lit < 0)
                fprintf(S->fp, "~");
            fprintf(S->fp, "x%d", abs(lit));
        }
        if (type == GE)
        {
            fprintf(S->fp, " >= ");
        }
        else
        {
            fprintf(S->fp, " <= ");
        }
        fprintf(S->fp, "%lld ;\n", b);
    }
    l.clear();
}

void print_pol(simplify *S)
{
    std::vector<long long>& l = S->ineq;
    if(S->fp){
        fprintf(S->fp, "pol %lld", l[0]);

        for (unsigned i = 1; i < l.size(); i += 2)
        {
            fprintf(S->fp, " %lld %c" , l[i], l[i + 1]);
        }
        fprintf(S->fp, ";\n");
    }
    l.clear();
}
void generate_amo_proof(simplify *S, unsigned id)
{
    int len = S->card_one_size[id];
    int *labels = (int *)malloc(sizeof(int) * len * len);
    for (int delta = 1; delta < len; delta++)
    {
        for (int i = 0; i + delta < len; i++)
        {
            int j = i + delta;
            if (j == i + 1)
            {
                S->ineq.push_back(S->card_one[id][i]);
                S->ineq.push_back(-1);
                S->ineq.push_back(S->card_one[id][j]);
                S->ineq.push_back(-1);
                print_inequality(S, GE, -1);
                labels[i * len + j] = ++S->pbcounter;
            }
            else
            {
                S->ineq.push_back(S->card_one[id][i]);
                S->ineq.push_back(-1);
                S->ineq.push_back(S->card_one[id][j]);
                S->ineq.push_back(-1);
                print_inequality(S, GE, -1);
                ++S->pbcounter;
                S->ineq.push_back(labels[i * len + j - 1] - S->pbcounter - 1);
                S->ineq.push_back(labels[(i + 1) * len + j] - S->pbcounter - 1);
                S->ineq.push_back('+');
                S->ineq.push_back(-1);
                S->ineq.push_back('+');
                S->ineq.push_back(2);
                S->ineq.push_back('d');
                print_pol(S);
                labels[i * len + j] = ++S->pbcounter;
            }
        }
    }
}

int bipartite_check(simplify *S)
{
    int *ey = (int *)malloc(sizeof(int) * (S->vars + 1));
    int *sgn = (int *)malloc(sizeof(int) * (S->vars + 1));
    int *baksgn = (int *)malloc(sizeof(int) * (S->vars + 1));
    int *mark = (int *)malloc(sizeof(int) * (S->clauses + 1));
    int **e = (int **)malloc(sizeof(int *) * (S->clauses + 1));
    memset(e, 0, sizeof(int *) * (S->clauses + 1));
    memset(ey, 0, sizeof(int) * (S->vars + 1));
    memset(sgn, 0, sizeof(int) * (S->vars + 1));
    memset(mark, 0, sizeof(int) * (S->clauses + 1));
    long long cntvar = 0, cntcls = 0;
    for (int i = 0; i < S->cards; i++)
    {
        bool used = false;
        for (int j = 0; j < S->card_one_size[i]; j++)
        {
            if (ey[abs(S->card_one[i][j])])
            {
                used = true;
                break;
            }
        }
        if (!used)
        {
            for (int j = 0; j < S->card_one_size[i]; j++)
            {
                ey[abs(S->card_one[i][j])] = i;
                sgn[abs(S->card_one[i][j])] = S->card_one[i][j];
                cntvar++;
            }
            cntcls += (S->card_one_size[i]) * (S->card_one_size[i] - 1ll) / 2;
        }
    }
    int delta = 0;
    for (int i = 1; i <= S->vars; i++)
    {
        if (sgn[i] == 0)
        {
            sgn[i] = S->vars + 1;
            ey[i] = S->cards;
            S->cards++;
            delta++;
        }
    }
    memcpy(baksgn, sgn, sizeof(int) * (S->vars + 1));
    for (int i = 1; i <= S->clauses; i++)
    {
        if (S->clause_delete[i])
            continue;
        int flag = true;
        for (int j = 0; j < S->clause_size[i]; j++)
        {
            int lit = S->clause[i][j];
            if (sgn[abs(lit)] == S->vars + 1)
            {
                sgn[abs(lit)] = lit;
                baksgn[abs(lit)] = lit;
            }
            if (sgn[abs(lit)] != lit)
            {
                flag = false;
            }
        }
        if (flag)
        {
            mark[i] = 1;
            e[i] = (int *)malloc(sizeof(int) * S->clause_size[i]);
            cntcls++;
            for (int j = 0; j < S->clause_size[i]; j++)
            {
                int lit = S->clause[i][j];
                e[i][j] = ey[abs(lit)];
                sgn[abs(lit)] = 0;
            }
        }
    }

    int *vst = (int *)malloc(sizeof(int) * (S->cards));
    int *match = (int *)malloc(sizeof(int) * (S->cards));
    memset(match, -1, sizeof(int) * (S->cards));

    bool matched = true;
    std::vector<unsigned> conflict_set_clauses, conflict_set_cards;
    for (int i = 1; i <= S->clauses; i++)
    {
        if (mark[i])
        {
            memset(vst, 0, sizeof(int) * (S->cards));
            if (!dfs(S, i, vst, match, e))
            {
                matched = false;
                memset(vst, 0, sizeof(int) * (S->cards));
                dfs_mark(S, i, vst, match, e, conflict_set_clauses, conflict_set_cards);
                break;
            }
        }
    }
    if (matched == true && cntcls == S->clauses)
    {
        int *matchc = (int *)malloc((S->clauses + 1) * sizeof(int));
        bool *vst = (bool *)calloc((S->vars + 1), sizeof(bool));
        memset(matchc, -1, (S->clauses + 1) * sizeof(int));
        for (int i = 0; i < S->cards; i++)
        {
            if (match[i] != -1)
            {
                matchc[match[i]] = i;
            }
        }
        for (int i = 1; i <= S->clauses; i++)
        {
            if (matchc[i] == -1)
            {
                continue;
            }
            bool flag = true;
            for (int j = 0; j < S->clause_size[i]; j++)
            {
                int lit = S->clause[i][j];
                if (flag && ey[abs(lit)] == matchc[i])
                {
                    vst[abs(lit)] = 1;
                    flag = false;
                    break;
                }
                else
                {
                }
            }
        }
        for (int i = 1; i <= S->vars; i++)
        {
            if (!vst[i])
            {
                if (abs(baksgn[i]) != i)
                {
                    baksgn[i] = i;
                }
            }
        }
        free(vst);
        free(matchc);
    }
    S->pbcounter = 0;
    std::vector<int> line;
    if (matched == false)
    {
        // pseudo boolean proof
        std::vector<int> polpos, polneg;
        for (unsigned i = 0; i < conflict_set_clauses.size(); i++)
        {
            unsigned c = conflict_set_clauses[i];
            for (int j = 0; j < S->clause_size[c]; j++)
            {
                int lit = S->clause[c][j];
                S->ineq.push_back(lit);
                S->ineq.push_back(1);
            }
            print_inequality(S, GE, 1);
            S->pbcounter++;
            polpos.push_back(S->pbcounter);
        }
        sort(conflict_set_cards.begin(), conflict_set_cards.end(), lequ);
        unsigned j = 0;
        for (unsigned i = 0; i < conflict_set_cards.size(); i++)
        {
            if (i == 0 || conflict_set_cards[i] != conflict_set_cards[j - 1])
            {
                conflict_set_cards[j] = conflict_set_cards[i];
                j++;
            }
        }
        conflict_set_cards.resize(j);
        for (unsigned i = 0; i < conflict_set_cards.size(); i++)
        {
            generate_amo_proof(S, conflict_set_cards[i]);
            polneg.push_back(S->pbcounter);
        }
        S->ineq.push_back(polpos[0] - S->pbcounter - 1);
        for (unsigned i = 1; i < polpos.size(); i++)
        {
            S->ineq.push_back(polpos[i] - S->pbcounter - 1);
            S->ineq.push_back('+');
        }
        for (unsigned i = 0; i < polneg.size(); i++)
        {
            S->ineq.push_back(polneg[i] - S->pbcounter - 1);
            S->ineq.push_back('+');
        }

        print_pol(S); // conflict
        polpos.clear();
        polneg.clear();
    }
    
    free(vst);
    free(match);
    free(ey);
    free(sgn);
    free(mark);
    free(baksgn);
    for (int i = 1; i <= S->clauses; i++)
    {
        free(e[i]);
    }
    free(e);
    S->cards -= delta;
    if (matched == false)
    {
        return 0;
    }
    return 1;
}

int simplify_bip(simplify *S)
{
    S->M_card = (int)(2e7 / S->vars);
    if (S->M_card <= 10000)
        S->M_card = 10000;
    S->card_one = (int **)malloc(sizeof(int *) * (S->M_card));
    S->card_one_size = (int *)malloc(sizeof(int) * (S->M_card));
    int sone = search_almost_one(S);
    if (!sone)
    {
        for (int i = 0; i < S->cards; i++)
            free(S->card_one[i]);
        free(S->card_one);
        free(S->card_one_size);
        return 1;
    }

    int bip = bipartite_check(S);

    if (bip == 0)
    {
        for (int i = 0; i < S->cards; i++)
            free(S->card_one[i]);
        free(S->card_one);
        free(S->card_one_size);
        return 0;
    }
    return 1;
}

bool kissat_simplify(std::string& filename, cnf2wl& formula, bool entered_file, std::string& proof_filename)
{
    simplify *S = simplify_init();
    if(proof_filename.empty())
        S->fp = nullptr;
    else
        S->fp = fopen(proof_filename.c_str(), "w");
    if(S->fp)
        fprintf(S->fp, "pseudo-Boolean proof version 3.0\n");
    simplify_parse(S, filename, entered_file);

    if (S->vars <= 1e6 && S->clauses <= 4e7)
    {
        int res = simplify_bip(S);
        if (!res)
        {
            simplify_release(S);
            free(S);
            return false;
        }
    }
    /*
    int res = simplify_easy_clause(S);

    if (!res)
    {
        simplify_release(S);
        free(S);
        return false;
    }

    res = simplify_resolution(S);
    if (!res)
    { // this never happens...
        simplify_release(S);
        free(S);
        return false;
    }

    */


    formula.reserve(S->vars, S->clauses + S->known_size);
    std::vector<int> construct_clause;
    for (int i = 1; i <= S->clauses; i++)
    {
        int v = i;
        for (int j = 0; j < S->clause_size[v]; j++)
        {
            construct_clause.push_back(S->clause[v][j]);
        }
        formula.add_clause(construct_clause);
        construct_clause.clear();
    }
    for(int i = 0; i < S->known_size; i++){
        construct_clause.push_back(S->known[i]);
        formula.add_clause(construct_clause);
        construct_clause.clear();
    }
    simplify_release(S);
    return true;
}

#endif //SATSUMA_PARSER_H
