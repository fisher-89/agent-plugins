# desktop-change-orchestration Delta

## MODIFIED Requirements

### Requirement: 薄图 walker 相位循环与路由红线

Desktop 后端 SHALL 提供编排运行时（walker，core/orchestration），对 change run 执行硬编码的单图相位循环，每相位依序：

1. core/workflow 写面 `phase_next`（进程内）→ next_phase、已插值的 executor / evaluator prompt、`allowed_backtrack_phases` 白名单；
2. 写面 `phase_start`（开启阶段、attempt 计时）；
3. spawn executor agent（compose_turn 新会话）；
4. implement / test-gen 相位后执行 static-check（checks 边界 spawn 步）；test-execution 相位以确定性 runner 门禁步承接相位主体（checks 边界 spawn 步，绿跑零 agent，语义见 desktop-test-execution）；
5. spawn evaluator agent（新会话，不落账）；
6. 解析 verdict → 桌面代调写面 `phase_log`；
7. 写面 `phase_next` → pass 推进 / fail 重试（≤5，与插件 `MAX_RETRY_TIMES` 一致）/ 重试上限唤决策 agent 分叉。

红线：walker MUST NOT 持有任何相位转移规则——每步过渡 SHALL 问写面 `phase_next`（路由权威与 walker 同进程仍不自持规则）；`PIPELINE_PHASES` 与后端 `detail.rs` 相位列 SHALL 保持纯布局身份，MUST NOT 上位为路由权威。推进节奏 SHALL 为 phase 内自动、phase 间停等用户确认后继续（`auto_next_phase` 发起参数开启时 phase 间停等跳过，语义见「自动确认运行模式」）；重启 run SHALL 自 `active_phase` 续走（已 pass 相位 MUST NOT 重头执行）。

#### Scenario: 相位内全循环自动走完

- **WHEN** 以假引擎（预录 AgentEvent 序列）+ 假写面（fake port，预录 phase-next / phase-start / phase-log 响应）驱动某相位 run
- **THEN** walker 依序执行 ①→⑦：executor 会话发起、evaluator 会话发起、`phase_log` 以解析 verdict 代调，pass 后经写面 `phase_next` 推进下一相位，全程无人工干预

#### Scenario: test-execution 相位确定性承接

- **WHEN** walker 走到 test-execution 相位且 runner 结论 pass
- **THEN** 相位以 runner 门禁步承接（零 executor / evaluator 会话），机械 checklist 代写 `phase_log` 落账 pass 后推进；fail / error 走反馈边语义（见 desktop-test-execution）

#### Scenario: fail 重试与预算上限

- **WHEN** evaluator verdict 为 fail 且重试未超预算
- **THEN** walker 不唤决策 agent 自走重试同相位；连续 fail 达 5 次后停止自走并按决策协议分叉

#### Scenario: 中断续走不重头

- **WHEN** walker 于 implement#2 运行中被停止（或桌面重启）后重新发起该 change 的 run
- **THEN** walker 自 active_phase=implement 的下一 attempt 续走，已 pass 的 proposal / dev-design 等相位不重新执行；中断相位经进程内锚点（sessionAnchors 复活）可感知并被标定

### Requirement: 三类节点统一 walker 与可观测均一

Walker SHALL 将每相位循环表达为三类节点：

- **WorkerAgent 节点**（executor / evaluator / decision）：经 compose_turn 开新会话，转录可观测、可停止；
- **ToolStep 节点**（phase-start / phase-log / backtrack 相位机步进程内直调写面 + static-check / test-execution 检查域 spawn 步落 checks 边界）：无智能；
- **Gate 节点**（verdict 解析 / retry 计数 / 白名单校验）：纯 Rust 分支。

ToolStep MUST NOT 以命令式内联实现：其执行结果 SHALL 作为图上节点状态可观测（失败 = 可见失败态，与 WorkerAgent 失败同等呈现）。节点状态 SHALL 为派生视图 = workflow.json 状态（active_phase / eval）× 按 provenance 反查的 session 集；MUST NOT 为节点状态建立持久表（不建 flow_runs 表、workflow.json schema 不动）。

#### Scenario: ToolStep 失败同等可见

- **WHEN** static-check 或 test-execution spawn 步以非零退出 / 显式 Err 收场（或写面相位机步返回错误）
- **THEN** 图上该节点呈失败态（与 executor 会话失败同等的可观测性），run 不静默越过该节点

