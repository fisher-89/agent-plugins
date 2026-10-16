# desktop-change-orchestration Specification (Delta)

## MODIFIED Requirements

### Requirement: 薄图 walker 相位循环与路由红线

Desktop 后端 SHALL 提供编排运行时（walker，core/orchestration），对 change run 执行**计划图驱动的 step 循环**：每相位依序迭代其 `PhaseDefinition.steps`（`StepDefinition` 序列，kind = exec | agent | gate | git，定义见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」），不再在 `drive()` 内硬编码步序。每步执行后 SHALL 问写面 `phase_next` 决定推进 / 重试 / 决策（路由权威不变）；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 两个门禁布局常量 SHALL 退役（门禁步由 step 定义声明）。archive 相位 SHALL 为 walker 的第 10 个路由相位（`REQUIREMENT_TABLE` 第 9 相位，`PhaseDefinition.executor=None` / `evaluator=None`），其 11 步序列按序执行；drive 主循环 MUST NOT 保留 `next.evaluator.is_none()` 的冗余提前检查（真正消费 evaluator 的 `step_evaluator` / `step_executor` 自有 None 检查兜底）。

相位内 step 序列的特殊控制流 SHALL 以 step 级语义表达，MUST NOT 以 walker 内 step 类型特判实现：

- **feedback 边**（门禁步失败 → 同一 agent 会话修复 → 重跑）：失败步 `on_fail` 声明的定向反馈循环，独立计数 ≤5，超限升格相位 fail；每次重跑为 `instance_no` 递增的**新节点**，图上以 `feedback` 边表达。注入对象分角色：static-check / test-execution 失败注入本相位 executor 会话，verdict 解析失败注入本相位 evaluator 会话；
- **verdict 解析失败**：解析错误诊断反馈边续注同一 evaluator 会话修复重跑（MUST NOT 臆测 verdict、MUST NOT 静默跳过）；超限升格相位 fail 进相位重试 / 决策分叉；
- **归档步失败**：archive 相位步 `on_fail=None` 的失败即 `Terminal`（不唤决策、不进相位重试）；`Cleanup` 为唯一例外——尽力步恒以 `passed` 收口（失败仅 step detail 警告，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）；
- **phase 间 confirm 停等**：仍为 run 级 `auto_next_phase` 开关，并支持「按指定 phase 停止」（archive 为固定停止点）。

红线保持：walker MUST NOT 持有任何相位转移规则——每步过渡 SHALL 问写面 `phase_next`；`PIPELINE_PHASES` 与后端 `detail.rs` 相位列 SHALL 保持纯布局身份（10 列含 archive 与 code-analyze 占位列），MUST NOT 上位为路由权威。重启 run SHALL 自 active_phase 续走（active_phase 持久化于 db `ChangeRecord`，已 pass 相位 MUST NOT 重头执行，续走由 `phase_next` 依 eval 历史重算）。写面全部落库载体为 workspace 库，walker 对载体无感知。

#### Scenario: 相位内计划步全循环自动走完

- **WHEN** 以假引擎 + 假写面 + 假 vcs port 驱动某相位 run（含 archive 相位 11 步）
- **THEN** walker 依该相位 `PhaseDefinition.steps` 序逐 step 执行（exec / agent / gate / git 按声明），pass 后经写面 `phase_next` 推进下一相位，全程无人工干预

#### Scenario: archive 相位确定性承接

- **WHEN** walker 走到 archive 相位且 11 步全 pass
- **THEN** 相位以 Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize → PhaseLog 承接（零 executor / evaluator 会话），末尾 PhaseLog 空 checklist 落账 pass 后推进 done

#### Scenario: evaluator 检查下沉不破坏安全不变量

- **WHEN** 审查 drive 主循环与 `step_evaluator` / `step_executor`
- **THEN** drive 无 `next.evaluator.is_none()` 检查；声明 Evaluator / VerdictGate 步而 `evaluator=None` 的漂移仍被 `step_evaluator` 的 None 检查拦截；archive 相位（`evaluator=None` 且 steps 无 Evaluator 步）零拦截

#### Scenario: fail 重试与预算上限

- **WHEN** evaluator verdict 为 fail 且重试未超预算
- **THEN** walker 不唤决策 agent 自走重试同相位；连续 fail 达 5 次后停止自走并按决策协议分叉

#### Scenario: 中断续走不重头

- **WHEN** walker 于 implement#2 运行中被停止（或桌面重启）后重新发起该 change 的 run
- **THEN** walker 自 db `ChangeRecord.active_phase`=implement 的下一 attempt 续走，已 pass 的 proposal / dev-design 等相位不重新执行；archive 相位完成后 run completed

#### Scenario: test-execution 相位确定性承接

- **WHEN** walker 走到 test-execution 相位且 runner 结论 pass
- **THEN** 相位以 runner 门禁步承接（零 executor / evaluator 会话），机械 checklist 代写 `phase_log` 落账 pass 后推进；fail / error 走反馈边语义（见 desktop-test-execution）

### Requirement: 四类节点统一 walker 与可观测均一

Walker SHALL 将每相位循环表达为四类节点（step 定义声明 kind）：

