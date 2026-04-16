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
#ifndef NPROOFS
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
#endif

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
#ifndef NPROOFS
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
#endif
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
#ifndef NPROOFS
    proof *proof = S->solver->proof;
#endif
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
#ifndef NPROOFS
        backup_clause(S, i);
#endif
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
#ifndef NPROOFS
            print_del_backup(S);
#endif
            continue;
        }
        S->clause_size[i] = t;
        for (int j = 0; j < t; j++)
        {
            if (S->clause[i][j] > 0)
                S->occurp_size[S->clause[i][j]]++;
            else
                S->occurn_size[-S->clause[i][j]]++;
#ifndef NPROOFS
            if ((t == 0 || t == 1 || t < l) && proof != NULL)
            {
                int var = S->clause[i][j];
                PUSH_STACK(proof->line, var);
                proof->literals += 1;
            }
#endif
        }
#ifndef NPROOFS
        if ((t == 0 || t == 1 || t < l) && proof != NULL)
        {
            print_added_proof_line(proof);
            print_del_backup(S);
        }
#endif
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
#ifndef NPROOFS
                backup_clause(S, o);
#endif
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
#ifndef NPROOFS
                    print_del_backup(S);
#endif
                    continue;
                }
#ifndef NPROOFS
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
#endif
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
#ifndef NPROOFS
                backup_clause(S, o);
#endif
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
#ifndef NPROOFS
                    print_del_backup(S);
#endif
                    continue;
                }
#ifndef NPROOFS
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
#endif
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
bool kissat_simplify(kissat *solver, int *maxvar, file *file)
{
    S = simplify_init();
    S->solver = solver;
    uint64_t lineno_ptr;
    simplify_parse(S, file, &lineno_ptr);
    printf("c after parse time = %lf, var = %d, clauses = %d\n", kissat_process_time(), S->vars, S->clauses);
#ifndef NPROOFS
    proof *proof = solver->proof;
#endif
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
#ifndef NPROOFS
            if (proof != NULL)
            {
                proof->literals += 1;
            }
#endif
        }
        kissat_add(solver, 0);
    }
    for(int i = 0; i < S->known_size; i++){
        kissat_add(solver, S->known[i]);
#ifndef NPROOFS
        if (proof != NULL)
        {
            proof->literals += 1;
        }
#endif
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