#### Scenario: 节点状态纯派生

- **WHEN** 审查编排运行时的持久化面
- **THEN** 无 flow_runs 表、无 workflow.json 之外的 run 私有状态文件；任一节点状态均可由 workflow.json + 会话记录（按 provenance）重算得出

### Requirement: 零 CLI 子进程写通道与 crate 依赖方向

Walker MUST NOT 经任何 CLI 子进程写 workflow.json（dev-team CLI 子命令面不存在）：所有 workflow 状态变更 SHALL 经 core/workflow 写面进程内直调（desktop-workflow-write-face 契约）。依赖方向 SHALL 为 orchestration → workflow；`crates/infra/devteam` MUST NOT 存在（整体删除）。检查域门禁（static-check 与 test-execution）的进程 spawn 为编排域的 spawn 例外家族且 MUST 落 checks 边界 infra 层 `crates/infra/checks`（spawn 不进 core——core/orchestration 自身零进程 spawn，经 port 缝下沉；边界定义见 desktop-checks-domain）。

#### Scenario: 零 CLI 写触点

- **WHEN** 审查编排运行时与命令组源码中对 workflow.json 的写触点
- **THEN** 零 CLI 子进程调用、零 devteam 子进程依赖；全部状态变更经进程内写面调用返回确认

#### Scenario: spawn 边界保持

- **WHEN** 审查 core/orchestration 源码的进程创建调用
- **THEN** orchestration 模块内零进程 spawn；检查域 spawn（static-check / test-execution）经 port 缝由 checks 边界 `crates/infra/checks` 实现承载

### Requirement: static-check spawn 步门禁

implement / test-gen 相位的 executor 收口后，walker SHALL 必经 static-check 步（checks 边界 `crates/infra/checks` spawn 执行，承接原 SubagentStop hook 的门禁职责）。static-check 失败 SHALL 走**定向反馈边**：将捕获的诊断反馈注入同一 executor 会话（Continue）修复，独立计数上限 5 次（沿原 hook `loop_limit` 语义）；超限 SHALL 升格为相位 fail（walker 以桌面代写的 fail checklist 经写面 `phase_log` 落账，不跑 evaluator，进入重试 / 决策路径并消耗相位 retry 预算）。MUST NOT 将反馈边重试直接计入相位 retry 预算。非 implement / test-gen 相位 MUST NOT 触发 static-check。

#### Scenario: 修复反馈边

- **WHEN** executor 收口后 static-check 报 lint / fmt 错误
- **THEN** walker 将诊断文本注入同一 executor 会话（Continue）重试修复，反馈边计数 +1；修复后 static-check 通过则相位继续 evaluator 步

#### Scenario: 反馈边超限升格

- **WHEN** 同一相位内定向反馈边已达 5 次仍不通过
- **THEN** walker 以代写 fail checklist 经写面 `phase_log` 落账（不跑 evaluator），相位按 fail 收场进入重试 / 决策路径，walker 不再注入反馈边

#### Scenario: 门禁相位限定

- **WHEN** proposal / dev-design 等非 implement / test-gen 相位 executor 收口
- **THEN** walker 不发起 static-check spawn，直接进入 evaluator 步

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/port.rs` | 「相位机 + 工具步」进程内缝（封闭集扩展） | `ToolCommand` 增 `TestExecution { change, … }` 变体；`ToolStepOutput` 增 `TestExecution(最小载荷)`；`TestExecutionRunner` port 与 `StaticCheckRunner` 并列（spawn 不进 core）；实现落 checks 边界 `crates/infra/checks`，core/orchestration 零依赖 checks |
| `crates/core/orchestration/src/walker.rs` | 检查域门禁步接入 | static-check 反馈边沿用（`STATIC_CHECK_FEEDBACK_LIMIT` 不变）；test-execution 相位 runner 门禁步 + 独立反馈预算（计数器分立、上限 5、超限升格相位 fail） |
| `crates/infra/agent/src/worker.rs` | WorkerAgentPort 实现（收窄） | compose_turn 新会话 + StopRegistry 终止 + 密封转录 + provenance `source="change"`；static-check spawn 缝移出（落 `crates/infra/checks`，见 desktop-checks-domain） |
