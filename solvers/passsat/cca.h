#ifndef _CCA_H_
#define _CCA_H_

#include "basis.h"
#include <vector>
#include <cassert>
#include <cstring>	
#define LONG_LONG_MIN -9223372036854775807

#define pop(stack) stack[--stack##_fill_pointer]
#define push(item, stack) stack[stack##_fill_pointer++] = item

inline void CCA_Solver::unsat(int clause)
{
	assert(active_clause[clause]);
	index_in_unsat_stack[clause] = unsat_stack_fill_pointer;
	push(clause, unsat_stack);
	// update appreance count of each var in unsat clause and update stack of vars in unsat clauses
	int v;
	for (lit *p = clause_lit[clause]; (v = p->var_num) != 0; p++)
	{
		unsat_app_count[v]++;
		if (unsat_app_count[v] == 1)
		{
			index_in_unsatvar_stack[v] = unsatvar_stack_fill_pointer;
			push(v, unsatvar_stack);
		}
	}
}

inline void CCA_Solver::sat(int clause)
{
	assert(active_clause[clause]);
	int index, last_unsat_clause;

	// since the clause is satisfied, its position can be reused to store the last_unsat_clause
	last_unsat_clause = pop(unsat_stack);
	index = index_in_unsat_stack[clause];
	unsat_stack[index] = last_unsat_clause;
	index_in_unsat_stack[last_unsat_clause] = index;

	// update appreance count of each var in unsat clause and update stack of vars in unsat clauses
	int v, last_unsat_var;
	for (lit *p = clause_lit[clause]; (v = p->var_num) != 0; p++)
	{
		unsat_app_count[v]--;
		if (unsat_app_count[v] == 0)
		{
			last_unsat_var = pop(unsatvar_stack);
			index = index_in_unsatvar_stack[v];
			unsatvar_stack[index] = last_unsat_var;
			index_in_unsatvar_stack[last_unsat_var] = index;
		}
	}
}

void CCA_Solver::print_solution(char *filename)
{
	int i;
	cout << filename << endl;
	cout << "v ";
	for (i = 1; i <= num_vars; i++)
	{
		if (cur_soln[i] == 0)
			cout << "-";
		cout << i;
		if (i % 10 == 0)
			cout << endl
				 << "v ";
		else
			cout << ' ';
	}
	cout << "0" << endl;
}

int CCA_Solver::verify_sol()
{
	int c, j;
	int flag;

	if (simplify == 0)
	{
		for (c = 0; c < num_clauses; ++c)
		{
			if (active_clause[c] == false)
				continue;
			flag = 0;
			for (j = 0; j < clause_lit_count[c]; ++j)
				if (cur_soln[clause_lit[c][j].var_num] == clause_lit[c][j].sense)
				{
					flag = 1;
					break;
				}

			if (flag == 0)
			{ // output the clause unsatisfied by the solution
				cout << "c clause " << c << " is not satisfied" << endl;

				cout << "c ";
				for (j = 0; j < clause_lit_count[c]; ++j)
				{
					if (clause_lit[c][j].sense == 0)
						cout << "-";
					cout << clause_lit[c][j].var_num << " ";
				}
				cout << endl;

				for (j = 0; j < clause_lit_count[c]; ++j)
					cout << cur_soln[clause_lit[c][j].var_num] << " ";
				cout << endl;

				return 0;
			}
		}
	}

	if (simplify == 1)
	{
		for (c = 0; c < num_clauses; ++c)
		{
			if (active_clause[c] == false)
				continue;
			flag = 0;
			for (j = 0; j < org_clause_lit_count[c]; ++j)
				if (cur_soln[org_clause_lit[c][j].var_num] == org_clause_lit[c][j].sense)
				{
					flag = 1;
					break;
				}

			if (flag == 0)
			{ // output the clause unsatisfied by the solution
				cout << "c clause " << c << " is not satisfied" << endl;

				if (clause_delete[c] == 1)
					cout << "c this clause is deleted by UP." << endl;

				cout << "c ";
				for (j = 0; j < org_clause_lit_count[c]; ++j)
				{
					if (org_clause_lit[c][j].sense == 0)
						cout << "-";
					cout << org_clause_lit[c][j].var_num << " ";
				}
				cout << endl;

				for (j = 0; j < org_clause_lit_count[c]; ++j)
					cout << cur_soln[org_clause_lit[c][j].var_num] << " ";
				cout << endl;

				return 0;
			}
		}
	}

	return 1;
}