- **WorkerAgent 节点**（executor / evaluator / decision / spec_sync / resolve_conflicts 的 agent 会话步）：经 compose_turn 开新会话，转录可观测、可停止；
- **ToolStep 节点**（phase-start / phase-log / backtrack 相位机步进程内直调写面 + static-check / test-execution 检查域 spawn 步落 checks 边界 + preflight / seal 内联校验与写面步）：无智能；
- **Git 节点**（commit / rebase / merge / cleanup / finalize vcs 步，落 vcs 边界 `crates/infra/vcs`）：经独立 vcs 缝 `Arc<dyn ArchiveVcsPort>` 进程 spawn 执行；`cleanup` 为尽力步——失败仅 step detail 警告、步恒 `passed`（不呈失败态，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）；
- **Gate 节点**（verdict 解析 / retry 计数 / 白名单校验）：纯 Rust 分支。

ToolStep / Git MUST NOT 以命令式内联实现：其执行结果 SHALL 作为图上节点状态可观测（失败 = 可见失败态，与 WorkerAgent 失败同等呈现）。节点状态 SHALL 为派生视图 = **库读史（`RunRecord` / `RunNodeRecord` / `RunEdgeRecord`）∪ 进程内注册表在飞 run（`RunEntry` 节点 + 边累积器）**；节点行 SHALL 直接携 `session_id`（会话转录经 session id 直查，不再按 provenance 反查会话集）；执行图节点 SHALL 以稳定身份键 `(run_id, phase, attempt, step, instance_no)` 寻址，边 SHALL 以 `RunEdgeRecord` 四值 `edge_kind ∈ { forward, feedback, retry, backtrack }` 显式落库。步词汇 SHALL 为全词汇封闭集（agent 阶段 + 脚本阶段 + 流程面步骤 + 三门 gate + 9 个归档步），落库与回显同一份词汇。已收口 run 的任一节点状态与边 SHALL 可由库读史单独重算得出。

#### Scenario: ToolStep / Git 失败同等可见

- **WHEN** static-check / test-execution spawn 步、vcs 步（commit / rebase / merge / finalize）或写面相位机步以非零退出 / 显式 Err 收场
- **THEN** 图上该节点呈失败态（与 executor 会话失败同等的可观测性），run 不静默越过该节点

#### Scenario: Cleanup 尽力步警告可见不呈失败

- **WHEN** archive 相位 Cleanup 步的 `git worktree remove` 非零退出
- **THEN** 该节点 `passed` 收口、`detail` 携警告（worktree 路径 + 手动清理指引）在图上可读；run 续走 Seal / Finalize / PhaseLog 正常收口，链不因清理失败停摆

#### Scenario: 节点与边两源派生可重算

- **WHEN** 审查编排运行时的持久化面与读路
- **THEN** runs / run_nodes / run_edges 三表在案、在飞 run 节点与边驻进程内注册表（RunEntry）；已收口 run 的任一节点状态与边均可由库读史重算得出，不依赖进程内存

#### Scenario: 归档步词汇上图与边四值可辨

- **WHEN** 一次 run 含 archive 相位 11 步（含 git 步与空 checklist 落账）
- **THEN** 归档 9 个步词节点全落 `RunNodeRecord`（四类节点类型可辨），串行 forward 边落边表；同 run 若另含 feedback / retry / backtrack，四 kind 边齐备且显式落库

### Requirement: 零 CLI 子进程写通道与 crate 依赖方向

Walker MUST NOT 经任何 CLI 子进程变更 change 流程状态（dev-team CLI 子命令面不存在）：所有 workflow 状态变更 SHALL 经 core/workflow 写面进程内直调（desktop-workflow-write-face 契约，落库 workspace 库），MUST NOT 读写 workflow.json。依赖方向 SHALL 为 orchestration → workflow；`crates/infra/devteam` MUST NOT 存在（整体删除）。进程 spawn 例外家族为检查域门禁（static-check / test-execution，落 `crates/infra/checks`）与 vcs 域归档 git 步（commit / rebase / merge / cleanup / finalize，落 `crates/infra/vcs`）——两者均经 port 缝下沉、spawn 不进 core（core/orchestration 自身零进程 spawn；检查域见 desktop-checks-domain，vcs 域见 desktop-crate-layout）。

#### Scenario: 零 CLI 写触点

- **WHEN** 审查编排运行时与命令组源码中对 change 流程状态的变更触点
- **THEN** 零 CLI 子进程调用、零 devteam 子进程依赖、零 workflow.json 读写；全部状态变更经进程内写面调用返回确认并落库

#### Scenario: spawn 边界保持（checks 与 vcs 双例外）

- **WHEN** 审查 core/orchestration 源码的进程创建调用
- **THEN** orchestration 模块内零进程 spawn；检查域 spawn（static-check / test-execution）经 port 缝由 `crates/infra/checks` 承载，git 步 spawn（含 cleanup 的 worktree 移除）经 `ArchiveVcsPort` 缝由 `crates/infra/vcs` 承载

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/walker.rs`（step_cleanup，新） | Cleanup 计划步执行 | 记录解析（id → 记录 → worktree 字段）；缺席 / legacy 幂等跳过；remove 失败吞为 detail 警告（步恒 `passed`、链必收口）；经 `ArchiveVcsPort` 缝调用 |
| `crates/core/orchestration/src/state.rs`（ChangeStepKind） | 步词面 | `Cleanup`（`as_str` = `cleanup`）入封闭集；与 `RunStepKind` 同词面 |
| `crates/core/orchestration/src/run_history.rs` | 词面映射 | `ChangeStepKind::Cleanup → RunStepKind::Cleanup` |
| `crates/core/orchestration/src/port.rs`（ArchiveVcsPort 清理面，扩） | vcs 缝扩展 | worktree 在场探测 + 移除（`git worktree remove [--force]`）；实现驻 `crates/infra/vcs`；`Err` 面带手动清理引导 |
