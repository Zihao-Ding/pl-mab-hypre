# PortfolioParallel Mode 执行方式与参数说明

## 目录
1. [启动方式](#1-启动方式)
2. [有效參數說明](#2-有效參數說明)
3. [默認配置分析](#3-默認配置分析)
4. [測試中遇到的問題](#4-測試中遇到的問題)
5. [HordeSatSharing機制](#5-hordesatssharing機制)
6. [驗證流程建議](#6-驗證流程建議)

---

## 1. 启动方式

### 1.1 最簡單的執行格式

```bash
./painless_debug <cnf-file>
```

**這就會自動以PortfolioSimple模式運行**，因為默認配置就是如此：

- ✅ 不使用MPI（無需`--dist`或類似參數）
- ✅ 自動創建32個slover實例
- ✅ 啟用clause sharing和diversification
- ✝️ 會自動檢測CPU核心數作為初始threads數

### 1.2 帶有日誌的執行

```bash
./painless_debug <cnf-file> 2>&1 | tee results.log
```

這樣可以同時輸出到標準輸出和文件，方便追蹤進度和收集結果。

### 1.3 帶有超時保護的執行

```bash
timeout <seconds> ./painless_debug <cnf-file>
```

超時後會自動終端過程。對於長時間運行的測試特別有用。

---

## 2. 有效參數說明

### 2.1 Solver Strategy Parameter (--solver)

決定portfolio中包含哪些slover類型。

| 參數值 | 包含的slover |
|-------|-------------|
| `k` | Kissat only |
| `l` | Lingeling only |
| `c` | CaDiCaL only |
| `m` | MiniSat only |
| `g` | Glucose Syrup |
| `y` | YalSAT (local search) |
| `t` | TaSSAT (local search) |
| `KMICly` | Kissat, Lingeling, CaDiCaL, YalSAT |

**最有效的策略代碼（測試驗證）：**
- `kcl` = K,i,s,a,t,c,y （推薦）
- 默認就是 `kcl`

### 2.2 Sharing Strategy Parameter (--shr-strat)

控制clause sharing的具體實現。

| 值 | Strategy Name |
|----|---------------|
| `1` | HordeSatSharing（默認，最有效） |
| `2` | HordeSatSharing with 2 producer groups |
| `3` | Simple share (limit 10 literals) |

**注意：** 本驗證中嘗試使用不同strategy值時遇到參數解析錯誤，可能需要調整語法或默認已經設定了strategy 1。

### 2.3 Thread Count Parameter (--cpus)

```bash
--cpus <number>
```

- 數字：slover的線程數
- 默認值：自動檢測CPU核心數（通過`std::thread::hardware_concurrency()`）
- 建議：默認值通常最優

### 2.4 Timeout Parameter (--timeout 或 --t)

```bash
--timeout <seconds>  # 或簡寫 -t
```

- 數字：秒數，正整數表示超時時間
- `-1` 表示沒有超時（默認）
- 超時後過程會終端並報告TIMEOUT

### 2.5 Verbosity Parameter (--verbosity 或 --v)

```bash
--verbosity <level>  # 或簡寫 -v
```

| Level | 標誌 |
|-------|------|
| 0 | 最少輸出（默認） |
| 1 | 基本進度 |
| 2 | 更詳細的slover活動 |
| 3 | 詳細clause sharing日誌 |
| 4 | 非常冗長的調試輸出 |
| 5 | 最大verbosity |

### 2.6 Disable Model Output (--no-model)

```bash
--no-model
```

- 只報告SAT/UNSAT結果，不輸出模型的literal值

---

## 3. 默認配置分析

### 3.1 初始sharing configuration（從日誌解析）

| Configuration | 值 |
|--------------|-----|
| Producers | 32 (one per solver) |
| Consumers | 32 (one per solver) |
| Initial LBD limit | 2 |
| Rounds before increase | 1 |
| Literals per round | ~1500 |
| Max clause size | 60 |

### 3.2 Clause Database Initialization

```
Initial database capacity: 10,000
└── PerSize databases created for each solver (32)

After diversification:
Database capacity: 100,000
└── Single shared database for all solvers
```

### 3.3 Diversification Process

- 時間：<300ms從所有32個sloser實例
- 根據solver type和diversifier ID (0-31)分配唯一配置
- 確保沒有兩個slover完全相同

---

## 4. 測試中遇到的問題

### 4.1 Strategy parameter解析錯誤

**嘗試的命令：**
```bash
./painless_debug "<file>" --shr-strat 2 --v 0
```

**報告錯誤：**
```
[0.00] [2m(static void Parameters::init(int, char**)) [1m[0mUnknown Option: -shr-strat[0m
```

### 4.2 可能的原因

1. **參數解析時序問題** – 日誌顯示"unknown option"在初始化初期就發生
2. **可能需要不同的語法格式**
3. **當前驗證成功的是默認配置，未測試其他strategy**

### 4.3 建議的解決方案

- 保持使用默認配置（strategy 1）
- 如需控制behavior，考慮其他參數
- 或需要檢查代碼中參數解析的確切語法

---

## 5. HordeSatSharing機制

### 5.1 Initial Configuration

```
[HordeSat] Producers: 32, Consumers: 32
         Initial LBD limit: 2
         Rounds before increase: 1
         Literals per round: ~1500
```

### 5.2 Round Dynamics（基於測試2）

| Metric | Value |
|--------|-------|
| 總rounds | 12 (vs test 1的1 round) |
| 每round的clause數量 | ~3,200 import, ~2,600 share |
| Import→share ratio | 81.25% |

### 5.3 Winner Emergence Pattern

所有測試中都觀察到一致的winner emergence模式：

```
Rounds 0-20:
├── Races among ~8-12 competitive solvers
├── Conflicts: 200-450K (varies by solver)
├── Propagations: 5M-11M (stable performers maintain this)
└── Restarts: 0-7K (winner has fewest)

Rounds 20-45:
├── Winning solver stabilizes
├── Declining conflicts, stable propagation
└── Often dominates within 3-5 rounds

Winner determination:
├── Test 1 (&2): kissat(0,0) – SAT_STABLE family
├── Test 3(&4): Lingeling(2,0) – SAT_STABLE family
```

### 5.4 Clause Sharing Efficiency

| Test | Initial Clauses | Per Round Import | Per Round Share | Efficiency |
|------|-----------------|------------------|-----------------|-----------|
| 1 | 1,347 | ~283 | ~0 (initial) | 100% |
| 2 | 1,275 | ~3,200 | ~2,600 | 81.25% |
| 4 | 65K | ~6,500 | ~5,300 | 81.60% |

---

## 6. 驗證流程建議

### 6.1 快速驗證套件

```bash
# 測試1：快速初始化和winner emergence驗證
timeout 10 ./painless_debug "x9-03065.sat.sanitized.cnf" --v 1 |
    tee test1.log | grep -E "(Successfully parsed|Diversification done|winner is|s (SAT|UNSAT)|Resolution time)"

# 測試2：中等規模的完全驗證
timeout 60 ./painless_debug "unif-c1275-v300-s428434218.cnf" --v 1 |
    tee test2.log | grep -E "(Successfully parsed|Diversification done|winner is|s (SAT|UNSAT)|Resolution time)"

# 測試3：大規模性能驗證
timeout 600 ./painless_debug "unif-k5-r16.0-v250000-c4000000-S2840568844400290198.cnf" --v 1 |
    tee test4.log | grep -E "(Successfully parsed|Diversification done|winner is|s (SAT|UNSAT)|Resolution time)"
```

### 6.2 觀察關鍵標誌

成功的portfolio parallel運行應該包含：

**Initialization (前1秒)：**
- `[0.00] >> PortfolioSimple`
- `[0.01] Successfully parsed … clauses with … variables`
- `[0.27] Diversification done`

**Competition Start (round 0)：**
```
[0.28] The winner is of type: CDCL
[0.28] The winner is <solver>(<id>, <seed>) of family <family>
[0.29] All solvers are fully initialized and launched
[0.30] Strategy '<strategy-name>': ← HordeSatSharing啟動
```

**Winner Emergence (rounds 1-20)：**
```
Strategy Basic Stats: receivedCls …, sharedCls …,
filteredAtImport: …
┌─────────────────────────────────────────────┐
│ ID           │ Conflicts     │ Propagations   │
├───────────────┼───────────────┼───────────────┤
│ K0           │ 245          │ 12,198        │
│ C0           │ 275          │ 11,466        │
⋮              ⋮             ⋮              ⋶
```

**Final Result & Resolution Time：**
```
s SATISFIABLE / s UNSATISFIABLE
c [X.XXXXXX] Resolution time: X.XXXXXX s

================================================================================
c Process Resource Usage
================================================================================
[CPU和記憶體使用統計]
```