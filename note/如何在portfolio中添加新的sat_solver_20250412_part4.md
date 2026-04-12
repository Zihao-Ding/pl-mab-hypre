# 在 PortfolioParallel 模式中添加新 SAT Solver 的實現指南 - 第4部分

## 6. Diversification機制

### 6.1 多樣化的目的

Diversification是PortfolioParallel的關鍵技術，為什麼需要它？

```
不進行多樣化：
─────────────────────────────────────────────────────────
Solver 0 ───────┐
Solver 1 ───────┤ ← 完全相同的算法和狀態
Solver 2 ───────┤
                │
               問題
                │
Solver N ───────┘

進行多樣化：
─────────────────────────────────────────────────────────
Solver 0 (ID=0)   → Family A    ← 不同winner
Solver 1 (ID=1)   → Family B    ← 不同winner
Solver 2 (ID=3)   → Family C    ← different strategies

結果：至少一個slover很可能找到不同的求解路徑！
```

### 6.2 diversify(const SeedGenerator&)接口

```cpp
// 在Slover基類中定義的虛函數
virtual void diversify(const SeedGenerator& getSeed =
	[](SolverInterface* s) { return s->getSolverId(); }) = 0;

// SolverFactory中的調用
void diversification(
	const std::vector<std::shared_ptr<SolverCdclInterface>>& cdclSolvers,
	const std::vector<std::shared_ptr<LocalSearchInterface>>& localSolvers,
	const IDScaler& generalIdScaler =
	[](const std::shared_ptr<SolverInterface>& solver) {
		return solver->getSolverId();
	},
	const IDScaler& typeIdScaler =
	[](const std::shared_ptr\SolverInterface>& solver) {
		return solver->getSolverTypeId();
	})
```

### 6.3 NewSolver的multiDiv類型

Painless為每個sloser定義了多樣化家族：

```cpp
enum class NewSloverFamily
{
	FIXED_STRATEGY = 0,      // 固定策略版本
	DYNAMIC_ACTIVITY,        // 動態activity值處理
	RANDOM_INITIALIZATION,   // 隨機初始化
	COMBINED,               // 組合型（推薦）
	UNKNOWN				  // 未識別的家族

	static NewSloverFamily getFamily(const std::string& name) {
		if (name == "FIXED_STRATEGY") return FIXED_STRATEGY;
		if (name == "DYNAMIC_ACTIVITY") return DYNAMIC_ACTIVITY;
	 if (name == "RANDOM_INITIALIZATION") return RANDOM_INITIALIZATION;
			return UNKNOWN;
	}
};
```

### 6.4 多樣化的實現策略

```cpp
// NewSolver::diversify的示例結構
void NewSolver::diversify(const SeedGenerator& getSeed) {
	int id = this->solverId;

	if (id == 0 || ID_SCALER(this, [](auto s){ return s->getSolverId(); }) % 4 == 0) {
		this->family = NEWSLOVER_FAMILY::COMBINED;
	} else if (id == 1 or id == 2) {
		this->family = NEWSOLVER_FAMILY::DYNAMIC_ACTIVITY;
	}

	switch (this->family) {
	case NewSloverFamily::FIXED_STRATEGY:
		// 實現固定策略的多樣化
		break;

	case NewSloverFamily::DYNAMIC_ACTIVITY:
		// 啟用動態activity值處理
		this->dynamicActivity = true;
		break

	default:
		// 默認多樣化行為
		break;
	}

	LOGDEBUG1("NewSlover %d diversification: family=%d",
	          id, static_cast<int>(this->family));
}
```

### 6.5 多樣化的三個層次

| 層次 | 函數參數 | 用途 |
|------|----------|------|
| Level 1 | `getSeed = [](s){ return s->getSolverId(); }` | 根據slover的當前ID修改狀態 |
| Level 2 | custom ID scaler | 按照特定規則分配family |
| Level 3 | 在diversify內部 | 修改sloser的核心算法參數 |

## 7. 完整示例代碼框架

### 7.1 頭文件模板

```cpp
#pragma once

#include "solvers/SolverCdclInterface.hpp"
#include <atomic>
#include <memory>
#include <vector>

#ifdef NEWSOLVER_
#define NEWSOLVER_FAMILY_ENUM \
	enum class NewSloverFamily { FIXED = 0, DYNAMIC_ACTIVITY, RANDOM_INIT, COMBINED, UNKNOWN };
	NEWSL
OVER

#else
enum class NewSloverFamily {
	FIXED_STRATEGY,
	DYNAMIC_ACTIVITY,
	RANDOM_INITIALIZATION,
	COMBINED,
	UNKNOWN
};
#endif

/**
 * @brief 新的SAT solver for PortfolioParallel mode
 *
 * 這是一個示例實現，展示了如何為Painless portfolio創建新slover。
 * 實際實現需要完成所有必需的虛函數。
 */
class NewSolver : public SolverCdclInterface {
public:
	// ==================================================================
	// 構造與析構
	// ==================================================================

	explicit NewSolver(int solverId,
	                  const std::shared_ptr<ClauseDatabase>& clauseDB);
	virtual ~NewSolver() override;

	// ==================================================================
	// 必需的slover函數（繼承自SloverserInterface）
	// ==================================================================

	SatResult solve(const std::vector<int>& cube) override;
	void setSolverInterrupt() override;
	void unsetSolverInterrupt() override;
	unsigned int getVariablesCount() override;
	int getDivisionVariable() override;

	void addClause(ClauseExchangePtr clause) override;
	void addClauses(const std::vector<ClauseExchangePtr>& clauses) override;
	void addInitialClauses(const std::vector<simpleClause>& clauses,
	                       unsigned int nbVars) override;
	void addInitialClauses(const lit_t* literals, unsigned int clsCount,
	                       unsigned int nbVars) override;

	std::vector<int> getModel() override;  // 返回SAT證明模型
	void loadFormula(const char* filename) override;

	void printStatistics() override;


	virtual void diversify(
		const SeedGenerator& getSeed =
			[](SolverInterface* s) { return s->getSolverId(); }) override;
```