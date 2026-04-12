# 子句共享策略详解

## 概述

子句分享是 Portfolio SAT 的重要机制，允许同时运行的求解器之间交换有价值的搜索信息（子句）。

```
┌─────────────────────────────────────────────────────────────┐
│                    子句共享层次结构                          │
└─────────────────────────────────────────────────────────────┘

PortfolioSimple (策略管理器)
    │
    ├── 局部共享策略 (LocalSharingStrategy)
    │   └── 直接内存访问，进程内通信
    │       ├── HordeSatSharing
    │       └── SimpleSharing
    │
    └── 全局共享策略 (GlobalSharingStrategy)
        └── MPI跨进程通信
            ├── AllGatherSharing
            └── MallobSharing


```

---

## SharingEntity 抽象基类

文件：`src/sharing/SharingEntity.hpp`

### 核心接口

```cpp
class SharingEntity {
public:
    virtual ~SharingEntity() = default;

    // 获取唯一的共享实体ID
    virtual unsigned int getSharingId() const = 0;

    // 将子句导入到策略内部数据库
    virtual bool importClause(const ClauseExchangePtr& clause) = 0;

    // 导入多条子句
    virtual void importClauses(
        const std::vector<ClauseExchangePtr>& clauses
    ) {
        for (auto c : clauses) importClause(c);
    }

    // 将此策略注册为某个数据库的消费者
    virtual bool addClient(std::shared_ptr<SharingEntity> client) = 0;

    // 获取所有需要接收子句的求解器列表
    virtual std::vector<std::shared_ptr<SharingEntity>>
        getClients() const = 0;
};
```

### 共享状态管理

```cpp
protected:
    unsigned int sharing_id;                    // 实体唯一ID

    // 生产者-消费者关系
    std::unordered_map<unsigned int,
                       std::vector<std::shared_ptr<SharingEntity>>>
        producers;

    std::vector<std::shared_ptr<SharingEntity>>
        clients;

    std::mutex entities_mutex;
};
```

---

## SharingStrategy 抽象基类

文件：`src/sharing/SharingStrategy.hpp`

### 策略生命周期

```cpp
class SharingStrategy : public SharingEntity {
public:
    // 创建后必须调用此函数建立生产者-消费者关系
    void connectConstructorProducers() {
        std::shared_lock lock(producers_mutex);
        for (auto& wp : producers) {
            if (auto prod = wp.lock()) {
                prod->addClient(shared_from_this());
            }
        }
    }

    // 每轮共享的入口点
    virtual bool doSharing() = 0;

    // 策略特定的睡眠时间（微秒）
    virtual std::chrono::microseconds getSleepingTime();

    // 打印统计信息
    virtual void printStats();
};
```

### HordeSatSharing 的 LBD 限制机制

```cpp
class HordeSatSharing : public SharingStrategy {
protected:
    unsigned int initial_lbd_limit;      // 初始最大LBD值
    unsigned int round_before_increase;   // 多少轮后可能增加限制
    int round = 0;

    // 每个 生产者的剩余导出配额
    std::unordered_map<unsigned int,
                       std::atomic<unsigned long>>
        literals_per_producer;

    // 子句的LBD值限度和每轮总字面量限制
    unsigned long literal_per_round;

    std::vector<ClauseExchangePtr> selection;
};
```

### 导出子句的选择算法

```cpp
bool HordeSatSharing::doSharing() {
    round++;

    // 1. 从数据库获取所有新鲜候选子句
    auto candidates = clauseDB->getAllClauses(this);

    if (candidates.empty()) return true;

    std::vector<ClauseExchangePtr> exported;
    unsigned int producer_count = producers.size();

    for (auto& clause : candidates) {
        unsigned int pid = clause.from;

        // 检查配额
        auto& quota = literals_per_producer[pid];
        if (quota >= literal_per_round[pid]) continue;

        // 检查是否足够新鲜（避免重复处理）
        if (!clause.fresh) continue;

        // ─────────────────────────────────────
        // HordeSat 的核心选择启发式
        // ─────────────────────────────────────

        bool acceptable = true;

        for (auto& lit : clause.literals) {
            auto& lbd = clause_lbd[pid][lit];

            if (lbd > current_lbd_limit_[pid]) {
                unacceptable |=
                    (++violations[lit] < MAX_CONSECUTIVE_VIOLATIONS);
            }
        }

        if (!acceptable || violations.empty()) continue;

        // ─────────────────────────────────────
        // 更新配额和统计
        // ─────────────────────────────────────

        quota += clause.literals.size();
        exported.push_back(clause);

        if (quota >= literal_per_round[pid]) {
            pid = (pid + 1) % producer_count;
        }
    }

    if (exported.empty()) return false;

    // 将导出的子句发送给所有消费者
    for (auto& clause : exported) {
        for (auto& consumer : clients) {
            exportClauseToClient(clause, consumer);
        }
```

