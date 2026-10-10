# desktop-change-orchestration Specification

## Purpose

Desktop 后端 change run 编排运行时（薄图 walker，core/orchestration）：对 PGE 工作流执行硬编码单图相位循环，三类节点（WorkerAgent / ToolStep / Gate）统一可观测；路由权威驻 core/workflow 写面（walker 零自持转移规则）；提供运行控制命令面（发起 / 停止 / 应答 / 确认）与决策协议。

## Requirements

### Requirement: 薄图 walker 相位循环与路由红线

Desktop 后端 SHALL 提供编排运行时（walker，core/orchestration），对 change run 执行硬编码的单图相位循环，每相位依序：

1. core/workflow 写面 `phase_next`（进程内）→ next_phase、已插值的 executor / evaluator prompt、`allowed_backtrack_phases` 白名单；
2. 写面 `phase_start`（开启阶段、attempt 计时）；
3. spawn executor agent（compose_turn 新会话）；
4. implement / test-gen 相位后执行 static-check（checks 边界 spawn 步）；test-execution 相位以确定性 runner 门禁步承接相位主体（checks 边界 spawn 步，绿跑零 agent，语义见 desktop-test-execution）；
5. spawn evaluator agent（新会话，不落账）；
6. 解析 verdict → 桌面代调写面 `phase_log`；
7. 写面 `phase_next` → pass 推进 / fail 重试（≤5，与插件 `MAX_RETRY_TIMES` 一致）/ 重试上限唤决策 agent 分叉。

红线：walker MUST NOT 持有任何相位转移规则——每步过渡 SHALL 问写面 `phase_next`（路由权威与 walker 同进程仍不自持规则）；`PIPELINE_PHASES` 与后端 `detail.rs` 相位列 SHALL 保持纯布局身份，MUST NOT 上位为路由权威。推进节奏 SHALL 为 phase 内自动、phase 间停等用户确认后继续（`auto_next_phase` 发起参数开启时 phase 间停等跳过，语义见「自动确认运行模式」）；重启 run SHALL 自 active_phase 续走（active_phase 持久化于 db `ChangeRecord`，已 pass 相位 MUST NOT 重头执行，续走由 `phase_next` 依 eval 历史重算）。写面全部落库载体为 workspace 库（desktop-workflow-write-face / desktop-change-state-store），walker 对载体无感知。

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
- **THEN** walker 自 db `ChangeRecord.active_phase`=implement 的下一 attempt 续走，已 pass 的 proposal / dev-design 等相位不重新执行；中断相位经进程内锚点（sessionAnchors 复活）可感知并被标定

### Requirement: 自动确认运行模式（auto_next_phase）

Desktop run SHALL 支持「自动确认」运行模式：`RunRequest.auto_next_phase`（bool）为 run 级属性、发起时定格，经运行控制命令面发起入口透传。`auto_next_phase=true` 时，walker SHALL 在 phase 间确认点跳过 `ConfirmWait` 广播与 `wait_confirm` 挂起，直接推进下一相位；auto 模式 run MUST NOT 进入 `WaitingConfirm` 状态、MUST NOT 产生 `ConfirmWait` 事件（`RunUpdate` / `ChangeRunSnapshot` DTO 零改动，重挂恢复路径天然兼容）。`auto_next_phase=false`（默认）时既有停等语义不变。

语义边界：

- **ask 中断不自动应答**：auto 模式下决策 agent 的 ask 到来 SHALL 照常进入 `WaitingAsk` 停等用户应答——ask 是重试预算耗尽后的真实提问，需要人的判断；自动确认只作用于 phase 间停等。
- **终态不自动归档**：全相位 pass SHALL 照常以 `completed` 收口（"Ready for archiving"）停给用户，MUST NOT 自动触发归档。

运行中停等节奏 MUST NOT 可切换：节奏变更 SHALL 经停止后重发（自 `active_phase` 续走）；auto 模式下用户干预的合法动作与既有运行面一致（停止）。

#### Scenario: auto run 直通收口

