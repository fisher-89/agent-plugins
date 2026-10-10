# 设计: unify-run-state-persistence

> **变更**: unify-run-state-persistence
> **日期**: 2026-10-09

---

## 提案与规格同步状态

`proposal.md` 与 `specs/**`（五能力 delta：state-store 两 ADDED + 六模型修订、queries 详情聚合修订、orchestration 两修订 + 两 ADDED、flow-view 三修订、corpus-regression 一 ADDED）已由提案阶段定稿；本设计不重复其内容，在 proposal 九项「待决问题」之上逐项定稿（D1–D9），并补设计面决策 D10–D13。改动基线核实在场：`control.rs` 头注释决策原文、`walker.rs:walk_run` 终态出口单点（drive → guard.finish）、`store.rs` change 域操作面与 `change_port.rs` 委托胶水、`detail.rs` 聚合、双钩 + `run-state.ts` reducer、`graph.ts` 双源拼缝——全部可查。

**关键既有约束（设计必须绕开的）**：`RunUpdate` 不只服务 change run——`crates/infra/agent/src/worker.rs` 经 `RunEventSink` 把 `RunUpdate::SessionEvent`（携载荷）透传给 **change run 与归档链两个 sink**（`ChangeFlowSink` / `ArchiveSink`）。降位只能落在 IPC 面（Channel），不能动内部 seam 类型——归档链零触点是本变更红线（proposal「不要修改」未列，但 flow-view delta 的 liveEvents 退役仅指 change run 面，归档链实时面是另一机制 `use-archive-flow`，本设计保持零触点）。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| run 中性类型与 port 扩展 | `RunStatus`（五值）/ `RunStepKind`（五值封闭集）/ `RunStepStatus`（四值）/ `RunStateRecord` / `RunStepStateRecord` / `RunStartCommand` / `RunFinishCommand`（含 `RunStepEntry`）；`ChangeStateStore` 增 run 域读写五方法 | `crates/core/workflow/src/state.rs`（扩展） | crate 内 serde/specta | 中性类型与持久化记录分离既有纪律；词汇本体单点（写命令与读投影同类型消费） |
| run 写面 | `run_start`（建档在案 + run_id 非空 + 同 id 冲突校验 → running 行）/ `run_finish`（run 行在案且 running + status 终态三值校验 → 终态更新 + 步整包 + active_phase 清位单事务） | `crates/core/workflow/src/write/run.rs`（新） | state.rs | `phase_start` / `phase_log` 校验-后委派同构；时间戳命令携带（corpus 确定性） |
| 详情读面扩展 | `ChangeDetail` 增 `runs: Vec<ChangeRunEntry>`（每 run 内嵌 `steps`，全史不截）；runs 按 `started_at` 升序、steps 按 `seq` 升序；时间戳 ISO 串 + null 口径不变 | `crates/core/workflow/src/queries/detail.rs`（扩展） | state.rs | 纯 derive；`RunStatus` / `RunStepKind` / `RunStepStatus` 中性类型直用（`ChangeStatus` 先例）；文档形态 change 恒空 runs |
| run 持久化模型 | `RunRecord`（native_model id=13，v1：run_id 主键 / change 二级索引 / status / reason / started_at / finished_at）；`RunStepRecord`（id=14，v1：u128 打包主键 `(hash64(run_id), seq)` + run_id 二级索引 + phase / attempt / step / status / session_id / detail / timestamp） | `crates/infra/store/src/model.rs`（扩展） | native_db/native_model | 打包键先例 `SessionEventRecord`（大端序 = 数值序，seq 空洞合法）；additive 注册零迁移 |
| run 域操作面 + 启动标定 | `start_change_run` / `finish_change_run`（单事务：终态 + 步整包 + active_phase 清位）/ `calibrate_interrupted_runs`（running → interrupted + 全量 active_phase 清位，幂等）/ `list_change_runs` / `list_run_steps`；`open_workspace` 内嵌标定调用 | `crates/infra/store/src/store.rs`（扩展） | model.rs | 标定先读扫后写（零残留零写事务）；标定记因附中断语境 |
| run 落库 port 缝 | `RunHistoryPort` trait（`run_started` / `run_finished`，sync 零 tokio）；`StoreRunHistory` 适配器（持 `Arc<dyn ChangeStateStore>` 委派写面）；**步词汇过滤单点** `persisted_step(ChangeStepKind) -> Option<RunStepKind>` + finish 组装 helper（seq = emit 序盖戳） | `crates/core/orchestration/src/port.rs`（trait）、`run_history.rs`（新文件：适配器 + 单点谓词 + 组装） | workflow（既有） | walker / control 源码零过滤逻辑（grep 可验）；core 零 infra 依赖不变 |
| 注册表步累积器 | `RunEntry` 增 `steps: Vec<ChangeStepState>`（emit 序追加，全词汇 10 类）与 `started_at`；`publish` 累积 + 广播 `RunNotice`；`snapshot()` 含 steps；头注释决策原文改写（翻案留痕） | `crates/core/orchestration/src/control.rs`（扩展） | crate 内 | 进程内零 IO 不变量保持（store 句柄不进 control） |
| 通知降位 | `RunNotice`（五 kind-only 变体：step / sessionEvent / ask / confirmWait / finished，tag `ipc`）为唯一 IPC 信封；`RunUpdate` 保留为内部 seam（worker sink / ArchiveSink 零改动）；`ChangeRunSnapshot` 增 `steps` + `startedAt` | `crates/core/orchestration/src/state.rs`（扩展） | serde/specta | 通知仅失效信号，查询结果权威 |
| walker 落库缝 | `walk_run` 起点第一写（`run_started`，失败 fail-fast）+ 终态出口单点第二写（`guard.steps()` → run_history 组装 → `run_finished` best-effort → `guard.finish`）；`RunRequest` 增 `started_at` | `crates/core/orchestration/src/walker.rs`（扩展） | run_history/port/control | 全死亡路径仍汇聚 `guard.finish` 唯一出口；每 run 恰两写 |
| 合并查询装配 | `get_change_detail` 返回 `ChangeDetailUnified { detail, activeRun }`（shell DTO + specta）：core 聚合 + 注册表快照（含 steps / ask / 停等态）一面出；`ActiveRunView` = 快照投影（startedAt ISO 化） | `src/commands/changes/mod.rs`（扩展） | workflow + orchestration + store | 读时合并零写路径；blank root / 开库失败 None 语义不变 |
| 通知桥与命令面瘦身 | `change_flow_start` / `change_flow_watch` Channel 面 `RunNotice` 化；start 装配 `StoreRunHistory` + 铸 `started_at`；**`change_flow_state` 命令退役**（统一查询承接重挂恢复） | `src/commands/change_flow/mod.rs`（扩展） | orchestration | stop / confirm / answer 请求-应答零改动；归档链命令组零触点 |
| 前端两钩并一 + 图常驻 | `useChangeDetail` 承接统一视图取数（查询缓存 + 失效重取 + 300ms 尾随去抖）；`useChangeFlowRun` 缩为控制动作 + 通知订阅（activeRun 在场才订阅，finished 后通道自然断开）；`run-state.ts` reducer 收缩（`applyRunUpdate` / `seedRunState` / `initialRunState` / liveEvents / seq 去重删除，`runStepNodes` 改统一视图输入）；`graph.ts` 拼缝单源化；`useRunViewEffects` 回落分支删除；转录实时面统一转录库重查（150ms 去抖） | `packages/desktop/src/views/changes/**`（见变更清单） | bindings | 图常驻 = `runs[].steps ∪ activeRun.steps` 同一转换函数；步词汇 snake/camel 归一点单点 |
| 版本交付 | `packages/desktop` 0.4.27 → 0.4.28 | `packages/desktop/package.json` | — | D13 |