// initiation of the algorithm
void CCA_Solver::init(int tries)
{
	int v, c;
	int i, j;
	int clause;
	bool restart = tries>0;
	// Initialize edge weights
	if (restart)
	{
		ave_weight = 1;
		delta_total_weight=0;
		for (c = 0; c < num_clauses; c++)
		{
			if (active_clause[c])
				clause_weight[c] = 1;
		}
	}

	// init solution
	for (v = 1; v <= num_vars; v++)
	{
		if (fix[v] == 0 && active_var[v] == false)
		{
			cur_soln[v] = -1;
		}
		if (need_reset_clause) {
			unsat_app_count[v] = 0;
		}
		if (restart) {
			if (fix[v] == 0 && active_var[v] == true)
			{
				if (tries % 2 == 0)
					cur_soln[v] = rand() % 2;
				else if(opt_soln[v]!=-1)
					cur_soln[v] = opt_soln[v];
				else 
					cur_soln[v] = 1-cur_soln[v];
				unsat_app_count[v] = 0;
				conf_change[v] = 1;
				time_stamp[v] = 0;
			}
		}
	}

	/* figure out sat_count, and init unsat_stack */
	if (restart || need_reset_clause)
	{
		unsat_stack_fill_pointer = 0;
		unsatvar_stack_fill_pointer = 0;
		for (c = 0; c < num_clauses; ++c)
		{
			if (clause_delete[c] == 1 || !active_clause[c])
				continue;
			
			sat_count[c] = 0;

			for (j = 0; j < clause_lit_count[c]; ++j)
			{
				if (cur_soln[clause_lit[c][j].var_num] == clause_lit[c][j].sense)
				{
					sat_count[c]++;
					sat_var[c] = clause_lit[c][j].var_num;
				}
			}

			if (sat_count[c] == 0)
				unsat(c);
		}
	}
	need_reset_clause = false;
	/*figure out var score*/
	int lit_count;
	goodvar_stack_fill_pointer = 0;
	for (v = 1; v <= num_vars; v++)
	{
		if (fix[v] == 1 || !active_var[v])
		{
			score[v] = -1000000;
			continue;
		}

		score[v] = 0;

		lit_count = var_lit_count[v];

		for (i = 0; i < lit_count; ++i)
		{
			c = var_lit[v][i].clause_num;
			if (!active_clause[c])
				continue;
			if (sat_count[c] == 0)
				score[v] += clause_weight[c];
			else if (sat_count[c] == 1 && var_lit[v][i].sense == cur_soln[v])
				score[v] -= clause_weight[c];
		}
		if (score[v] > 0)
		{
			already_in_goodvar_stack[v] = 1;
			push(v, goodvar_stack);
		}
		else
			already_in_goodvar_stack[v] = 0;
	}

	time_stamp[0] = 0;
}

int CCA_Solver::pick_var()
{
	int i, k, c, v;
	int best_var;
	lit *clause_c;
	best_var = goodvar_stack[0];

	for (i = 1; i < goodvar_stack_fill_pointer; ++i)
	{
		v = goodvar_stack[i];
		if (score[v] > score[best_var])
			best_var = v;
		else if (score[v] == score[best_var] && time_stamp[v] < time_stamp[best_var])
			best_var = v;
	}
	return best_var;
}

