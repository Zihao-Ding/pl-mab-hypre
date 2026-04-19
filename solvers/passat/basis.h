#ifndef _BASIS_H_
#define _BASIS_H_

#include <iostream>
#include <fstream>
#include <cstdlib>
#include <cmath>
#include <sys/times.h>
#include <unistd.h>
#include <vector>

using namespace std;

enum type
{
	SAT3,
	SAT5,
	SAT7,
	strSAT
};

/* limits on the size of the problem. */
#define MAX_VARS 4000010
#define MAX_CLAUSES 20000000

// Define a data structure for a literal in the SAT problem.
struct lit
{
	int clause_num; // clause num, begin with 0
	int var_num;	// variable num, begin with 1
	int sense;		// is 1 for true literals, 0 for false literals.
};
static struct tms start_time, local_run_time;
static double get_runtime()
{
	struct tms stop;
	times(&stop);
	return (double)(stop.tms_utime - start_time.tms_utime + stop.tms_stime - start_time.tms_stime) / sysconf(_SC_CLK_TCK);
}
static double get_local_runtime()
{
	struct tms stop;
	times(&stop);
	return (double)(stop.tms_utime - local_run_time.tms_utime + stop.tms_stime - local_run_time.tms_stime) / sysconf(_SC_CLK_TCK);
}
static void start_timing()
{
	times(&start_time);
}
static void start_local_timing()
{
	times(&local_run_time);
}
/*parameters of the instance*/
class CCA_Solver
{
private:
	int num_vars;	 // var index from 1 to num_vars
	int num_clauses; // clause index from 0 to num_clauses-1
	int max_clause_len;
	int min_clause_len;
	int formula_len = 0;
	double avg_clause_len;
	double ratio;

	/* literal arrays */
	lit **var_lit;		   // var_lit[i][j] means the j'th literal of var i.
	int *var_lit_count;	   // amount of literals of each var
	lit **clause_lit;	   // clause_lit[i][j] means the j'th literal of clause i.
	int *clause_lit_count; // amount of literals in each clause

	lit **org_clause_lit;	   // clause_lit[i][j] means the j'th literal of clause i.
	int *org_clause_lit_count; // amount of literals in each clause
	int simplify = 0;

	/* Information about the variables. */
	int *score;
	int *time_stamp;
	int *conf_change;
	int **var_neighbor;
	int *var_neighbor_count;
	int *fix;
	int *score2;
	int *goodvar_stack2;
	int goodvar_stack2_num;
	/* Information about the clauses */
	int *clause_weight;
	int *sat_count;
	int *sat_var;

	// unsat clauses stack
	int *unsat_stack; // store the unsat clause number
	int unsat_stack_fill_pointer;
	int *index_in_unsat_stack; // which position is a clause in the unsat_stack

	// variables in unsat clauses
	int *unsatvar_stack;
	int unsatvar_stack_fill_pointer;
	int *index_in_unsatvar_stack;
	int *unsat_app_count; // a varible appears in how many unsat clauses

	// configuration changed decreasing variables (score>0 and confchange=1)
	int *goodvar_stack;
	int goodvar_stack_fill_pointer;
	int *already_in_goodvar_stack;

	// unit clauses preprocess
	lit *unitclause_queue;
	int unitclause_queue_beg_pointer = 0;
	int unitclause_queue_end_pointer = 0;
	int *clause_delete;

	/* Information about solution */
	int *cur_soln; // the current solution, with 1's for True variables, and 0's for False variables
	int *opt_soln;
	// cutoff
	int max_tries = 10000;
	int tries;
	int max_flips = 2000000000;
	int step;

	int selected_nums;
	int *selected;
	int *best_vars;
	int *scores;
	int *vars2;
	int *sel_cs;
	int threshold;
	float p_scale; // w=w*p+ave_w*q
	float q_scale = 0;
	int scale_ave; // scale_ave==ave_weight*q_scale

	int q_init = 0;
	int ave_weight = 1;
	int delta_total_weight = 0;
	int *temp_lit; // the max length of a clause can be MAX_VARS

	bool *active_var;
	bool *active_clause;
	int *remain_vars_in_clause;
	int active_var_num;
	int active_clause_num;
	int rem_step;
	bool dump = false;
	int opt_unsat_num;
	bool all_varable_active;
	int smooth_time;
	// int * unsat_cnt;
	int * flip_cnt;
	bool need_reset_clause;

private:
	int pick_var();
	void pick_vars();

	void flip(int flipvar);
	void flip2(int flipvar);

	void unsat(int clause);
	void sat(int clause);

	bool unit_propagation();

	void smooth_clause_weights();
	void update_clause_weights();
	void scale_all_weights();

	void set_clause_weighting();

	void init(int tries);

	void allocate_memory();

public:
	CCA_Solver();
	int build_instance(char *filename);
	int build_instance(const std::vector<std::vector<int>>& clauses, unsigned int nbVars);
	int build_neighbor_relation();
	void settings();
	bool local_search(char *filename);
	void free_memory();
	bool preprocess();
	void print_solution(char *filename);
	int verify_sol();

	int get_var_num()
	{
		return num_vars;
	}
	int get_clause_num()
	{
		return num_clauses;
	}
	lit **get_var_lit()
	{
		return var_lit;
	}
	int *get_var_lit_count()
	{
		return var_lit_count;
	}
	lit **get_clause_lit()
	{
		return clause_lit;
	}
	int *get_clause_lit_count()
	{
		return clause_lit_count;
	}
	int *get_initial_solution()
	{
		return cur_soln;
	}
	int *get_deleted_clause()
	{
		return clause_delete;
	}
	int **get_var_neighbor()
	{
		return var_neighbor;
	}
	int *get_var_neighbor_count()
	{
		return var_neighbor_count;
	}
	int *get_flip_cnt()
	{
		return flip_cnt;
	}
	// int *get_unsat_cnt()
	// {
	// 	return unsat_cnt;
	// }
	void add_active_vars(std::vector<int> &core, bool addall = false);
	void init_active_state();
};
#endif
