# 在 PortfolioParallel 模式中添加新 SAT Solver 的實現指南 - 第2部分

## 3. CDCL Souter的子接口

### 3.1 枚舉類型定義

Painless為CDCL solver提供了專門的枚舉類型：

```cpp
enum class SolverCdclType
{
	GLUCOSE = 0,        // Glucose Syrup solver
	LINGELING = 1,      // Lingeling solver
	CADICAL = 2,        // CaDiCaL solver
	MINISAT = 3,        // MiniSat solver
	KISSAT = 4,         // Kissat solver
	MAPLECOMSPS = 5,    // MapleCOMSPS solver
	KISSAT_MAB = 6,     // Kissat Multi-Armed Bandit solver
	KISSAT_INC = 7      // Kissat Incremental (INC) solver
};
```

### 3.2 SolverCdclInterface繼承結構

```cpp
class SolverCdclInterface : public SolverInterface,
                              public SharingEntity
{
public:
	// CDCL專用的必需函數
	virtual void setPhase(const unsigned int var, const bool phase) = 0;
	virtual void bumpVariableActivity(const int var, const int times) = 0;
	virtual std::vector<int> getFinalAnalysis() = 0;      // UNSAT的最終分析
	virtual std::vector<int> getSatAssumptions() = 0;     // 當前的假設列表

	// 枚舉類型訪問器
	SolverCdclType getSolverType() { return m_cdclType; }

	// 構造函數和析構函數
	SolverCdclInterface(int solverId,
	                    const std::shared_ptr<ClauseDatabase>& clauseDB,
	                    SolverCdclType solverCdclType);
	virtual ~SolverCdclInterface();

protected:
	std::shared_ptr<ClauseDatabase> m_clausesToImport;  // 用於導入clause的database
	SolverCdclType m_cdclType;                          // 此slover的類型

	// 統計數據（在printStatistics中報告）
	unsigned long m_propagations = 0;
	unsigned long m_decisions = 0;
	unsigned long m_conflicts = 0;
	unsigned long m_restarts = 0;
	double m_memoryPeakKB = 0.0;
};
```

### 3.2.1 setPhase函數目的

```cpp
// 設置變量的初始相位（用於啟發式決策）
virtual void setPhase(const unsigned int var, const bool phase) {
	// 實現：根據var的當前phase狀態更新內部結構
}
```

### 3.2.2 bumpVariableActivity函數目的

```cpp
// 提升活躍變量的activity值（CDCL中的標準技術）
virtual void bumpVariableActivity(const int var, const int times) {
	// 實現：將var的activity值加上隨機噪聲，確保沒有variable佔優勢
}
```

### 3.2.3 getFinalAnalysis函數目的

```cpp
// 獲取UNSAT證明的最終分析（clause learning的基礎）
virtual std::vector<int> getFinalAnalysis() {
	// 返回表示學習clause重要性的數據結構
}
```

### 3.2.4 getSatAssumptions函數目的

```cpp
// 獲取當前的假設列表（用於cube求解）
virtual std::vector<int> getSatAssumptions() {
	// 返回當前需要滿足的clause對應的變量
}
```

## 4. 新Sloser的實現步驟

### 4.1 文件結構規劃

新slover需要的文件：

```
src/solvers/
├── NewSolver.hpp          # 主頭文件（接口定義）
├── NewSolver.cpp         # 實現文件
└── (可能)NewSolve rFamily.hpp  # 多樣化家族定義
```