**不变组件（零触点核对结论）**：`crates/infra/agent/src/worker.rs` 与 `RunEventSink` seam（`RunUpdate` 类型原样）；归档链全族（`archive_flow.rs` / `ArchiveSink` / `use-archive-flow.ts` / archive-panel——其 liveEvents 是归档自身机制，非 change run 面）；`steps.rs` 审计路径与 `StepRecord` 七臂词汇；相位机写面语义（phase_next / phase_log / backtrack / decision_log——仅 finish 缝新增 active_phase 处置）；`StoreSnapshot` / `LocalToolSteps` 装配形态；`main.rs`（启动标定内嵌 store 打开，装配零改动）；会话转录落库与 `StopRegistry`；`plugins/dev-team` 全部；`openspec/specs/**` 基线。

---

## 关键设计决策

| # | 问题（proposal 待决 / 设计面） | 定稿 | 理由 |
|---|------|------|------|
| D1 | active_phase 终态处置形态（待决 1） | **清位**，不转静态记录：`finish_change_run` 单事务内 `active_phase = None`；启动标定 sweep 兜底清全部悬挂（见 D12）。「上次运行停在哪」不进 ChangeRecord——interrupted run 的标定 `reason` 携残留语境（有 active_phase 时附「中断于 phase X attempt N」），信息留痕零 schema 成本 | 续走锚点等价性论证：`phase_next` 路由只读 `PhaseRecord` 评估史（`has_phase_passed` + 窗口计数），**零消费 active_phase**；`phase_log` 的「开相前置」在新 run 中恒由 `phase_start` 先行满足（walker 循环 ②→⑥ 定序），`phase_start` 又是无条件覆写——清位对续走语义零影响。静态记录方案要新字段 / 新投影面，收益已被 runs[].status + reason 覆盖（D9） |
| D2 | 步过滤单点挂点（待决 2） | **落库写面消费谓词**，读面零过滤：词汇本体 = `workflow::state::RunStepKind`（五值 snake_case 封闭集），落库命令 `RunStepEntry.step` 即该类型——**store 结构上收不到忽略集**；10→5 映射单点 = `orchestration/src/run_history.rs::persisted_step(ChangeStepKind) -> Option<RunStepKind>`，finish 组装 helper 消费；`walker.rs` / `control.rs` 源码零过滤逻辑（AC-3 grep 面） | 读层过滤会让「库内有但出线无」两套真相（词汇漂移无守卫）；写面封闭类型使忽略集不可表达，词汇单点 = 类型本身（写命令与 `detail.rs` 投影同一类型消费） |
| D3 | RunUpdate 降位信封（待决 3） | **新增 `RunNotice` 五 kind-only 变体**（`step` / `sessionEvent` / `ask` / `confirmWait` / `finished`，tag `ipc` 零载荷）为唯一 IPC 面；`RunUpdate` 原样保留为进程内 seam（worker.rs → ChangeFlowSink / ArchiveSink 仍携载荷消费，`publish` 内部累积快照面后向 broadcast 总线只发 Notice）。`Channel<RunNotice>` 进 `change_flow_start` / `change_flow_watch`；bindings 再生成（`RunUpdate` 随命令签名退役自然出 bindings） | 拆掉 `RunUpdate` 载荷会炸归档链（ArchiveSink 消费 `SessionEvent{session_id, event}` 载荷）——归档链零触点红线；kind 位保留给客户端分流去抖（D8：sessionEvent → 转录重查，其余 → 统一视图重查），零载荷 kind-only 比最小载荷省 run_id 冗余（订阅本身 per (root, change) 寻址） |
| D4 | `change_flow_state` 快照命令存留（待决 4） | **退役**（命令 + handler 注册 + 前端调用点删除）：重挂恢复 = 统一查询 `get_change_detail`（activeRun 含 steps / ask / 停等态）+ `change_flow_watch` 补订；`ChangeRunSnapshot` 类型保留（合并查询与 `subscribe` 内部消费，含 steps / startedAt） | 独立快照命令与 activeRun 面完全重叠——保留即第二取数路；重挂时序 = 先查统一视图、activeRun 非终态才 watch（间隙丢通知由后续通知自愈，run 静默期最坏少一拍至手动刷新，R7 语义既定） |
| D5 | run 行 start 写落位（待决 5） | **walker 起点直调**：`walk_run` 第一语句经 `RunHistoryPort::run_started` 落 running 行；失败 fail-fast（emit Finished failed + `guard.finish`，零相位执行）；`RunRequest` 增 `started_at`（命令层发起时铸造，与 `begin_run` 同值入 RunEntry） | 两写时序全部收在 walker 可测面（AC-2 假引擎直驱可断「恰两次」）；命令层装配会引入「begin_run 成功但 start 行写失败」的注册表补偿路径（现有 RunGuard 无 abort 面）；start 失败 = store 已坏，后续相位写必败，fail-fast 语义最诚 |
| D6 | finish 落包失败收敛（待决 8） | **best-effort 单次尝试 + 启动标定兜底**：`run_finished` Err 不阻断收口（`guard.finish` 照常，run 终态与订阅释放不受损）；start 行残留 running → 下次启动 sweep 标 interrupted（reason「重启标定」）——审计面自愈，内存面与库面短暂不一致为既定窗口。不重试、不显式错误面 | 反面（finish 失败即不除名 / 报错挂起）会让注册表条目泄漏阻塞后续发起，比缺一段运行史更伤；`audit_outcome` best-effort 先例同构 |
| D7 | RunStepRecord 主键与索引（待决 7） | **u128 打包键 `(hash64(run_id) << 64) \| seq` + `run_id` 二级索引**（`SessionEventRecord` 先例复用 `session_key_hash` / `event_key_serde`）；**seq = 全词汇 emit 序**（RunEntry 追加序，被过滤步占号产生库内空洞——排序键语义合法，先例同源）；step `timestamp` = 命令携带的 `finished_at`（整包同刻，corpus 确定性） | native_db 无复合主键（Spike① 留痕）；自增 id + 索引的迭代序不保 seq 语义且需二次排序；emit 序编号使库面行与 RunEntry 累积器同一下标可对照（重挂 live 面 ∪ 库面拼接免重排） |
| D8 | 高频通知节流（待决 6） | **客户端尾随去抖，通知侧零 coalesce**：统一视图重查 300ms、转录重查 150ms（trailing，按通知类别分流——D3 kind 位）；通知仅失效信号，去抖窗内到达即重置计时，最后一拍必达 | 服务端 coalesce 需在 broadcast 总线加定时聚合层（新机制 + 容量语义变化）；客户端去抖零服务端改动，风暴上界 = 每秒 ~3 次合并查询（含 runs/steps 聚合，量级可控）；去抖参数为前端常量单点 |
| D9 | 详情头部「上次运行」呈现（待决 9） | 随 D1 取清位，**不新增静态记录字段**：头部运行中标注 = `activePhase`（在飞真相）∪ `activeRun`（六值状态机）；收口后状态面经 `runs[]` 尾行（status / reason / 起止）可达，`RunControlPanel` 终态徽章 + 收口记因改读 runs 尾行（`activeRun` 退场后） | 信息已在 runs[] 全史内，静态记录是第二真相源；「运行中假象」由清位 + 标定双杀（AC-4/5） |
| D10 | 累积器词汇口径（设计面：两 spec 条款缝合） | **RunEntry 累积全词汇 10 类**（运行中 ToolStep / Gate 节点照 flow-view 既有条款上图可辨）；**落库与收口后常驻回显 = 5 类**（`persisted_step` 单点）。口径一致指「落户子集 = 收口后回显子集」——运行中 live 面按 flow-view「运行态节点与实时执行视图」条款保留全词汇（临时节点收口后褪去为既定语义，AC-9 断言五词汇常驻） | flow-view MODIFIED 条款明文要求运行中三门 / 相位机步节点可辨可观测（ToolStep 失败同等可见）；若累积器也滤 5，运行中静默跳步不可观测，直接违反该条款。收口后图 = 库读史派生（5）+ 在飞活步（10）叠加，无两面 emit（Step 无载荷广播，通知只失效） |
| D11 | 统一查询形态（设计面） | `get_change_detail` 返回 **`ChangeDetailUnified { detail, activeRun }`**：`detail.runs[]` 为库读史全量（**在飞 run 的 start 行已在库——status=running、steps 恒空**，run 清单面运行中可见）；`activeRun` 为注册表活面（六值状态机 + ask + steps 全词汇）。前端图转换 = `runs[].steps（5 类，snake 词）∪ activeRun.steps（10 类，camel 词）`，词汇归一（`static_check`→`staticCheck` / `test_execution`→`testExecution`）收转换函数单点 | 两面一刀而非把活步塞进 runs[].steps：steps DTO 类型是五值封闭集（D2 结构防线），塞 10 类需 union 类型破坏词汇单点；runs[] 含 running 行使运行史清单与库真相连续（AC-7 一次返回、无双命令拼接） |
| D12 | 启动标定挂点（设计面） | **内嵌 `Store::open_workspace`**：打开后先读扫——存在 `RunRecord.status=running` 残留或任一 `ChangeRecord.active_phase=Some` 才开写事务（幂等；零残留零写事务）；标定 = 全部 running 行 → interrupted（`finished_at` = 标定时刻，reason 附中断语境 + 残留 active_phase 词汇）**+ 全量 active_phase 清位**。命令层 / main.rs 零显式调用 | workspace 库惰性打开（`for_root` 首开缓存复用），挂 open 点 = 每 workspace 首次触及即标定，免装配遗漏；进程起点无在飞 run 是结构性事实（redb 文件锁单进程写，跨进程并行 run 不可达）——全量清位语义精确；标定方法同时 `pub`（corpus / 测试构造中断样本直调） |
| D13 | 版本与守线 | `packages/desktop` 0.4.27 → **0.4.28**（tauri.conf 自动跟随、Cargo.toml 不随动）；`plugins/dev-team` 2.10.44 零改动；bindings 全量再生成（RunNotice / ChangeDetailUnified / ActiveRunView / ChangeDetail.runs / snapshot 扩面 / change_flow_state 退役） | 既有惯例；生成物零手改 |

