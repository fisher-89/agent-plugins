# desktop-change-orchestration Specification

## Purpose

Desktop 后端 change run 编排运行时（薄图 walker，core/orchestration）：对 PGE 工作流执行硬编码单图相位循环，三类节点（WorkerAgent / ToolStep / Gate）统一可观测；路由权威驻 core/workflow 写面（walker 零自持转移规则）；提供运行控制命令面（发起 / 停止 / 应答 / 确认）与决策协议。

## Requirements

### Requirement: 薄图 walker 相位循环与路由红线

Desktop 后端 SHALL 提供编排运行时（walker，core/orchestration），对 change run 执行**计划图驱动的 step 循环**：每相位依序迭代其 `PhaseDefinition.steps`（`StepDefinition` 序列，kind = exec | agent | gate | git，定义见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」），不再在 `drive()` 内硬编码步序。每步执行后 SHALL 问写面 `phase_next` 决定推进 / 重试 / 决策（路由权威不变）；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 两个门禁布局常量 SHALL 退役（门禁步由 step 定义声明）。archive 相位 SHALL 为 walker 的第 10 个路由相位（`REQUIREMENT_TABLE` 第 9 相位，`PhaseDefinition.executor=None` / `evaluator=None`），其 10 步序列按序执行；drive 主循环 MUST NOT 保留 `next.evaluator.is_none()` 的冗余提前检查（真正消费 evaluator 的 `step_evaluator` / `step_executor` 自有 None 检查兜底）。

相位内 step 序列的特殊控制流 SHALL 以 step 级语义表达，MUST NOT 以 walker 内 step 类型特判实现：

- **feedback 边**（门禁步失败 → 同一 agent 会话修复 → 重跑）：失败步 `on_fail` 声明的定向反馈循环，独立计数 ≤5，超限升格相位 fail；每次重跑为 `instance_no` 递增的**新节点**，图上以 `feedback` 边表达。注入对象分角色：static-check / test-execution 失败注入本相位 executor 会话，verdict 解析失败注入本相位 evaluator 会话；
- **verdict 解析失败**：解析错误诊断反馈边续注同一 evaluator 会话修复重跑（MUST NOT 臆测 verdict、MUST NOT 静默跳过）；超限升格相位 fail 进相位重试 / 决策分叉；
- **归档步失败**：archive 相位步 `on_fail=None` 的失败即 `Terminal`（不唤决策、不进相位重试）；
- **phase 间 confirm 停等**：仍为 run 级 `auto_next_phase` 开关，并支持「按指定 phase 停止」（archive 为固定停止点）。

红线保持：walker MUST NOT 持有任何相位转移规则——每步过渡 SHALL 问写面 `phase_next`；`PIPELINE_PHASES` 与后端 `detail.rs` 相位列 SHALL 保持纯布局身份（10 列含 archive 与 code-analyze 占位列），MUST NOT 上位为路由权威。重启 run SHALL 自 active_phase 续走（active_phase 持久化于 db `ChangeRecord`，已 pass 相位 MUST NOT 重头执行，续走由 `phase_next` 依 eval 历史重算）。写面全部落库载体为 workspace 库，walker 对载体无感知。

#### Scenario: 相位内计划步全循环自动走完

- **WHEN** 以假引擎 + 假写面 + 假 vcs port 驱动某相位 run（含 archive 相位 10 步）
- **THEN** walker 依该相位 `PhaseDefinition.steps` 序逐 step 执行（exec / agent / gate / git 按声明），pass 后经写面 `phase_next` 推进下一相位，全程无人工干预

#### Scenario: archive 相位确定性承接

- **WHEN** walker 走到 archive 相位且 10 步全 pass
- **THEN** 相位以 Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Seal → Finalize → PhaseLog 承接（零 executor / evaluator 会话），末尾 PhaseLog 空 checklist 落账 pass 后推进 done

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

### Requirement: 自动确认运行模式（auto_next_phase）

Desktop run SHALL 支持「自动确认」运行模式：`RunRequest.auto_next_phase`（bool）为 run 级属性、发起时定格，经运行控制命令面发起入口透传。`auto_next_phase=true` 时，walker SHALL 在 phase 间确认点跳过 `ConfirmWait` 广播与 `wait_confirm` 挂起，直接推进下一相位；auto 模式 run MUST NOT 进入 `WaitingConfirm` 状态、MUST NOT 产生 `ConfirmWait` 事件。`auto_next_phase=false`（默认）时既有停等语义不变。

`auto_next_phase` SHALL 泛化出「按指定 phase 停止」能力：run SHALL 支持在指定相位前停止（stop-before-phase，形态由 design 定稿——`RunRequest.stop_before: Option<String>` 或等价策略），archive 为该能力的固定停止点。无论 auto / 手动模式，run 走完 8 个路由相位后 SHALL 停在 archive 前（archive 为独立手动执行门）：MUST NOT 自动推进进入 archive 相位；用户显式确认后才进入 archive 相位。

语义边界：

- **ask 中断不自动应答**：auto 模式下决策 agent 的 ask 到来 SHALL 照常进入 `WaitingAsk` 停等用户应答；自动确认只作用于 phase 间停等。
- **归档前停等**：全 8 路由相位 pass 后 SHALL 停在 archive 前（"Ready for archiving" 改写为「等待确认进入归档」），MUST NOT 自动归档；archive 相位完成后 run 才 `completed`。

运行中停等节奏 MUST NOT 可切换：节奏变更 SHALL 经停止后重发（自 `active_phase` 续走）；auto 模式下用户干预的合法动作与既有运行面一致（停止）。

#### Scenario: auto run 直达 archive 前停等

- **WHEN** 以 auto_next_phase=true 发起多相位 run（各相位 evaluator 全 pass）
- **THEN** 更新流逐相位自动推进（零 `ConfirmWait`），至 archive 前停等（stop-before-archive），未确认不进 archive 相位、未 completed；用户确认后 archive 相位执行、run completed

