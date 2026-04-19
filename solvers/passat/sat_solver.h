#include "basis.h"
#include <vector>
#include <map>
#include <random>
#include <atomic>

class SATSolver
{
private:
    CCA_Solver solver;
    char *file_name;
    int num_vars;    // var index from 1 to num_vars
    int num_clauses; // clause index from 0 to num_clauses-1

    lit **var_lit;         // var_lit[i][j] means the j'th literal of var i.
    int *var_lit_count;    // amount of literals of each var
    lit **clause_lit;      // clause_lit[i][j] means the j'th literal of clause i.
    int *clause_lit_count; // amount of literals in each clause

    int *remain_vars_in_clause;
    int *unit_clause_queue;
    int unit_clause_num;
    int *clause_sat_count;

    int *cur_solution;
    int *local_solution;
    bool *in_conflict;
    int *clause_delete;
    int num_remain_vars;
    double *scores;
    double *score_var;
    int level_num;
    std::vector<int> variable_indices;
    std::vector<int> conflict_core_vars;
    int add_thred;
    int add_coe;

    int *flip_cnt;
    int hot_variable = -1;
    int **var_neighbor;
    int *var_neighbor_count;
    std::vector<int> neighbor_of_hot;
    // int *unsat_cnt;

    std::atomic<bool> terminate_solver{false};

private:
    int unitPropagation();
    bool isSatisfied();
    bool selectUnassignedVar();
    bool process();
    bool propagateLiteral(int var, int value);
    void getVariablesByScore();
    void reset(int level);
    void init();
    void alloc_memory();
    bool inputInit(int time);
    bool forwarding_check();
    void confilct_expansion(int add_num);
    void verify();
    // void get_neighbor_of_conflict(int c);

public:
    SATSolver(char *filename);
    int build_instance(char *filename);

    void free_memory();
    bool solve();

    SATSolver();
    void set_terminate_solver_cmd(bool val);
    void add_initial_clauses(const std::vector<std::vector<int>>& clauses, unsigned int nbVars);
    int preprocess();
};