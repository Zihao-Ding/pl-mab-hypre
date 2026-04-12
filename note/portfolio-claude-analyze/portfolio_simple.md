# PortfolioSimple 实现详解

## 概述

PortfolioSimple 是 Painless SAT 求解器的默认并行策略，通过同时运行多个不同的SAT求解器并共享子句来实现高效的并行搜索。

## 架构总览

```
                    PortfolioSimple (顶层策略)
                        ══════════
           ┌───────────┴───────────┬───────────┐
           ▼                       ▼           ▼
     CDCL 求解器              局部搜索      子句共享线程
     (多个实例)               (多个实例)        (多个)
    ═════════              ══════════       ═════════
    │                       │               │
    ├─ GlucoseSyrup         ├─ YalSat       ├─ HordeSatSharing
    ├─ Lingeling           ├─ TaSSAT       ├─ SimpleSharing
    ├─ Kissat              └───────────────┤ ├─ AllGatherSharing
    ├─ Cadical                              └─ MallobSharing
    └─ MapleCOMSPS

           ══════════       ══════════       ═════════
          顺序工作线程      子句共享策略     其他求解器
          (每个solver)      (clauses exchange)
```

## 主要组件列表

### 1. 求解器集合

```cpp
// CDCL 后台推理求解器 - 使用冲突驱动子句学习
std::vector<std::shared_ptr<SolverCdclInterface>> cdclSolvers;

// 局部搜索求解器 - 快速原型验证
std::vector<std::shared_ptr<LocalSearchInterface>> localSolvers;
```

### 2. 预处理器（可选）

```cpp
// PRS (Partial Relevancy Search) 预处理
std::vector<std::shared_ptr<PreprocessorInterface>> preprocessors;
```

### 3. 子句共享策略

```cpp
// 进程内局部共享 - 每个求解器直接通信
std::vector<std::shared_ptr<SharingStrategy>> localStrategies;

// 分布式全局共享 - 通过 MPI 跨进程通信
std::vector<std::shared_ptr<GlobalSharingStrategy>> globalStrategies;
```

### 4. 子句交换线程

```cpp
// 实际执行子句共享的独立线程
std::vector<std::unique_ptr<Sharer>> sharers;
```

---

## 初始化流程详解

### 阶段1: 程序入口点

文件：`src/painless.cpp`

```cpp
int main(int argc, char** argv) {
    Parameters::init(argc, argv);

    // 设置 MPI（如果使用分布式模式）
    if (__globalParameters__.enableDistributed) {
        MPI_Init_thread(NULL, NULL, MPI_THREAD_SERIALIZED, &provided);
    }

    // 创建 PortfolioSimple 策略作为默认工作策略
    working = new PortfolioSimple();

    // 启动主求解线程
    std::thread mainWorker(&WorkingStrategy::solve,
                          (WorkingStrategy*)working,
                          cube);

    // 等待结果或超时...
}
```

### 阶段2: PortfolioSimple::solve() 初始化

文件：`src/working/PortfolioSimple.cpp`

#### 2.1 PRS 预处理（可选）

```cpp
if (mpi_rank <= 0 && __globalParameters__.prs) {
    // 创建预处理器实例
    this->preprocessors.push_back(
        std::make_shared<preprocess>(0)
    );

    // 加载公式并尝试求解
    for (auto& preproc : preprocessors) {
        SatResult res = preproc->solve({});
        if (res == SAT || res == UNSAT) {
            finalResult = res;
            this->join(this, finalResult,
                       res == SAT ? finalModel : {});
            return;  // 预处理直接解决
        }
    }

    // 提取简化后的子句用于主求解器
    initClauses = std::move(lastSimplification->
                            getSimplifiedFormula());
}
```

#### 2.2 创建 CDCL 和局部搜索求解器

文件：`src/solvers/SolverFactory.cpp`

```cpp
void SolverFactory::createSolvers(
    int maxSolvers,
    char importDBType,
    std::string portfolio,
    std::vector<std::shared_ptr<SolverCdclInterface>>& cdclSolvers,
    std::vector<std::shared_ptr<LocalSearchInterface>>& localSolvers)
{
    unsigned int typeCount = portfolio.size();

    // 为每个求解器创建一个实例
    for (size_t i = 0; i < maxSolvers && typeCount > 0; i++) {
        createSolver(
            portfolio.at(i % typeCount),  // 类型: 'g','l','k','c', etc.
            importDBType,
            cdclSolvers,
            localSolvers
        );
    }
}
```

