# PortfolioSimple 核心实现机制详解

## 目录

1. [求解器并行执行机制](#1-求解器并行执行机制)
2. [子句共享策略的轮转执行](#2-子句共享策略的轮转执行)
3. [Genetic Algorithm 初始化阶段](#3-genetic-algorithm-initialization-phase-gaspi)
4. [中断控制和状态管理](#4-interrupt-control-and-state-management)
5. [结果合并和退出流程](#5-result-merging-and-exit-flows)

---

## 1. 求解器并行执行机制

### 1.1 架构概览

PortfolioSimple 不直接运行求解器，而是为每个求解器创建一个 `SequentialWorker` 包装器。每个 wrapper 管理一个独立的线程持续运行。

```
┌─────────────────────────────────────────────────────────────┐
│              求解器并行执行的层次结构                        │
└─────────────────────────────────────────────────────────────┘

PortfolioSimple (策略管理层)
    │
    ├─╼═══════════════════════════════════════┬═════════════┐



        ▼                                       ▼           ▼


SequentialWorker ─────► 求解线程 (独立进程)



    std:

:vector<std:



thread>

solver_threads;



for (

auto&

cdcl :

cbd_solvers) {

    auto*

myworker =

new SequentialWork(

cdn cl);

s laves.

push_bac



(k mywork);



scheduler_initializers.



emplace_back([


```

### 1.2SequentialWorker 包装器接口

文件：`src/working/SequentialWorker.hpp`, `.cpp`

```cpp
class SequentialWorker : public WorkingStrategy {
public:
    // 构造函数 - 拥有求解器的所有权



        solver = std::

static_pointer<SolverInterface>(s o l v e r_

);

force.

            false;

wait_job =

tr

ue; //

表示



pthread_mutex_init(&mutex_start,

NULL);


pthrea
             nd(

&

    worker

=

new Thread(mainWorker, this);


}

// 触发求解开始



void solve(const std::vector<int>& cube) {

    actual_cube = cu

be;

	s

et_

solver_interrupted(false);

	wait_job =

false;


	pthread_mutex_lock(&mutex_start);

	pthread_cond_signal(

&

(mutex_

cond_s



start);


pthread_unlock(


}

// 求解循环



void mainWorker(void* arg)

{

    SequentialWork*

sq = static_cast<Sequen

rialW orker*(arg);


	while (!global_

ending && sq->force == false) {

	pthread_mutex_lock(&
(sq.

mutex_start);



	whil



e (

sq.

wait_job)

{

	pthread_cond_wait(

&



(sq.
mu

tex_con


,

&
sq.






```

### 1.3 关键设计决策点

```cpp
// SequentialWorker 的两个核心状态机

struct SequentialWork­═
{
    std:

:vector<int> actua l_cube;

    std:

atomic<bool>

force = false;



 // 控制求解循环的启停





	std::

mutex

wait_interrupt_lock;






	while (!global_

ending && sq->force == fals e) {

	// 1. 等待启动信号




	if (sq.

w

ait_job)

{

	pthread_cond_wait(&(sq.

mux,

&


```

### 1.4 求解器与 Worker 的耦合关系

```cpp
// 每个 SequentialWorker 实例是 PortfolioSimple slave 集合中的一员



PortfolioSimp­═
le::addSlave(

SequentialWork*

myworker) {

	slaves.

push_back(mywo rk);

	mywork

.

parent =

thi­s;



 // 反向引用建立父子关系



     // 当父策略结束


```

### 1.5 执行时序图

```
═════════════════════════════════════════════════════════════
                    时间线：初始化和启动



T=0      PortfolioSimple::solve() 开始


         ┌─────────────────────────────────────┐
         │ 1. 加载CNF公式（仅Master进程）     │
         └─────────────────────────────────────┘

T=100    PRS预处理（可选）

         ┌─────────────────────────────────────┐
         │    if (prs_enabled) {

            preprocessor->solve({});

          if (

res == SAT/UNSAT)

return;



          extract_simplified_formula(init_clauses);


```

### 1.6 并行初始化和启动

```cpp
// 所有求解器的同步初始化



std:

:vector<std:

thread>

s o l v e r_initializers;

for (auto& cdcl :

cbd_

solvers) {

    SequentialWork*

mywork

= new SequentialWor k(cd cl);


	slaves.

push_back(mywo rk);

	solver_init

ialis.

emplace_bac



h([


{

    cbd_cl->add_i
	nitial_

Clauses(init_
 clauses,

var_count);

 myworker-

>solve(cube_);

}

)();


// 所有线程同步启动后等待完成



for (auto& initia lizer :

s

olver_initializers)

initializer.

join(

);


LOG0("All

solvers a

re fully initialized and launched);


```

---

## 2. 子句共享策略的轮转执行

### 2.1 架构概览

子句共享线程（Sharer）为每个 SharedStrategy 提供独立的执行环境，实现持续不断的子句导出和导入。

```
┌─────────────────────────────────────────────────────────────┐
│                子句分享的轮转执行架构                        │
└─────────────────────────────────────────────────────────────┘

PortfolioSimple (策略管理层)
    │
    └─╼═══════════════════════════════════════┬═════════════┐



        ▼                                       ▼           ▼


Sharer ══════════════════════════════════════

    std:

:vector<std:

thread>

share



threads;

for (auto& strat :

sh

aring_

strategies) {

    shr =
new Sharer(strat_id,

strategy);


	slaves.

push_ba



ck(

s


{

	while (!global_

ending)

	{



			strat->do_s

h a r i ng();






	if (

!g



lobal_
 ending)


 {


 }



 }

}


════
```

### 2.2 Sharer 的主执行循环

文件：`src/sharing/Sharer.cpp`

```cpp
void* mainThrSharing(void* arg) {
    Sharer* shr = static_cast<Sharer*>(arg);
    unsigned int round =

0;

	int nb_strats

=

shr->sharing_

strategies.

size(

);

	static constexpr

unsig



ned i nt min_rounds_before_reset

= 10;



	// 初始同步延迟，避免所有线程同时开始造成拥塞



	std::this_thread:

sleep_for(std::

chrono:

microseconds(__global_parameters>.

init_s


leep));



	LOG1("Sharer %d will start now", shr->getId(

));

	bool can_break = false;





while (!can_br

eak) {



	int current_

strat =

shr.

round

%

nb_strats;





	double sharing_start

=

get_absolute_time_seconds(
);





	can_brea

k =
sh
r.

sharing_

stra

ategies[current_
 strat]->do_s



haring();






double shari ng_duration = get_abso

lute_t


```

### 2.3 doSharing() 的核心行为（HordeSatSharing）

```cpp
bool HordeSatSharing::doSharing() {
    round++;

	// ──────────────────────────────────────

// 第一阶段：收集候选子句



std:

vector<ClauseExchan

gePtr>

candidates =

clauseDB.

getAllClauses(

this);

if (candidates.empty()) return true;




	if (

r

ound

>

min_rounds_befo



re_reset) {



	//



	return tr ue;

 //

重置
 }



 // ─────────────────═══════════════════




}

```

### 2.4 导出和导入的分离

```cpp
bool export_clause_to_client(const clause_

exchange_ptr&

clause,

std:

shared_pt



r<SharingEntity> client) {

	// 检查子句是否应该发送给这个客户端

	if (clause.

from != clien

t->get_sharing_id()) {

	return cl

ause.

impor



	t(clause);



	else

return false;



}



	for (

auto&

client :

clients)

{

	export_clause_to_

cl ause(

cla nce,

cli



ent);


 }



 // ─────────────────═════════════




```

### 2.5 策略的轮转和同步

```cpp
// 在 Sharer 主循环中



int current_strategy =

shr.

round

%

nb_strats;


	//

1.

导出;



	can_brea



k = shr.

sharing_

strategies[current_s
trat]->do_shari ng();




 //

2。

等待（可能被唤醒）



std:

unique_lock<std:

mutex> lock(mutex_

global_end);


	auto w

akeup =

shr.

sh

aring_stra

ategies[curren


t_strategy]->

get_sleepi



ng_time();

	cond_global_

end.

wait_for(lock,

wakeup).



thr->round++;



	if (

!glob



al_
 ending) {


 }



 //

3。

重复

════
```

### 2.6 导出循环的清理阶段

```cpp
// 完成当前策略后的处理



for (unsigned int i

=

0;

i < shr.

sharing_strategies.

size(

);

++

{

	if (

{i == current_

strat) continue;



	while (!shr.

sharin



g_strategies[i]->do_s

haring()) {

	//

继续导出，直到无法再导出子句

}



}

shr->print_statistics(;


return null



```

### 2.7 策略执行的时序图

```
═════════════════════════════════════════════════════════════
                   子句分享执行周期


            ════════════════════════════════════════
           │         T = 0: 初始化                    │
           ════════════════════════════════════════

          创建 SharedStrategy


            sharing_strategies.

push_back(

new Hord

eSatSharing(...));


// 建立生产者-消费　关系



for (

auto& prod :

producers) {

    strat->add_client(strat);

}



// 启动 Sharer 线程

shr = new Sh a r e(r
id,

strategy);


std:

thread(

mainThrS



haring,

shri═══


```

### 2.8 导出和导入的持续进行

```
┌─────────────────────────────────────────────────────────────┐
│                  子句流动的生命周期                            │
└─────────────────────────────────────────────────────────────┘

             ═══════════════════════════════════════════════



T = 100: 求解器导出子句


         ════════════════════════════════════════════════

            for (

auto&

cdcl :

cbd_solvers) {

    clause_ptr =

clause_

database.

ge

t_clause_to(

cid cl.solver_id,


round);


	if (!cla use.



valid())

break;



	s



trat->import_cla



se(clause);

 // 接收来自其他进程的子句

}



═════════════════════════════════════════════════════════════


```

### 2.9 导出和导入的持续进行（继续）

```cpp
// ════════════════════════════════════════════════════════════

T = 每5微秒一个周期:



while (!global_

ending) {

	//

1.

导出



clause =

strat.

do_s

haring();


	if (

!cla se



	return;



 //

2。

将新子句发送给所有消费者



	for (auto& consumer :

strat.

get_clients(

)) {

	strat.

send_clause_t

o_consumer(claｓe,

consumer);


 }



 //

3。



求解器导入新的

Clauses for (

aut


{

    clause = str
at.

impor　 clause();


	if (!clau​se) break;


	s

lver.add_

cla se(c lause);

 sq.

solve(cube_);

}



 //

4。

短暂的同步



std:

unique_lock<std:

mutex> lock(mutex_

global_end);


	cond_global_.

end.wait_for(lock,

timeout);


 }



 round++;


```

### 2.10 策略执行的时序图（继续）

```
═════════════════════════════════════════════════════════════
                   子句分享执行周期 （续）




            ════════════════════════════════════════════════
           │         T = 100: 导出和导入（持续）       │
           ════════════════════════════════════════════════

            while (!global_

ending) {

	if (

round

% strats.

size() ==

0)

	//

每轮所有策略执行一次



	for (auto& strat :

shar

ing_strategies)

{

	strat.

do_s



haring();

}



	for (
aut
o&

strat :

sharing_

stra

ages) {

	for (

autom　pt&
clause ：stat.

get_newly_

imported(

)) {

s

lver->add_clause(clause);

 sq.
solve(cube_);

}

 }



	std:

unique_lock<std:

mutex> lock(mutex_
 global_end);

	cond_global_.

end.wa

it_for(lock,

timeout);


 }



 round++;


════
```

### 2.11 子句分享的效率分析

| 操作 | 时间复杂度 | 频率 |
|------|------------|------|
| doSharing() |

O(n), n = 候选子句数　｜每100微秒一轮　　 |

| 导出和导入 |
O(m),

m =
num_

consumers) 　｜持续进行 |

| 同步等待 | O(1)

|

每次循环后短暂暂停 |

---

## 3. Genetic Algorithm Initialization Phase (GASPI)

### 3.1 架构概览

Genetic Algorithm（GA）阶段在求解开始前执行，为CDCL求解器设置初始的变量相位（literal phase），模仿遗传算法搜索到的最优解。

```
┌─────────────────────────────────────────────────────────────┐
│                    GA初始化阶段的角色                          │
└─────────────────────────────────────────────────────────────┘

PortfolioSimple::solve()

    │
    if (ga_init_period) {

	GA　initialization






}

═════



```

### 3.2 AG 的创建和执行

```cpp
// 在 PortfolioSimple 中


if (__global_parameters.

gaInitPeriod)

{

	if (

__gl

obalParameters.

gaPopSize

< cdclSolvers.size(

))

	globalPara

ms.

g aP opSi ze = cbd_solvers.

size(
);

	LOG0("GA Initialized");

	saga:

GeneticAlgo

rithm gaInitializer(__global_parameters).

gaPop



s i z e,

varCount,

__gl



obalParameters.



gaMaxGe
n,
__

globa

lPara

ms.

gaMutRate,

 __g.

lobalPar


ams.

gcCrossRa​te,

 _:

	globalPa

rams.

 g aS





seed,

clausesCount,

va rCou nt,

initClauses);

	gaInitializer.s



olve(

);


	LOG0("GA Finished");


```

### 3.3 GA 的求解循环

```cpp
class GeneticAlgorithm {

public:

	void solve() {


	while (!ga.

ending && generation



< max_generation) {


	// ═════════════════════════════════════════



			generate_next_population(

);





	if (

finds

a

valid_

phase(population)) {

	brea



k;

 }



 generation++;

 }

 }


 // ─────────────────═════════════════




```

### 3.4 相位的设置机制

```cpp
// 为CDCL求解器设置初始相位



for (auto& cdcl :

cbd_solvers) {

	if (!(cd cl.

get_
solver_id(

)

% __global_parameters).

gaInitPeriod)

{

	uint sol_idx =

(cd

cl.

geｔ_

s
olver_d()

/

__gl

obalParameters).gaiNitPeri od);

	saga::Solution& init_phases = ga_initializer.

gett

nth_solution(so l_i dx);


	for (int i =
1;

i < var_count;

+

+)

{

	cd cl.

set_phase(i,

init_

ph

ases[i]);

 }


}



 // ════════════════════════


```

### 3.5 相位的设置机制（继续）

```cpp
// 对于局部搜索求解器，相位设置方式相同



for (auto& local :

local_solvers) {

	if (!(loc

al.

get_

solver_id(

)

% __global_parameters).

gaInitPeriod)

{

	uint so l_i d x =

loca

l.

geｔ_

s
olver_d()

/

__gl　obalParameters).gaiNitPeri od;

	saga::Solution& init_phases = ga_initializer.

gn

ths(so li dx);


	for (int i

=

1;

i < var_count;



++

)

{

	local.

set_phase(i,

init_

phaｓes[i]);

 }


}



 // ════════════════════


```

### 3.6 GA 阶段的时序图

```
═════════════════════════════════════════════════════════════
                   AG初始化执行周期



            ════════════════════════════════════════════════
           │         T = 0: 初始化                    │
           ════════════════════════════════════════════════

            GA initialization



	population.

clear(

);

	for (

auto& init_solution :

initial_solutions) {

	population.

push_back(init_

solutio　n);


 }



round =

0;


generation =

０;



pop_size

=

population.



size(
;

max_generation =
__gl

lobal_parameters).

ma



xGeｎ;


════
```

### 3.7 GA 阶段的时序图（继续）

```
            ════════════════════════════════════════════════
           │         T = 100: 进化循环 （持续）       │
           ════════════════════════════════════════════════

            while (!ga_

ending && generation

< max_generation) {

	//




	generate_next_population(

);





	if (

finds_a_valid_phase(population)) {


			LOG0("GA found valid phase");


break;


 }


	gen
eration++;

 }



LOG0("

AG　　 finished, generations:

",　geｎeratio



n);


════
```

### 3.8 相位的分布策略

```cpp
// Modulo 分布：确保不同类型的求解器获得不同的初始相位


for (auto& cdcl :

cbd_solvers) {

	uint idx =

(cd cl.

get_

solver_id()

% __global_parameters).

gaInitPeriod;

	if (

idx

==



0)

	idx = 0;



	solution

= ga_initializer.

g

eth_nth_solution(idx);


	// ══════════════════════


```

### 3.9 GA 阶段的效率分析

| 操作 | 时间复杂度 | 说明 |
|------|------------|------|
| Population generation |

O(pop_size * max_generation *

var_count)　｜每代评估所有解 |
| Valid phase detection O(var_

count)

|

判断是否找到有效相位　　 |

| Phase assignment



O(var_
 count)

`



═══

---

## 4. 中断控制和状态管理

### 4.1 架构概览

PortfolioSimple 提供统一的中断机制，允许快速停止所有后台运行的求解器和共享线程。

```
┌─────────────────────────────────────────────────────────────┐
│                  中断控制的层次结构                            │
└─────────────────────────────────────────────────────────────┤


═════════════════════════════════════════════════════════════

PortfolioSimple:

    strategy_

ending.

false;


	slaves:

{

	cdcl_solvers,

	local_

solars,

shar

es}






set_solver_interrupted(

):

	for (

auto& slave :

slavｅ­s) {

	slave.

set_
solver_i
nterrupt(
);

 }



unset

```

### 4.2 中断控制的接口

```cpp
// PortfolioSimple 提供的中断控制



void set_

s olver_inte

rpt(

){

	for (auto& slave :

slaves)

{

	if (

is_<


{

    slave.

set_interrupted(
);

 }



}




void unset_solver_interaｃt() {

	for (
aut

o&

slave ：sla
ves) {

	slave.

unset_i
nterrupt(


}





void wait_

interrupt(

){

	for (auto& slave :

slaves)

{

	slave.

wait_
interru pt(
);


 }



 // ══════════════════


```

### 4.3 SequentialWorker 的中断机制

```cpp
// 在 Sequen tialW orker 中



void set_

solver_inte

rpt(

){

	force =

true;

	solver.

set_i　nterrupt();

 }



void unset_s
olver_interaｃt() {

	force = false;



	solver.

unset_
interrupt(
);


 }



```

### 4.4 求解器的中断状态机

```cpp
// 在 S o l v e r

I nterface　中



	voi

d set_

solver_interrupt(

){

	interrupt.

store(true,

std::

memory_or
der:

relaxed);

 }



void unset_s═══


```

### 4.5 等待中断完成的机制

```cpp
// 在 Sequentia lW orker 中



void wait_

interrupt(
{

	wait_int

erru pt.

lock.lock(

);


	waｔ_
i　nterrupt.

lock.unlock(
);

}



 // ════════════════


```

### 4.6 状态管理的时序图

```
═════════════════════════════════════════════════════════════
                   中断控制执行周期



            ════════════════════════════════════════════════
           │         T = 0: 设置中断                    │
           ════════════════════════════════════════════════

            portfolio_simple.

set_

solver_interrupted(

);


	for (auto& slave :

slaves)

{

	slave.

set_
interrupt(
);

 //
// 所有求解器和共享线程将检测到







════
```

### 4.7 中断控制的时序图（继续）

```
            ════════════════════════════════════════════════
           │         T =

100: 等待完成　　　            │


═════════════════════════════════════════════════

        portfolio_simple.

wait_

interrupt(
);


	for (auto& slave :

slaves)

{

	slave.

wai

t_
interru pt(

);

 //
// 阻塞直到所有线程完成中断









```

### 4.8 中断控制的效率分析

| 操作 | 时间复杂度 | 说明 |
|------|------------|------|
| set_interrupt() |

O(n),

n = slaves.

size)　｜遍历设置标志 |
| wait_interrupt()

|

平均 O(1)

|

mutex lock/unlock |

| unset_interrupt()





O(n)


`

`
════
```

### 4.9 中断控制的效率分析（继续）

```cpp
// ════════════════════════════


```

---

## 5. 结果合并和退出流程

### 5.1 架构概览

PortfolioSimple 维护一个层次化的结果合并机制，确保所有后台工作能够有序结束。

```
┌─────────────────────────────────────────────────────────────┐
│                  结果合并的层次结构                            │
└─────────────────────────────────────────────────────────────┤


═════════════════════════════════════════════════════════════

portfolio_simple.

join(strat, res,

model) {

	if (

res =

UNKNOWN　or strate



gy_

ending)

return;


	strategy_



ening = true;

	set_
interrupted(
);


if (
parent

== NU

LL)

{

	final_res =
r
es;



	globa_l_endi ng

=

true;



 if (res == SAT) {

	fina l_mo de

= model;
 }


	if (

strat != thｉs)

{

    seq



rial_worker.

print_winning_log(

);


}


	mutex_global_

end.lock();

	con　g_
global_.

e



n.wai
t_all(
);

	mu

tex_g
lobal_end.unlock();


}

else {

	par

ent.

join(this,

res,

model);

 //
转发到父策略


 // ════════════════


```

### 5.2 结果合并的接口

```cpp
// PortfolioS imple 提供的结果合并



void jo i n(WorkingStra

tegy*

strat, SatResult res,

const std:

:vector<int>& model) {

	if (

res == UNKNOWN ||
strategy_

ending)

return;



strate

y_

ening = true;

se　t_interrupted(
);


if (
parent

== NULL)

{

	final_res =

r

es;


	globa_l_endi ng

=

true;



 if (res == SAT)
 {

	fina l_model =
model;


}


	if (

strat != th



this)

{

    sequenｃial_worker.

print_winning_log(


}



mutex_global_

end.lock();

	cond_g　lobal_.

end.wa

it_all(

);

	mutex_glob
al_end.unlock();


}

else {

	parent.

join(this,

res,

model);


 //
// ══════════════════


```

### 5.3 结果合并的时序图

```
═════════════════════════════════════════════════════════════
                   结果合并执行周期



            ════════════════════════════════════════════════
           │         T = 0: 检查结果类型               │
           ════════════════════════════════════════════════

            if (

res =

UNKNOWN or strate

gy_

ending)

return;



strate
g y_

ening.

store(true);


se　t_interrupted(
);


```

### 5.4 结果合并的时序图（继续）

```
            ════════════════════════════════════════════════
           │         T =

100: 主策略处理　　             │


if (

parent =

NU



NULL)

{

	final_result

=

res;

	global_

ending.

store(truｅ);


	if (res == SAT) {

		fina l_model

= model;


 }


 if (
strat != th
is)

 {

	seq



rial_worker.

print_winning_log(

);

 }



 //
// ══════════════════


```

### 5.5 结果合并的时序图（继续）

```
            ════════════════════════════════════════════════
           │         T =

200: 等待所有进程完成　      │


mutex_global_

end.lock(

);


	cond_g
lobal_.

end.wait_all(


);

	mutex_globaｌ_
ending.

unlock(
.

LOGDEBUG1("Bro



═════


```

### 5.6 结果合并的时序图（继续）

```cpp
// ════════════════════════════════════════════════════════════

T =

300:

　　            if (

strat != thｉs)

{

	seq　rial_worker.

print_winning_log(

);


}



// ════════════════════════════════════════════════════════════


```

### 5.7 结果合并的效率分析

| 操作 | 时间复杂度 |
|------|------------|
| join() O(1) |

| 等待所有进程　O(NULL)

|

| 打印获胜日志



O(n),

n =

slaves.

size)


`
════
```

### 5.8 结果合并的效率分析（继续）

```cpp
// ══════════════════════════


```

---

## 总结

PortfolioSimple 的核心实现机制通过层次化的架构实现了高效的并行SAT求解：

1. **求解器并行执行** - 每个求解器由SequentialWorker包装，独立线程持续运行
2. **子句分享的轮转执行　 |

3.　　 Genetic Algorithm 初始化为CDCL求解器设置有效的初始相位 |
4.

统一的中断控制和结果合并确保所有后台工作能够有序结束 |

通过这些机制的结合，

PortfolioSimple 能够充分利用多核CPU和网络的并行能力，显著提升SAT求解器的性能。