#### Scenario: 手动模式语义不变

- **WHEN** 以 auto_next_phase=false（默认）发起同一 run
- **THEN** `ConfirmWait` 逐相位流出、挂起至 `change_flow_confirm` 应答，proceed=false / 等待期间取消 → 受控 `stopped`（与既有契约逐字一致）

#### Scenario: auto run 的 ask 仍停等人

- **WHEN** auto 模式 run 中决策 agent 输出 ask 动作（重试预算耗尽后的提问）
- **THEN** run 照常进入 `WaitingAsk` 并呈现问题与选项，walker 挂起等应答；MUST NOT 自动选择任何选项，用户应答后 run 继续

#### Scenario: 发起定格与停止重发

- **WHEN** 运行中的 run 需要变更停等节奏
- **THEN** 唯一路径为停止当前 run 后以目标节奏重新发起（自 `active_phase` 续走，已 pass 相位不重头执行）；运行中不存在节奏切换命令或事件

### Requirement: 四类节点统一 walker 与可观测均一

Walker SHALL 将每相位循环表达为四类节点（step 定义声明 kind）：

- **WorkerAgent 节点**（executor / evaluator / decision / spec_sync / resolve_conflicts 的 agent 会话步）：经 compose_turn 开新会话，转录可观测、可停止；
- **ToolStep 节点**（phase-start / phase-log / backtrack 相位机步进程内直调写面 + static-check / test-execution 检查域 spawn 步落 checks 边界 + preflight / seal 内联校验与写面步）：无智能；
- **Git 节点**（commit / rebase / merge / finalize vcs 步，落 vcs 边界 `crates/infra/vcs`）：经独立 vcs 缝 `Arc<dyn ArchiveVcsPort>` 进程 spawn 执行；
- **Gate 节点**（verdict 解析 / retry 计数 / 白名单校验）：纯 Rust 分支。

ToolStep / Git MUST NOT 以命令式内联实现：其执行结果 SHALL 作为图上节点状态可观测（失败 = 可见失败态，与 WorkerAgent 失败同等呈现）。节点状态 SHALL 为派生视图 = **库读史（`RunRecord` / `RunNodeRecord` / `RunEdgeRecord`）∪ 进程内注册表在飞 run（`RunEntry` 节点 + 边累积器）**；节点行 SHALL 直接携 `session_id`（会话转录经 session id 直查，不再按 provenance 反查会话集）；执行图节点 SHALL 以稳定身份键 `(run_id, phase, attempt, step, instance_no)` 寻址，边 SHALL 以 `RunEdgeRecord` 四值 `edge_kind ∈ { forward, feedback, retry, backtrack }` 显式落库。步词汇 SHALL 为全词汇封闭集（agent 阶段 + 脚本阶段 + 流程面步骤 + 三门 gate + 8 个归档步），落库与回显同一份词汇。已收口 run 的任一节点状态与边 SHALL 可由库读史单独重算得出。

#### Scenario: ToolStep / Git 失败同等可见

- **WHEN** static-check / test-execution spawn 步、vcs 步（commit / rebase / merge / finalize）或写面相位机步以非零退出 / 显式 Err 收场
- **THEN** 图上该节点呈失败态（与 executor 会话失败同等的可观测性），run 不静默越过该节点

#### Scenario: 节点与边两源派生可重算

- **WHEN** 审查编排运行时的持久化面与读路
- **THEN** runs / run_nodes / run_edges 三表在案、在飞 run 节点与边驻进程内注册表（RunEntry）；已收口 run 的任一节点状态与边均可由库读史重算得出，不依赖进程内存

#### Scenario: 归档步词汇上图与边四值可辨

- **WHEN** 一次 run 含 archive 相位 10 步（含 git 步与空 checklist 落账）
- **THEN** 归档 8 个新步词节点全落 `RunNodeRecord`（四类节点类型可辨），串行 forward 边落边表；同 run 若另含 feedback / retry / backtrack，四 kind 边齐备且显式落库

### Requirement: 零 CLI 子进程写通道与 crate 依赖方向

Walker MUST NOT 经任何 CLI 子进程变更 change 流程状态（dev-team CLI 子命令面不存在）：所有 workflow 状态变更 SHALL 经 core/workflow 写面进程内直调（desktop-workflow-write-face 契约，落库 workspace 库），MUST NOT 读写 workflow.json。依赖方向 SHALL 为 orchestration → workflow；`crates/infra/devteam` MUST NOT 存在（整体删除）。进程 spawn 例外家族为检查域门禁（static-check / test-execution，落 `crates/infra/checks`）与 vcs 域归档 git 步（commit / rebase / merge / finalize，落 `crates/infra/vcs`）——两者均经 port 缝下沉、spawn 不进 core（core/orchestration 自身零进程 spawn；检查域见 desktop-checks-domain，vcs 域见 desktop-crate-layout）。

#### Scenario: 零 CLI 写触点

- **WHEN** 审查编排运行时与命令组源码中对 change 流程状态的变更触点
- **THEN** 零 CLI 子进程调用、零 devteam 子进程依赖、零 workflow.json 读写；全部状态变更经进程内写面调用返回确认并落库

#### Scenario: spawn 边界保持（checks 与 vcs 双例外）

- **WHEN** 审查 core/orchestration 源码的进程创建调用
- **THEN** orchestration 模块内零进程 spawn；检查域 spawn（static-check / test-execution）经 port 缝由 `crates/infra/checks` 承载，git 步 spawn 经 `ArchiveVcsPort` 缝由 `crates/infra/vcs` 承载

### Requirement: 会话挂靠 provenance 与转录 session id 直查