**支持的求解器类型：**

| 字符 | 求解器类 | 类型 |
|------|----------|------|
| `'g'` | GlucoseSyrup | CDCL |
| `'l'` | Lingeling | CDCL |
| `'k'` | Kissat | CDCL |
| `'c'` | Cadical | CDCL |
| `'M'` | MapleCOMSPS | CDCL |
| `'m'` | MiniSat | CDCL |
| `'I'` | KissatINC | CDCL |
| `'K'` | KissatMAB | CDCL |
| `'y'` | YalSat | Local Search |
| `'t'` | TaSSAT | Local Search |

#### 2.3 求解器多样化（Diversification）

文件：`src/solvers/SolverFactory.cpp`

```cpp
void SolverFactory::diversification(
    const std::vector<std::shared_ptr<SolverCdclInterface>>& cdclSolvers,
    const std::vector<std::shared_ptr<LocalSearchInterface>>& localSolvers,
    const IDScaler& gIDScaler,
    const IDScaler& typeIDScaler)
{
    // 设置求解器 ID
    for (auto cdclSolver : cdclSolvers) {
        cdclSolver->setSolverId(gIDScaler(cdclSolver));
        cdclSolver->setSolverTypeId(typeIDScaler(cdclSolver));
    }

    for (auto localSolver : localSolvers) {
        localSolver->setSolverId(gIDScaler(localSolver));
        localSolver->setSolverTypeId(typeIDScaler(localSolver));
    }

    // 执行求解器的多样化修改
    for (auto cdclSolver : cdclSolvers) {
        cdclSolver->diversify();
    }

    for (auto localSolver : localSolvers) {
        localSolver->diversify();
    }
}
```

**多样化的目的：**
- 避免所有求解器执行完全相同的搜索
- 通过修改内部状态（启发式、数据结构等）创建差异化的行为
- 确保并行效率最大化

#### 2.4 创建子句共享策略

文件：`src/sharing/SharingStrategyFactory.cpp`

```cpp
void SharingStrategyFactory::instantiateLocalStrategies(
    int strategyNumber,
    std::vector<std::shared_ptr<SharingStrategy>>& localStrategies,
    std::vector<std::shared_ptr<SolverCdclInterface>>& cdclSolvers)
{
    // 创建可共享的实体列表（所有求解器）
    std::vector<std::shared_ptr<SharingEntity>> allEntities;
    allEntities.insert(allEntities.end(),
                      cdclSolvers.begin(),
                      cdclSolvers.end());

    switch (strategyNumber) {
        case 1: {  // HordeSatSharing
            localStrategies.emplace_back(new HordeSatSharing(
                lsharedDB,
                __globalParameters__.sharedLiteralsPerProducer,
                __globalParameters__.hordeInitialLbdLimit,
                __globalParameters__.hordeInitRound,
                allEntities,  // 所有生产者
                allEntities   // 所有消费者（每个人都能收到每条子句）
            ));
            break;
        }
        case 2: {  // HordeSatSharing (2 groups)
            // 第一组：前半部分求解器，接收所有子句
            localStrategies.emplace_back(new HordeSatSharing(
                lsharedDB, ...,
                {allEntities.begin(), allEntities.begin() + size/2},
                allEntities
            ));
            // 第二组：后半部分求解器，只接收后继行为的子句
            local_strategies.emplace_back(new HordeSatSharing(
                lsharedDB2, ...,
                {allEntities.begin() + size/2, all_entities.end()},
                all_entities
            ));
            break;
        }
        case 3: {  // SimpleSharing
            local_strategies.emplace_back(new SimpleSharing(
                lshared_db,
                __global_parameters__.simple_share_limit,
                __global_parameters__.shared_literals_per_producer,
                all_entities,
                all_entities
            ));
            break;
        }
    }

    // 连接生产者-消费者关系
    for (auto& lstrat : local_strategies) {
        lstrat->connectConstructorProducers();
    }
}
```

#### 2.5 创建全局共享策略