### 边界留痕：attempt 复用撞键形态

`phase_start` 的 attempt 推导 = 该相位**已落账条目数 + 1**——run 中途死亡（start 后未 log）不产生 PhaseRecord，下次 run 重开同相位复用同一 attempt 号。此时两 run 的步节点键 `phase:attempt:step` 理论可重（proposal「天然不撞」的例外形态）。处置：前端 `runStepNodes` 既有槽位机制（同键 running→终态配对、重号 `:seq` 后缀堆叠）+ 统一序 `(run started_at, emit seq)` 稳定分层，节点可溯 run 归属（runs 面按 run 分组）；库面无影响（行键含 run_id）。AC-9 场景（两次 run 全史叠加）在「第二次自续走锚点推进」形态下不触发本例外。

---

## 读写时序定形（walk_run 落库缝）

```
命令层 change_flow_start：
  前置校验（建档 / 相位表 / exec root / 归档互斥）→ begin_run(root, change, run_id, started_at)
  → 装配 StoreRunHistory(store) → subscribe + Channel<RunNotice> 转发 → spawn walk_run → 提前 resolve

walk_run：
  history.run_started(run_id, change, started_at)          ← 第一写（running 行；Err → fail-fast 收口）
  drive(...)                                               ← 既有循环零改动（emit_step 经 control 累积 + Notice）
  steps = guard.steps()                                    ← RunEntry 累积器快照（全词汇，emit 序）
  history.run_finished(组装: persisted_step 过滤 5 落 + seq=emit 序 + finished_at)   ← 第二写（best-effort, D6）
    └ store 单事务：RunRecord 终态/reason/finished_at + RunStepRecord 整包 + active_phase 清位（D1）
  guard.finish(status, reason)                             ← 既有唯一终态出口（广播 Finished + 除名）

启动（每 workspace 库首开）：
  sweep：running 行 → interrupted（reason 附语境）+ 全量 active_phase 清位（D12）

读路（get_change_detail）：
  change_detail(layout, store, name)                       ← runs[] 全史（running 行 steps 恒空）
  + control.snapshot(root, change)                         ← 活面（六值 + ask + steps + startedAt）
  → ChangeDetailUnified{ detail, activeRun }               ← 前端通知到达即（去抖后）重查本命令
```

