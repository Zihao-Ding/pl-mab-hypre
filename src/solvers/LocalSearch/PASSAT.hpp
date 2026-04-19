#pragma once

#define PASSAT_

#include "containers/SimpleTypes.hpp"
#include "solvers/LocalSearch/LocalSearchInterface.hpp"

#include "../solvers/passat/sat_solver.h"

/* Includes not having any ifdef macro */
// extern "C"
// {
// #include "passat/sat_solver.h"
// }

typedef double clsweight_t;

namespace passat
{
class PASSAT : public LocalSearchInterface
{
  public:
	PASSAT(int _id, unsigned long flipsLimit, unsigned long maxNoise);

	~PASSAT();

	unsigned int getVariablesCount() {}

	int getDivisionVariable() {}

	void setSolverInterrupt();

	void unsetSolverInterrupt();

	void setPhase(const unsigned int var, const bool phase) {}

	SatResult solve(const std::vector<int>& cube);

	void addClause(ClauseExchangePtr clause) {}

	void addClauses(const std::vector<ClauseExchangePtr>& clauses) {}

	void addInitialClauses(const lit_t* literals, unsigned int clsCount, unsigned int nbVars) {};

	void addInitialClauses(const std::vector<simpleClause>& clauses, unsigned int nbVars);

	void loadFormula(const char* filename) {}

	std::vector<int> getModel();

	void printStatistics() {}

	void printParameters() {}

	void diversify(const SeedGenerator& getSeed) { 
		// TODO 
	};

  private:
	float PASSATWeightToTransfer(const float victimWeight) { return 0.0; }

	SatResult simpleInnerLoop() {}

  private:
  	std::unique_ptr<SATSolver> sat_solver;
	// SATSolver sat_solver;

	/// @brief State attribute to test if the solver should terminate or not
	std::atomic<bool> terminateSolver;

	/// @brief The maximum number of flips the search can reach
	unsigned long m_flipsLimit;

	/// @brief The maximum noise in randomization
	unsigned long m_maxNoise;
};}