- **WHEN** 以 auto_next_phase=true 发起多相位 run（各相位 evaluator 全 pass）
- **THEN** 更新流零 `ConfirmWait` 信封，逐相位自动推进直至 `completed`（reason = "All phases have passed evaluation. Ready for archiving."），快照面全程不经 `waitingConfirm`，归档未自动触发

#### Scenario: 手动模式语义不变

- **WHEN** 以 auto_next_phase=false（默认）发起同一 run
- **THEN** `ConfirmWait` 逐相位流出、挂起至 `change_flow_confirm` 应答，proceed=false / 等待期间取消 → 受控 `stopped`（与既有契约逐字一致）

#### Scenario: auto run 的 ask 仍停等人

- **WHEN** auto 模式 run 中决策 agent 输出 ask 动作（重试预算耗尽后的提问）
- **THEN** run 照常进入 `WaitingAsk` 并呈现问题与选项，walker 挂起等应答；MUST NOT 自动选择任何选项，用户应答后 run 继续

#### Scenario: 发起定格与停止重发

- **WHEN** 运行中的 run 需要变更停等节奏
- **THEN** 唯一路径为停止当前 run 后以目标节奏重新发起（自 `active_phase` 续走，已 pass 相位不重头执行）；运行中不存在节奏切换命令或事件

### Requirement: 三类节点统一 walker 与可观测均一

Walker SHALL 将每相位循环表达为三类节点：

- **WorkerAgent 节点**（executor / evaluator / decision）：经 compose_turn 开新会话，转录可观测、可停止；
- **ToolStep 节点**（phase-start / phase-log / backtrack 相位机步进程内直调写面 + static-check / test-execution 检查域 spawn 步落 checks 边界）：无智能；
- **Gate 节点**（verdict 解析 / retry 计数 / 白名单校验）：纯 Rust 分支。

ToolStep MUST NOT 以命令式内联实现：其执行结果 SHALL 作为图上节点状态可观测（失败 = 可见失败态，与 WorkerAgent 失败同等呈现）。节点状态 SHALL 为派生视图 = **库读史（`RunRecord` / `RunStepRecord`，desktop-change-state-store「run 运行史落库」）∪ 进程内注册表在飞 run（`RunEntry` 步累积器）** × 按 provenance 反查的 session 集；run 状态落库为既定基线——原「MUST NOT 建立 run 私有持久表（不建 flow_runs 表）」红线随 unify-run-state-persistence 决策翻案显式立项（`control.rs` 头注释决策原文同步改写；StepRecord 仍为步骤审计行、MUST NOT 承担节点状态源）。步词汇过滤 SHALL 单点定义（落库子集 = 回显子集，五留五忽略封闭词汇，desktop-change-state-store），walker 与 control MUST NOT 持有过滤逻辑。已收口 run 的任一节点状态 SHALL 可由库读史单独重算得出（重启 / 重挂后不依赖进程内存或 run 私有状态文件）。

#### Scenario: ToolStep 失败同等可见

- **WHEN** static-check 或 test-execution spawn 步以非零退出 / 显式 Err 收场（或写面相位机步返回错误）
- **THEN** 图上该节点呈失败态（与 executor 会话失败同等的可观测性），run 不静默越过该节点

#### Scenario: 节点状态两源派生可重算

- **WHEN** 审查编排运行时的持久化面与读路
- **THEN** runs / run_steps 两表在案（desktop-change-state-store）、在飞 run 状态驻进程内注册表（RunEntry）；已收口 run 的任一节点状态均可由库读史重算得出，不依赖进程内存

### Requirement: 零 CLI 子进程写通道与 crate 依赖方向

Walker MUST NOT 经任何 CLI 子进程变更 change 流程状态（dev-team CLI 子命令面不存在）：所有 workflow 状态变更 SHALL 经 core/workflow 写面进程内直调（desktop-workflow-write-face 契约，落库 workspace 库），MUST NOT 读写 workflow.json。依赖方向 SHALL 为 orchestration → workflow；`crates/infra/devteam` MUST NOT 存在（整体删除）。检查域门禁（static-check 与 test-execution）的进程 spawn 为编排域的 spawn 例外家族且 MUST 落 checks 边界 infra 层 `crates/infra/checks`（spawn 不进 core——core/orchestration 自身零进程 spawn，经 port 缝下沉；边界定义见 desktop-checks-domain）。

