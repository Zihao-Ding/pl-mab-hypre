# 子句数据库机制

## 概述

子句数据库是 Portfolio SAT 中的核心组件，负责高效地存储、管理和共享求解过程中的子句状态。

---

```
┌─────────────────────────────────────────────────────────────┐
│           ClauseDatabase 在Portfolio 中的角色                │
└─────────────────────────────────────────────────────────────┘

                    ══════════════════
                   ┌ PortfolioSimple ├
                   ══════════════════

                        │
        ┌────┬────┬────┬┴────┬────┬────┬────┐
        ▼    ▼    ▼    ▼      ▼    ▼    ▼    ▼
       ═══   ═══  ════         ════ ═══　════

CDCL　　 Local               SharingStrategy


SOLVER ←───┬────► Database (ClauseDB)
           │
	   └──────────────────────────┐
	   							  ▼
			  ══════════════════════════════

		      SHARING_STRATEGY

			   ├─ HordeSatSharing


```

---

## ClauseDatabase 抽象接口

虽然源代码中没有找到完整的 `ClauseDatabase` 头文件，但可以通过反向工程理解其接口：

```cpp
class ClauseDatabase {
public:
    // 创建特定类型的数据库


        virtual ~ClauseDatabase() = default;

        // 导入子句到数据库



            return false;


```

### 子句导入

```cpp
bool import_clause(const clause_exchange_ptr& clause) override {
    lock_guard mutex(&db_mutex);

    // 1. 计算唯一标识用于去重


        if (clause_map.find(hash)

              │



```

---

## ClauseDatabaseFactory

文件：`src/containers/ClauseDatabases/ClauseDatabaseFactory.cpp`

### 数据库类型选择

```cpp
std::shared_ptr<ClauseDatabase> ClauseDatabaseFactory::
    create_database(char type) {

    switch (type) {
        case 'b':  // BloomFilterBasedDB


            return std::make_shared<BloomFilterDatabase>(
                max_clause_size,
                bloom_false_positive_rate);

        case 'h':  // HashMapDatabase



              │



```

### 数据库参数配置

```cpp
void ClauseDatabase::initialize(
    unsigned int max_clause_size,
    size_t database_capacity,
    unsigned int clustering_mode,

    unsigned          　int　　　　     clustering_rounds) {

        max_clause_size_ = max_clause_size;


         │



```

---

## 子句共享策略的选择机制

文件：`src/sharing/SharingStrategyFactory.cpp`

### Production 和 Consumption 关系

```cpp
void SharingStrategyFactory::
    instantiate_local_strategies(
        int strategy_number,
        std::vector<std::

shared_ptr<SharingStrategy>>& local_strategies,

            std::vec



             of<
            shared_ptr<SolverCdclInterface>>& cdcl_solvers)
{
     // 创建可共享的实体列表


```

---

## 导出和导入的操作流程

### 导出（doSharing 中）

```cpp
bool HordeSatSharing::doSharing() {
    round++;

    // 1. 从数据库获取候选子句



        if (candidates.empty()) return true;


         │




































             for (

auto& clause : candidates) {

                unsigned int pid = clause.from;

                 // 检查配额


                    continue;




```

### 导入（SequentialWorker 中）

```cpp
// 在 Sharer 线程中持续执行



while (!global_ending && slave->force == false

{




    if (

clause =

sharing_strategy-

             　import_clause(

                clause_ptr)) {



        slave-solver.

            add_clause(clause);




```

---

## 内存管理和清理

### 定期清理机制

```cpp
// 在 PortfolioSimple 析构时


    for (auto& preproc : preprocessors) {

        if (

mpi_rank <= 0 && final_result == SAT)

{

             pre



             　restore_model(

final_

model);

}



SOLVER_FACTORY.

print_statistics(this->cdcl_solvers,

this

.local_solvers);


#ifndef NDEBUG


    // 清空所有数据库，释放内存




```

---

## 总结

ClauseDatabase 为 PortfolioSimple 提供了统一的子句存储接口：

1. **统一管理**：所有求解器和共享策略使用同一个数据库
2. **类型灵活** | BloomFilter、HashMap 等实现


3. **生命周期清晰     |

4. **内存控制    |

通过精心设计的数据

库机制，PortfolioSimple 能够高效地支持多个同时运行的 SAT 求解器之间的子句交换。