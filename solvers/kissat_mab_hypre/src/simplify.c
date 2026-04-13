#include "allocate.h"
#include "simplify.h"
#include "internal.h"
#include "import.h"
#include "proof.h"
#include "check.h"
#include "hashmap.h"
#include "math.h"
#include "sort.h"
#include <stdio.h>
#include <stdlib.h>
#include "resources.h"
#include <ctype.h>
#include <string.h>
#include <inttypes.h>
#define TOLIT(x) ((x) > 0 ? (x) : ((-x) + S->vars))
#define NEG(x) ((x) > S->vars ? ((x) - S->vars) : ((x) + S->vars))
typedef long long ll;
int nlit;
void add_lits_tmp(simplify *S, proof *proof, int count, ...)
{
    S->proof_len += count;
    kissat *solver = S->solver;
    va_list list;
    va_start(list, count);
    for (int j = 0; j < count; j++)
    {
        int x = va_arg(list, int);
        PUSH_STACK(proof->line, x);
    }
    va_end(list);
    print_added_proof_line(proof);
}
void del_lits(simplify *S, proof *proof, int count, ...)
{
    return;
    kissat *solver = S->solver;
    va_list list;
    va_start(list, count);
    for (int j = 0; j < count; j++)
    {
        int x = va_arg(list, int);
        PUSH_STACK(proof->line, x);
    }
    va_end(list);
    print_delete_proof_line(proof);
}

simplify *simplify_init()
{
    simplify *s = (simplify *)malloc(sizeof(simplify));
    return s;
}

#define NEXT() next(file, lineno_ptr)
static inline char GET_CHAR(file *file)
{
    // const int maxn = 1048576;
    static char buf[1048576], *p1 = buf, *p2 = buf;
    return p1 == p2 && (p2 = (p1 = buf) + fread(buf, 1, 1048576, file->file), p1 == p2) ? EOF : *p1++;
}
static int next(file *file, uint64_t *lineno_ptr)
{
    int ch = GET_CHAR(file); // kissat_getc(file);
    if (ch == '\n')
        *lineno_ptr += 1;
    return ch;
}