#### Scenario: 零 CLI 写触点

- **WHEN** 审查编排运行时与命令组源码中对 change 流程状态的变更触点
- **THEN** 零 CLI 子进程调用、零 devteam 子进程依赖、零 workflow.json 读写；全部状态变更经进程内写面调用返回确认并落库

#### Scenario: spawn 边界保持

- **WHEN** 审查 core/orchestration 源码的进程创建调用
- **THEN** orchestration 模块内零进程 spawn；检查域 spawn（static-check / test-execution）经 port 缝由 checks 边界 `crates/infra/checks` 实现承载

### Requirement: 会话挂靠 provenance 与节点状态派生

Walker 发起的 executor / evaluator / decision 会话 SHALL 携带 provenance：`source="change"`、`source_ref=<change>/<phase>/<role>/<attempt>`。节点运行态 SHALL 由按 provenance 反查的会话集派生（`agentSessions` 按 source / sourceRef 过滤既有面）；会话转录 SHALL 可观测：运行中实时流、结束后重放一致。会话停止 SHALL 复用 StopRegistry 既有终止面，对已终态目标幂等忽略。

#### Scenario: 按归属反查会话集

- **WHEN** 某 change 的 implement#2 相位已发起 executor 与 evaluator 会话
- **THEN** 按 `source="change"` + `source_ref="<change>/implement/executor/2"` 等过滤查询可枚举对应会话及其运行态，派生出节点状态，无需任何额外注册表

#### Scenario: 转录可观测与停止

- **WHEN** executor 会话运行中用户请求停止
- **THEN** 该会话经 StopRegistry 收敛为 `stopped`，walker 收到终止终态并按停止收敛处理 run；对已终态会话的重复停止请求幂等忽略

### Requirement: 变更文件上下文降级 git diff

Desktop run 的 executor / evaluator prompt 组装 SHALL 以 git diff（工作区变更面）提供变更文件上下文（承接原 file_log「让 agent 识别变更文件」的既有用途）。file_log 概念 SHALL 随双向墙整体退役：desktop 全链 MUST NOT 读写任何 `file_log`（不提取转录写路径、不调 `change-files`、不展示 file_log 条目）；存量 skill 路径 workflow.json 内的 file_log 数据随其文件留档不动，desktop 不再解析与展示。

#### Scenario: desktop 全链零 file_log 触点

- **WHEN** 扫描 desktop 源码（core / infra / commands / views）对 file_log 的读与写触点
- **THEN** 零触点；既有 skill 路径产生的 file_log 数据不被触碰（workflow.json 原样留档）

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

### Requirement: verdict 解析与 phase-log 代写

Evaluator 会话 SHALL 依 prompt 约定在最终消息输出 checklist JSON（不自调 MCP `phase_log`）。Walker SHALL 解析该 JSON 并代调写面 `phase_log`（进程内）落 eval 记录（PhaseRecord + ChecklistItemRecord 原子落库）；解析失败 SHALL 将运行停给用户（呈现原始输出与失败原因），MUST NOT 臆测 verdict、MUST NOT 静默跳过落账。checklist JSON 结构 SHALL 以插件既有 eval-checklist 形态为准（phase / attempt 信封 + 逐项 pass / evidence）。

#### Scenario: verdict 代写落账

- **WHEN** evaluator 最终消息输出合法 checklist JSON（verdict=pass）
- **THEN** walker 以该 checklist 经写面 `phase_log` 落 eval 记录（含 phase / attempt 信封），`phase_next` 随后推进

#### Scenario: 解析失败停给用户

- **WHEN** evaluator 最终消息不含可解析的 checklist JSON（结构漂移或空输出）
- **THEN** run 停止并呈现 evaluator 原始输出与解析失败原因，状态库无新增 eval 记录，用户可显式重试该 attempt