```cpp
void SharingStrategyFactory::instantiateGlobalStrategies(
    int strategyNumber,
    std::vector<std::shared_ptr<GlobalSharingStrategy>>& global_strategies)
{
    switch (strategyNumber) {
        case 1: {  // AllGatherSharing
            global_strategies.emplace_back(new AllGatherSharing(
                gshared_db,
                __global_parameters__.global_shared_literals
            ));
            break;
        }
        case 2: {  // MallobSharing
            global_strategies.emplace_back(new MallobSharing(
                gshared_db,
                __global_parameters__.global_shared_literals,
                __global_parameters__.mallob_max_buffer_size,
                __global_parameters__.mallob_lbd_limit,
                __global_parameters__.mallob_size_limit,
                __global_parameters>.mallob_sharings_per_second,
                __global_parameters>.mallob_max_compensation,
                __global_parameters>.mallob_reshare_period
            ));
            break;
        }
        case 3: {  // GenericGlobalSharing (ring topology)
            global_strategies.emplace_back(new GenericGlobalSharing(
                gshared_db,
                subscriptions,    // [right_neighbor, left_neighbor]
                subscribers,      // [right_neighbor, left_neighbor]
                __global_parameters>.global_shared_literals
            ));
            break;
        }
    }

    // MPI 初始化
    for (auto& gstrat : global_strategies) {
        if (!gstrat->initMpiVariables()) {
            dist = false;
            global_strategies.erase(gstrategies.begin() + i);
        }
    }
}
```

### 阶段3: 启动求解和共享线程

```cpp
void PortfolioSimple::solve(const std::vector<int>& cube) {
    // ... 前面的初始化 ...

    // 为每个 CDCL 求解器创建顺序工作线程
    for (auto& cdcl : cdclSolvers) {
        SequentialWorker* myworker = new SequentialWorker(cdcl);
        this->addSlave(myworker);  // 管理 slave 的生命周期

        std::thread initializer([myworker, &cube, &cdcl,
                                 &initClauses, varCount]() {
            cdcl->addInitialClauses(initClauses, varCount);

            myworker->solve(cube');   // 启动持续求解
        });
        solver_initializers.emplace_back(std::move(initializer));
    }

    // 为每个局部搜索求解器创建顺序工作线程
    for (auto& local : local_solvers) {
        SequentialWorker* myworker = new SequentialWorker(local);
        this->addSlave(myworker);

        std::thread initializer([myworker, &cube, &local,
                                 &init_clauses, var_count]() {
            local->addInitialClauses(init_clauses, var_count;
            myworker->solve(cube');
        });
        solver_initializers.emplace_back(std::move(initializer));
    }

    // 等待所有初始化完成
    for (auto& initializer : solver_initializers) {
        initializer.join();
    }

    LOG0("All solvers are fully initialized and launched");

    // 创建并启动子句共享线程
    std::vector<std::shared_ptr<SharingStrategy>> sharing_strategies;
    for (auto lstrat : local_strategies) {
        sharing_strategies.push_back(lstrat);
    }
    for (auto gstrat : global_strategies) {
        sharing_strategies.push_back(gstrat);
    }

    SharingStrategyFactory::launchSharers(
        sharing_strategies,
        this->sharers
    );
}
```

---

## 子句共享机制

### SharedEntity 层次结构

```
PortfolioSimple (策略管理器)
    │
    ├─╼════════════════════════════════════════╼┄
    │                                          │
    ▼                                          ▼
SharingStrategy ←────── 实现接口 ──────→ SharingEntity
 (策略抽象)            │                    (实体抽象)
    │                                          │
    ├─ doSharing()      ◄─────────────────────┤
    ├─ importClause()   ════════════════════  │
    └─ exporters/       ══════════════════    │
                                        (共享状态)
                                          │
                         ┌──────────────────┴──────────────┐
                         ▼                                 ▼
              SharingStrategy::doSharing()         importClause()
                      │                                    │
        ┌─────────────┴─────────────┬──────────────────────┴─────────────┐
        ▼                           ▼                                      ▼
   选择和导出子句                     │                                  接收并验证

    HordeSatSharing                  │                          检查 LBD 限度和库容量

    ┌────────────────────────────────┼──────────────────────────────────┐
    ▼                                 ▼                                   ▼
// 从数据库获取候选子句          continue;                             true
clauses = clauseDB->getAllClauses()                                    │
                                                                    false

        └─────────────────────┬─────────────────────┴─────────────────────┘
                              ▼                                       ▼
                    更新生产者统计                      返回成功/失败标志

    updateProducerStats(clause, producer_id)                          return true
```