---

## 数据模型

### store 模型（`infra/store/src/model.rs`，workspace 维度）

```rust
/// run 运行史主行（unify-run-state-persistence 翻案「不建 flow_runs 表」）。
#[native_model(id = 13, version = 1)] #[native_db]
pub struct RunRecord {
    #[primary_key] pub run_id: String,        // walker 既有 run-<millis> 词汇
    #[secondary_key] pub change: String,
    pub status: RunStatus,                    // running|completed|stopped|failed|interrupted（interrupted 仅标定产生）
    pub reason: Option<String>,               // 终态记因 / 标定记因
    pub started_at: i64,                      // UTC unix 毫秒（run_start 命令携带）
    pub finished_at: Option<i64>,             // 终态 / 标定时刻；running 恒 None
}

/// run 步节点史行（图史面；与 StepRecord 审计职责分立，词汇重叠不双写）。
#[native_model(id = 14, version = 1)] #[native_db]
pub struct RunStepRecord {
    #[primary_key] #[serde(with = "event_key_serde")]
    pub step_key: u128,                       // (hash64(run_id) << 64) | seq —— D7
    #[secondary_key] pub run_id: String,
    pub phase: String, pub attempt: u32,
    pub step: RunStepKind,                    // 五值封闭集（executor|evaluator|decision|static_check|test_execution）
    pub status: RunStepStatus,                // running|passed|failed|stopped（收口在途步可留 running）
    pub session_id: Option<String>,           // WorkerAgent 步会话槽
    pub detail: Option<String>,               // 人读记因 / 摘要（有界，写面截断同 diagnose_brief 口径）
    pub timestamp: i64,                       // 落包时刻（= finished_at，D7）
}
```

