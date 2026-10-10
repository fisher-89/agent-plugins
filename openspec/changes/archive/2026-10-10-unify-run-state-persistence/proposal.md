# 提案: unify-run-state-persistence

> **变更**: unify-run-state-persistence
> **日期**: 2026-10-09
> **状态**: draft
> **探索**: `openspec/changes/unify-run-state-persistence/explore.md`（范围决议四问 + 残余问题原则均已用户拍板在案）

---

## 问题

详情页两套流程数据各持一个真相源：`useChangeDetail`（workspace 库派生面，显式 refresh 拉取）与 `useChangeFlowRun`（进程内注册表面，快照恢复 + Channel 实时流）。由此产生一组口径断崖：

| 数据 | 运行中页面显示 | 完成后查询 | 一致? |
|------|--------------|-----------|------|
| run 状态机（running / waitingAsk / …） | ✓ 注册表 | ✗ 库里无此概念（仅 active/archived） | ✗ |
| run 终态 + reason | ✓ 冻结的 run.state | ✗ 广播完即除名 | ✗ |
| 步节点史（executor / 决策 / gate / 工具步） | ✓ 前端 reducer 内存累积 | ✗ 零落库，只剩 attempt 级 verdict | ✗ 粒度断崖 |
| phase / attempt 相位 | Step 事件跟随 | `record.active_phase` | ⚠ 悬挂 |
| 会话事件 | liveEvents 内存缓存 | 转录库（attempt 槽位可达） | 半一致（形态不同） |

两个关键认识（explore 已核实）：

1. **active_phase 悬挂是最尖锐的不一致**。`phase_start` 写入、`phase_log` 清位（`workflow/src/state.rs:88`「开相在位、落账清位」）——run 中途死亡（会话失败、verdict 解析失败、用户停止）时 `phase_log` 永不执行，`active_phase` 停在 `Some`，此后任何时刻查详情头部都渲染「运行中」，而实际没有 run 在跑。库在撒谎且永远撒下去。
2. **「不建 flow_runs 表」是写死的决策，不是实现遗漏**。`control.rs` 头注释明文：「run 生命周期由本注册表承载（不建 flow_runs 表）；run 终态即除名，桌面重启后 run 消失」。本变更等于翻掉这条决策，须作为决策翻案显式立项（spec 红线条款同步修订），而非当作补丁。

另有拼缝成本：图 = `buildFlowGraph(detail, runNodes)` 双源合成；终态迁移时 `useRunViewEffects` 触发一次显式 refresh 让图「回落 ChangeDetail 派生规则」（`run-state.ts` 头注释明说不持久化）；重挂恢复 `initialRunState` 的 steps 恒 `[]`——步史在重挂后同样丢失。

## 提案

运行中返回的状态一并落库，实时面与完成后面走同一条读路；读写架构推拉反转（运行时仅通知刷新，客户端自行请求）：

