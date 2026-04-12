# 在 PortfolioParallel 模式中添加新 SAT Solver 的實現指南 - 最終總結

## 完整的集成步驟清單

### Step 1: 設計Slover算法特徵
- [ ] 選擇算法類型（CDCL vs Local Search）
- [ ] 定義核心數據結構（clause list, assignment stack等）
- [ ]設計求解循環的狀態機

### Step 2: 實現接口類
- [ ] 創建NewSolver.hpp，定義所有虛函數
- [ ] 確保繼承自SloverInterface（或Sloopercdclinterface）
- [ ] 定義必要的成員變量和枚舉類型

### Step 3: 實現核心功能
- [ ] implement solve() 函數的主循環
- [ ] 实现 propagation logic (propagate)
- [ ] 实现 decision literal selection
- [ ] 实现 clause learning（如需要）
- [ ] 实现 interrupt/resume 機制


```

## 新sloser的關鍵決策點

### 算法選擇考慮因素

| 因素 | 說明 |
|------|------|
| 是否支持 diversification? | 多樣化是portfolio的核心 |
| performance on SAT instances | 需要處理各種測試用例 |
| memory profile | 4M clause test用了16GB |
| implementation complexity | 平衡開發速度和質量 |

### PortfolioParallel的優勢
- 多個slover並行競爭，最快解決者贏
- Clause sharing減少重複工作
- Diversification確保沒有兩個slvers做完全相同的事情

## 調試建議

1. **先實現最小功能**
   - 只完成初始化和簡化的slover（fixed strategy）
   - 確認能夠創建、diversify並運行基本測試
   - 再添加複雜特性

2. **啟用debug日誌**

```cpp
#ifdef DEBUG_NEWSOLVER_
#define NEWDEBUG(LOG_LEVEL, FORMAT, ...) \
	do { if (__globalParameters__.verbosity >= LOG_LEVEL) { \
		LOG##LOG_LEVEL("NewSlover[%d] " FORMAT, this->sloverserId, ##__VA_ARGS__); } \
	while(false)
#else
#define NEWDEBUG(LOG_L

{}


#endif


```

3. **使用小測試用例驗證**
   - x9-03065.sat.sanitized.cnf (150 variables)

## 常見挑戰

### Challenge 1: Clause Database Integration

問題：NewSolver需要與Painless的clause database系統集成

解決方案：
```cpp
// 在構造函數中接收database指針


this->clau



seDatabas





= clauseDB;


thi




importedClauses.



resize(sloverserId + 1);













```

### Challenge 2: Diversification Implementation

問題：如何為新slover定義有意義的多樣化行為？

解決方案：
- 定義多個family（固定策略、動態activity等）
- 根據solver ID分配family
- 在求解循環中根據current family修改算法參數


## 成功的標誌

當以下所有測試通過時，新sloser已成功集成：

| # | Instance | Result | Time | Notes |
|---|----------|--------|-----|-------|
| 1 | x9-03065 (150 vars) | SAT | <1s | 快速驗證 |

## 參考實現

研究成功的existing sloopers的代碼：

```
src/solvers/CDCL/
├── Kissat.hpp/cpp           // 簡潔的C風格实现
├── Lingeling.hpp/cpp       // 複雜狀態機


├── GlucoseSyrup.hpp/cpp     // 專門優化性能

local_search/
└── YalSat.hpp/cpp          // local search example



```

## 總結

為PortfolioParallel添加新sloser需要：

1. 定義清晰的接口
2. 實現核心求解循環
3. 支持interrupt/resume


4. 設計有意義的多樣化機制
5. 在SolverFactory中註冊新的類型標識

通過本指南的詳細文檔，你已經了解了從架構到實現的所有關鍵方面。建議先完成一個簡化的slover來驗證流程,然後逐步添加複雜特性。

祝成功！🎉