`workspace_models()` 注册两模型（八模型 → 十模型，additive 零迁移）；存量库打开路径零触碰（v1 新表）。

### 中性类型与 port（`workflow/src/state.rs`）

```rust
pub enum RunStatus { Running, Completed, Stopped, Failed, Interrupted }      // serde lowercase
pub enum RunStepKind { Executor, Evaluator, Decision, StaticCheck, TestExecution }  // serde snake_case
pub enum RunStepStatus { Running, Passed, Failed, Stopped }                  // serde camelCase
pub struct RunStateRecord { run_id, change, status: RunStatus, reason: Option<String>, started_at: i64, finished_at: Option<i64> }
pub struct RunStepStateRecord { seq: u64, run_id, phase, attempt: u32, step: RunStepKind, status: RunStepStatus, session_id: Option<String>, detail: Option<String>, timestamp: i64 }
pub struct RunStartCommand { run_id: String, change: String, started_at: i64 }
pub struct RunStepEntry { seq: u64, phase: String, attempt: u32, step: RunStepKind, status: RunStepStatus, session_id: Option<String>, detail: Option<String> }
pub struct RunFinishCommand { run_id, change, status: RunStatus /* 终态三值，写面校验 */, reason: Option<String>, finished_at: i64, steps: Vec<RunStepEntry> }

// ChangeStateStore 增（读半边两条 + 写半边两条）：
fn list_runs(&self, change: &str) -> Result<Vec<RunStateRecord>, StoreFault>;
fn list_run_steps(&self, run_id: &str) -> Result<Vec<RunStepStateRecord>, StoreFault>;
fn run_start(&self, command: &RunStartCommand) -> Result<(), StoreFault>;
fn run_finish(&self, command: &RunFinishCommand) -> Result<(), StoreFault>;
```