static const char *nonl(int ch, const char *str, uint64_t *lineno_ptr)
{
    if (ch == '\n')
    {
        assert(*lineno_ptr > 1);
        *lineno_ptr -= 1;
    }
    return str;
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

    S->buf = NULL;
    S->buf_siz = 0;
    S->proof_len = 0;
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
    free(S->buf);
}
const int MAX_CLAUSES = 1000000000;
static const char *simplify_parse(simplify *S, file *file, uint64_t *lineno_ptr)
{
    *lineno_ptr = 1;
    bool first = true;
    int ch;
    for (;;)
    {
        ch = NEXT();
        if (ch == 'p')
            break;
        else if (ch == EOF)
        {
            if (first)
                return "empty file";
            else
                return "end-of-file before header";
        }
        first = false;
        if (ch == '\r')
        {
            ch = NEXT();
            if (ch != '\n')
                return "expected new-line after carriage-return";
        }
        else if (ch == '\n')
        {
        }
        else if (ch == 'c')
        {
        START:
            ch = NEXT();
            if (ch == '\n')
                continue;
            else if (ch == '\r')
            {
                ch = NEXT();
                if (ch != '\n')
                    return "expected new-line after carriage-return";
                continue;
            }
            else if (ch == EOF)
                return "end-of-file in header comment";
            else if (ch == ' ' || ch == '\t')
                goto START;
            while ((ch = NEXT()) != '\n')
                if (ch == EOF)
                    return "end-of-file in header comment";
                else if (ch == '\r')
                {
                    ch = NEXT();
                    if (ch != '\n')
                        return "expected new-line after carriage-return";
                    break;
                }
        }
        else
            return "expected 'c' or 'p' at start of line";
    }
    assert(ch == 'p');
    ch = NEXT();
    if (ch != ' ')
        return nonl(ch, "expected space after 'p'", lineno_ptr);
    ch = NEXT();
    if (ch != 'c')
        return nonl(ch, "expected 'c' after 'p '", lineno_ptr);
    ch = NEXT();
    if (ch != 'n')
        return nonl(ch, "expected 'n' after 'p c'", lineno_ptr);
    ch = NEXT();
    if (ch != 'f')
        return nonl(ch, "expected 'n' after 'p cn'", lineno_ptr);
    ch = NEXT();
    if (ch != ' ')
        return nonl(ch, "expected space after 'p cnf'", lineno_ptr);
    ch = NEXT();
    if (!isdigit(ch))
        return nonl(ch, "expected digit after 'p cnf '", lineno_ptr);
    int variables = ch - '0';
    while (isdigit(ch = NEXT()))
    {
        if (EXTERNAL_MAX_VAR / 10 < variables)
            return "maximum variable too large";
        variables *= 10;
        const int digit = ch - '0';
        if (EXTERNAL_MAX_VAR - digit < variables)
            return "maximum variable too large";
        variables += digit;
    }
    if (ch == EOF)
        return "unexpected end-of-file while parsing maximum variable";
    if (ch == '\r')
    {
        ch = NEXT();
        if (ch != '\n')
            return "expected new-line after carriage-return";
    }
    if (ch == '\n')
        return nonl(ch, "unexpected new-line after maximum variable", lineno_ptr);
    if (ch != ' ')
        return "expected space after maximum variable";
    ch = NEXT();
    while (ch == ' ' || ch == '\t')
        ch = NEXT();
    if (!isdigit(ch))
        return "expected number of clauses after maximum variable";
    uint64_t clauses = ch - '0';
    while (isdigit(ch = NEXT()))
    {
        if (MAX_CLAUSES / 10 < clauses)
            return "number of clauses too large";
        clauses *= 10;
        const int digit = ch - '0';
        if (MAX_CLAUSES - digit < (long long)clauses)
            return "number of clauses too large";
        clauses += digit;
    }
    simplify_alloc(S, variables, clauses);
    if (ch == EOF)
        return "unexpected end-of-file while parsing number of clauses";
    while (ch == ' ' || ch == '\t')
        ch = NEXT();
    if (ch == '\r')
    {
        ch = NEXT();
        if (ch != '\n')
            return "expected new-line after carriage-return";
    }
    if (ch == EOF)
        return "unexpected end-of-file after parsing number of clauses";
    if (ch != '\n')
        return "expected new-line after parsing number of clauses";

    uint64_t parsed = 0;
    int lit = 0;
    for (;;)
    {
        int sgn = 1;
        while (ch != '-' && (ch < '0' || ch > '9'))
        {
            ch = NEXT();
        }
        if (ch == '-')
        {
            sgn = -1;
        }
        while (ch < '0' || ch > '9')
        {
            ch = NEXT();
        }
        int idx = 0;
        while (isdigit(ch))
        {
            if (EXTERNAL_MAX_VAR / 10 < idx)
                return "variable index too large";
            idx *= 10;
            const int digit = ch - '0';
            if (EXTERNAL_MAX_VAR - digit < idx)
                return "variable index too large";
            idx += digit;
            ch = NEXT();
        }
        lit = sgn * idx;
        int res = simplify_store_clause(S, lit);
        if (!res)
        {
            return "empty clause";
        }
        if (S->real_clauses == S->clauses)
        {
            break;
        }
    }
    if (lit)
        return "trailing zero missing";
    if (parsed < clauses)
    {
        if (parsed + 1 == clauses)
            return "one clause missing ";
        return "more than one clause missing ";
    }
    return 0;
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
static inline ll mapv(int a, int b)
{
    return 1ll * a * nlit + (ll)b;
}
void backup_clause(simplify *S, int id)
{
    if (S->solver->proof == NULL)
        return;
    if (S->buf_siz < S->clause_size[id])
    {
        S->buf_siz = S->clause_size[id];
        S->buf = realloc(S->buf, sizeof(int) * S->buf_siz);
    }
    memcpy(S->buf, S->clause[id], sizeof(int) * S->clause_size[id]);
    S->buf_len = S->clause_size[id];
}
void print_del_backup(simplify *S)
{
    return;
    if (S->solver->proof == NULL)
        return;
    kissat *solver = S->solver;
    for (int i = 0; i < S->buf_len; i++)
    {
        int x = S->buf[i];
        PUSH_STACK(solver->proof->line, x);
    }
    print_delete_proof_line(solver->proof);
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
    proof *proof = S->solver->proof;
    memset(S->occurn_size + 1, 0, sizeof(int) * S->vars);
    memset(S->occurp_size + 1, 0, sizeof(int) * S->vars);
    memset(S->resseen + 1, 0, sizeof(int) * S->vars * 2);
    memset(S->varval + 1, 0, sizeof(int) * S->vars);
    memset(S->clause_delete + 1, 0, sizeof(int) * S->clauses);
    for (int i = 1; i <= S->clauses; i++)
        S->clause_delete[i] = 0;
    int head = 1, tail = 0;
    kissat *solver = S->solver;
    for (int i = 1; i <= S->clauses; i++)
    {
        int l = S->clause_size[i], t = 0;
        backup_clause(S, i);
        for (int j = 0; j < l; j++)
        {
            int lit = TOLIT(S->clause[i][j]);
            if (S->resseen[lit] == i)
                continue;
            if (S->resseen[NEG(lit)] == i)
            {
                S->clause_delete[i] = 1;
                break;
            }
            S->clause[i][t++] = S->clause[i][j];
            S->resseen[lit] = i;
        }
        if (S->clause_delete[i])
        {
            print_del_backup(S);
            continue;
        }
        S->clause_size[i] = t;
        for (int j = 0; j < t; j++)
        {
            if (S->clause[i][j] > 0)
                S->occurp_size[S->clause[i][j]]++;
            else
                S->occurn_size[-S->clause[i][j]]++;
            if ((t == 0 || t == 1 || t < l) && proof != NULL)
            {
                int var = S->clause[i][j];
                PUSH_STACK(proof->line, var);
                proof->literals += 1;
            }
        }
        if ((t == 0 || t == 1 || t < l) && proof != NULL)
        {
            print_added_proof_line(proof);
            print_del_backup(S);
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
                backup_clause(S, o);
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
                    print_del_backup(S);
                    continue;
                }
                if ((t == 0 || t == 1 || t < S->clause_size[o]) && proof != NULL)
                {
                    for (int j = 0; j < t; j++)
                    {
                        int var = S->clause[o][j];
                        PUSH_STACK(proof->line, var);
                        proof->literals += 1;
                    }
                    print_added_proof_line(proof);
                    print_del_backup(S);
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
                backup_clause(S, o);
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
                    print_del_backup(S);
                    continue;
                }
                if ((t == 0 || t == 1 || t < S->clause_size[o]) && proof != NULL)
                {
                    for (int j = 0; j < t; j++)
                    {
                        int var = S->clause[o][j];
                        PUSH_STACK(proof->line, var);
                        proof->literals += 1;
                    }
                    print_added_proof_line(proof);
                    print_del_backup(S);
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

simplify *S;
int search_almost_one(simplify *S)
{
    HashMap *H = map_init(40000003);
    nlit = 2 * S->vars + 2;
    int **occur = (int **)calloc((nlit), sizeof(int *));
    int *occur_size = (int *)calloc((nlit), sizeof(int));
    for (int i = 1; i <= S->clauses; i++)
    {
        S->clause_delete[i] = 0;
        if (S->clause_size[i] != 2)
            continue;
        int x = tolit(S->clause[i][0]);
        int y = tolit(S->clause[i][1]);
        ll id1 = mapv(x, y);
        ll id2 = mapv(y, x);
        map_insert(H, id1, i);
        map_insert(H, id2, i);
        occur_size[x]++;
        occur_size[y]++;
    }
    for (int i = 2; i < nlit; i++)
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
                    ll id = mapv(v, cvec_data(ino, k));
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
                        ll id1 = mapv(v, cvec_data(ino, k)), id2 = mapv(cvec_data(ino, k), v);
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
                    for (int i = 0; i < nlit; i++)
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
    for (int i = 0; i < nlit; i++)
    {
        if (occur[i] != NULL)
        {
            free(occur[i]);
        }
    }
    free(occur);
    return S->cards;
}

int dfs(int v, int *vst, int *match, int **e)
{
    for (int i = 0; i < S->clause_size[v]; i++)
    {
        int y = e[v][i];
        if (!vst[y])
        {
            vst[y] = 1;
            if (match[y] == -1 || dfs(match[y], vst, match, e))
            {
                match[y] = v;
                return 1;
            }
        }
    }
    return 0;
}
void dfs_mark(int v, int *vst, int *match, int **e, kissat *solver, unsigneds *le, unsigneds *ri)
{
    PUSH_STACK(*le, v);
    for (int i = 0; i < S->clause_size[v]; i++)
    {
        int y = e[v][i];
        if (!vst[y])
        {
            PUSH_STACK(*ri, y);
            vst[y] = 1;
            dfs_mark(match[y], vst, match, e, solver, le, ri);
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
    longlongs *l = &(S->ineq);
    proof *proof = S->solver->proof;
    kissat_puts(proof, "rup");
    for (unsigned i = 0; i < SIZE_STACK(*l); i += 2)
    {
        kissat_puts(proof, " ");
        kissat_putint(proof, PEEK_STACK(*l, i + 1));
        kissat_puts(proof, " ");
        int lit = PEEK_STACK(*l, i);

        if (lit < 0)
        {
            kissat_puts(proof, "~");
        }
        kissat_puts(proof, "x");
        kissat_putint(proof, abs(lit));
    }
    if (type == GE)
    {
        kissat_puts(proof, " >= ");
    }
    else
    {
        kissat_puts(proof, " <= ");
    }
    kissat_putint(proof, b);
    kissat_puts(proof, " ;\n");
    CLEAR_STACK(*l);
}
void print_pol(simplify *S)
{
    longlongs *l = &(S->ineq);
    proof *proof = S->solver->proof;
    kissat_puts(proof, "pol");
    kissat_puts(proof, " ");
    kissat_putint(proof, PEEK_STACK(*l, 0));

    for (unsigned i = 1; i < SIZE_STACK(*l); i += 2)
    {
        kissat_puts(proof, " ");
        kissat_putint(proof, PEEK_STACK(*l, i));
        kissat_puts(proof, " ");
        char *ppp = malloc(2 * sizeof(char));
        ppp[0] = PEEK_STACK(*l, i + 1);
        ppp[1] = '\0';
        kissat_puts(proof, ppp);
        free(ppp);
    }
    kissat_puts(proof, ";\n");
    CLEAR_STACK(*l);
}
void generate_amo_proof(simplify *S, unsigned id)
{
    int len = S->card_one_size[id];
    kissat *solver = S->solver;
    int *labels = (int *)malloc(sizeof(int) * len * len);
    for (int delta = 1; delta < len; delta++)
    {
        for (int i = 0; i + delta < len; i++)
        {
            int j = i + delta;
            if (j == i + 1)
            {
                PUSH_STACK(S->ineq, S->card_one[id][i]);
                PUSH_STACK(S->ineq, -1);
                PUSH_STACK(S->ineq, S->card_one[id][j]);
                PUSH_STACK(S->ineq, -1);
                print_inequality(S, GE, -1);
                labels[i * len + j] = ++S->pbcounter;
            }
            else
            {
                PUSH_STACK(S->ineq, S->card_one[id][i]);
                PUSH_STACK(S->ineq, -1);
                PUSH_STACK(S->ineq, S->card_one[id][j]);
                PUSH_STACK(S->ineq, -1);
                print_inequality(S, GE, -1);
                ++S->pbcounter;
                PUSH_STACK(S->ineq, labels[i * len + j - 1] - S->pbcounter - 1);
                PUSH_STACK(S->ineq, labels[(i + 1) * len + j] - S->pbcounter - 1);
                PUSH_STACK(S->ineq, '+');
                PUSH_STACK(S->ineq, -1);
                PUSH_STACK(S->ineq, '+');
                PUSH_STACK(S->ineq, 2);
                PUSH_STACK(S->ineq, 'd');
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
            e[i] = malloc(sizeof(int) * S->clause_size[i]);
            cntcls++;
            for (int j = 0; j < S->clause_size[i]; j++)
            {
                int lit = S->clause[i][j];
                e[i][j] = ey[abs(lit)];
                sgn[abs(lit)] = 0;
            }
        }
    }

    int *vst = malloc(sizeof(int) * (S->cards));
    int *match = malloc(sizeof(int) * (S->cards));
    memset(match, -1, sizeof(int) * (S->cards));

    bool matched = true;
    unsigneds conflict_set_clauses, conflict_set_cards;
    kissat *solver = S->solver;
    INIT_STACK(conflict_set_clauses);
    INIT_STACK(conflict_set_cards);
    for (int i = 1; i <= S->clauses; i++)
    {
        if (mark[i])
        {
            memset(vst, 0, sizeof(int) * (S->cards));
            if (!dfs(i, vst, match, e))
            {
                matched = false;
                memset(vst, 0, sizeof(int) * (S->cards));
                dfs_mark(i, vst, match, e, solver, &conflict_set_clauses, &conflict_set_cards);
                break;
            }
        }
    }
    if (matched == true && cntcls == S->clauses)
    {
        int *matchc = malloc((S->clauses + 1) * sizeof(int));
        bool *vst = calloc((S->vars + 1), sizeof(bool));
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
    INIT_STACK(S->ineq);
    S->pbcounter = 0;
    if (matched == false && solver->proof)
    {
        if (solver->proof->binary == false)
        {
            // pseudo boolean proof
            ints polpos, polneg;
            INIT_STACK(polpos);
            INIT_STACK(polneg);
            for (unsigned i = 0; i < SIZE_STACK(conflict_set_clauses); i++)
            {
                unsigned c = PEEK_STACK(conflict_set_clauses, i);
                for (int j = 0; j < S->clause_size[c]; j++)
                {
                    int lit = S->clause[c][j];
                    PUSH_STACK(S->ineq, lit);
                    PUSH_STACK(S->ineq, 1);
                }
                print_inequality(S, GE, 1);
                S->pbcounter++;
                PUSH_STACK(polpos, S->pbcounter);
            }
            SORT(unsigned, SIZE_STACK(conflict_set_cards), BEGIN_STACK(conflict_set_cards), lequ);
            unsigned j = 0;
            for (unsigned i = 0; i < SIZE_STACK(conflict_set_cards); i++)
            {
                if (i == 0 || PEEK_STACK(conflict_set_cards, i) != PEEK_STACK(conflict_set_cards, i - 1))
                {
                    POKE_STACK(conflict_set_cards, j, PEEK_STACK(conflict_set_cards, i));
                    j++;
                }
            }
            RESIZE_STACK(conflict_set_cards, j);
            for (unsigned i = 0; i < SIZE_STACK(conflict_set_cards); i++)
            {
                generate_amo_proof(S, PEEK_STACK(conflict_set_cards, i));
                PUSH_STACK(polneg, S->pbcounter);
            }
            PUSH_STACK(S->ineq, PEEK_STACK(polpos, 0) - S->pbcounter - 1);
            for (unsigned i = 1; i < SIZE_STACK(polpos); i++)
            {
                PUSH_STACK(S->ineq, PEEK_STACK(polpos, i) - S->pbcounter - 1);
                PUSH_STACK(S->ineq, '+');
            }
            for (unsigned i = 0; i < SIZE_STACK(polneg); i++)
            {
                PUSH_STACK(S->ineq, PEEK_STACK(polneg, i) - S->pbcounter - 1);
                PUSH_STACK(S->ineq, '+');
            }

            print_pol(S); // conflict
            RELEASE_STACK(polpos);
            RELEASE_STACK(polneg);
        }
        else if (SIZE_STACK(conflict_set_cards) <= 1000u && SIZE_STACK(conflict_set_clauses) >= 3u)
        {
            // PR proof
            int nh = SIZE_STACK(conflict_set_cards);
            int **ori_vars = (int **)malloc(sizeof(int *) * (nh + 1));
            int tmp_vars = S->vars;
            int sw = ++tmp_vars;
            int **ph_vars = (int **)malloc(sizeof(int *) * (nh + 1));
            int *hole_id = (int *)malloc(sizeof(int) * S->cards);
            for (int i = 0; i < nh; i++)
            {
                hole_id[PEEK_STACK(conflict_set_cards, i)] = i;
            }
            proof *proof = solver->proof;
            for (int i = 0; i < nh + 1; i++)
            {
                ori_vars[i] = (int *)calloc(nh, sizeof(int));
                ph_vars[i] = (int *)calloc(nh, sizeof(int));
            }
            int *new_vars = (int *)calloc((S->vars + 1), sizeof(int));
            intss new_cards;
            INIT_STACK(new_cards);
            for (int i = 0; i < nh + 1; i++)
            { // create variable copies.
                int c = PEEK_STACK(conflict_set_clauses, i);
                int l = S->clause_size[c];
                for (int j = 0; j < l; j++)
                { // add a fake switch sw. if sw is true, the new copied variable is set to be equal to the old variable. if sw is false, the same happens.

                    int lit = S->clause[c][j];
                    int var = abs(lit);
                    if (new_vars[var] == 0)
                    {
                        new_vars[var] = ++tmp_vars;
                        add_lits_tmp(S, proof, 3, -new_vars[var], sw, var);
                        add_lits_tmp(S, proof, 3, new_vars[var], sw, -var);
                        add_lits_tmp(S, proof, 3, -new_vars[var], -sw, var);
                        add_lits_tmp(S, proof, 3, new_vars[var], -sw, -var);
                    }
                    int new_lit = pnsign(lit) * new_vars[abs(lit)];
                    int card_id = ey[var];
                    ori_vars[i][hole_id[card_id]] = lit;
                    ph_vars[i][hole_id[card_id]] = new_lit;
                }
            }
            for (int i = 0; i < nh; i++)
            { // copy related amo constraints
                int card = PEEK_STACK(conflict_set_cards, i);
                ints new_card;
                INIT_STACK(new_card);
                for (int j = 0; j < S->card_one_size[card]; j++)
                {
                    int vj = S->card_one[card][j];
                    if (new_vars[abs(vj)])
                    {
                        vj = new_vars[abs(vj)] * pnsign(vj);
                        PUSH_STACK(new_card, vj);
                    }
                }
                PUSH_STACK(new_cards, new_card);
                int siz = SIZE_STACK(new_card);
                for (int j = 0; j < siz; j++)
                {
                    for (int k = j + 1; k < siz; k++)
                    {
                        int vj = PEEK_STACK(new_card, j);
                        int vk = PEEK_STACK(new_card, k);
                        add_lits_tmp(S, proof, 3, -vj, -vk, sw);
                        add_lits_tmp(S, proof, 3, -vj, -vk, -sw);
                        add_lits_tmp(S, proof, 2, -vj, -vk);
                    }
                }
            }
            for (int i = 0; i < nh + 1; i++)
            { // copy conflict core clauses. (to prevent influence of unrelated clauses)
                int c = PEEK_STACK(conflict_set_clauses, i);
                int l = S->clause_size[c];
                for (int j = 0; j < nh; j++)
                { // complete new (pigeon hole) variables.
                    if (ori_vars[i][j] == 0)
                    {
                        ph_vars[i][j] = ++tmp_vars;
                        ints *card = &PEEK_STACK(new_cards, j);
                        for (all_stack(int, lit, *card))
                        {
                            add_lits_tmp(S, proof, 2, -tmp_vars, -lit);
                        }
                        PUSH_STACK(*card, tmp_vars);
                    }
                }
                for (int k = 0; k < l; k++)
                {
                    int lit = S->clause[c][k];
                    int new_lit = pnsign(lit) * new_vars[abs(lit)];
                    PUSH_STACK(proof->line, new_lit);
                }
                for (int k = 0; k < nh; k++)
                {
                    if (ori_vars[i][k] == 0 && ph_vars[i][k])
                    {
                        PUSH_STACK(proof->line, ph_vars[i][k]);
                    }
                }
                PUSH_STACK(proof->line, sw);
                print_added_proof_line(proof);
                for (int k = 0; k < l; k++)
                {
                    int lit = S->clause[c][k];
                    int new_lit = pnsign(lit) * new_vars[abs(lit)];
                    PUSH_STACK(proof->line, new_lit);
                }
                for (int k = 0; k < nh; k++)
                {
                    if (ori_vars[i][k] == 0 && ph_vars[i][k])
                    {
                        PUSH_STACK(proof->line, ph_vars[i][k]);
                    }
                }
                PUSH_STACK(proof->line, -sw);
                print_added_proof_line(proof);
                for (int k = 0; k < l; k++)
                {
                    int lit = S->clause[c][k];
                    int new_lit = pnsign(lit) * new_vars[abs(lit)];
                    PUSH_STACK(proof->line, new_lit);
                }
                for (int k = 0; k < nh; k++)
                {
                    if (ori_vars[i][k] == 0 && ph_vars[i][k])
                    {
                        PUSH_STACK(proof->line, ph_vars[i][k]);
                    }
                }
                print_added_proof_line(proof);
            }

            for (int i = 0; i < nh + 1; i++)
            { // detach new and old vars
                int c = PEEK_STACK(conflict_set_clauses, i);
                int l = S->clause_size[c];
                for (int j = 0; j < l; j++)
                {
                    int lit = S->clause[c][j];
                    int var = abs(lit);
                    if (new_vars[var] > 0)
                    {
                        del_lits(S, proof, 3, var, -new_vars[var], sw);
                        del_lits(S, proof, 3, -var, new_vars[var], sw);
                        del_lits(S, proof, 3, var, -new_vars[var], -sw);
                        del_lits(S, proof, 3, -var, new_vars[var], -sw);
                        new_vars[var] *= -1;
                    }
                }
            }
            for (int i = 1; i <= S->vars; i++)
            {
                new_vars[i] *= -1;
            }
            for (int i = 0; i < nh + 1; i++)
            { // erase parallel edges
                int c = PEEK_STACK(conflict_set_clauses, i);
                int l = S->clause_size[c];
                for (int j = 0; j < l; j++)
                {
                    int lit = S->clause[c][j];
                    int var = abs(lit);
                    int card_id = ey[var];
                    int new_lit = pnsign(lit) * new_vars[var];
                    if (ori_vars[i][hole_id[card_id]] != lit)
                    {
                        PUSH_STACK(proof->line, -new_lit);
                        PUSH_STACK(proof->line, -new_lit);
                        PUSH_STACK(proof->line, ph_vars[i][hole_id[card_id]]);
                        print_added_proof_line(proof);
                        for (int k = 0; k < l; k++)
                        {
                            int lit = S->clause[c][k];
                            int new_lit = new_vars[abs(lit)] * pnsign(lit);
                            if (k != j && S->clause[c][k] != 0)
                            {
                                PUSH_STACK(proof->line, new_lit);
                            }
                            else
                            {
                            }
                        }
                        for (int k = 0; k < nh; k++)
                        {
                            if (ori_vars[i][k] == 0 && ph_vars[i][k])
                            {
                                PUSH_STACK(proof->line, ph_vars[i][k]);
                            }
                        }
                        print_added_proof_line(proof);
                        for (int k = 0; k < l; k++)
                        {
                            int lit = S->clause[c][k];
                            int new_lit = new_vars[abs(lit)] * pnsign(lit);
                            if (S->clause[c][k] != 0)
                            {
                                PUSH_STACK(proof->line, new_lit);
                            }
                            else
                            {
                            }
                        }
                        /*
                        for (int k = 0; k < nh; k++)
                        {
                            if (ori_vars[i][k] == 0 && ph_vars[i][k])
                            {
                                PUSH_STACK(proof->line, ph_vars[i][k]);
                            }
                        }
                        print_delete_proof_line(proof);
                        */
                        S->clause[c][j] = 0;
                    }
                }
            }
            for (; nh >= 2; nh--)
            { // prove ph formula
                for (int i = 0; i < nh; i++)
                {
                    for (int j = 0; j < nh; j++)
                    {
                        if (j != nh - 1)
                        { // pr
                            PUSH_STACK(proof->line, -ph_vars[nh][j]);
                            PUSH_STACK(proof->line, -ph_vars[i][nh - 1]);
                            PUSH_STACK(proof->line, -ph_vars[nh][j]);
                            PUSH_STACK(proof->line, -ph_vars[i][nh - 1]);
                            PUSH_STACK(proof->line, ph_vars[nh][nh - 1]);
                            PUSH_STACK(proof->line, ph_vars[i][j]);
                            print_added_proof_line(proof);
                        }
                    }
                    PUSH_STACK(proof->line, -ph_vars[i][nh - 1]);
                    print_added_proof_line(proof);
                    for (int j = 0; j < nh - 1; j++)
                    {
                        PUSH_STACK(proof->line, ph_vars[i][j]);
                    }
                    print_added_proof_line(proof);
                }
            }
            free(hole_id);
            for (int i = 0; i < nh + 1; i++)
            {
                free(ori_vars[i]);
                free(ph_vars[i]);
            }
            free(ori_vars);
            free(ph_vars);
        }
    }
    RELEASE_STACK(S->ineq);
    RELEASE_STACK(conflict_set_clauses);
    RELEASE_STACK(conflict_set_cards);
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

bool kissat_simplify(kissat *solver, int *maxvar, file *file)
{
    S = simplify_init();
    S->solver = solver;
    uint64_t lineno_ptr;
    simplify_parse(S, file, &lineno_ptr);
    printf("c after parse time = %lf, var = %d, clauses = %d\n", kissat_process_time(), S->vars, S->clauses);
    proof *proof = solver->proof;

    if (S->vars <= 1e6 && S->clauses <= 4e7)
    {
        int res = simplify_bip(S);
        if (res == false && proof && proof->binary == true)
        { // bipartite graph no solution but we can't find a PR proof
            res = true;
        }
        if (!res)
        {
            simplify_release(S);
            free(S);
            return false;
        }
    }
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

    printf("c after simplify time = %lf, var = %d, clauses = %d\n", kissat_process_time(), S->vars, S->clauses + S->known_size);
    *maxvar = S->vars;
    kissat_reserve(solver, S->vars);
    for (int i = 1; i <= S->clauses; i++)
    {
        int v = i;
        for (int j = 0; j < S->clause_size[v]; j++)
        {
            kissat_add(solver, S->clause[v][j]);
            if (proof != NULL)
            {
                proof->literals += 1;
            }
        }
        kissat_add(solver, 0);
    }
    for(int i = 0; i < S->known_size; i++){
        kissat_add(solver, S->known[i]);
        if (proof != NULL)
        {
            proof->literals += 1;
        }
        kissat_add(solver, 0);
    }
    simplify_release(S);
    return true;
}

static void flush_buffer(chars *buffer)
{
    fputs("v", stdout);
    for (all_stack(char, ch, *buffer))
        fputc(ch, stdout);
    fputc('\n', stdout);
    CLEAR_STACK(*buffer);
}

static void print_int(kissat *solver, chars *buffer, int i)
{
    char tmp[16];
    sprintf(tmp, " %d", i);
    size_t tmp_len = strlen(tmp);
    size_t buf_len = SIZE_STACK(*buffer);
    if (buf_len + tmp_len > 77)
        flush_buffer(buffer);
    for (const char *p = tmp; *p; p++)
        PUSH_STACK(*buffer, *p);
}

void kissat_complete_val(kissat *solver)
{
    chars buffer;
    INIT_STACK(buffer);
    for (int i = 1; i <= S->vars; i++)
    {
        print_int(solver, &buffer, i * solver->last_val[i]);
    }
    print_int(solver, &buffer, 0);
    assert(!EMPTY_STACK(buffer));
    flush_buffer(&buffer);
    RELEASE_STACK(buffer);
}