1. **决策翻案：runs + run_steps 两表**。workspace 库增 `RunRecord`（run_id / change / status / reason / 起止时刻）与 `RunStepRecord`（run_id / phase / attempt / step / status / seq / session_id / detail / 时间戳）两张 run 面模型；**全史保留**不截 last_run（运行史审计面），attempt 经 `phase_start` max+1 分配跨 run 不撞号，全史叠加在同一列面分层。
2. **每 run 两写，零每步写放大**：run 行 **start 时建**（status=running，支撑启动标定）、finish 时终态更新 + 步整包单事务落。写缝收在 walker 侧单点（`walk_run` 终态出口 `guard.finish`，walker.rs:108），`ChangeFlowControl` 保持进程内零 IO 不变量。崩溃丢该 run 过程史（start 行经启动标定可见 interrupted）为 A 骨架既定权衡。
3. **步史口径 = 有内容的步，单点过滤**：封闭集 10 变体留 5——agent 阶段（Executor / Evaluator / Decision）与脚本阶段（StaticCheck / TestExecution）落库；流程面步骤（PhaseStart / PhaseLog **及三门 VerdictGate / RetryGate / WhitelistGate**——本 proposal 确认三门归忽略集，纯 Rust 分支无会话无独立产物）不落。落户子集 = 回显子集，同一份过滤词汇单点定义；walker / control 零过滤逻辑。与既有 `StepRecord` 审计面职责分立（审计 vs 图节点史，词汇近互补）。
4. **启动标定 + active_phase 悬挂杀除**：启动发现库中 run 残留 running → 标 interrupted（重启后 run 客观已死，库不能继续说 running）；库中 run 状态词汇 = running + 三终态（completed / stopped / failed）+ interrupted（仅标定产生）。finish 落包同一事务处置 active_phase（清位或转「上次运行停在 X」静态记录，形态 design 定稿），杀掉悬挂；续走锚点语义不破（已 pass 相位不重头）。
5. **停等态不落库**：waitingAsk / waitingConfirm 仅存内存注册表——应答通道（oneshot）本就活在进程内，落了重启也无人接；但合并查询可见。
6. **出线：ChangeDetail 增 runs + steps**。golden 契约字段演进走显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕）；形态（null + ISO 串、纯 derive）冻结不动；时间戳 i64 毫秒落库、ISO 串出线既有口径不变。
7. **读路统一（推拉反转）**：合并查询落接口层——`get_change_detail`（或其扩展）一次返回「库读史 ∪ 内存在飞 run」统一视图，客户端不再双命令拼接。`RunUpdate` 降位为变更通知（载荷形态 design 定稿），客户端收通知自行重查；步累积器从前端 reducer 移驻服务端 `RunEntry`（注册表条目自持步列表，重挂快照含 steps）；前端 `applyRunUpdate` 状态机镜像 / liveEvents 累积 / seq 去重大面积解散，两钩并一（`useChangeFlowRun` 缩为控制动作 + 通知订阅）；抽屉实时转录面退役 liveEvents，口径统一到转录库（通知 → 重查转录，`use-session-transcript` 既有查询面）。stop / confirm / answer 控制面不变（请求-应答，非推送）。
8. **图常驻渲染**：完成后图不回落——步节点从库读史派生常驻，与实时面同一条派生路径（同一转换函数消费合并查询结果），「图回落 ChangeDetail 派生规则」语义退场；全史叠加（非仅最近 run），在飞 run 活步叠加其上，键 `phase:attempt:step` 天然不撞。
9. **版本交付**：`packages/desktop` 0.4.27 → 0.4.28；`plugins/dev-team` 零改动。

## 能力

### 新增能力

- 无（本变更修改五个既有能力）。

### 修改的能力

- **desktop-change-state-store** — 六模型（增 RunRecord / RunStepRecord）；run 运行史落库（每 run 两写、步词汇单点、与 StepRecord 审计职责分立）；启动标定 interrupted 与 active_phase 悬挂处置。
- **desktop-change-queries** — 详情聚合出线 runs + steps 全史（golden 显式重写、时间口径不变）；运行中标注与在飞 run 一致（悬挂假象退场）。
- **desktop-change-orchestration** — 翻案「不建 flow_runs 表」红线（节点状态派生源 = 库读史 ∪ 注册表在飞 run）；run 落库写缝与 RunEntry 累积器；运行控制命令面读路统一（RunUpdate 通知降位 + 合并查询）；版本交付 0.4.28。
- **desktop-change-flow-view** — 运行态节点收口后常驻渲染（回落语义退场、全史叠加）；取数模型改通知触发重查（增量并入条款退场、前端镜像解散、两钩并一）；转录联动实时面统一转录库（liveEvents 退役）。
- **desktop-corpus-regression** — 语料增 run 维度样本（多 run 全史 / interrupted / 步词汇与 sessionId 两态），golden 覆盖 runs/steps 投影。

### 引用沿用（零 delta）

