#include "sat_solver.h"
#include "basis.h"
#include "build.h"
#include "cca.h"
#include "preprocessor.h"
#include "cw.h"
#include <cassert>
#include <algorithm>
using namespace std;

void SATSolver::init()
{
    add_coe = 1;
    unit_clause_num = 0;
    level_num = 0;
    for (int i = 0; i < num_clauses; i++)
    {
        if (clause_delete[i] == 0)
        {
            remain_vars_in_clause[i] = clause_lit_count[i]; // -1 means unassigned
            clause_sat_count[i] = 0;
            if (clause_lit_count[i] == 1)
            {
                unit_clause_queue[unit_clause_num++] = i;
            }
        }
    }
    for (int i = 1; i <= num_vars; i++)
    {
        cur_solution[i] = -1;
    }
}

bool SATSolver::inputInit(int time)
{
    conflict_core_vars.clear();
    int max_flip=0;
    hot_variable=-1;
    for (int i = 1; i <= num_vars; i++)
    {
        if (local_solution[i] != -1)
        {
            if (flip_cnt[i] > max_flip)
            {
                max_flip = flip_cnt[i];
                hot_variable = i;
            }
            if (!in_conflict[i])
            {
                in_conflict[i] = true;
                num_remain_vars--;
            }
            cur_solution[i] = local_solution[i];
            if (propagateLiteral(i, cur_solution[i]))
                add_coe = 2;
        }
    }
    neighbor_of_hot.clear();
    if (hot_variable != -1)
    {
        for (int j = 0; j < var_neighbor_count[hot_variable]; j++)
        {
            int neighbor_var = var_neighbor[hot_variable][j];
            if (local_solution[neighbor_var] == -1)
            {
                neighbor_of_hot.push_back(neighbor_var);
            }
        }
    }
    if (neighbor_of_hot.size() > 0)
    {
        sort(neighbor_of_hot.begin(), neighbor_of_hot.end(), [this](int a, int b)
             {
                 if (score_var[a] == score_var[b])
                 {
                     return abs(scores[a * 2] - scores[a * 2 + 1]) > abs(scores[b * 2] - scores[b * 2 + 1]);
                 }
                 return score_var[a] > score_var[b]; // 按照 score_var 降序排序);
             });
    }
    if (time == 0)
    {
        if (num_remain_vars<10000){
            add_thred = 50;
        } else if (num_remain_vars<50000){
            add_thred = 50+((num_remain_vars-10000)/100);
        } else {
            add_thred = 500+(num_remain_vars-50000)/200;
        }
    }
    return true;
}

bool SATSolver::propagateLiteral(int var, int value)
{
    bool conflict = false;
    for (int j = 0; j < var_lit_count[var]; j++)
    {
        int c = var_lit[var][j].clause_num;

        if (var_lit[var][j].sense == value)
        {
            clause_sat_count[c]++;
        }
        else
        {
            remain_vars_in_clause[c]--;
            if (remain_vars_in_clause[c] == 1 && clause_sat_count[c] == 0)
            {
                unit_clause_queue[unit_clause_num++] = c;
            }
            else if (remain_vars_in_clause[c] == 0 && clause_sat_count[c] == 0)
            {
                conflict = true;
            }
        }
    }
    return conflict;
}

bool SATSolver::isSatisfied()
{
    for (int i = 0; i < num_clauses; i++)
    {
        if (clause_sat_count[i] == 0 && clause_delete[i] != 1)
            return false;
    }
    return true;
}

int SATSolver::unitPropagation()
{
    bool conflict;
    for (int i = 0; i < unit_clause_num; i++)
    {
        int c = unit_clause_queue[i];
        if (clause_delete[c] != 1 && clause_sat_count[c] == 0 && remain_vars_in_clause[c] == 1)
        {
            for (int j = 0; j < clause_lit_count[c]; j++)
            {
                int v = clause_lit[c][j].var_num;
                if (cur_solution[v] == -1)
                {
                    if (!in_conflict[v])
                    {
                        conflict_core_vars.push_back(v);
                        in_conflict[v] = true;
                    }
                    cur_solution[v] = clause_lit[c][j].sense;
                    bool conflict = propagateLiteral(v, clause_lit[c][j].sense);
                    if (conflict)
                    {
                        return i;
                    }
                    break;
                }
            }
        }
    }
    unit_clause_num = 0;
    return -1;
}

