#ifndef _BUILD_H_
#define _BUILD_H_

#include "basis.h"

CCA_Solver::CCA_Solver() {}

int CCA_Solver::build_neighbor_relation()
{
    int *neighbor_flag = new int[num_vars + 1];
    int i, j, count;
    int v, c;
    struct tms start, stop, now;
    times(&start);

    for (v = 1; v <= num_vars; ++v)
    {
        var_neighbor_count[v] = 0;

        if (fix[v] == 1)
            continue;

        for (i = 1; i <= num_vars; ++i)
            neighbor_flag[i] = 0;
        neighbor_flag[v] = 1;

        for (i = 0; i < var_lit_count[v]; ++i)
        {
            c = var_lit[v][i].clause_num;
            if (clause_delete[c] == 1)
                continue;

            for (j = 0; j < clause_lit_count[c]; ++j)
            {
                if (neighbor_flag[clause_lit[c][j].var_num] == 0)
                {
                    var_neighbor_count[v]++;
                    neighbor_flag[clause_lit[c][j].var_num] = 1;
                }
            }
        }

        neighbor_flag[v] = 0;

        var_neighbor[v] = new int[var_neighbor_count[v] + 1];

        count = 0;
        for (i = 1; i <= num_vars; ++i)
        {
            if (fix[i] == 1)
                continue;

            if (neighbor_flag[i] == 1)
            {
                var_neighbor[v][count] = i;
                count++;
            }
        }
        var_neighbor[v][count] = 0;
        times(&now);
        double comp_time = double(now.tms_utime - start.tms_utime + now.tms_stime - start.tms_stime) / sysconf(_SC_CLK_TCK);
        if (comp_time > 1200)
        {
            delete[] neighbor_flag;
            neighbor_flag = NULL;
            return 1;
        }
    }

    delete[] neighbor_flag;
    neighbor_flag = NULL;
    return 0;
}

int CCA_Solver::build_instance(char *filename)
{
    char line[1000000];
    char tempstr1[10];
    char tempstr2[10];
    int cur_lit;
    int i, j;
    int v, c; // var, clause

    ifstream infile(filename);

    /*** build problem data structures of the instance ***/
    infile.getline(line, 1000000);
    while (line[0] != 'p')
        infile.getline(line, 1000000);

    sscanf(line, "%s %s %d %d", tempstr1, tempstr2, &num_vars, &num_clauses);
    ratio = double(num_clauses) / num_vars;

    allocate_memory();
    for (c = 0; c < num_clauses; c++)
    {
        clause_lit_count[c] = 0;
        clause_delete[c] = 0;
    }
    for (v = 1; v <= num_vars; ++v)
    {
        var_lit_count[v] = 0;
        fix[v] = 0;
    }

    max_clause_len = 0;
    min_clause_len = num_vars;

    // Now, read the clauses, one at a time.
    for (c = 0; c < num_clauses; c++)
    {
        infile >> cur_lit;

        while (cur_lit != 0)
        {
            temp_lit[clause_lit_count[c]] = cur_lit;
            clause_lit_count[c]++;

            infile >> cur_lit;
        }

        clause_lit[c] = new lit[clause_lit_count[c] + 1];

        for (i = 0; i < clause_lit_count[c]; ++i)
        {
            clause_lit[c][i].clause_num = c;
            clause_lit[c][i].var_num = abs(temp_lit[i]);
            if (temp_lit[i] > 0)
                clause_lit[c][i].sense = 1;
            else
                clause_lit[c][i].sense = 0;

            var_lit_count[clause_lit[c][i].var_num]++;
        }
        clause_lit[c][i].var_num = 0;
        clause_lit[c][i].clause_num = -1;

        // unit clause
        if (clause_lit_count[c] == 1)
        {
            unitclause_queue[unitclause_queue_end_pointer++] = clause_lit[c][0];
            clause_delete[c] = 1;
        }

        if (clause_lit_count[c] > max_clause_len)
            max_clause_len = clause_lit_count[c];
        else if (clause_lit_count[c] < min_clause_len)
            min_clause_len = clause_lit_count[c];

        formula_len += clause_lit_count[c];
    }
    infile.close();

    avg_clause_len = (double)formula_len / num_clauses;

    if (unitclause_queue_end_pointer > 0)
    {
        simplify = 1;
        for (c = 0; c < num_clauses; c++)
        {
            org_clause_lit_count[c] = clause_lit_count[c];
            org_clause_lit[c] = new lit[clause_lit_count[c] + 1];
            for (i = 0; i < org_clause_lit_count[c]; ++i)
            {
                org_clause_lit[c][i] = clause_lit[c][i];
            }
        }
    }

    // creat var literal arrays
    for (v = 1; v <= num_vars; ++v)
    {
        var_lit[v] = new lit[var_lit_count[v] + 1];
        var_lit_count[v] = 0; // reset to 0, for build up the array
    }
    // scan all clauses to build up var literal arrays
    for (c = 0; c < num_clauses; ++c)
    {
        for (i = 0; i < clause_lit_count[c]; ++i)
        {
            v = clause_lit[c][i].var_num;
            var_lit[v][var_lit_count[v]] = clause_lit[c][i];
            ++var_lit_count[v];
        }
    }
    for (v = 1; v <= num_vars; ++v) // set boundary
        var_lit[v][var_lit_count[v]].clause_num = -1;

    return 1;
}

