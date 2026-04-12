# 分布式模式实现详解

## 概述

Painless 支持通过 MPI 实现的分布式并行策略，多个进程通过网络共享子句和求解状态。

---

```
┌─────────────────────────────────────────────────────────────┐
│                    分布式模式架构                            │
└─────────────────────────────────────────────────────────────┘

进程 0 (Master/Winner-Takes-All)
    │
    ├── 创建 PortfolioSimple 策略
    ├── 运行求解器
    └── 当找到解时:
        ├── 序列化模型和统计信息 → 所有进程


进程 i > 0 (Worker)
    │
    ├── 接收问题实例
    ├── 运行独立的求解器集合
    └── 收到最终结果后:


所有进程同步退出

```

---

## MPI初始化

文件：`src/painless.cpp`

```cpp
int main(int argc, char** argv) {
    Parameters::init(argc, argv);

    dist = __global_parameters__.enable_distributed;

    if (dist) {
        int provided;
        MPI_Init_thread(NULL,
                       NULL,
                       MPI_THREAD_SERIALIZED,
                       &provided);

        // 检查线程支持级别
        if (provided < MPI_THREAD_SERIALIZED) {
            LOGERROR("Required MPI thread level not available");
            dist = false;  // 回退到单进程模式


```

### 线程类型说明

| MPI Thread Level | 说明 |
|------------------|------|
| `MPI_THREAD_SINGLE` | 不支持并行线程 |
| `MPI_THREAD_FUNNELLED` | 其他进程可请求锁定访问 |
| `MPI_THREAD_SERIALIZED` | 每个进程有自己的锁，安全共享 |
| `MPI_THREAD_MULTIPLE` | 支持任意级别的嵌套并行 |

Painless 选择 **SERIALIZED**，允许在多线程环境下安全使用 MPI。

---

## 进程角色划分

### Master Process (mpi_rank = 0)

```
┌─────────────────────────────────────────────────────┐
│                  Master 进程职责                      │
└─────────────────────────────────────────────────────┘

1. 问题实例加载和预处理
   ├─ 解析 CNF 文件
   └─ 执行 PRS 预处理（可选）

2. 求解器创建和多样化
   ├─ 创建 CDCL 和局部搜索求解器列表
   └─ 应用求解析构化修改

3. 子句共享策略创建
   ├─ 局部策略（进程内）
   └─ 全局策略（跨进程）

4. 持续运行直到:
   └─ 找到解或超时


```

### Worker Process (mpi_rank > 0)

```
┌─────────────────────────────────────────────────────┐
│                  Worker 进程职责                      │
└─────────────────────────────────────────────────────┘

1. 接收问题实例和初始子句
   ├─ 广播预处理结果或原始公式


2. 创建独立的求解器集合（可能不同）:
   └─ 使用不同的 portfolio 字符串创建不同的求解器组合

3. 运行独立搜索:
   └─ 不依赖 Master 的状态



4. 监控进程间通信:
   ├─ 接收导出的子句
   └─ 实时导入到本地求解器


5. 等待最终结果:


```

---

## Winner-Takes-All 语义

分布式模式采用 winner-takes-all（获胜者通吃）策略：

| 场景 | Master 的行为 | Workers 的行为 |
|------|--------------|---------------|
| **找到解** | 序列化模型 → MPI_Allgather → 所有进程退出 | 接收最终结果，清理并退出 |
| **UNSAT 证明** | 广播 UNSAT 标记 -> 所有进程确认 | 确认后退出 |

### 结果广播机制

```cpp
// Master 宣布结果


if (mpi_rank == mpi_winner) {
    if (final_result == SatResult::SAT) {
        log_solution("SATISFIABLE");

        // 序列化模型到缓冲区...（可能很大）
        serialize_model_to_buffer(final_model, model_buf,
                                 &model_size);

        // 广播给所有进程
        MPI_Bcast(&final_result, 1, MPI_INT, mpi_winner,
                 MPI_COMM_WORLD);
    } else if (final_result == SatResult::UNSAT) {
        MPI_Bcast(&final_result, 1, MPI_INT,

              │



```

### Worker 的响应

```cpp
// Worker 接收结果


if (mpi_rank != mpi_winner) {
    // 只接收结果类型，不处理详细模型（winner 处理了）
    if (final_result == SatResult::SAT ||
        final_result == SatResult::UNSAT) {

        MPI_Bcast(&received_final_result_bcast,
                 1, MPI_INT,

              │



```

---

## 子句的分布式共享

### AllGatherSharing（最简单的全局策略）