Walker 发起的 executor / evaluator / decision / spec_sync / resolve_conflicts 会话 SHALL 携带 provenance：`source="change"`、`source_ref=<id>/<phase>/<role>/<attempt>`（身份段为 change id——单一身份锚贯穿，name 可变不入寻址键；归档步两会话的 source_ref 定式由 design 定稿，MUST 在同 phase 同 attempt 内唯一，避免 spec_sync 与 resolve_conflicts 撞号）。该 `source_ref` SHALL 由 walker 在会话创建时写入并随会话记录持久化（`SessionProvenance` 字段面保持），作为来源归属留痕。节点运行态与转录寻址 SHALL 经 run 节点行的 `session_id` 直查（run 节点行携该步会话 id，见 desktop-change-state-store「run 执行图落库」；前端不再按 provenance 反查会话集）。会话转录 SHALL 可观测：运行中实时流、结束后重放一致。会话停止 SHALL 复用 StopRegistry 既有终止面，对已终态目标幂等忽略。

#### Scenario: provenance 留痕与 session id 直查

- **WHEN** 某 change 的 implement#2 相位已发起 executor 与 evaluator 会话（或 archive 相位已发起 spec_sync 会话）
- **THEN** 两会话记录各自 `SessionProvenance.source="change"` 且 `source_ref="<id>/<phase>/<role>/<attempt>"` 逐字在案（归档步两会话 source_ref 同 phase 同 attempt 内唯一）；节点转录经其 run 节点行 `session_id` 直查（无需 `agentSessions` 按 source / sourceRef 反查）

#### Scenario: 转录可观测与停止

- **WHEN** executor 会话运行中用户请求停止
- **THEN** 该会话经 StopRegistry 收敛为 `stopped`，walker 收到终止终态并按停止收敛处理 run；对已终态会话的重复停止请求幂等忽略

### Requirement: 变更文件上下文降级 git diff

Desktop run 的 executor / evaluator prompt 组装 SHALL 以 git diff（工作区变更面）提供变更文件上下文（承接原 file_log「让 agent 识别变更文件」的既有用途）。file_log 概念 SHALL 随双向墙整体退役：desktop 全链 MUST NOT 读写任何 `file_log`（不提取转录写路径、不调 `change-files`、不展示 file_log 条目）；存量 skill 路径 workflow.json 内的 file_log 数据随其文件留档不动，desktop 不再解析与展示。

#### Scenario: desktop 全链零 file_log 触点

- **WHEN** 扫描 desktop 源码（core / infra / commands / views）对 file_log 的读与写触点
- **THEN** 零触点；既有 skill 路径产生的 file_log 数据不被触碰（workflow.json 原样留档）

### Requirement: static-check spawn 步门禁

implement / test-gen 相位的 executor 收口后，walker SHALL 必经 static-check 步（checks 边界 `crates/infra/checks` spawn 执行，承接原 SubagentStop hook 的门禁职责；该步由相位 step 定义声明，非 `STATIC_CHECK_PHASES` 常量注入）。static-check 步 SHALL 经 id → 记录 → name 解析（`store.get_change(id).map(name)`，与 test-execution 分支同式）把 change name 传入 `StaticCheckRunner::run(root, name)`——name 供 infra 侧 output 落盘（recorded，`output_ref` 回填，`<report_dir>/exec-outputs/<exec_id>.log`）；未建档（无 `ChangeRecord`）显式 `Err`。static-check 失败 SHALL 走**定向反馈边**：将捕获的诊断反馈注入同一 executor 会话（Continue）修复，独立计数上限 5 次（沿原 hook `loop_limit` 语义）；每次修复重跑为 `instance_no` 递增的新节点、图上以 `feedback` 边表达。超限 SHALL 升格为相位 fail（walker 以桌面代写的 fail checklist 经写面 `phase_log` 落账，不跑 evaluator，进入重试 / 决策路径并消耗相位 retry 预算）。MUST NOT 将反馈边重试直接计入相位 retry 预算。非 implement / test-gen 相位 MUST NOT 触发 static-check（其 step 序列不含 static-check 步）。

#### Scenario: 修复反馈边

- **WHEN** executor 收口后 static-check 报 lint / fmt 错误
- **THEN** walker 将诊断文本注入同一 executor 会话（Continue）重试修复，反馈边计数 +1，新修复节点 `instance_no` 递增；修复后 static-check 通过则相位继续 evaluator 步

#### Scenario: 反馈边超限升格

- **WHEN** 同一相位内定向反馈边已达 5 次仍不通过
- **THEN** walker 以代写 fail checklist 经写面 `phase_log` 落账（不跑 evaluator），相位按 fail 收场进入重试 / 决策路径，walker 不再注入反馈边

#### Scenario: 门禁相位限定

- **WHEN** proposal / dev-design 等非 implement / test-gen 相位 executor 收口
- **THEN** walker 不发起 static-check spawn（该相位 step 序列不含 static-check 步），直接进入 evaluator 步

#### Scenario: static-check 步 name 解析与 recorded

- **WHEN** walker 走到某已建档 change 的 static-check 步（假 store 供给 name）
- **THEN** `StaticCheckRunner::run(root, name)` 收到该 change 的裸名，`StaticCheckOutcome.output_ref` 回填（passed 与 failed 两分支均携 `output_ref`），节点可经 stdout 面板回查；change 未建档则显式 `Err`

### Requirement: verdict 解析与 phase-log 代写

Evaluator 会话 SHALL 依 prompt 约定在最终消息输出 checklist JSON（不自调 MCP `phase_log`）。Walker SHALL 解析该 JSON 并代调写面 `phase_log`（进程内）落 eval 记录（PhaseRecord + ChecklistItemRecord 原子落库）；解析失败 SHALL 走反馈边（解析错误诊断续注同一 evaluator 会话修复重跑，独立计数 ≤5），MUST NOT 臆测 verdict、MUST NOT 静默跳过落账；超限 SHALL 升格相位 fail（桌面代写 fail phase-log）进相位重试 / 决策分叉。checklist JSON 结构 SHALL 瘦身为 `{ verdict, report, checklist }`（checklist 逐项 `{ item, pass, evidence }`）；phase / attempt / skipped SHALL 由 walker 盖戳落账（evaluator 不回声），`EvaluatorChecklist` 解析类型 MUST NOT 要求 phase / attempt / skipped 字段。确定性相位（test-execution 机械 checklist、archive 空 checklist）SHALL 走 `step_phase_log` 的独立分支，不经 evaluator verdict 解析。