int CCA_Solver::build_instance(const std::vector<std::vector<int>>& clauses, unsigned int nbVars)
{
    num_vars = (int)nbVars;
    num_clauses = (int)clauses.size();
    allocate_memory();
    for (int c = 0; c < num_clauses; ++c)
    {
        clause_lit_count[c] = 0;
        clause_delete[c] = 0;
    }
    for (int v = 1; v <= num_vars; ++v)
    {
        var_lit_count[v] = 0;
        fix[v] = 0;
    }

    max_clause_len = 0;
    min_clause_len = num_vars;

    for (int c = 0; c < num_clauses; ++c) {
        std::vector<int> cur_clause = clauses[c];
        for (auto cur_lit : cur_clause) {
            temp_lit[clause_lit_count[c]] = cur_lit;
            clause_lit_count[c]++;
        }
        clause_lit[c] = new lit[clause_lit_count[c] + 1];
        int i;
        for (i = 0; i < clause_lit_count[c]; ++i)
        {
            clause_lit[c][i].clause_num = c;
            clause_lit[c][i].var_num = abs(temp_lit[i]);
            if (temp_lit[i] > 0)
                clause_lit[c][i].sense = 1;
            else
                clause_lit[c][i].sense = 0;

            var_lit_count[clause_lit[c][i].var_num]++;
        }
        clause_lit[c][i].var_num = 0;
        clause_lit[c][i].clause_num = -1;

        // unit clause
        if (clause_lit_count[c] == 1)
        {
            unitclause_queue[unitclause_queue_end_pointer++] = clause_lit[c][0];
            clause_delete[c] = 1;
        }

        if (clause_lit_count[c] > max_clause_len)
            max_clause_len = clause_lit_count[c];
        else if (clause_lit_count[c] < min_clause_len)
            min_clause_len = clause_lit_count[c];

        formula_len += clause_lit_count[c];
    }

    avg_clause_len = (double)formula_len / num_clauses;

    if (unitclause_queue_end_pointer > 0)
    {
        simplify = 1;
        for (int c = 0; c < num_clauses; c++)
        {
            org_clause_lit_count[c] = clause_lit_count[c];
            org_clause_lit[c] = new lit[clause_lit_count[c] + 1];
            for (int i = 0; i < org_clause_lit_count[c]; ++i)
            {
                org_clause_lit[c][i] = clause_lit[c][i];
            }
        }
    }

    // creat var literal arrays
    for (int v = 1; v <= num_vars; ++v)
    {
        var_lit[v] = new lit[var_lit_count[v] + 1];
        var_lit_count[v] = 0; // reset to 0, for build up the array
    }
    // scan all clauses to build up var literal arrays
    for (int c = 0; c < num_clauses; ++c)
    {
        for (int i = 0; i < clause_lit_count[c]; ++i)
        {
            int v = clause_lit[c][i].var_num;
            var_lit[v][var_lit_count[v]] = clause_lit[c][i];
            ++var_lit_count[v];
        }
    }
    for (int v = 1; v <= num_vars; ++v) // set boundary
        var_lit[v][var_lit_count[v]].clause_num = -1;

    return 1;
}