### Requirement: 决策协议

写面 `phase_next` 返回重试上限（fail 且需跨相位跳转）时，walker SHALL 唤起决策 agent（compose_turn 新会话）；retry 预算内 SHALL NOT 唤起（walker 自走重试）。决策 agent 输入 SHALL 有界且来自 db 状态与写面响应：fail checklist（fail 项 + evidence）、`allowed_backtrack_phases`（写面 `phase_next` 白名单）、候选相位最近一次 eval report。决策输出 SHALL 为封闭集：

- `{ action: "backtrack", backtrack_to ∈ 白名单, reason ≤500 }` — 自治，walker 经写面 `backtrack` 执行；
- `{ action: "retry" }` — 同相位重试；
- `{ action: "stop", reason }` — 终止 run；
- `{ action: "ask", question, options[] }` — 无法裁决，UI 中断提问。

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验（工具侧兜底）：越权目标 SHALL 被拒绝且 run 停给用户，MUST NOT 落任何越权回跳账（状态库零变更）。决策输出解析失败按「verdict 解析失败」同款停给用户处理。

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

- **发起**：按 change 名发起 run，`auto_next_phase` 参数（bool；false=默认停等节奏，true=自动确认模式）随发起定格该 run 的停等节奏并透传 walker；发起 SHALL 先解析 exec root（db 记录 worktree 字段 → worktree 路径，None → 主 root，见「run 双 root 组合与 exec root 解析」）；沿 agent 执行先例提前 resolve（run 记录进入运行态后即返回），RunRecord 起始行（status=running）SHALL 随发起建立（desktop-change-state-store「run 运行史落库」；写落位——命令层装配 vs walker 起点直调——由 design 定稿）；
- **停止**：终止当前 WorkerAgent 会话并收敛 run 为受控终态；对非运行态目标幂等忽略，MUST NOT 报错或误改既有终态；
- **应答**：ask 中断态下回传用户应答（选项或自由文本），驱动 run 继续；
- **确认**：phase 间停等点的用户确认（继续 / 终止）。

**读路统一（推拉反转）**：`get_change_detail`（或其扩展）SHALL 一次返回「库读史 ∪ 内存在飞 run」统一视图——库面 runs / steps 读史（desktop-change-queries）并入在飞 run 的状态 / 停等与 ask 载荷 / RunEntry 步表，合并发生在读时（零每步写放大前提不变）；客户端 MUST NOT 双命令拼接（useChangeDetail + useChangeFlowRun 双真相源拼缝退场）。`RunUpdate` SHALL 降位为变更通知（无步 / 会话事件载荷或最小载荷，信封形状 design 定稿）：客户端收通知后自行重查统一视图，通知仅失效信号、查询结果为权威；高频通知（每步 / 每会话事件）SHALL 经客户端合并去抖或通知侧 coalesce 抑制重查风暴（策略 design 定稿）。stop / confirm / answer 控制面 SHALL 维持请求-应答形态不变（非推送）。

运行状态 Channel SHALL 与 agent 执行流同构（执行流通道例外，不属轮询取数）。发起前置校验 SHALL 覆盖：目标 change 已建档（db `ChangeRecord` 在案且 `workflow_type=requirement` 相位表在位）、无同 workspace 同 change 并行 run——并行冲突键 SHALL 为 `(workspace root, change)` 复合（修正既有仅按 change 名做键的两 workspace 同名假冲突先例 bug；worktree 隔离解锁同 workspace 多 change 真并行）——无建档的 change（存量 CLI change）SHALL 显式拒绝发起（文档形态 change 不可运行）。

#### Scenario: 发起提前 resolve 与通知流

- **WHEN** 前端发起某 change 的 run
- **THEN** invoke 在 run 进入运行态后即 resolve（run 起始行已落），后续节点 / 相位状态变化经 Channel 以变更通知流出，客户端收通知重查统一视图；终态以通知收尾、客户端重取定局

#### Scenario: 通知触发重查统一视图