### 通知与快照（`orchestration/src/state.rs`）

```rust
/// 变更通知（唯一 IPC 信封；通知仅失效信号，查询结果权威）。
#[serde(tag = "ipc", rename_all = "camelCase")]
pub enum RunNotice { Step, SessionEvent, Ask, ConfirmWait, Finished }

/// 快照（合并查询活面；终态即 None 语义不变）。
pub struct ChangeRunSnapshot {
    run_id: String, status: ChangeRunStatus, phase: Option<String>, attempt: Option<u32>,
    ask: Option<AskPayload>, started_at: i64,                 // 新增（毫秒）
    steps: Vec<ChangeStepState>,                              // 新增（累积器快照，全词汇 emit 序）
}
```

### port 与适配（`orchestration/src/port.rs` + `run_history.rs`）

```rust
pub trait RunHistoryPort: Send + Sync {
    fn run_started(&self, command: &RunStartCommand) -> Result<(), String>;
    fn run_finished(&self, command: &RunFinishCommand) -> Result<(), String>;
}

// run_history.rs（单点谓词 + 组装；walker/control 零过滤词汇）
pub fn persisted_step(kind: ChangeStepKind) -> Option<RunStepKind>   // 唯一 10→5 映射
pub fn finish_command(req: &RunRequest, status, reason, finished_at, steps: &[ChangeStepState]) -> RunFinishCommand
pub struct StoreRunHistory { store: Arc<dyn ChangeStateStore> }      // impl RunHistoryPort → workflow::write::run_start/run_finish
```