void CCA_Solver::pick_vars()
{
	int i, j, v;
	selected_nums = 10;
	for (i = 0; i < 10; i++)
	{
		sel_cs[i] = unsat_stack[rand() % unsat_stack_fill_pointer];
	}
	int best_vars_num = 0;
	for (i = 0; i < 10; i++)
	{
		best_vars[best_vars_num] = clause_lit[sel_cs[i]][rand() % clause_lit_count[sel_cs[i]]].var_num;
		if (selected[best_vars[best_vars_num]])
		{
			selected_nums--;
		}
		else
		{
			selected[best_vars[best_vars_num]] = 1;
			best_vars_num++;
		}
	}
	for (i = 0; i < selected_nums; i++)
		selected[best_vars[i]] = 0;
	if (selected_nums == 1)
	{
		flip(best_vars[0]);
		time_stamp[best_vars[0]] = step;
		return;
	}
	long long max_score1 = LONG_LONG_MIN, max_score2 = LONG_LONG_MIN;
	int num1, num2;

	for (i = 0; i < selected_nums; i++)
	{
		scores[i] = score[best_vars[i]];
		if (score[best_vars[i]] > max_score1)
		{
			max_score1 = score[best_vars[i]];
			num1 = i;
		}
		else if (score[best_vars[i]] == max_score1)
		{
			if (time_stamp[best_vars[i]] < time_stamp[best_vars[num1]])
					num1 = i;
		}
	}
	for (i = 0; i < selected_nums; i++)
	{
		flip2(best_vars[i]);
		if (goodvar_stack2_num > 0)
		{
			if (goodvar_stack2_num < 50)
			{
				vars2[i] = goodvar_stack2[0];
				for (j = 1; j < goodvar_stack2_num; ++j)
				{
					v = goodvar_stack2[j];
					if (score2[v] > score2[vars2[i]])
						vars2[i] = v;
					else if (score2[v] == score2[vars2[i]])
					{
						if (time_stamp[v] < time_stamp[vars2[i]])
							vars2[i] = v;
					}
				}
			}
			else
			{
				vars2[i] = goodvar_stack2[rand() % goodvar_stack2_num];
				for (j = 1; j < 50; ++j)
				{
					v = goodvar_stack2[rand() % goodvar_stack2_num];
					if (score2[v] > score2[vars2[i]])
						vars2[i] = v;
					else if (score2[v] == score2[vars2[i]])
					{
						if (time_stamp[v] < time_stamp[vars2[i]])
							vars2[i] = v;
					}
				}
			}
			scores[i] += score2[vars2[i]];
		}
		else
		{
			scores[i] -= 1000;
		}

		if (scores[i] > 0)
		{
			flip(best_vars[i]);
			flip(vars2[i]);
			time_stamp[best_vars[i]] = step;
			time_stamp[vars2[i]] = step;
			return;
		}

		if (scores[i] > max_score1)
		{
			if (scores[i] > max_score2)
			{
				max_score2 = scores[i];
				num2 = i;
			}
			else if (scores[i] == max_score2)
			{
				if (time_stamp[best_vars[i]] + time_stamp[vars2[i]] < time_stamp[best_vars[num2]] + time_stamp[vars2[num2]])
					num2 = i;
			}
		}
	}

	update_clause_weights();

	/*focused random walk*/

	if (max_score1 >= max_score2)
	{
		flip(best_vars[num1]);
		time_stamp[best_vars[num1]] = step;
	}
	else
	{
		flip(best_vars[num2]);
		flip(vars2[num2]);
		time_stamp[best_vars[num2]] = step;
		time_stamp[vars2[num2]] = step;
	}
}