SATSolver::SATSolver(char *filename)
{
    file_name = filename;
    solver = CCA_Solver();
}

void SATSolver::getVariablesByScore()
{
    for (int i = 0; i < num_vars; ++i)
    {
        variable_indices[i] = i + 1; // 变元序号从 1 开始
    }

    // 使用 std::sort 按照 score_var 降序排序
    std::sort(variable_indices.begin(), variable_indices.end(), [this](int a, int b)
              {
                  if (score_var[a] == score_var[b])
                  {
                      return abs(scores[a * 2] - scores[a * 2 + 1]) > abs(scores[b * 2] - scores[b * 2 + 1]);
                  }
                  return score_var[a] > score_var[b]; // 按照 score_var 降序排序
              });
}

int SATSolver::build_instance(char *filename)
{
    if (solver.build_instance(filename) == 0)
    {
        cout << "Invalid filename: " << filename << endl;
        return -1;
    }

    bool satisfible = solver.preprocess();
    
    if (!satisfible)
    {
        cout << "s UNSATISFIABLE" << endl;
        FILE *rf = fopen("result-sat.txt", "a+");
        fprintf(rf, "my\t%s\t-2\n", filename);
        fclose(rf);
        solver.free_memory();
        return -1;
    }
    solver.init_active_state();
    int res = solver.build_neighbor_relation();
    if (res == 1)
    {
        cout << "s UNKNOWN" << endl;
        FILE *rf = fopen("result-sat.txt", "a+");
        fprintf(rf, "my\t%s\t-1\n", filename);
        fclose(rf);
        solver.free_memory();
        return -1;
    }
    num_vars = solver.get_var_num();
    num_remain_vars = num_vars;
    num_clauses = solver.get_clause_num();
    var_lit = solver.get_var_lit();
    var_lit_count = solver.get_var_lit_count();
    clause_lit = solver.get_clause_lit();
    clause_lit_count = solver.get_clause_lit_count();
    clause_delete = solver.get_deleted_clause();
    local_solution = solver.get_initial_solution();
    var_neighbor = solver.get_var_neighbor();
    var_neighbor_count = solver.get_var_neighbor_count();
    flip_cnt = solver.get_flip_cnt();
    alloc_memory();
    
    for (int i = 1; i <= num_vars; i++)
    {
        scores[i * 2] = 0.0;
        scores[i * 2 + 1] = 0.0;
        score_var[i] = 0.0;
        in_conflict[i] = false;
        for (int j = 0; j < var_lit_count[i]; j++)
        {
            int c = var_lit[i][j].clause_num;
            if (clause_delete[c] != 1)
            {
                scores[i * 2 + var_lit[i][j].sense] += 1.0 / clause_lit_count[c];
                score_var[i] += 1.0 / clause_lit_count[c];
            }
        }
    }
    getVariablesByScore();
    return 0;
}

void SATSolver::alloc_memory()
{
    int alloc_var_length = num_vars + 10;
    int alloc_clause_length = num_clauses + 10;
    remain_vars_in_clause = new int[alloc_clause_length]();
    unit_clause_queue = new int[alloc_clause_length];
    clause_sat_count = new int[alloc_clause_length]();
    cur_solution = new int[alloc_var_length];
    in_conflict = new bool[alloc_var_length];
    scores = new double[alloc_var_length * 2];
    score_var = new double[alloc_var_length];
    variable_indices.resize(num_vars);
}

void SATSolver::free_memory()
{
    solver.free_memory();
    delete[] remain_vars_in_clause;
    delete[] unit_clause_queue;
    delete[] clause_sat_count;
    // delete[] var_assign_level;
    delete[] cur_solution;
    delete[] in_conflict;
    delete[] scores;
    delete[] score_var;
}