```cpp
void doSharing() override {
    // Step 1: 收集每进程当前的子句库大小


    std::vector<size_t> clause_counts(mpi_world_size);
    MPI_Allgather(&local_clause_count,
                 sizeof(size_t),
                 MPI_BYTE,
                 &clause_counts[0],
                 mpi_world_size * sizeof(size_t),
                 MPI_COMM_WORLD);

    // Step 2: 每个进程接收所有子句


    std::vector<std::vector<char>> recv_buffers(
        mpi_world_size);
    std::vector<int> recv_sizes(mpi_world_size, -1);


```

### MallobSharing（高效的P2P策略）

```cpp
void doSharing() override {
    // Step 1: 为每对进程准备发送和接收


    for (int peer = 0; peer < mpi_world_size; peer++) {
        if (peer == mpi_rank) continue;

        prepare_send_queue_for_peer(peer);


```

---

## 子句导出和导入的同步

### 导出时机控制

```cpp
// 在 PortfolioSimple 中



if (!global_ending) {
    std::unique_lock<std::mutex> lock(mutex_global_end);

    // 等待超时或通知



        cond_global_end.wait_for(lock,
                               std::chrono::seconds(
                                   __global_parameters__.
                                     sharing_timeout));


```

### 导入后的求解器状态

```cpp
// SequentialWorker 持续运行



while (!global_ending && slave->force == false) {
    // 从共享策略获取新导入的子句



        clause = sharing_strategy->
                import_clause(clause_ptr);



            if (!clause || !slave->solver-
                    can_accept_clause(
                        clause, pid)) break;


```

---

## 进程间通信的开销管理

### 通信频率控制

```cpp
class SharingStrategy {
protected:
    // 防止频繁通信造成CPU瓶颈


        auto now = std::chrono::steady_clock::now();
        auto dur =

            std::chrono::duration_cast<
                std::chrono::microseconds>(
                    now - last_comm_time);

        if (dur < min_comm_interval) {

              │



```

### 异步通信模式

```cpp
// MallobSharing 的异步机制


    for (auto& peer_state : this->peer_states) {
        // 发送空包表示继续运行




            MPI_Iprobe(

                  │



```

---

## 分布式模式的特殊挑战

### 1. 子句内容的进程间不一致

**问题：** 不同进程可能以不同顺序处理相同的子句

**解决方案：**
- 使用哈希值识别重复子句
- 只导入尚未学习过的子句


### 2. 模型序列化和通信开销

**问题：** SAT 解模型可能非常大，广播失败

**Painless 的方法：**
```cpp
// Master 序列化



if (model_size > MAX_MODEL_BUF_SIZE) {
    // 压缩或截断...



        LOGWARN("Model serialization failed,

              │



```

### 3. 进程崩溃或不响应

**问题：** 某个进程可能失败，阻塞其他进程

**Painless 的方法：**
```cpp
// 设置超时检测


if (__global_parameters__.timeout > 0) {
    auto start =

        std::chrono::steady_clock::now();

    while ((auto)



         │



```

---

## 分布式模式的优点和缺点

### 优点

| 优点 | 说明 |
|------|------|
| **真正的并行** | 多进程在网络上通信，不是单机多线程 |
| **资源利用好** | 不同进程运行在不同的机器上 |

### 缺点

| 缺点 | 说明 |
|------|------|
| **Winner-Takes-All** | 只有 Master 的结果有效, workers 的工作可能不同 |
| **通信开销** | 子句导出导入的网络传输成本 |
| **实现复杂** | 需要处理进程同步、失败等情况 |

---

## 分布式模式的选择建议

```
┌─────────────────────────────────────────────────────┐
│          何时使用分布式模式                          │
└─────────────────────────────────────────────────────┘

推荐:
✓ 多机集群环境（不是单机多进程）
✓ 子句库大小适中（MallobSharing 更优）
✓ 能接受 winner-takes-all 语义
✓ 需要 MPI 的高级功能（异步通信、probe等）

不推荐:
✗ 单机多核（直接用 PortfolioSimple，local strategies 效果更好）
✗ 子句库极大（AllGatherSharing 开销太高）
✗ 需要收集所有进程的最终结果


```

---

## 总结

分布式模式通过 MPI 提供了跨机器并行的能力：

1. **Master-Worker**：单个 Master 运行，Workers 跟随
2. **Winner-Takes-All**：只有 Master 的结果有效
3. **通信策略**：AllGather 或 Mallob 两种P2P选择
4. **同步机制**：条件变量和超时检测

虽然实现复杂且有一定限制，但为 SAT 求解器的横向扩展提供了重要方向。