void CCA_Solver::flip(int flipvar)
{
	flip_cnt[flipvar]++;
	cur_soln[flipvar] = 1 - cur_soln[flipvar];

	int i, j;
	int v, c;

	lit *clause_c;

	int org_flipvar_score = score[flipvar];

	// update related clauses and neighbor vars
	for (lit *q = var_lit[flipvar]; (c = q->clause_num) >= 0; q++)
	{
		clause_c = clause_lit[c];
		if (!active_clause[c])
			continue;
		if (cur_soln[flipvar] == q->sense)
		{
			++sat_count[c];

			if (sat_count[c] == 2) // sat_count from 1 to 2
				score[sat_var[c]] += clause_weight[c];
			else if (sat_count[c] == 1) // sat_count from 0 to 1
			{
				sat_var[c] = flipvar; // record the only true lit's var
				for (lit *p = clause_c; (v = p->var_num) != 0; p++)
					score[v] -= clause_weight[c];

				sat(c);
			}
		}
		else // cur_soln[flipvar] != cur_lit.sense
		{
			--sat_count[c];
			if (sat_count[c] == 1) // sat_count from 2 to 1
			{
				for (lit *p = clause_c; (v = p->var_num) != 0; p++)
				{
					if (p->sense == cur_soln[v])
					{
						score[v] -= clause_weight[c];
						sat_var[c] = v;
						break;
					}
				}
			}
			else if (sat_count[c] == 0) // sat_count from 1 to 0
			{
				for (lit *p = clause_c; (v = p->var_num) != 0; p++)
					score[v] += clause_weight[c];
				unsat(c);
			} // end else if

		} // end else
	}

	score[flipvar] = -org_flipvar_score;

	/*update CCD */
	int index;

	conf_change[flipvar] = 0;
	// remove the vars no longer goodvar in goodvar stack
	for (index = goodvar_stack_fill_pointer - 1; index >= 0; index--)
	{
		v = goodvar_stack[index];
		if (score[v] <= 0)
		{
			goodvar_stack[index] = pop(goodvar_stack);
			already_in_goodvar_stack[v] = 0;
		}
	}

	// update all flipvar's neighbor's conf_change to be 1, add goodvar
	int *p;
	for (p = var_neighbor[flipvar]; (v = *p) != 0; p++)
	{
		if (!active_var[v])
			continue;
		conf_change[v] = 1;

		if (score[v] > 0 && already_in_goodvar_stack[v] == 0)
		{
			push(v, goodvar_stack);
			already_in_goodvar_stack[v] = 1;
		}
	}
}
void CCA_Solver::flip2(int flipvar)
{
	cur_soln[flipvar] = 1 - cur_soln[flipvar];

	int i, j;
	int v, c;

	lit *clause_c;

	int org_flipvar_score = score[flipvar];

	for (i = 0; i < var_neighbor_count[flipvar]; i++)
	{
		if (active_var[var_neighbor[flipvar][i]])
			score2[var_neighbor[flipvar][i]] = score[var_neighbor[flipvar][i]];
	}

	// update related clauses and neighbor vars
	for (lit *q = var_lit[flipvar]; (c = q->clause_num) >= 0; q++)
	{
		clause_c = clause_lit[c];
		if (!active_clause[c])
			continue;
		if (cur_soln[flipvar] == q->sense)
		{
			int sc = sat_count[c] + 1;
			if (sc == 2)
			{ 
				score2[sat_var[c]] += clause_weight[c];
			}
			else if (sc == 1) // sat_count from 0 to 1
			{
				for (lit *p = clause_c; (v = p->var_num) != 0; p++)
					score2[v] -= clause_weight[c];
			}
		}
		else // cur_soln[flipvar] != cur_lit.sense
		{
			int sc = sat_count[c] - 1;
			if (sc == 1) // sat_count from 2 to 1
			{
				for (lit *p = clause_c; (v = p->var_num) != 0; p++)
				{
					if (p->sense == cur_soln[v])
					{
						score2[v] -= clause_weight[c];
						break;
					}
				}
			}
			else if (sc == 0) // sat_count from 1 to 0
			{
				for (lit *p = clause_c; (v = p->var_num) != 0; p++)
					score2[v] += clause_weight[c];
			} // end else if

		} // end else
	}

	cur_soln[flipvar] = 1 - cur_soln[flipvar];
	// score[flipvar] = -org_flipvar_score;
	score2[flipvar] = -org_flipvar_score;
	/*update CCD */

	// int v;
	goodvar_stack2_num = 0;
	// remove the vars no longer goodvar in goodvar stack
	// add goodvar
	for (i = 0; i < var_neighbor_count[flipvar]; ++i)
	{
		v = var_neighbor[flipvar][i];
		if (active_var[v] && score2[v] > 0)
		{
			goodvar_stack2[goodvar_stack2_num] = v;
			goodvar_stack2_num++;
		}
	}
}

void CCA_Solver::settings()
{
	set_clause_weighting();
}