- **WHEN** run 运行期间某步状态变化通知到达
- **THEN** 客户端经单命令统一视图重查（库读史 ∪ 在飞 run 含步表），无第二命令拼接、无前端载荷累积

#### Scenario: 控制面请求-应答不变

- **WHEN** 用户对 waitingAsk 态应答、对 phase 间停等确认、对运行中 run 停止
- **THEN** 三控制命令沿既有请求-应答契约执行（提前 resolve / 幂等 / 回流驱动语义不变），不经通知通道

#### Scenario: 停止幂等

- **WHEN** 对已收口（completed / stopped / failed）的 run 调停止命令
- **THEN** 幂等忽略：状态不变、不报错、不产生新事件

#### Scenario: 发起前置校验与复合键并行

- **WHEN** 对无 db 建档的 change（存量 CLI change）发起 run、或同 workspace 同 change 已有运行中的 run 再次发起、或另一 workspace 存在同名 change 的运行中 run
- **THEN** 前两者发起命令显式 `Err`（呈现未建档或并行冲突原因），不进入运行态、不落任何账；第三者不受影响正常发起（复合键下异 workspace 同名不构成冲突）

#### Scenario: auto_next_phase 随发起透传

- **WHEN** 前端以 auto_next_phase=true 发起某 change 的 run，随后另一 change 以 auto_next_phase=false 发起
- **THEN** 两 run 各自按发起参数定格节奏运行：前者全程无 phase 间停等，后者照常停等确认；参数经命令面透传至 `RunRequest.auto_next_phase`

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **引擎恒 CLI**：change 执行会话恒走 claude code CLI 引擎（插件 dev-team 天然加载，protect-files 护栏过渡期依赖）；SDK 引擎 MUST NOT 承载 change 执行（无插件加载机制）。
2. **工作流类型**：仅 requirement 工作流全链；bug-fix / test-only 相位表后续独立 change。
3. **单图硬编码**：walker 只硬编码 walk 本相位循环图；MUST NOT 实现通用图引擎或图定义文件格式。
4. **file_log 载体退役**：desktop run 与视图全链零 file_log 触点（git diff 供变更上下文）为既定裁定；外部归档对账（openspec CLI 归档改名后的 db status 翻转时机）留 design 定稿（见 proposal 待决问题）。
5. **protect-files 过渡依赖**：步前实时护栏过渡期依赖插件 hook；插件真下线前需独立 change 补桌面原生护栏（内核 permission 层方向），本期仅留痕。

（原「sessionAnchors 精度损失」「sweep-phase 缺口」两条边界随写面进程内锚点复活而消失，不再留痕；原「file_log 不记录」边界随本变更升级为载体退役，不再是单侧不写。）

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考，后续变更无需重新论证是否知情

#### Scenario: 引擎边界生效

- **WHEN** 审查 walker 发起 WorkerAgent 会话的引擎解析路径
- **THEN** 恒解析为 CLI 引擎（默认 agent 解析），无 SDK 分支；change run 发起不依赖 SDK 连接配置

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.27` 升级为 `0.4.28`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 `2.10.44` 与三类交付产物）。

#### Scenario: 版本号升级与插件零改动

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` 的 version 为 0.4.28；`plugins/dev-team` 版本与交付产物零改动

### Requirement: run 落库写缝与累积器驻服务端

run 落库写缝 SHALL 收在 walker 侧单点：`walk_run` 终态出口（`guard.finish` 处）一次完成终态更新 + 步整包 + active_phase 处置（同事务，desktop-change-state-store「run 运行史落库」「启动标定与 active_phase 悬挂处置」）；`ChangeFlowControl` SHALL 保持进程内零 IO 不变量（不持 store 句柄，store 写经 walker 侧 port / 命令层装配注入）。步累积器 SHALL 移驻服务端：`RunEntry`（注册表条目）自持步列表（emit 序追加、通知触发重读），walker 经 `RunGuard::emit` 间达零变化；重挂快照 SHALL 含 steps（重挂恢复步表不再恒空）。启动装配 SHALL 执行启动标定（残留 running → interrupted，操作面见 desktop-change-state-store）。停止 / 会话失败 / verdict 解析失败等一切 run 死亡路径 SHALL 汇聚同一 finish 落包出口（终态出口唯一既有不变量）。