### HordeSatSharing.doSharing() 算法

```cpp
bool HordeSatSharing::doSharing() {
    round++;

    // 1. 从数据库获取所有候选子句
    auto candidate_clauses = clauseDB->getAllClauses(this);

    if (candidate_clauses.empty()) {
        return true;  // 没有更多子句可共享
    }

    // 2. 为每个生产者选择要导出的子句
    std::vector<ClauseExchangePtr> exported;
    for (auto& clause : candidate_clauses) {
        unsigned int producer_id = clause.from;

        if (literals_per_producer[producer_id] >=
            literals_per_round[producer_id]) {
            continue;  // 达到该生产者本轮的导出限制
        }

        // 检查子句是否新鲜（刚导入不到一轮）
        if (!clause.fresh) {
            continue;
        }

        // 更新统计
       _literals_per_producer[producer_id]++;
        exported.push_back(clause);
    }

    if (exported.empty()) {
        return false;  // 继续下一轮
    }

    // 3. 将导出的子句发送给所有消费者

```

### 子句共享线程（Sharer）

文件：`src/sharing/Sharer.cpp`

```cpp
void* mainThrSharing(void* arg) {
    Sharer* shr = static_cast<Sharer*>(arg);
    int nb_strats = shr->sharing_strategies.size();
    unsigned int round = 0;

    // 初始同步延迟，避免所有线程同时启动造成混乱
    std::this_thread::sleep_for(
        std::chrono::microseconds(__global_parameters__.init_sleep
    );

    bool can_break = false;

    // 主共享循环
    while (!can_break) {
        int current_strategy = shr->round % nb_strats;

        // ─────────────────────────────────────────────────────
        // 阶段1: 执行当前策略的共享操作
        // ─────────────────────────────────────────────────────

        double sharing_start = getAbsoluteTimeSeconds();

        can_break = shr->sharing_strategies[current_strategy]->
                   doSharing();

        double sharing_duration =
            getAbsoluteTimeSeconds() - sharing_start;

        std::cout << "[Sharer " << shr->getId()
                  << "] Round " << round
                  << " done in " << sharing_duration
                  << "s\n";

        // ─────────────────────────────────────────────────────
        // 阶段2: 等待（被条件变量唤醒）
        // ─────────────────────────────────────────────────────

        std::unique_lock<std::mutex> lock(mutex_global_end);

        auto wakeup_status = cond_global_end.wait_for(
            lock,
            shr->sharing_strategies[current_strategy]->
                getSleepingTime()
        );

        shr->round++;
    }

    // ─────────────────────────────────────────────────────
    // 清理：完成剩余策略的最后一轮共享
    // ─────────────────────────────────────────────────────

    for (unsigned int i = 0; i < shr->sharing_strategies.size(); i++) {
        if (i == current_strategy) continue;

        while (!shr->sharing_strategies[i]->doSharing()) {
            /* 继续共享直到策略声明完成 */
        }
    }

    return nullptr;
}
```

---

## 求解器持续运行

每个求解器都有自己独立的线程（SequentialWorker），通过 `SharedStrategy` 实时接收新导入的子句：

```cpp
// 在 Sharer 线程中不断执行
while (!global_ending) {
    // 1. 从共享策略获取新导入的子句
    clause = sharing_strategy->importClause(clause_ptr);

    if (!clause) break;  // 没有更多子句

    // 2. 将子句添加到求解器内部数据库
    solver->add_clause(clause);


```

---

## 结果合并机制

### 当某个求解器找到解时

```cpp
void PortfolioSimple::join(WorkingStrategy* strat,
                           SatResult res,
                           const std::vector<int>& model) {

    if (res == UNKNOWN || strategy_ending) return;

    strategy_ending = true;
    set_solver_interrupt();  // 中断所有求解器

    if (this == parent) {  // 这是顶层策略
        final_result = res;
        global_ending = true;

        if (res == SAT) {
            final_model = model';
        }

        mutex_global_end.lock();
        cond_global_end.notify_all();'
        mutex_global_end.unlock();

    } else {              // 向父策略转发结果
        parent->join(this, res, model);
    }
}
```