#### Scenario: verdict 代写落账

- **WHEN** evaluator 最终消息输出合法 checklist JSON（verdict=pass，无 phase / attempt / skipped 字段）
- **THEN** walker 以该 checklist 盖 phase / attempt / skipped 后经写面 `phase_log` 落 eval 记录，`phase_next` 随后推进

#### Scenario: 解析失败反馈边修复

- **WHEN** evaluator 最终消息不含可解析的 checklist JSON（结构漂移或空输出）
- **THEN** 门步节点呈失败态，解析失败原因注入同一 evaluator 会话（Continue，`feedback` 边）修复后重解析（`instance_no` 递增新节点）；当轮修复期间状态库无新增 eval 记录

#### Scenario: 解析失败超限升格

- **WHEN** 同 attempt 内解析失败经 5 次反馈修复仍未收敛
- **THEN** walker 代写 fail phase-log（report 携升格记因、fail checklist 携解析错误、executor + evaluator 双槽位）升格相位 fail，回写面 `phase_next` 进相位重试 / 决策分叉

### Requirement: 决策协议

写面 `phase_next` 返回重试上限（fail 且需跨相位跳转）时，walker SHALL 唤起决策 agent（compose_turn 新会话）；retry 预算内 SHALL NOT 唤起（walker 自走重试）。决策 agent 输入 SHALL 有界且来自 db 状态与写面响应：fail checklist（fail 项 + evidence）、`allowed_backtrack_phases`（写面 `phase_next` 白名单）、候选相位最近一次 eval report。决策输出 SHALL 为封闭集：

- `{ action: "backtrack", backtrack_to ∈ 白名单, reason ≤500 }` — 自治，walker 经写面 `backtrack` 执行；
- `{ action: "retry" }` — 同相位重试；
- `{ action: "stop", reason }` — 终止 run；
- `{ action: "ask", question, options[] }` — 无法裁决，UI 中断提问。

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验（工具侧兜底）：越权目标 SHALL 被拒绝且 run 停给用户，MUST NOT 落任何越权回跳账（状态库零变更）。决策输出解析失败显式失败停给用户（决策会话不设反馈边，区别于 verdict 解析失败）。

#### Scenario: 白名单内自治回跳

- **WHEN** test-execution#1 fail 达重试上限且决策 agent 输出 `{action:"backtrack", backtrack_to:"test-gen", reason:...}`（test-gen 在白名单内）
- **THEN** walker 经写面 `backtrack` 落回跳记录（落库），`phase_next` 随后路由至 test-gen 重做，reason 随边标签可读

#### Scenario: 越权决议被兜底拦截

- **WHEN** 决策 agent 输出的 backtrack_to 不在 allowed_backtrack_phases 内
- **THEN** 写面 `backtrack` 二次校验拒绝（状态库零变更），run 停给用户并呈现越权详情

#### Scenario: ask 中断提问

- **WHEN** 决策 agent 输出 `{action:"ask", question:..., options:[...]}`
- **THEN** run 进入中断等待态，UI 呈现问题与选项；用户应答后 run 按应答继续（应答经运行控制命令面回流 walker）

### Requirement: run 双 root 组合与 exec root 解析

change run SHALL 以双 root 组合执行（能力语义见 desktop-change-worktree）：`change_flow_start` SHALL 解析 exec root——db 记录带 `worktree` → 该 worktree 绝对路径；记录 `worktree=None`（legacy）→ 主 workspace root——`RunRequest.root` SHALL 恒为 exec root，会话 cwd（`turn.root → SessionCtx.workspace_root`）、工件根（`Layout::resolve`）、git diff 与检查器（static-check / test-execution）执行目录 SHALL 全部随 exec root 落位。store / db 半边 SHALL 恒以 workspace root 解析：`compose_turn` 的 store 半边为命令层预解析注入的 workspace root 库实例，worktree 路径 MUST NOT 进入 `for_root`；`StoreSnapshot` 的 fs 半边（产物发现 / detail 装配）取 exec root、db 半边取注入的 store 实例（既有构造形态天然满足）。相位状态机零 fs、会话 resume 链（记录不存 cwd）MUST 保持零改动。SDK 路径沙箱前缀校验以 exec root 为锚（既有机制零改动），worktree change 的文件编辑囚于 worktree 内；worktree 内 `git diff HEAD` 恰为本 change 编辑集（「变更文件上下文降级 git diff」requirement 的 diff 面语义在 worktree 内自然收窄，无模板改动）。

#### Scenario: worktree run 全链落 worktree

- **WHEN** 以带 worktree 记录的 change 发起 run（假引擎驱动一相位）
- **THEN** executor / evaluator 会话 cwd = worktree、static-check 与 test-execution spawn 的执行目录 = worktree、git diff 上下文取自 worktree 且仅含本 change 编辑集

#### Scenario: store 身份恒 workspace root

- **WHEN** 审查 worktree change run 的组合根装配与 compose_turn 调用
- **THEN** store 实例由命令层以 workspace root `for_root` 解析后注入（provider / 会话 / 建档 / StepRecord 落 workspace 库），全程无以 worktree 路径进 `for_root` 的调用点

#### Scenario: legacy run 零变化

- **WHEN** 以 `worktree=None` 的存量建档 change 发起 run
- **THEN** exec root 解析为主 workspace root，既有 run 全链（cwd / Layout / diff / 检查器 / 快照）语义零变化

#### Scenario: 并行 change 各自 worktree