- desktop-agent-chat-infra — 转录库流式落库与查询面（`use-session-transcript`）被实时面复用，机制零改动。
- desktop-change-worktree — 双 root 组合与库身份锚定不变（run 落库经 workspace root 库，worktree 为执行锚）。
- desktop-workspace-store — workspace 库 additive 打开与 `for_root` 身份锚定不变。
- desktop-ipc-type-bindings — DTO 演进经 bindings 再生成惯例（一致性守卫拦截漂移）。

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` — RunRecord / RunStepRecord 新模型（native_model 版本治理；workspace 维度）
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` — run 域操作面（run_start / run_finish 整包 / 启动标定 sweep / run 史查询）；步词汇过滤谓词单点（挂点 design 定稿）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/` — run 落库 port 缝扩展（walker / 命令层消费，core 零 infra 依赖不变）
- `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` — RunUpdate 通知降位形状；ChangeRunSnapshot 增 steps
- `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` — RunEntry 步累积器；头注释决策原文改写；control 零 IO 不变量保持
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` — finish 落包缝（终态出口单点：终态更新 + 步整包 + active_phase 处置同事务）
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` — ChangeDetail 增 runs + steps 出线（golden 显式重写）
- `packages/desktop/src-tauri/src/commands/changes/mod.rs` — get_change_detail 合并查询装配（库读史 ∪ 在飞 run）
- `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` — 通知桥瘦身；启动标定装配；`change_flow_state` 快照命令处置（design 定稿）
- `packages/desktop/src/views/changes/hooks/use-change-detail.ts` + `use-change-flow-run.ts` — 两钩并一（通知触发重取、控制动作保留）
- `packages/desktop/src/views/changes/flow/run-state.ts` — reducer 收缩（liveEvents / seq 去重 / 状态机镜像解散）
- `packages/desktop/src/views/changes/change-detail-view.tsx` — `useRunViewEffects` 终态 refresh 回落拆除；`buildFlowGraph` 拼缝统一
- `packages/desktop/src/views/changes/flow/graph.ts` 等 — runStepNodes 数据源统一（库读史 ∪ 实时流同一路径）
- `packages/desktop/src/types/generated/bindings.ts` — 随 DTO 演进再生成
- `packages/desktop/package.json` — `version` 0.4.27 → 0.4.28

### 测试文件

- `crates/infra/store/src/model_test.rs` / `store_test.rs` — 两表回环、步词汇过滤、标定 sweep、存量库 additive 打开
- `crates/core/orchestration/src/control_test.rs` / `walker_test.rs` — 每 run 两写时序（零每步写放大）、RunEntry 累积器、重挂快照含 steps、control 零 IO
- `crates/core/workflow/src/queries/detail_test.rs` — runs/steps 出线、悬挂杀除后运行中标注
- `crates/core/workflow/tests/fixtures/` + `corpus_golden_test.rs` — run 维度种子 + `DESKTOP_GOLDEN_REWRITE=1` 显式重写
- `src/commands/changes/mod_test.rs` / `change_flow/mod_test.rs` — 合并查询统一视图、通知触发
- `packages/desktop/src/views/changes/**.test.ts(x)` — 钩子并一、reducer 收缩、常驻渲染、转录重查（data-testid 挂钩）

### 删除文件

- 无整文件删除（`run-state.ts` 部分分支解散，文件收缩保留）。

### 不要修改

- `plugins/dev-team` 全部（版本 2.10.44、三类交付产物）
- 相位机写面语义（phase_next / phase_log / backtrack——仅 finish 缝新增 active_phase 处置）
- 会话转录落库机制与 StopRegistry（复用，零改动）
- stop / confirm / answer 控制命令语义（请求-应答不变）
- 既有 `StepRecord` 审计职责与词汇（职责分立，不做合并迁移）
- `openspec/specs/**` 既有基线 spec（本变更只写 `openspec/changes/<name>/specs/**` delta，归档时合并）

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 两表与回环 | run 走完多相位后重开库查询：RunRecord（run_id / status / reason / 起止）与 RunStepRecord（phase / attempt / step / status / seq / session_id / detail）逐字段与写入一致；存量库 additive 打开零迁移；native_model 版本治理在案 |
| AC-2 | 每 run 两写 | 假引擎驱动一相位 run：库写事务恰两次（start 建 running 行 / finish 终态更新 + 步整包单事务），无每步写事务；整包原子（任一环节失败整体回滚） |
| AC-3 | 步词汇口径一致 | 一次 run 覆盖全部 10 步词汇后查 run_steps：恰 5 落（executor / evaluator / decision / static_check / test_execution），PhaseStart / PhaseLog / 三门零行；过滤词汇单点定义，walker / control 源码零过滤逻辑 |
| AC-4 | 启动标定 | 构造库中残留 running 行（模拟中途死亡）后重启：该行标 interrupted，详情不再呈运行中；live run 写入路径永不产生 interrupted |
| AC-5 | 悬挂杀除与续走 | run 在相位中途被停止（phase_log 未达）后查详情：头部不呈「运行中 · phase · attempt」；重新发起 run 自续走锚点推进，已 pass 相位不重头执行 |
| AC-6 | 出线与 golden | ChangeDetail 出线 runs + steps 全史（多 run 全留）；时间戳 ISO 串 + null 口径不变、纯 derive；diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认留痕；bindings 再生成一致性守卫绿 |
| AC-7 | 合并查询统一视图 | run 运行中调用 get_change_detail（或其扩展）：一次返回库读史 ∪ 在飞 run（状态 / 停等与 ask 载荷 / RunEntry 步表）；前端无双命令拼接 |
| AC-8 | 通知降位与前端解散 | RunUpdate 以变更通知流出（无步/事件载荷或最小载荷）；前端 `applyRunUpdate` 状态机镜像、liveEvents 缓存、seq 去重删除（knip 无未用导出残留）；stop / confirm / answer 请求-应答行为不变 |
| AC-9 | 图常驻渲染 | run 收口后刷新 / 重开详情：步节点（executor / evaluator / decision / static-check / test-execution）常驻上图不回落，与运行中同一转换函数派生；两次 run 全史叠加，键 `phase:attempt:step` 无冲突 |
| AC-10 | 重挂恢复与转录统一 | run 运行中重挂视图：步表非空且与 emit 序一致（库面 seq 取 emit 序、finish 保序落）；抽屉实时转录经转录库重查呈现，与运行时一致；重放路径行为不变 |
| AC-11 | 语料与管线交付 | 语料种子含 run 维度样本（≥2 run 全史、interrupted、步词汇与 sessionId 两态），golden 覆盖 runs/steps 投影；`vp test` / `client:check` / knip 全绿；desktop version 0.4.28；`plugins/dev-team` 零改动 |

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 决策翻案涟漪：三处书面决策被翻（control.rs 头注释「不建 flow_runs 表」、orchestration spec 节点状态纯派生红线、flow-view spec「Channel 刷新 MUST NOT 触发全量重取」条款） | spec 自相矛盾、实现无所适从 | 高（确定发生） | 本变更 delta 对三处逐条显式修订（ADDED/MODIFIED 留痕），头注释同步改写；翻案理由锚定 explore 范围决议 |
| R2 通知风暴 → 高频重查放大：每步 + 每会话事件双通道通知，每次触发合并查询（含 runs/steps 全史聚合） | CPU / IO 放大、UI 抖动 | 高 | 客户端合并去抖或通知侧粗粒度 coalesce（design 定节流策略）；通知仅失效信号、查询结果权威 |
| R3 崩溃丢半程步史：kill 中途无 finish，步整包零落 | 该 run 过程史不可回放 | 中 | A 骨架既定权衡（用户拍板）：start 行经启动标定 interrupted 可见「何时何因中断」，attempt 级评估史仍在 PhaseRecord；边界留痕 |
| R4 双步表并存漂移：StepRecord（审计）与 RunStepRecord（图史）词汇重叠 3 项（static_check / test_execution / decision~decision_log） | 双写不一致、误用其一当另一 | 中 | 职责分立写入 spec（审计 vs 视图），各自单点；不做合并迁移（收益低风险高） |
| R5 golden 重写面扩大：runs/steps 全史入 detail 线面 | golden 体量膨胀、diff 噪声、人工确认成本 | 中 | 显式重写流程逐字执行；语料 run 维度样本克制（覆盖两态即可不贪多） |
| R6 前端状态机解散回归面大：applyRunUpdate / seedRunState / liveEvents / seq 去重 / useRunViewEffects 回落大面积删除 | 拼接 bug、实时面回归 | 中 | vitest 直测转换层 + data-testid 挂钩；AC-7/8/9/10 逐面断言 |
| R7 通知-重查竞态：通知到达时查询可能未含最新态（写后读滞后） | UI 少一拍更新 | 低 | 通知为失效信号、查询结果权威的语义定死；下一通知或显式刷新兜底；design 确认无需补拍机制 |
| R8 active_phase 处置与续走锚点耦合：清位后续走锚点改依 eval / run 史重算 | 续走 attempt 序错位、重头执行 | 低 | 处置形态（清位 vs 静态记录）与锚点关系 design 定稿；AC-5 断言续走不重头 |
| finish 落包失败的收敛语义：store 写失败时 run 已终态 | 库缺该 run 史、状态面与内存面短暂不一致 | 低 | 待决问题留 design（重试 / 标定兜底 / 显式错误面）；start 行已建保证 interrupted 可见 |

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 落库时机 | A 骨架 + 终态语义：run 行 start 建 running、finish 终态 + 步整包（每 run 两写） | 收齐在 walker / RunGuard 零成本；零每步写放大；start 行支撑启动标定 | B publish 写透每步随发随写（写放大、crate 边界改动大）；C 只并读面不落库（终态除名后仍不一致，基本不满足诉求） |
| 步史粒度 | 有内容的步：agent 阶段（Executor / Evaluator / Decision）+ 脚本阶段（StaticCheck / TestExecution），10 变体留 5 | 流程面步骤与三门无会话无独立产物，图史价值为零；attempt 级评估史已由 PhaseRecord 承载 | 全量 10 变体落（噪声 + 写放大）；仅 run 终态最小面（图完成后粒度断崖仍在） |
| 三门归属（探索推断，本 proposal 确认） | VerdictGate / RetryGate / WhitelistGate 归流程面忽略集 | 纯 Rust 分支的图上可辨节点，状态可由 verdict / PhaseRecord / 决策记录推导，无独立回放面 | 三门落库（五留变八，噪声） |
| 完成后渲染 | 常驻不回落：步节点从库派生，与实时面同一派生路径，全史叠加 | 用户拍板决议 2/3：「完成后图回落」语义退场；肉眼可见的一致才是诉求达成 | 回落为刻意产品语义（范围骤减，否决） |
| run 历史保留 | 全史：runs + steps 全量留存 | 运行史审计面（哪个 attempt 属于哪次 run）；attempt 跨 run 不撞号天然分层 | 只留 last_run（丢审计面） |
| 停等态 | waitingAsk / waitingConfirm 不落库，仅内存注册表（合并查询可见） | 应答通道（oneshot）活在进程内，落了重启无人接；决议 4 | 停等态落库（假象：重启后似可应答） |
| 库中 run 状态词汇 | running + completed / stopped / failed + interrupted（仅启动标定产生） | 与 ChangeRunStatus 六值解耦：停等两值不落；interrupted 表达「重启后客观已死」 | 直接复用六值词汇（含永不成立的库侧停等态） |
| 读路架构 | 推拉反转：RunUpdate 降位通知 + 接口层合并查询（库读史 ∪ 在飞 run）+ 前端查询缓存失效重取 | 用户拍板残余问题原则：口径一致、接口层合并、运行时仅通知；步过滤与拼接问题随之单点收口 | 维持推送载荷 + 前端镜像（两真相源根源仍在） |
| 写缝位置 | walker 侧单点（walk_run 终态出口），control 保持零 IO | 终态出口唯一（walker.rs:108 guard.finish）；control「进程内无 IO」不变量保持 | control 持 store 句柄（污染注册表）；命令层 ChangeFlowSink 写透（每步写放大） |
| 步过滤位 | 单点谓词、walker / control 零过滤（挂点 design 定稿） | 落户子集 = 回显子集，两处消费同一词汇，无两面 emit | walker emit 时分流（两面口径漂移根源） |
| run_steps 序 | seq 取 emit 序、finish 整包保序落 | 重挂恢复 live 面与库面拼接依赖稳定序（残余问题原则收口微问题 3） | 库内自增 id 按 finish 包序（与 emit 序脱钩） |
| 双步表关系 | StepRecord（审计）与 RunStepRecord（图史）职责分立并存 | 词汇近互补（审计面恰以流程面步为主）；合并迁移收益低风险高 | 收敛单表（迁移面大，否决） |
| 版本交付 | desktop 0.4.27 → 0.4.28；dev-team 零改动 | 既有惯例（package.json 单点、tauri.conf 跟随、Cargo.toml 不随动） | — |

### 待决问题（design 相位定稿，不阻塞本 proposal）

- active_phase 终态处置形态：清位 vs 转「上次运行停在 X」静态记录；与续走锚点（active_phase vs eval/run 史重算）的关系及语义等价性论证。
- 步过滤单点挂点：store 写面消费谓词 vs 读层过滤（两者均满足 walker / control 零过滤）。
- RunUpdate 降位后信封形状：零载荷 kind-only vs 最小载荷（run_id / 变更类别），及 bindings 演进面。
- `change_flow_state` 快照命令存留：合并查询承接重挂恢复后，独立快照命令退役还是缩位。
- run 行 start 写的落位：命令层装配（change_flow_start 已持 store 句柄）vs walker 起点直调写面。
- 高频通知节流策略：客户端合并去抖 vs 通知侧粗粒度 coalesce（阈值与窗口）。
- RunStepRecord 主键与索引形态（(run_id, seq) 复合 vs 自增 id + 索引）。
- finish 落包失败的收敛语义（重试 / 显式错误面 / 启动标定兜底）。
- 详情头部「上次运行」信息面呈现形态（若 active_phase 处置取静态记录）。