bool CCA_Solver::local_search(char *filename)
{
	int flipvar;
	int satisfy_flag = 0;
	bool restart = false;
	bool good_sol = false;
	int add_flips_num = max(min(20000000, active_var_num * 1000), 5000000);

	for (tries = 0; tries <= max_tries; tries++)
	{
		settings();
		init(tries);
		max_flips = add_flips_num;
		for (step = 0; step < max_flips; step++)
		{
			if (unsat_stack_fill_pointer == 0)
				break;
			if (unsat_stack_fill_pointer < opt_unsat_num)
			{
				max_flips = step + add_flips_num;
				opt_unsat_num = unsat_stack_fill_pointer;
				for (int v = 1; v <= num_vars; v++)
					opt_soln[v] = cur_soln[v];
			}
			if (goodvar_stack_fill_pointer > 0)
			{
				flipvar = pick_var();
				flip(flipvar);
				time_stamp[flipvar] = step;
			}
			else
			{
				pick_vars();
			}

			if (step % 1000 == 0)
			{
				double elapse_time = get_runtime();
				if (elapse_time >= 1200)
					break;
			}
		}
		if (unsat_stack_fill_pointer == 0)
		{
			if (verify_sol() == 1)
			{
				return true;
			}
			else
				cout << "c Sorry, something is wrong." << endl; /////
		}
		else if (restart && opt_unsat_num<3 && !all_varable_active)
		{
			for (int v = 1; v <= num_vars; v++)
				cur_soln[v] = opt_soln[v];
			need_reset_clause = true;
			return true;
		}
		restart = true;
		double elapse_time = get_runtime();
		if (elapse_time >= 1200)
			break;
	}
	return false;
}

void CCA_Solver::add_active_vars(std::vector<int> &core, bool addall)
{
	if (!addall)
	{
		for (int var : core)
		{
			active_var_num++;
			active_var[var] = true;
			cur_soln[var] = rand() % 2;
			time_stamp[var] = 0;
			conf_change[var] = 1;
			unsat_app_count[var] = 0;
			for (int j = 0; j < var_lit_count[var]; j++)
			{
				int c = var_lit[var][j].clause_num;
				remain_vars_in_clause[c]++;
				if (remain_vars_in_clause[c] == clause_lit_count[c])
				{
					active_clause[c] = true;

					clause_weight[c] = 1;
					sat_count[c] = 0;

					for (int k = 0; k < clause_lit_count[c]; ++k)
					{
						conf_change[clause_lit[c][k].var_num] = 1;
						if (cur_soln[clause_lit[c][k].var_num] == clause_lit[c][k].sense)
						{
							sat_count[c]++;
							sat_var[c] = clause_lit[c][k].var_num;
						}
					}
					if (sat_count[c] == 0)
						unsat(c);
					active_clause_num++;
				}
			}
		}
		if (active_var_num == num_vars)
		{
			all_varable_active = true;
		}
	}
	else
	{
		all_varable_active = true;
		for (int i = 1; i <= num_vars; i++)
		{
			if (!fix[i])
				active_var_num++;
			active_var[i] = true;
			cur_soln[i] = rand() % 2;
			time_stamp[i] = 0;
			conf_change[i] = 1;
			unsat_app_count[i] = 0;
		}
		for (int i = 0; i < num_clauses; i++)
		{
			if (!clause_delete[i])
			{
				active_clause[i] = true;

				clause_weight[i] = 1;
				sat_count[i] = 0;

				for (int k = 0; k < clause_lit_count[i]; ++k)
				{
					conf_change[clause_lit[i][k].var_num] = 1;
					if (cur_soln[clause_lit[i][k].var_num] == clause_lit[i][k].sense)
					{
						sat_count[i]++;
						sat_var[i] = clause_lit[i][k].var_num;
					}
				}
				if (sat_count[i] == 0)
					unsat(i);
				active_clause_num++;
			}
		}
	}
	opt_unsat_num = unsat_stack_fill_pointer+1;
	for (int v = 1; v <= num_vars; v++)
		opt_soln[v] = cur_soln[v];
	cout << active_var_num << " active vars. " << active_clause_num << " active clauses." << num_clauses << " " << num_vars << endl;
	memset(flip_cnt, 0, sizeof(int)*num_vars);
	
}

#endif