---

## 子句共享线程（Sharer）

文件：`src/sharing/Sharer.hpp`, `src/sharing/Sharer.cpp`

### Sharer 的作用

每个 SharedStrategy 启动一个独立的线程，负责：
1. 持续从求解器导入新子句
2. 为下一轮选择要导出的子句
3. 通过网络MPI发送给其他进程的消费者


```

### 主循环实现

```cpp
void* mainThrSharing(void* arg) {
    Sharer* shr = static_cast<Sharer*>(arg);
    int nb_strats = shr->sharing_strategies.size();
    unsigned int round = 0;

    // 初始同步延迟，避免所有线程同时开始造成拥塞
    std::this_thread::sleep_for(
        std::chrono::microseconds(__global_parameters__.init_sleep
    );

    bool can_break = false;

    // ──────────────────────────────────────
    // 主共享循环：轮转执行各策略的导出操作
    // ──────────────────────────────────────

    while (!can_break) {
        int current_strategy = shr->round % nb_strats;

        double sharing_start = get_absolute_time_seconds();

        can_break = shr->sharing_strategies[current_strategy]->
                   doSharing();  // 策略返回是否应继续共享


```

### 导出循环的退出条件

```cpp
can_break = shr->sharing_strategies[current_strategy]->doSharing();
std::unique_lock<std::mutex> lock(mutex_global_end);

// 如果全局已结束，或策略特定超时...
cond_global_end.wait_for(lock,
    shr->sharing_strategies[current_strategy}->get_sleeping_time());

shr->round++;
```

### 清理阶段 - 完成剩余工作

```cpp
for (unsigned int i = 0; i < shr->sharing_strategies.size(); i++) {
    if (i == current_strategy) continue;

    while (!shr->sharing_strategies[i]->doSharing()) {
        // 继续共享直到策略声明无法再导出子句


```

---

## 全局共享策略

### AllGatherSharing

文件：`src/sharing/GlobalStrategies/AllGatherSharing.hpp`, `.cpp`

#### 原理最简单的全局共享

```cpp
class AllGatherSharing : public GlobalSharingStrategy {
public:
    bool doSharing() override {
        // 1. 收集所有进程的全部子句库
        std::vector<std::vector<ClauseExchangePtr>>
            all_clauses(mpi_world_size);

        MPI_Allgather(&local_clause_count,
                     sizeof(size_t),
                     MPI_BYTE,
                     &all_clauses[0],
                     (mpi_world_size * local_clause_count) *
                      sizeof(ClauseExchangePtr),
                     MPI_BYTE,
                     MPI_COMM_WORLD);

        // 2. 每个进程接收所有子句
        for (int src = 0; src < mpi_world_size; src++) {
            for (auto& clause : all_clauses[src]) {
                if (can_import(clause) && !locally_seen[clause.hash]) {
                    import_clause(clause);
                }
            }
        }

        return false;
    }


```

#### 复杂度分析

| 操作 | 时间复杂度 |
|------|------------|
| 收集子句数量 | O(p)，p = 进程数 |
| 广播所有子句 | O(p²n)，n = 子句数 |

**优点：**
- 实现简单可靠
- 每个进程保证收到完整的最新子句

**缺点：**
- 通信开销大O(n)
- 浪费网络带宽（重复传输）
- 不适合超大子句库


---

### MallobSharing

文件： `src/sharing/GlobalStrategies/MallobSharing.hpp`, `.cpp`

#### 核心思想 - 内存高效的P2P共享

```cpp
class MallobSharing : public GlobalSharingStrategy {
protected:
    // 每个进程维护的发送和接收状态
    struct PeerState {
        std::queue<ClauseExchangePtr> send_queue;
        std::vector<bool> receive_bitmap;

        MPI_Request req_send;
        MPI_Request req_recv;


```

### 导出循环

```cpp
bool doSharing() override {
    round++;

    // 1. 为每对进程准备一轮共享
    for (auto& peer_state : peer_states) {
        prepare_send_queue(&peer_state);

        if (!prepare_receive_queue(&peer_state)) {
            return false;
        }

        break;
```

### MPI 异步通信

```cpp
void start_communication() {
    // 为每个进程启动异步发送和接收
    for (auto& peer : peers) {
        mpiutils::ISendClauses(send_buffers[peer.ranking],
                               send_counts[peer.ranking),
                               &peer.req_send);

        mpiutils::IRecvClauses(recv_buffers[peer.ranking,
                                       recv_caps[peer.ranking),
                               &peer.req_recv);
    }
}


```

### 断开连接和清理

```cpp
void disconnect() override {
    std::vector<int> buf(1, DISCONNECT_TAG);

    for (int peer = 0; peer < mpi_world_size; peer++) {
        if (mpiutils::Isend(&buf,
                           sizeof(int),
                           Disconn,
                           &reqs[peer * MAX_PEERS + 0));

        mpiutils::Irecv(&buf,

                      &
```

---

## 策略选择和配置

文件：`src/sharing/SharingStrategyFactory.cpp`

### 求解器类型的字符编码

| 字符 | 说明 |
|------|------|
| `'g'` | GlucoseSyrup (CDCL) |
| `'l'` | Lingeling (CDCL) |
| `'k'` | Kissat (CDCL) |
| `'c'` | Cadical (CDCL) |
| `'M'` | MapleCOMSPS (CDCL) |
| `'m'` | MiniSat (CDCL) |
| `'I'` | KissatINC (CDCL) |
| `'K'` | KissatMAB (CDCL) |
| `'y'` | YalSat (Local Search) |
| `'t'` | TaSSAT (Local Search) |

### 策略编号映射

```cpp
void SharingStrategyFactory::instantiateLocalStrategies(
    int strategy_number,
    std::vector<std::shared_ptr<SharingStrategy>>& local_strategies,
    std::vector<std::shared_ptr<SolverCdclInterface>>& cdcl_solvers)
{
    if (strategy_number == 0) {
        // 随机选择：1, 2 或 3
        std::random_device dev;
        std::mt19937 rng(dev());
        std::uniform_int_distribution<> dist(1, 3);
        strategy_number = dist(rng);


```

---

## 子句导入的验证机制

### ClauseExchange 的完整性检查

```cpp
bool importClause(const ClauseExchangePtr& clause) override {
    // 检查子句是否来自允许的生产者
    if (clause.from != getting_producer_id_) {
        return false;
    }

    lock_guard<std::mutex> lock(clauses_mutex);

    // 1. 计算哈希快速判断是否存在
    uint64_t hash = clause.hash;
    auto it = imported_clauses.find(hash);
    if (it != imported_clauses.end()) {
        // 比较内容确认是否相同


```

### LBD（Literal Block Distance）限制

```cpp
// Literal Block Distance: 子句中最长连续单调片段的长度
unsigned int compute_lbd(const std::vector<int>& literals,
                         const AutoDerivation& derivation) {
    unsigned int lbd = 0;
    auto it = literals.begin();

    while (it != literals.end()) {
        // 检查文字的所有派生关系...

        if (++lbd > LBD_LIMIT_) break;
```

---

## 性能统计跟踪

### SharingStatistics 结构

```cpp
struct SharingStatistics {
    std::atomic<unsigned long> received_clauses{0};
    std::atomic<unsigned long> shared_clauses{0};

    unsigned long filtered_at_import = 0;

    // 每个策略的子句导入次数


```

---

## 总结

| 策略类型 | 共享范围 | 复杂度 | 适用场景 |
|---------|----------|--------|----------|
| HordeSatSharing | 进程内单组/双组 | O(n) 选择 | 默认选择，平衡性能 |
| SimpleSharing | 进程内简单分组 | O(n) | 实现最简单 |
| AllGatherSharing | 跨进程全部 | O(n²) 通信 | 小子句库可靠选择 |
| MallobSharing | 跨进程P2P | O(n log n) | 大子句库高效 |

PortfolioSimple 的成功很大程度上依赖于这些共享策略的高效实现。