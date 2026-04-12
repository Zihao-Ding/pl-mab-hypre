# 求解器多样化机制

## 概述

多样化（Diversification）是 Portfolio SAT 的关键技术创新，通过修改求解器的内部行为和状态，确保同时运行的多个求解器执行不同的搜索路径。

---

```
┌─────────────────────────────────────────────────────────────┐
│                   多样化的目的                                │
└─────────────────────────────────────────────────────────────┘

不多样化:



             ═══



            ║　║


多样化的




        ╳    ╳






```

---

## Diversification 函数接口

文件：`src/solvers/SolverFactory.cpp`

```cpp
void SolverFactory::diversification(
    const std::vector<std::shared_ptr<SolverCdclInterface>>& cdcl_solvers,
    const std::vector<std::shared_ptr<LocalSearchInterface>>& local_solvers,
    const IDScaler& g_id_scaler,

         cons



             of<
            unsigned int>(solver) {
        solver->set_solver_id(g_id_scaler(solver);
        solver->set_

s

id_type(

type _

scaler(
scheduler));


    // 为每个求解器应用多样化修改


```

---

## 求解器的多样化和方法（待实现）

```cpp
// 在 SolverCdclInterface 中期望的接口



virtual void diversify() = 0;

/*

多样化的可能策略:

1. 决策启发式选择:
   - 使用不同的变量排序规则

2. 推导顺序:






3.

数据结构修改:


    -

different　clause　　 learning strategies




4.



行为标志:





*/

```

---

## 总结

多样化是 PortfolioSimple 能够有效工作的基础，通过为每个求解器创建独特的行为特征：

1. **唯一标识** - 每个求解器有唯一的ID和类型
2. **状态修改    |

3. **实时切换   |

虽然当前代码库中的 diversify() 实现可能相对简单

但这个机制的思想对于并行SAT的效率至关重要。