bool SATSolver::forwarding_check()
{
    map<int, char> forward_check;

    vector<int> assigned_var;
    bool conflict = false;
    for (int i = 0; i < unit_clause_num; i++)
    {
        int c = unit_clause_queue[i];
        // assert(c != 0);
        if (clause_delete[c] != 1 && clause_sat_count[c] == 0 && remain_vars_in_clause[c] == 1)
        {
            conflict = false;
            for (int j = 0; j < clause_lit_count[c]; j++)
            {
                int v = clause_lit[c][j].var_num;
                if (cur_solution[v] == -1)
                {
                    assigned_var.push_back(v);
                    if (forward_check.find(v) != forward_check.end())
                    {
                        if (forward_check[v] != clause_lit[c][j].sense)
                        {
                            conflict = true;
                        }
                    }
                    else
                    {
                        forward_check[v] = clause_lit[c][j].sense;
                    }
                    break;
                }
            }
            if (conflict)
            {
                for (auto var : assigned_var)
                {
                    if (in_conflict[var] == false)
                    {
                        conflict_core_vars.push_back(var);
                        in_conflict[var] = true;
                    }
                }
                return true;
            }
        }
    }
    return false;
}

bool SATSolver::selectUnassignedVar()
{
    int var = 0;
    if (neighbor_of_hot.size() > 0)
    {
        for (int i = 0; i < neighbor_of_hot.size(); i++)
        {
            int v = neighbor_of_hot[i];
            if (cur_solution[v] == -1)
            {
                var = v;
                break;
            }
        }
    }
    if (var == 0) {
        neighbor_of_hot.clear();
        // neighbor_of_conflict.clear();
        for (int i = 0; i < num_vars; i++)
        {
            int v = variable_indices[i];
            if (cur_solution[v] == -1)
            {
                var = v;
                break;
            }
        }
    }
    if (var == 0)
        return true;

    if (!in_conflict[var])
    {
        conflict_core_vars.push_back(var);
        in_conflict[var] = true;
    }
    cur_solution[var] = scores[var * 2] > scores[var * 2 + 1] ? 0 : 1;
    bool conflict = propagateLiteral(var, cur_solution[var]);
    if (!conflict)
    {
        conflict = forwarding_check();
    }

    if (conflict)
    {
        unit_clause_num = 0;

        for (int j = 0; j < var_lit_count[var]; j++)
        {
            int c = var_lit[var][j].clause_num;

            if (var_lit[var][j].sense == cur_solution[var])
            {
                clause_sat_count[c]--;
            }
            else
            {
                remain_vars_in_clause[c]++;
            }
        }
        cur_solution[var] = 1 - cur_solution[var];

        conflict = propagateLiteral(var, cur_solution[var]);

        if (!conflict)
        {
            conflict = forwarding_check();
        }
    }
    return conflict;
}

bool SATSolver::process()
{
    int con_id;

    if ((con_id = unitPropagation()) != -1)
    {
        for (int i = con_id; i < unit_clause_num; i++)
        {
            int c = unit_clause_queue[i];
            if (clause_delete[c] != 1 && clause_sat_count[c] == 0 && remain_vars_in_clause[c] == 1)
            {
                for (int j = 0; j < clause_lit_count[c]; j++)
                {
                    int v = clause_lit[c][j].var_num;
                    if (cur_solution[v] == -1)
                    {
                        if (!in_conflict[v])
                        {
                            conflict_core_vars.push_back(v);
                            in_conflict[v] = true;
                        }
                        // cur_solution[v] = clause_lit[c][j].sense;
                    }
                }
            }
        }
        return false; 
    }
    if (isSatisfied())
    {
        return true; 
    }
    bool conflict = selectUnassignedVar();
    if (conflict)
    {
        return false; 
    }

    level_num++;
    if (level_num > 60000)
    {
        return false;
    }
    if (process())
    {
        return true;
    }
    return false;
}

void SATSolver::confilct_expansion(int add_num)
{
    int target_size = add_num;

    for (int j = num_vars - 1; j >= 0; j--)
    {
        int v = variable_indices[j];
        if (!in_conflict[v])
        {
            in_conflict[v] = true;
            conflict_core_vars.push_back(v);
            target_size--;
            if (target_size == 0)
                break;
        }
    }
}

void SATSolver::verify()
{
    for (int i = 0; i < num_clauses; i++)
    {
        if (clause_delete[i] != 1)
        {
            bool sat = false;
            for (int j = 0; j < clause_lit_count[i]; j++)
            {
                int v = clause_lit[i][j].var_num;
                int s = clause_lit[i][j].sense;
                if (cur_solution[v] == s)
                {
                    sat = true;
                    break;
                }
            }
            if (!sat)
            {
                cout << "Verification failed!" << endl;
                return;
            }
        }
    }
    cout << "s SATISFIABLE" << endl;
    cout << "v ";
    for (int i = 1; i <= num_vars; i++)
    {
        if (cur_solution[i] == 1)
            cout << i << " ";
        else if (cur_solution[i] == 0)
            cout << -i << " ";
    }
    cout << "0" << endl;
}