- **WHEN** 同 workspace 两个带各自 worktree 的 change 同时发起 run（假引擎）
- **THEN** 两 run 互不干扰（各自 exec root 的编辑 / diff / 检查隔离），db 写入经既有进程内并发支撑

### Requirement: 运行控制命令面

命令面 SHALL 提供 change run 的控制入口，命令体收拢为编排运行时公共 API 的薄包装（沿三件事纪律）：

- **发起**：按 change id 发起 run（id → 记录解析 worktree / name），`auto_next_phase` 参数（bool；false=默认停等节奏，true=自动确认模式）与「按指定 phase 停止」参数（形态 design 定稿；archive 为固定停止点）随发起定格并透传 walker；发起 SHALL 先解析 exec root；沿 agent 执行先例提前 resolve（run 记录进入运行态后即返回），RunRecord 起始行（status=running）SHALL 随发起建立；
- **停止**：终止当前 WorkerAgent 会话并收敛 run 为受控终态（archive 相位进行中的停止复用本命令）；对非运行态目标幂等忽略；
- **应答**：ask 中断态下回传用户应答；
- **确认**：phase 间停等点（含 archive 前停等点）的用户确认（继续 / 终止）。

**读路统一（推拉反转）**：`get_change_detail`（或其扩展）SHALL 一次返回「库读史 ∪ 内存在飞 run」统一视图——库面 runs / steps 读史并入在飞 run 的状态 / 停等与 ask 载荷 / RunEntry 步表，合并发生在读时；客户端 MUST NOT 双命令拼接（`useChangeDetail` + `useChangeFlowRun` 双真相源拼缝退场；归档链 `useArchiveFlow` 第三真相源随 archive_flow 命令面退役）。`RunUpdate` SHALL 降位为变更通知：客户端收通知后自行重查统一视图。stop / confirm / answer 控制面 SHALL 维持请求-应答形态不变。

运行状态 Channel SHALL 与 agent 执行流同构。发起前置校验 SHALL 覆盖：目标 change 已建档、无同 workspace 同 change 并行 run——并行冲突键 SHALL 为 `(workspace root, change id)` 复合——无建档的 change SHALL 显式拒绝发起。

#### Scenario: 发起提前 resolve 与通知流

- **WHEN** 前端发起某 change 的 run
- **THEN** invoke 在 run 进入运行态后即 resolve（run 起始行已落），后续节点 / 相位状态变化经 Channel 以变更通知流出，客户端收通知重查统一视图；终态以通知收尾

#### Scenario: 通知触发重查统一视图

- **WHEN** run 运行期间某步状态变化通知到达（含 archive 相位步）
- **THEN** 客户端经单命令统一视图重查（库读史 ∪ 在飞 run 含步表），无第二命令拼接、无前端载荷累积

#### Scenario: 归档前停等确认续走

- **WHEN** run 到达 archive 前停等点，用户在停等面选择继续（或终止）
- **THEN** 确认后 walker 进入 archive 相位执行至收口；终止则 run 收敛为受控终态；停止命令对 archive 相位运行中目标幂等生效

#### Scenario: 停止幂等

- **WHEN** 对已收口（completed / stopped / failed）的 run 调停止命令
- **THEN** 幂等忽略：状态不变、不报错、不产生新事件

#### Scenario: 发起前置校验与复合键并行

- **WHEN** 对无 db 建档的 change 发起 run、或同 workspace 同一 change 已有运行中的 run 再次发起
- **THEN** 发起命令显式 `Err`（呈现未建档或并行冲突原因），不进入运行态、不落任何账；不同 change / 不同 workspace 不受影响

#### Scenario: stop-before-phase 随发起透传

- **WHEN** 前端发起 run（含「按指定 phase 停止」参数），随后另一 run 以不同停止点发起
- **THEN** 两 run 各自按发起参数定格停止点；archive 为固定停止点不随参数省略而自动归档

#### Scenario: 控制面请求-应答不变

- **WHEN** 用户对 waitingAsk 态应答、对 phase 间停等确认、对运行中 run 停止
- **THEN** 三控制命令沿既有请求-应答契约执行（提前 resolve / 幂等 / 回流驱动语义不变），不经通知通道

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **引擎恒 CLI**：change 执行会话恒走 claude code CLI 引擎（插件 dev-team 天然加载，protect-files 护栏过渡期依赖）；SDK 引擎 MUST NOT 承载 change 执行（无插件加载机制）。
2. **工作流类型**：requirement 与 test-only 工作流全链；bug-fix 相位表后续独立 change。
3. **计划图静态编译期、非通用图引擎**：计划图（workflow → phase → step 序列 + 依赖边 + artifact 清单）SHALL 为编译期 const 表 + `include_str!` 静态装配，MUST NOT 实现运行时加载的通用图引擎或图定义文件格式；执行图（节点 + 边）为 db 落库的运行记录，MUST NOT 上位为路由权威（路由仍问 `phase_next`）。原「walker 只硬编码 walk 本相位循环图」表述随本变更翻案显式立项（step 序列下沉为 `PhaseDefinition.steps`）。
4. **file_log 载体退役**：desktop run 与视图全链零 file_log 触点（git diff 供变更上下文）为既定裁定；外部归档对账（openspec CLI 归档改名后的 db status 翻转时机）留 design 定稿（见 proposal 待决问题）。
5. **protect-files 过渡依赖**：步前实时护栏过渡期依赖插件 hook；插件真下线前需独立 change 补桌面原生护栏（内核 permission 层方向），本期仅留痕。

（原「sessionAnchors 精度损失」「sweep-phase 缺口」两条边界随写面进程内锚点复活而消失，不再留痕；原「file_log 不记录」边界随本变更升级为载体退役，不再是单侧不写。）

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考（含计划图静态编译期翻案留痕），后续变更无需重新论证是否知情

#### Scenario: 引擎边界生效