#### Scenario: 写缝单点与 control 零 IO

- **WHEN** 审查 walker 终态路径与 control 源码
- **THEN** 落包仅出现在 walk_run 终态出口一处；control 无 store 句柄、无 IO 调用（进程内纯状态不变量保持）

#### Scenario: 全死亡路径同一出口

- **WHEN** run 分别以 completed / stopped（用户停止）/ failed（会话失败、verdict 解析失败）收口
- **THEN** 三路径均经同一 finish 落包出口：终态 + reason + 步整包 + active_phase 处置单事务落库，库读史与内存面终态一致

#### Scenario: 重挂恢复步表完整

- **WHEN** run 运行中（已 emit 多步）视图重挂后读取快照
- **THEN** 快照 steps 与已 emit 步逐条一致（emit 序），后续通知继续触发重读；不依赖前端累积

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration`（运行时域，tokio） | 薄图 walker：相位循环、三类节点执行、Gate 分支 | 红线：零相位转移规则（每步过渡问写面 phase-next）；ToolStep 节点化（不内联）；节点状态纯派生（db 状态 × provenance 反查）；模块内零进程 spawn；依赖方向 orchestration → workflow；对落库载体无感知 |
| `crates/core/orchestration/src/port.rs` | 「相位机 + 工具步」进程内缝（语义换血） | ToolStepPort 进程内直调 core/workflow 写面；ToolCommand 封闭集收缩（ChangeFiles 出局）；WorkerAgentPort 契约不变；测试假引擎 + 假写面双缝直驱 |
| `crates/core/orchestration/src/steps.rs`（LocalToolSteps） | 相位机步进程内直调写面 | 直调面签名不变（载体在写面内换血）；每步结果落 StepRecord 审计行（经写面） |
| `crates/core/orchestration/src/snapshot.rs` | ChangeDetail 只读装配 | 读源改 workspace 库 change 状态（经 queries）；磁盘扫描保留产物发现 |
| `crates/core/orchestration/src/verdict.rs` + `decision.rs`（不改） | 结构化输出解析 | 封闭 schema（checklist 信封 + 决策四动作封闭集）；解析失败显式失败停给用户 |
| `crates/core/orchestration/src/prompt.rs`（不改） | prompt 组装与运行时插值 | 模板取自 core/workflow 相位表单源 |
| `crates/core/orchestration/src/transcript.rs`（砍半） | 会话转录消费 | 仅保留 `final_assistant_text`（verdict 提取）；`extract_write_paths` 出局 |
| `crates/infra/agent/src/worker.rs` | WorkerAgentPort 实现 + static-check spawn 缝 | compose_turn 新会话 + StopRegistry 终止 + 密封转录；provenance `source="change"`；spawn 不进 core |
| `src/commands/` change_flow 命令组 | 运行控制 IPC 面 | 发起前置校验改 db 建档校验（workflow.json 可解析校验退役）；发起 / 停止 / 应答 / 确认契约不变 |
| `crates/core/workflow`（写面 + 读面，见 desktop-workflow-write-face） | change 状态域权威 | 读写同屋檐、落库 workspace 库；walker 经进程内直调消费，不复制路由语义 |
| `crates/infra/store`（复用） | 会话记录与反查 + change 状态落库 | `agentSessions` 按 source / sourceRef 过滤派生节点状态；不建 flow_runs 表 |
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
| `crates/core/orchestration/src/control.rs` | run 控制注册表 | `begin_run` / `subscribe` 键改 `(workspace root, change)` 复合；`RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` DTO 零改动 |
| `src/commands/change_flow/mod.rs` | 发起装配 | 前置校验后解析 exec root（db worktree 字段 → 路径，None → 主 root）；store 实例以 workspace root 解析后注入 compose（拆参形态见 desktop-agent-execution / design）；其余三件事纪律不变 |