void CCA_Solver::allocate_memory()
{
    int malloc_var_length = num_vars + 10;
    int malloc_clause_length = num_clauses + 10;
    var_lit = new lit *[malloc_var_length];
    var_lit_count = new int[malloc_var_length];
    clause_lit = new lit *[malloc_clause_length];
    clause_lit_count = new int[malloc_clause_length];
    org_clause_lit = new lit *[malloc_clause_length];
    org_clause_lit_count = new int[malloc_clause_length];
    score = new int[malloc_var_length];
    time_stamp = new int[malloc_var_length];
    conf_change = new int[malloc_var_length];
    var_neighbor = new int *[malloc_var_length];
    var_neighbor_count = new int[malloc_var_length];
    fix = new int[malloc_var_length];
    score2 = new int[malloc_var_length];
    goodvar_stack2 = new int[malloc_var_length];
    clause_weight = new int[malloc_clause_length];
    sat_count = new int[malloc_clause_length];
    sat_var = new int[malloc_clause_length];
    unsat_stack = new int[malloc_clause_length];
    index_in_unsat_stack = new int[malloc_clause_length];
    unsatvar_stack = new int[malloc_var_length];
    index_in_unsatvar_stack = new int[malloc_var_length];
    unsat_app_count = new int[malloc_var_length];
    goodvar_stack = new int[malloc_var_length];
    already_in_goodvar_stack = new int[malloc_var_length];
    cur_soln = new int[malloc_var_length];
    opt_soln = new int[malloc_var_length];
    clause_delete = new int[malloc_clause_length];
    unitclause_queue = new lit[malloc_var_length];
    temp_lit = new int[malloc_var_length];
    selected = new int[malloc_var_length];
    for (int i = 0; i < malloc_var_length; i++)
        selected[i] = 0;
    best_vars = new int[10];
    scores = new int[10];
    vars2 = new int[10];
    sel_cs = new int[10];
    need_reset_clause = false;

    active_clause = new bool[malloc_clause_length];
    active_var = new bool[malloc_var_length];
    remain_vars_in_clause = new int[malloc_clause_length];
    unsat_stack_fill_pointer = 0;
    unsatvar_stack_fill_pointer = 0;

    // unsat_cnt = new int[malloc_var_length];
    flip_cnt = new int[malloc_var_length];
}

void CCA_Solver::free_memory()
{
    int i;
    for (i = 0; i < num_clauses; i++)
    {
        delete[] clause_lit[i];
        delete[] org_clause_lit[i];
    }

    for (i = 1; i <= num_vars; ++i)
    {
        delete[] var_lit[i];
        delete[] var_neighbor[i];
    }
    delete[] var_lit;
    delete[] var_lit_count;
    delete[] clause_lit;
    delete[] clause_lit_count;
    delete[] org_clause_lit;
    delete[] org_clause_lit_count;
    delete[] var_neighbor;
    delete[] var_neighbor_count;
    delete[] score;
    delete[] time_stamp;
    delete[] conf_change;
    delete[] fix;
    delete[] score2;
    delete[] goodvar_stack2;
    delete[] clause_weight;
    delete[] sat_count;
    delete[] sat_var;
    delete[] unsat_stack;
    delete[] index_in_unsat_stack;
    delete[] unsatvar_stack;
    delete[] index_in_unsatvar_stack;
    delete[] unsat_app_count;
    delete[] goodvar_stack;
    delete[] already_in_goodvar_stack;
    delete[] cur_soln;
    delete[] opt_soln;
    delete[] clause_delete;
    delete[] unitclause_queue;
    delete[] temp_lit;
    delete[] selected;
    delete[] best_vars;
    delete[] scores;
    delete[] vars2;
    delete[] sel_cs;
    delete[] active_clause;
    delete[] active_var;
    delete[] remain_vars_in_clause;
    // delete[] unsat_cnt;
    delete[] flip_cnt;
}

void CCA_Solver::init_active_state()
{
    int c, v;
    active_clause_num = 0;
    active_var_num = 0;
    all_varable_active = false;
    for (v = 1; v <= num_vars; ++v)
    {
        active_var[v] = false;
        if (fix[v])
        {
            active_var_num++;
        }
    }
    for (c = 0; c < num_clauses; ++c)
    {
        active_clause[c] = false;
        remain_vars_in_clause[c] = 0;
    }
    rem_step = 0;
}

#endif