### 主线程等待所有工作完成

```cpp
// 在 main() 中
while (global_ending == false) {
    cond_global_end.wait(lock);  // 等待有人宣布结束
}

main_worker.join();             // 等待求解线程结束


delete working;
```

---

## 分布式MPI模式

### 初始化分布式模式

```cpp
if (__global_parameters__.enable_distributed) {
    MPI_Init_thread(NULL, NULL,
                   MPI_THREAD_SERIALIZED, &provided);

    mpi_rank = 0;          // 主进程（创建者）
    mpi_world_size = 1;    // 进程总数

    dist = true;
}
```

### 分布式行为

| 场景 | 行为 |
|------|------|
| **主进程找到解** | 广播结果 → 所有进程退出 |
| **子进程找到解** | 只返回模型，不继续运行 |
| **超时** | 主进程设置 timeout，所有进程停止 |

---

## 子句共享策略类型总结

### 局部共享（进程内）

| 策略名称 | 说明 | 优点 | 缺点 |
|---------|------|------|------|
| `HordeSatSharing` | 基于LBD的随机选择，每轮重新计算限制 | 实现简单，效率高 | 需要足够的求解器数量 |
| `SimpleSharing` | 按子句类型分组共享 | 内存使用清晰 | 策略较为粗粒度 |

### 全局共享（进程间）

| 策略名称 | 说明 | 优点 | 缺点 |
|---------|------|------|------|
| `AllGatherSharing` | 使用 MPI_Allgather 广播所有子句 | 实现简单，保证完整性 | 消息大小O(n²) |
| `MallobSharing` | 内存高效的P2P共享网络 | 避免冗余传输 | 实现复杂 |
| `GenericGlobalSharing` | 可配置的拓扑结构 | 灵活可配置 | 依赖MPI功能 |

---

## 关键数据结构

### ClauseExchange - 压缩子句表示

```cpp
struct ClauseExchange {
    unsigned int from;           // 谁发送的（求解器ID）
    std::vector<int> literals;   // 子句的文字
    bool fresh;                 // 是否最近导入的？

    // 序列化优化...
};
```

### ClauseDatabase - 子句内存管理

```cpp
class ClauseDatabase {
    // 求解器和共享策略之间的子句存储

    std::unordered_map<..., std::vector<ClauseExchangePtr>>
        clauses_by_producer;

    std::mutex db_mutex;

    std::vector<ClauseExchangePtr> getAllClauses(
        SharingStrategy* strategy) {
            // 返回该策略可以接受的子句
    }
};
```

---

## 性能优化技术

### 1. 求解器多样化（Diversification）

每个求解器在创建后会调用 `diversify()`，修改内部行为：

- **不同的决策启发式**
- **不同的简化顺序**
- **不同的数据结构配置**

```cpp
// 在 diversify() 中可能执行的修改
solver->set_use_alternative_decision_heuristic(true);
solver->set_different_ordering_for_division(true);
```

### 2. 子句共享同步

使用条件变量实现高效的等待/唤醒：

```cpp
// Sharer 线程：等待通知
std::unique_lock<std::mutex> lock(mutex_global_end);
cond_global_end.wait(lock, [] {
    return global_ending ||
           std::chrono::steady_clock::now() >= wakeup_point;
});

// 主线程：发送通知
mutex_global_end.lock();
cond_global_end.notify_all();
mutex_global_end.unlock();


```

### 3. 内存管理

- 候选子句的内存限制
- 导出后的清理机制
- 定期的数据库简化

---

## 调试和日志

PortfolioSimple 提供多个日志级别：

| 级别 | 用途 |
|------|------|
| `LOG0()` | 重要事件（初始化、结束） |
| `LOGWARN()` | 警告信息 |
| `LOGDEBUG1()` | 一般调试细节 |
| `LOGDEBUG2()` | 更详细的内部状态 |

---

## 总结

PortfolioSimple 的核心创新在于：

1. **并行多样性**：多个不同类型的求解器同时运行
2. **智能共享**：高效的子句交换机制减少重复工作
3. **灵活架构**：支持进程内和分布式两种共享模式
4. **平滑退出**：当任何求解器找到解时立即结束

这种策略通过充分利用多核CPU和网络带宽，显著提升了 SAT 求解器的并行效率。