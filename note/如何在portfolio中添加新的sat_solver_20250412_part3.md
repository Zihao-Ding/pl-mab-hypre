# 在 PortfolioParallel 模式中添加新 SAT Solver 的實現指南 - 第3部分

## 5. SolverFactory集成

### 5.1 createSolver函數簽名

```cpp
static SolverAlgorithmType createSolver(
	char type,                    // 字符類型標識，如 'n' 表示 NewSolver
	char importDBType,            // clause database類型：'s'=SingleBuffer, 'm'=Mallob, 'd'=PerSize
	std::shared_ptr<SolverInterface>& createdSolver);

static SolverAlgorithmType createSolver(
	char type,
	char importDBType,
	std::vector<std::shared_ptr<SolverCdclInterface>>& cdclSolvers,
	std::vector<std::shared_ptr<LocalSearchInterface>>& localSolvers);
```

### 5.2 在SolverFactory.cpp中添加case

```cpp
#ifdef NEWSOLVER_
	case 'n':
		if (__globalParameters__.solverType == SolverAlgorithmType::LOCAL_SEARCH) {
			createdSolver = std::make_shared<NewSolver>(
				id,
				importDB,
				__globalParameters__.localSearchFlips,
				__globalParameters__.maxDivNoise
			);
		} else {
			createdSolver = std::make_shared<NewSolver>(id, importDB);
		}
		return SolverAlgorithmType::CDCL;
#endif

#ifdef NEWSOLVER_LOCAL_
	case 'n':
		// Local search版本。
		break;
#endif
```

### 5.3 portfolio創建函數

```cpp
// 創建maxSolvers個slover，循環使用portfolio字符
void createSolvers(int maxSolvers,
                   char importDBType,
                   std::string portfolio,
                   std::vector<std::shared_ptr<SolverCdclInterface>>& cdclSolvers,
                   std::vector<std::shared_ptr<LocalSearchInterface>>& localSolvers)
{
	unsigned int typeCount = portfolio.size();
	LOGDEBUG1("Portfolio is '%s', of size %u", portfolio.c_str(), typeCount);
	for (size_t i = 0; i < maxSolvers && typeCount > 0; i++) {
		createSolver(portfolio.at(i % typeCount), importDBType,
		             cdclSolvers, localSolvers);
	}
}

// Portfolio字符示例：
// "n"     → 只創建 NewSolver (CDCL)
// "ny"    → 新sloser後面跟YalSat（local search）
// "nkcn"  → NewSolver, YalSat, CaDiCaL, NewSolver （循環重複）
```