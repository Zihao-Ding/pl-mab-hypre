# 在 PortfolioParallel 模式中添加新 SAT Solver 的實現指南 - 第1部分

## 1. 系統架構概述

### 1.1 Painless的Slover分類

Painless支持兩大sloser算法類型：

| 枚舉值 | 名稱 | 分類 |
|--------|------|------|
| `CDCL` | Conflict-Driven Clause Learning | CDCL solver |

### 1.2 新Sloser需要實現的核心功能

新slover必須提供以下接口。

#### 必需的虛函數（繼承自SolverInterface）

```cpp
// 求解入口點：使用cube進行求解
virtual SatResult solve(const std::vector<int>& cube) = 0;

// 中斷求解，stopSolve設為true時暫停；必須調用unsetSolverInterrupt恢復
virtual void setSolverInterrupt() = 0;
virtual void unsetSolverInterrupt() = 0;

// 獲取公式中的變量數量
virtual unsigned int getVariablesCount() = 0;

// 獲取一個適合進行分支的變量（用於決策）
virtual int getDivisionVariable() = 0;

// Clause管理：添加單個clause
virtual void addClause(ClauseExchangePtr clause) = 0;
virtual void addClauses(const std::vector<ClauseExchangePtr>& clauses) = 0;

// 初始化clause列表（首次調用時）
virtual void addInitialClauses(const std::vector<simpleClause>& clauses, unsigned int nbVars) = 0;
virtual void addInitialClauses(const lit_t* literals, unsigned int clsCount, unsigned int nbVars) = 0;

// 從DIMACS文件加載公式
virtual void loadFormula(const char* filename) = 0;

// 獲取SAT證明模型（僅當返回SAT時有效）
virtual std::vector<int> getModel() = 0;

// 多樣化接口：根據solver ID修改內部狀態
virtual void diversify(const SeedGenerator& getSeed) = 0;
```