### DTO（`workflow/src/queries/detail.rs` + shell `commands/changes/mod.rs`）

```rust
// core 线面（golden 契约；纯 derive，时间戳 ISO 串 + null）
pub struct ChangeRunStepRecord { seq: u64, phase: String, attempt: u32, step: RunStepKind, status: RunStepStatus, session_id: Option<String>, detail: Option<String> }
pub struct ChangeRunEntry { run_id: String, status: RunStatus, reason: Option<String>, started_at: Option<String>, finished_at: Option<String>, steps: Vec<ChangeRunStepRecord> }
// ChangeDetail += pub runs: Vec<ChangeRunEntry>（文档形态恒空数组）

// shell 统一视图（specta；ActiveRunView 为快照投影，startedAt ISO 化收命令层单点）
pub struct ChangeDetailUnified { pub detail: ChangeDetail, pub active_run: Option<ActiveRunView> }
pub struct ActiveRunView { run_id: String, status: ChangeRunStatus, phase: Option<String>, attempt: Option<u32>, ask: Option<AskPayload>, started_at: String, steps: Vec<ChangeStepState> }
```

线面片段（detail.runs 条目）：`{"runId":"run-1790841600000","status":"completed","reason":"All phases…","startedAt":"2026-10-01T08:00:00.000Z","finishedAt":"2026-10-01T08:04:00.000Z","steps":[{"seq":2,"phase":"implement","attempt":1,"step":"executor","status":"passed","sessionId":"ses-…","detail":null},…]}`

### 标定记因与去抖常量

| 面 | 定式 |
|---|---|
| 标定 reason | `重启标定：桌面进程中断，run 客观已终止` + 残留 active_phase 时附 `（中断于 phase {X} attempt {N}）` |
| finish 失败收敛 | best-effort 静默（D6）；无重试、无错误面（sweep 自愈留痕） |
| 去抖常量（前端单点） | 统一视图重查 300ms / 转录重查 150ms（trailing） |
| run_finished 校验 | run 行在案且 status=running、命令 status ∈ 终态三值（interrupted 拒绝——运行期写路径永不产生） |

---

## 变更清单核对（design 相对 proposal 的增补与落位）

