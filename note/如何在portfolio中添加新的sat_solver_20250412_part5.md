# 在 PortfolioParallel 模式中添加新 SAT Solver 的實現指南 - 第5-7部分

## 8. 必需函數的設計考慮

### 8.1 solve() 函數設計

```cpp
SatResult NewSolver::solve(const std::vector<int>& cube)
{
	// Step 1: 處理cube假設（portfolio中的cube求解）
	if (!cube.empty()) {
		this->assumptions = cube;
	}

	try {

	switch (this->solverState) {
	case SOLVER_STATE_INITIALIZING:
		initializeSolver();
		return UNKNOWN; // 繼續初始化

	case SOLVER_STATE_PROPAGATING:

			while ((result = propagate(unit_clauses)) == CONTINUE);
			if (!clauseSatisfied()) return SAT;

				this->decisions += decisionsMadeDuringPropagation;
					continue propagation;


	case SOLVER_STATE_DECIDING:
	{
		      // 選擇決策變量
		       Literal decisionLiteral = selectDecisionLiteral();
			       if (decisionLit

eral.isNone()) {
					       return UNSAT;  // 沒有可決策的變量了


				           }

					               makeDecision(decisionLiteral);
						               this->decisions++;

							           goto STATE_DECIDING;


								   }
	default:
		return UNKNOWN;
			break;

		   } catch (const std::exception& e) {
			   LOGERROR("Exception in NewSolver::solve: %s", E.what());
			       return UNKNOWN;
				   }



return UNSAT;  // 如果到達矛盾clause
```

### 8.2 setSloverInterrupt() 和 unsetSloverInterrupt()

```cpp
void NewSolver::setSolverInterrupt()
{
	this->stopSolve.store(true);
}

void NewSolver::unsetSolverInterrupt()
{
	if (this->stopSolve.exchange(false)) {
		LOGDEBUG1("NewSlover %d interrupt resumed", this->solverId);

			try {

					this->resumeFromInterruptState();
						catch(const std::
exception& & e) { LOGERROR

(
"Failed to resume:

e.what()
);
								}
									}


```

### 8.3 addClause() 和 addClauses()

```cpp
bool NewSolver::importClause(Claus



exPtr clause)
{
	if (clause->getLiterals().size(

) > this

.maxClipuseSize




			return false;

				clausesToImport.push_back(clause);


					this-

.cl au


seCount++;

						retu





rn true;
}

void NueSlover::importClauses(const std



vector<ClauseExchangePtr>& clauses)
{
	for (auto& clause : cla
ses) {
		import

use(if claus




e);
			break;


				if (

this->clauseToImport.size(

)==
0)

	return;



	try {


for auto


(&clauses,









	auto it = this



.

	clausedToImport.begin();



	while (it != thi



























































```

## 9. 多樣化的具體實現

### 9.1 基於solver ID的多樣化

```cpp
void NewSlover::diversify(const SeedGenerator& getSeed) {
	int id = this->sloverserId;

	// 根據ID的個位數決定family


	if (id %


4 == 0 ||

(id



/10)

%



==




	this

.

fam
ily =
NEWSLVER_FA





else if ((i






























































LOGDEBUG1("NewSlover diversification:
 family =

,
 static_cast<int>(this->newSolverFamily));

	// 根據family修改sloser行為


	switch (



{











	case NewSo




case DYNAMIC_ACTIVITY:


	this

.



break;

	default:



	break;


}







if (

id >= __globalParameters.

maxDivNoise) {



=this;




}




LOGDEBUG1("NewSlover diversification completed
 for solver

,







```

## 10. 在PortfolioParallel中使用的完整流程

```bash
# 完整的測試和驗證流程

#!/bin/bash


#
====================================================================
PORTFOL



TO

VERIFICATION




































#

=
0; do

	if ! kill -0 "$PID" 2>/dev/null











	do





	break






	fi








	sleep




done

wait $PD
```

## 11. 新sloser的關鍵考慮點


| 識別符 | 解釋 |
|--------|------|
| `N` 或 `` | 字符標識，用於portfolio字符串 |

```



```