bool SATSolver::solve()
{
    for (int i = 0; i < 10000000; i++)
    {
        init();
        bool success = true;
        if (i > 0)
        {
            success = solver.local_search(file_name);
        }
        if (success)
        {
            inputInit(i);
            bool res = process();
            if (res == false)
            {
                if (conflict_core_vars.size() < add_coe * max(add_thred, num_remain_vars / 20))
                {
                    int add_num = add_coe * max(add_thred, num_remain_vars / 20) - conflict_core_vars.size();
                    confilct_expansion(add_num);
                }
                solver.add_active_vars(conflict_core_vars);
                num_remain_vars -= conflict_core_vars.size();
            }
            else
            {
                verify();
                return res;
            }
        }
        double elapse_time = get_runtime();
        if (elapse_time >= 1200)
            break;
        if (terminate_solver)
            break;
    }
    return false;
}

SATSolver::SATSolver()
{
    solver = CCA_Solver();
}

void SATSolver::set_terminate_solver_cmd(bool val)
{
    terminate_solver = val;
}

void SATSolver::add_initial_clauses(const std::vector<std::vector<int>>& clauses, unsigned int nbVars)
{
    solver.build_instance(clauses, nbVars);
}

int SATSolver::preprocess()
{
    bool satisfible = solver.preprocess();
    
    if (!satisfible)
    {
        cout << "s UNSATISFIABLE" << endl;
        FILE *rf = fopen("result-sat.txt", "a+");
        // fprintf(rf, "my\t%s\t-2\n", filename);
        fclose(rf);
        solver.free_memory();
        return -1;
    }
    solver.init_active_state();
    int res = solver.build_neighbor_relation();
    if (res == 1)
    {
        cout << "s UNKNOWN" << endl;
        FILE *rf = fopen("result-sat.txt", "a+");
        // fprintf(rf, "my\t%s\t-1\n", filename);
        fclose(rf);
        solver.free_memory();
        return -1;
    }
    num_vars = solver.get_var_num();
    num_remain_vars = num_vars;
    num_clauses = solver.get_clause_num();
    var_lit = solver.get_var_lit();
    var_lit_count = solver.get_var_lit_count();
    clause_lit = solver.get_clause_lit();
    clause_lit_count = solver.get_clause_lit_count();
    clause_delete = solver.get_deleted_clause();
    local_solution = solver.get_initial_solution();
    var_neighbor = solver.get_var_neighbor();
    var_neighbor_count = solver.get_var_neighbor_count();
    flip_cnt = solver.get_flip_cnt();
    alloc_memory();
    
    for (int i = 1; i <= num_vars; i++)
    {
        scores[i * 2] = 0.0;
        scores[i * 2 + 1] = 0.0;
        score_var[i] = 0.0;
        in_conflict[i] = false;
        for (int j = 0; j < var_lit_count[i]; j++)
        {
            int c = var_lit[i][j].clause_num;
            if (clause_delete[c] != 1)
            {
                scores[i * 2 + var_lit[i][j].sense] += 1.0 / clause_lit_count[c];
                score_var[i] += 1.0 / clause_lit_count[c];
            }
        }
    }
    getVariablesByScore();
    return 0;
}

// int main(int argc, char *argv[])
// {
//     int seed = 1, i;
//     SATSolver sat_solver(argv[1]);

//     start_timing();
//     if (sat_solver.build_instance(argv[1]) != 0)
//     {
//         cout << "Invalid filename: " << argv[1] << endl;
//         return -1;
//     }

//     srand(seed);
//     bool res = sat_solver.solve();
//     FILE *rf = fopen("result-sat.txt", "a+");
//     if (res)
//     {
//         double elapse_time = get_runtime();

//         fprintf(rf, "my\t%s\t1\t%0.3f\n", argv[1], elapse_time);
//     }
//     else
//     {
//         fprintf(rf, "my\t%s\t-1\n", argv[1]);
//     }

//     sat_solver.free_memory();
// }