proposal「实现文件」全部在架构组件表内落位。**design 增补**（proposal 未逐文件列名、实现必需）：

- `crates/core/workflow/src/state.rs` — run 中性类型 + `ChangeStateStore` trait 扩展（proposal 归入 `write/` 目录条目的 port 缝扩展，类型与 trait 依 crate 纪律落 state.rs）
- `crates/core/workflow/src/write/run.rs` — 新文件（写面校验单点）
- `crates/core/orchestration/src/run_history.rs` — 新文件（port 适配器 + 过滤单点 + finish 组装）
- `crates/infra/store/src/change_port.rs` — trait 委托胶水随动
- `src/commands/changes/mod.rs` 内新增 shell DTO（`ChangeDetailUnified` / `ActiveRunView`）
- `packages/desktop/src/views/changes/flow/run-control-panel.tsx` + `detail-drawer.tsx` + `session-transcript-panel.tsx` + `hooks/use-session-transcript.ts` — props 面随动（activeRun 供能 / liveEvents 退役 / 转录重查刷新键）

`main.rs` 零改动（D12：标定内嵌 store 打开）；`crates/infra/agent` 零改动（D3）。

---

## 验收标准对齐

| AC | 落点 |
|----|------|
| AC-1 | 数据模型节（两模型 v1 + 十模型 additive 注册；回环经 store 域操作面；打包键先例） |
| AC-2 | D5（walker 起点第一写 / 终态出口第二写，假引擎直驱可断言恰两次）+ D7（单事务整包，原子回滚 store 面断言） |
| AC-3 | D2（`persisted_step` 单点 + `RunStepKind` 结构封闭；walker/control 零过滤 grep 面） |
| AC-4 | D12（open 内嵌标定幂等；运行期写路径 interrupted 拒绝——run_finish 校验节） |
| AC-5 | D1（finish 同事务清位 + sweep 兜底；续走锚点等价性论证——phase_next 零消费 active_phase） |
| AC-6 | DTO 节（runs/steps 全史、ISO + null、纯 derive）+ golden 显式重写任务 + bindings 守卫 |
| AC-7 | D11（ChangeDetailUnified 一次返回库读史 ∪ 活面含步表；无双命令拼接） |
| AC-8 | D3（RunNotice kind-only）+ 前端解散任务（applyRunUpdate / liveEvents / seq 去重删除，knip 守线）+ 控制面零改动核对 |
| AC-9 | D10 / D11（runs[].steps ∪ activeRun.steps 同一转换函数；全史叠加 + 槽位机制吸收 attempt 复用例外——边界留痕节） |
| AC-10 | D7（seq = emit 序，重挂步表与 emit 序一致）+ 转录重查（D8 150ms；`use-session-transcript` 既有查询面） |
| AC-11 | corpus 任务（≥2 run 全史 / interrupted / 五词汇与 sessionId 两态样本，经 store 域操作面构造）+ 版本 D13 + 管线守线任务 |

### 风险对齐（proposal R1–R8 + finish 收敛 → 设计落点）

R1 决策翻案涟漪 → specs delta 已逐条修订 + control.rs 头注释改写任务（D-组件表「注册表步累积器」行）；R2 通知风暴 → D8（客户端尾随去抖 300/150ms + kind 分流）；R3 崩溃丢步史 → D5/D6（start 行 + sweep 自愈，既定权衡留痕）；R4 双步表漂移 → D2（职责分立，词汇重叠不双写，spec 已留痕）；R5 golden 面扩大 → 全量 detail 投影增 `runs` 键 + 两新语料，显式重写流程任务；R6 前端解散回归 → 转换层纯函数直测 + data-testid 既有挂钩（run-status / run-ask-card / run-confirm-card / drawer-session-*）；R7 通知-重查竞态 → D3/D4（通知失效信号、查询权威；静默期少一拍至下一通知 / 手动刷新）；R8 active_phase-锚点耦合 → D1 等价性论证；finish 落包失败 → D6（best-effort + sweep 兜底）。
