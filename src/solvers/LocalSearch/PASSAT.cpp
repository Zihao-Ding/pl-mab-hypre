#include "PASSAT.hpp"
#include "utils/ErrorCodes.hpp"
#include "utils/NumericConstants.hpp"
#include "utils/Parameters.hpp"
#include "utils/Parsers.hpp"
#include "utils/System.hpp"

namespace passat
{
PASSAT::PASSAT(int _id, unsigned long flipsLimit, unsigned long maxNoise) 
    : m_flipsLimit(flipsLimit)
	, m_maxNoise(maxNoise)
	, LocalSearchInterface(_id, LocalSearchType::PASSAT)
{
    initializeTypeId<PASSAT>();
    this->sat_solver = std::make_unique<SATSolver>();
}

PASSAT::~PASSAT()
{
	// void tass_del (Yals *);
	// tass_del(this->myyals);
	// LOGDEBUG1("TaSSAT %d deleted!", this->getSolverId());
    this->sat_solver->free_memory();
}

void PASSAT::setSolverInterrupt()
{
	if (!this->terminateSolver) {
		LOG1("Asked TaSSAT %d to terminate", this->getSolverId());
		this->terminateSolver = true;
        sat_solver->set_terminate_solver_cmd(true);
	}
}

void PASSAT::unsetSolverInterrupt()
{
	this->terminateSolver = false;
    sat_solver->set_terminate_solver_cmd(false);
}

SatResult PASSAT::solve(const std::vector<int>& cube)
{
    SatResult res = SatResult::UNKNOWN;
    int preprocess = sat_solver->preprocess();
    if (preprocess = -1) {
        return res;
    }
    bool success = sat_solver->solve();
    if (success) {
        res = SatResult::SAT;
    }
    return res;
}

void PASSAT::addInitialClauses(const std::vector<simpleClause>& clauses, unsigned int nbVars)
{
    sat_solver->add_initial_clauses(clauses, nbVars);
}

std::vector<int> PASSAT::getModel()
{
    std::vector<int> model;
    return model;
}}