- **WHEN** 审查 walker 发起 WorkerAgent 会话的引擎解析路径
- **THEN** 恒解析为 CLI 引擎（默认 agent 解析），无 SDK 分支；change run 发起不依赖 SDK 连接配置

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.37` 升级为 `0.4.38`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 `2.10.44` 与三类交付产物）。

#### Scenario: 版本号升级与插件零改动

- **WHEN** 本变更实现完成（归档时执行版本升级）
- **THEN** `packages/desktop/package.json` 的 version 为 0.4.38；`plugins/dev-team` 版本与交付产物零改动

### Requirement: run 落库写缝与累积器驻服务端

run 落库写缝 SHALL 收在 walker 侧单点：`walk_run` 终态出口（`guard.finish` 处）一次完成终态更新 + 节点整包 + 边整包 + active_phase 处置（同事务，desktop-change-state-store「run 执行图落库」「启动标定与 active_phase 悬挂处置」）；`ChangeFlowControl` SHALL 保持进程内零 IO 不变量（不持 store 句柄，store 写经 walker 侧 port / 命令层装配注入）。累积器 SHALL 移驻服务端并升级为**节点 + 边双累积**：`RunEntry`（注册表条目）自持节点列表（全词汇、emit 序、`instance_no` 分配）与边列表（feedback / retry / backtrack 显式落边），walker 经 `RunGuard::emit` 间达零变化；重挂快照 SHALL 含 nodes + edges（重挂恢复不再恒空）。启动装配 SHALL 执行启动标定（残留 running → interrupted，操作面见 desktop-change-state-store）。停止 / 会话失败 / 决策解析失败 / 相位预算耗尽等一切 run 死亡路径 SHALL 汇聚同一 finish 落包出口（终态出口唯一既有不变量）。

#### Scenario: 写缝单点与 control 零 IO

- **WHEN** 审查 walker 终态路径与 control 源码
- **THEN** 落包仅出现在 walk_run 终态出口一处；control 无 store 句柄、无 IO 调用（进程内纯状态不变量保持）

#### Scenario: 全死亡路径同一出口

- **WHEN** run 分别以 completed / stopped（用户停止）/ failed（会话失败、决策解析失败）收口
- **THEN** 三路径均经同一 finish 落包出口：终态 + reason + 节点整包 + 边整包 + active_phase 处置单事务落库，库读史与内存面终态一致

#### Scenario: 重挂恢复节点与边完整

- **WHEN** run 运行中（已 emit 多节点与 feedback 边）视图重挂后读取快照
- **THEN** 快照 nodes 与 edges 与已 emit 逐条一致（emit 序、`instance_no` 稳定），后续通知继续触发重读；不依赖前端累积

### Requirement: 相位 prompt 组装面静态化（静态主体 + 动态 append）

executor / evaluator 会话 prompt SHALL 以写面下发的静态角色知识文本（desktop-phase-prompts / desktop-workflow-write-face）为主体；动态面 SHALL 全部 append 而非模板替换：上下文头一行 `change: <name>`、回溯原因行、static-check / 测试修复失败注入、verdict 解析失败修复注入（evaluator 续注）、输出协议附录（evaluator）、decision 三列表（决策会话）。prompt 组装 MUST NOT 使用 `<change>` / `<phase>` 模板替换。`WorkerTurnRequest.provenance.source_ref = <change>/<phase>/<role>/<attempt>` 与 `root` / `model_level` 结构化通道不变——phase / attempt 盖戳不依赖 prompt 文本。

#### Scenario: 上下文头在场

- **WHEN** walker 发起 executor / evaluator 会话
- **THEN** prompt 含一行 `change: <name>`（change 名自写面记录 name 供给）
- **AND** prompt 主体为静态角色知识文本，无 `<change>` / `<phase>` 模板替换痕迹

#### Scenario: 反馈注入仍 append

- **WHEN** static-check / test-execution 失败或 verdict 解析失败走定向反馈边
- **THEN** 修复反馈文本以 append 注入同一 agent 会话 prompt（不重写静态主体）——门禁步注入 executor 会话、verdict 门注入 evaluator 会话
- **AND** 回溯场景的回溯原因行照常 append

### Requirement: 角色要点静态化（role_brief 退役）

`core/orchestration/src/prompt.rs` SHALL 删除 `role_brief` 15 行静态表与 `strip_call_agent`（角色自述已溶解进 desktop-phase-prompts 各 md）。`executor_prompt` SHALL 不再前置「你以角色…」+ role_brief 段，改为直出静态 prompt 主体（不再消费 `agent_type`）；`evaluator_prompt` SHALL 仅追加输出协议附录。`prompt.rs` MUST NOT 残留按角色名分发要点的逻辑。

#### Scenario: role_brief 退役

- **WHEN** 审查 `prompt.rs` 与 executor prompt 组装
- **THEN** 无 `role_brief` 表、无 `strip_call_agent`、无 `__CALL_AGENT` 解析
- **AND** executor prompt 主体直出静态 md 文本（角色自述已含于其中）

#### Scenario: evaluator 协议仍唯一追加

- **WHEN** 组装 evaluator prompt
- **THEN** 仅追加输出协议附录，无角色要点前导、无 `__CALL_AGENT` 相关处理

### Requirement: evaluator 输出协议瘦身

`evaluator_protocol` 输出协议附录 SHALL 要求 evaluator 最终消息输出且仅输出一个 checklist JSON 对象，形状为 `{ verdict: "pass" | "fail", report: <≤2000 字符>, checklist: [{ item, pass, evidence }] }`；MUST NOT 要求 phase / attempt / skipped 回声（由 walker 按 provenance 盖戳）。协议 SHALL 保留禁调 MCP 写通道红线（phase_log / phase_next / phase_start / backtrack，落账由桌面编排代写）；checklist 至少一项、pass 为布尔、evidence 必须给事实依据。

#### Scenario: 协议形状瘦身

- **WHEN** 读取 evaluator 输出协议附录文本
- **THEN** 要求形状仅含 verdict / report / checklist 三键
- **AND** 无「输出 phase 字段」「输出 attempt 字段」「输出 skipped 字段」要求
- **AND** 仍含禁调 MCP 写通道红线

### Requirement: 计划图与执行图对齐与步粒度回退

计划图（源码，desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）与执行图（db，desktop-change-state-store「run 执行图落库」）SHALL 经 **step kind + phase + attempt + instance_no** 对齐：执行图节点是计划 step 的运行时实例，节点身份键 SHALL 为 `(run_id, phase, attempt, step, instance_no)`；执行图边 SHALL 显式落 `RunEdgeRecord`，`edge_kind ∈ { forward, feedback, retry, backtrack }`——archive 相位六段串行（实为 10 步）为 forward 边，无回边。计划图 MUST NOT 落库（定义恒为编译期静态）；执行图 MUST NOT 上位为路由权威。回退（backtrack）的语义单位 SHALL 保持 phase 粒度；本变更 MUST NOT 把回退载荷改为 step 粒度。

#### Scenario: 节点身份稳定且 feedback 独立

- **WHEN** 同 phase 同 attempt 内 static-check 失败触发 executor 修复重跑，随后 phase fail 重试该相位
- **THEN** executor 首次与修复各为 `instance_no` 递增的独立节点，feedback 边指向前一节点；phase 重试产生新 attempt，retry 边指向新 attempt 首节点

#### Scenario: 归档步为 forward 边

- **WHEN** archive 相位一次 run 收口后查询执行图
- **THEN** 归档 10 步节点（含 8 个新步词）以 forward 边串行落 `RunEdgeRecord`，零 feedback / retry / backtrack 边；`run_id` / `from_node_key` 两二级索引可达

#### Scenario: 边四值与查询

- **WHEN** 审查一次含前进 / 反馈 / 重试 / 回跳的 run 执行图
- **THEN** `RunEdgeRecord` 四 kind 齐备且 `run_id` / `from_node_key` 两二级索引可达；`backtrack` 边指向白名单内目标相位首节点

### Requirement: 确定性相位空 checklist 落账

archive 相位（无 evaluator、无 checklist 的确定性相位）SHALL 在末尾 PhaseLog 步以空 checklist 落账：`step_phase_log` 在 test_outcome 分支之后、verdict 分支之前插入 archive 分支——构造 `PhaseLogInput { phase: "archive", report: String::new(), checklist: vec![], skipped: false, executor/evaluator/decision 三槽位 None }` 并直接 `run_phase_log`。落账 SHALL 复用既有 `derive_verdict([])=Pass` 口径（空 checklist = 无条件通过），零 `if phase == archive` 特判、零新增落账函数。三分流按「产物在否」分离：archive 相位无 TestExecution step（test_outcome 恒 None）、无 Evaluator/VerdictGate step（verdict 恒 None），仅命中空 checklist 分支。

#### Scenario: 空 checklist 落账 pass

- **WHEN** archive 相位 10 步走完进入 PhaseLog 步
- **THEN** `run_phase_log` 以空 report + 空 checklist 落 `verdict=pass` 的 PhaseRecord，`active_phase` 清位（phase_log 单事务），`phase_next` 随后路由 done

#### Scenario: 三分流互不误判

- **WHEN** 分别驱动评估相位（verdict 产物）、test-execution 相位（test_outcome 产物）与 archive 相位（空 checklist）
- **THEN** `step_phase_log` 三分支按产物在否分离，无按 phase 名判断、无跨分支误判

### Requirement: 发起前置校验类型分派（严格拒绝未知类型）

`change_flow_start` 前置校验 SHALL 保留「`phase_table(record.workflow_type)` 返回 `None` → 显式拒绝」的门：`"requirement"` / `"test-only"` 出线放行（test-only 进表后自然放行），其余（含空白 / bug-fix）SHALL 显式 `Err`。错误文案 SHALL 通吃 `{requirement, test-only}`，MUST NOT 再写死「V1 仅 requirement」、MUST NOT 引入「未知类型静默跑 requirement」的 fallback。walker 全程 SHALL 不新增 workflow_type 分支——相位序 / 步序列仍由写面 `phase_next` 依 `record.workflow_type` 下发的相位表承载。

#### Scenario: test-only 发起放行

- **WHEN** 对已建档 `workflow_type="test-only"` 的 change 调用 `change_flow_start` 前置校验
- **THEN** 校验通过（test-only 相位表在位），run 按 `TEST_ONLY_TABLE` 相位序走（proposal → code-analyze → … → archive）

#### Scenario: 未知类型显式拒绝

- **WHEN** 对 `workflow_type="bug-fix"`（或空白）的 change 调用 `change_flow_start`
- **THEN** 显式 `Err`（文案含 `{requirement, test-only}` 支持面），不进入运行态、不落任何账

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration`（运行时域，tokio） | 薄图 walker：相位循环、三类节点执行、Gate 分支 | 红线：零相位转移规则（每步过渡问写面 phase-next）；ToolStep 节点化（不内联）；节点状态纯派生（库读史 run 节点行 ∪ 在飞 run，节点行携 `session_id`）；模块内零进程 spawn；依赖方向 orchestration → workflow；对落库载体无感知 |
| `crates/core/orchestration/src/port.rs` | 「相位机 + 工具步」进程内缝（语义换血） | ToolStepPort 进程内直调 core/workflow 写面；ToolCommand 封闭集收缩（ChangeFiles 出局）；WorkerAgentPort 契约不变；测试假引擎 + 假写面双缝直驱 |
| `crates/core/orchestration/src/steps.rs`（LocalToolSteps） | 相位机步进程内直调写面 | 直调面签名不变（载体在写面内换血）；每步结果落 StepRecord 审计行（经写面） |
| `crates/core/orchestration/src/snapshot.rs` | ChangeDetail 只读装配 | 读源改 workspace 库 change 状态（经 queries）；磁盘扫描保留产物发现 |
| `crates/core/orchestration/src/verdict.rs` + `decision.rs`（不改） | 结构化输出解析 | 封闭 schema（checklist 信封 + 决策四动作封闭集）；解析失败显式 Err 上抛——处置归 walker 按步声明分叉（verdict 反馈边 / 决策终态） |
| `crates/core/orchestration/src/prompt.rs`（不改） | prompt 组装与运行时插值 | 模板取自 core/workflow 相位表单源 |
| `crates/core/orchestration/src/transcript.rs`（砍半） | 会话转录消费 | 仅保留 `final_assistant_text`（verdict 提取）；`extract_write_paths` 出局 |
| `crates/infra/agent/src/worker.rs` | WorkerAgentPort 实现 + static-check spawn 缝 | compose_turn 新会话 + StopRegistry 终止 + 密封转录；provenance `source="change"`；spawn 不进 core |
| `src/commands/` change_flow 命令组 | 运行控制 IPC 面 | 发起前置校验改 db 建档校验（workflow.json 可解析校验退役）；发起 / 停止 / 应答 / 确认契约不变 |
| `crates/core/workflow`（写面 + 读面，见 desktop-workflow-write-face） | change 状态域权威 | 读写同屋檐、落库 workspace 库；walker 经进程内直调消费，不复制路由语义 |
| `crates/infra/store`（复用） | 会话记录 + change 状态落库 | `SessionRecord` 携 `SessionProvenance`（`source` / `source_ref` 留痕，会话创建时写入）；run 节点行携 `session_id` 供会话直查；不建 flow_runs 表 |
| `crates/infra/devteam` | **（删除）** | 整体删除（discover / runner），零遗留 |
| 既有 hooks（复用，不改） | 步前护栏与无外围账 | protect-files 原样生效（过渡期）；record-files / sweep-phase 未绑定 no-op（fail-open 刻意设计）；static-check 桌面模式由 spawn 步承接 |
| `crates/core/orchestration/src/walker.rs` | 停等节奏分支 | `RunRequest` + `auto_next_phase` 字段（run 级、发起定格）；drive() 确认点 `if !auto_next_phase` 才 emit `ConfirmWait` + `wait_confirm`；auto 模式不进 `WaitingConfirm`、零新事件；头注释口径同步 |
| `src/commands/change_flow/mod.rs` | 发起参数透传 | `change_flow_start` / `change_flow_start_with` 签名 + `auto_next_phase: bool`，透传 `RunRequest`；`Result<T, String>` 模板、前置校验、提前 resolve 不变 |
| `src/types/generated/bindings.ts`（重导） | IPC 类型跟随 | `changeFlowStart` + `autoNextPhase` 参数；`bindings:check` 守卫拦截过期生成物 |
| `crates/core/orchestration/src/control.rs` / `state.rs`（不改） | 零触点红线 | confirm 应答通道 / `RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` DTO 不动 |
| `crates/core/orchestration/src/port.rs` | 「相位机 + 工具步」进程内缝（封闭集扩展） | `ToolCommand` 增 `TestExecution { change, … }` 变体；`ToolStepOutput` 增 `TestExecution(最小载荷)`；`TestExecutionRunner` port 与 `StaticCheckRunner` 并列（spawn 不进 core）；实现落 checks 边界 `crates/infra/checks`，core/orchestration 零依赖 checks |
| `crates/core/orchestration/src/walker.rs` | 检查域门禁步接入 | static-check 反馈边沿用（`STATIC_CHECK_FEEDBACK_LIMIT` 不变）；test-execution 相位 runner 门禁步 + 独立反馈预算（计数器分立、上限 5、超限升格相位 fail） |
| `crates/infra/agent/src/worker.rs` | WorkerAgentPort 实现（收窄） | compose_turn 新会话 + StopRegistry 终止 + 密封转录 + provenance `source="change"`；static-check spawn 缝移出（落 `crates/infra/checks`，见 desktop-checks-domain） |
| `crates/core/orchestration/src/walker.rs` + `RunRequest` | 双 root run 载体 | `RunRequest.root` 恒 exec root（worktree / legacy 主 root）；fs 半边（Layout / diff / 检查 cwd / 快照产物发现）随 root 落位；store 经 port 注入不变 |
| `crates/core/orchestration/src/control.rs` | run 控制注册表键换锚 | `runs: HashMap<(String, String), RunEntry>` 键 `(workspace root, change)` → `(workspace root, change id)`（12 处构造点随动）；`RunUpdate` / `RunNotice` / `ChangeRunSnapshot` / `ChangeRunStatus` DTO 零改动（通知零载荷） |
| `crates/core/orchestration/src/walker.rs` | run 载体与 provenance | `RunRequest` / `RunStartCommand` / `RunFinishCommand` / `ToolCommand` 载荷 change → change id；provenance 定式 `<id>/<phase>/<role>/<attempt>`；`SessionAnchors` 键 `(change id, run_id)` |
| `crates/core/orchestration/src/port.rs` + `steps.rs` + `run_history.rs` + `snapshot.rs` | 载荷 id 化 | 相位机步直调写面携 change id；`TestExecutionRunner` / `WorkflowSnapshotPort` 签名随动（`detail(root, id)` → 经 queries id 寻址） |
| `crates/core/workflow/src/write/phase_next.rs`（SessionAnchors） | 进程内锚点键 | `(change, run_id)` → `(change id, run_id)`；锚点语义（续走不重头）不变 |
| `src/commands/change_flow/mod.rs` | 五命令 id 化 | start / stop / answer / confirm / watch 定位参数 == change id；`ChangeFlowSink` 键 `(root, id)`；前置校验（建档 / 互斥 / exec root 解析）按 id 读取记录；store 实例以 workspace root 解析后注入 compose（拆参形态见 desktop-agent-execution / design）；其余三件事纪律不变 |
