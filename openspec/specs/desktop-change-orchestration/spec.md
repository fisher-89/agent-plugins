# desktop-change-orchestration Specification

## Purpose

Desktop 后端 change run 编排运行时（薄图 walker，core/orchestration）：对 PGE 工作流执行硬编码单图相位循环，三类节点（WorkerAgent / ToolStep / Gate）统一可观测；路由权威驻 core/workflow 写面（walker 零自持转移规则）；提供运行控制命令面（发起 / 停止 / 应答 / 确认）与决策协议。

## Requirements

### Requirement: 薄图 walker 相位循环与路由红线

Desktop 后端 SHALL 提供编排运行时（walker，core/orchestration），对 change run 执行硬编码的单图相位循环，每相位依序：

1. core/workflow 写面 `phase_next`（进程内）→ next_phase、已插值的 executor / evaluator prompt、`allowed_backtrack_phases` 白名单；
2. 写面 `phase_start`（开相位、attempt 计时）；
3. spawn executor agent（compose_turn 新会话）；
4. implement / test-gen 相位后执行 static-check（infra spawn 步）；
5. spawn evaluator agent（新会话，不落账）；
6. 解析 verdict → 桌面代调写面 `phase_log`；
7. 写面 `phase_next` → pass 推进 / fail 重试（≤5，与插件 `MAX_RETRY_TIMES` 一致）/ 重试上限唤决策 agent 分叉。

红线：walker MUST NOT 持有任何相位转移规则——每步过渡 SHALL 问写面 `phase_next`（路由权威与 walker 同进程仍不自持规则）；`PIPELINE_PHASES` 与后端 `detail.rs` 相位列 SHALL 保持纯布局身份，MUST NOT 上位为路由权威。推进节奏 SHALL 为 phase 内自动、phase 间停等用户确认后继续；重启 run SHALL 自 `active_phase` 续走（已 pass 相位 MUST NOT 重头执行）。

#### Scenario: 相位内全循环自动走完

- **WHEN** 以假引擎（预录 AgentEvent 序列）+ 假写面（fake port，预录 phase-next / phase-start / phase-log 响应）驱动某相位 run
- **THEN** walker 依序执行 ①→⑦：executor 会话发起、evaluator 会话发起、`phase_log` 以解析 verdict 代调，pass 后经写面 `phase_next` 推进下一相位，全程无人工干预

#### Scenario: fail 重试与预算上限

- **WHEN** evaluator verdict 为 fail 且重试未超预算
- **THEN** walker 不唤决策 agent 自走重试同相位；连续 fail 达 5 次后停止自走并按决策协议分叉

#### Scenario: 中断续走不重头

- **WHEN** walker 于 implement#2 运行中被停止（或桌面重启）后重新发起该 change 的 run
- **THEN** walker 自 active_phase=implement 的下一 attempt 续走，已 pass 的 proposal / dev-design 等相位不重新执行；中断相位经进程内锚点（sessionAnchors 复活）可感知并被标定

### Requirement: 三类节点统一 walker 与可观测均一

Walker SHALL 将每相位循环表达为三类节点：

- **WorkerAgent 节点**（executor / evaluator / decision）：经 compose_turn 开新会话，转录可观测、可停止；
- **ToolStep 节点**（phase-start / phase-log / backtrack 相位机步进程内直调写面 + static-check spawn 步落 infra）：无智能；
- **Gate 节点**（verdict 解析 / retry 计数 / 白名单校验）：纯 Rust 分支。

ToolStep MUST NOT 以命令式内联实现：其执行结果 SHALL 作为图上节点状态可观测（失败 = 可见失败态，与 WorkerAgent 失败同等呈现）。节点状态 SHALL 为派生视图 = workflow.json 状态（active_phase / eval）× 按 provenance 反查的 session 集；MUST NOT 为节点状态建立持久表（不建 flow_runs 表、workflow.json schema 不动）。

#### Scenario: ToolStep 失败同等可见

- **WHEN** static-check spawn 步以非零退出收场（或写面相位机步返回错误）
- **THEN** 图上该节点呈失败态（与 executor 会话失败同等的可观测性），run 不静默越过该节点

#### Scenario: 节点状态纯派生

- **WHEN** 审查编排运行时的持久化面
- **THEN** 无 flow_runs 表、无 workflow.json 之外的 run 私有状态文件；任一节点状态均可由 workflow.json + 会话记录（按 provenance）重算得出

### Requirement: 零 CLI 子进程写通道与 crate 依赖方向

Walker MUST NOT 经任何 CLI 子进程写 workflow.json（dev-team CLI 子命令面不存在）：所有 workflow 状态变更 SHALL 经 core/workflow 写面进程内直调（desktop-workflow-write-face 契约）。依赖方向 SHALL 为 orchestration → workflow；`crates/infra/devteam` MUST NOT 存在（整体删除）。static-check 的进程 spawn 为编排域唯一 spawn 例外且 MUST 落 infra 层（spawn 不进 core——core/orchestration 自身零进程 spawn，经 port 缝下沉）。

#### Scenario: 零 CLI 写触点

- **WHEN** 审查编排运行时与命令组源码中对 workflow.json 的写触点
- **THEN** 零 CLI 子进程调用、零 devteam 子进程依赖；全部状态变更经进程内写面调用返回确认

#### Scenario: spawn 边界保持

- **WHEN** 审查 core/orchestration 源码的进程创建调用
- **THEN** orchestration 模块内零进程 spawn；static-check spawn 经 port 缝由 infra 层实现承载

### Requirement: 会话挂靠 provenance 与节点状态派生

Walker 发起的 executor / evaluator / decision 会话 SHALL 携带 provenance：`source="change"`、`source_ref=<change>/<phase>/<role>/<attempt>`。节点运行态 SHALL 由按 provenance 反查的会话集派生（`agentSessions` 按 source / sourceRef 过滤既有面）；会话转录 SHALL 可观测：运行中实时流、结束后重放一致。会话停止 SHALL 复用 StopRegistry 既有终止面，对已终态目标幂等忽略。

#### Scenario: 按归属反查会话集

- **WHEN** 某 change 的 implement#2 相位已发起 executor 与 evaluator 会话
- **THEN** 按 `source="change"` + `source_ref="<change>/implement/executor/2"` 等过滤查询可枚举对应会话及其运行态，派生出节点状态，无需任何额外注册表

#### Scenario: 转录可观测与停止

- **WHEN** executor 会话运行中用户请求停止
- **THEN** 该会话经 StopRegistry 收敛为 `stopped`，walker 收到终止终态并按停止收敛处理 run；对已终态会话的重复停止请求幂等忽略

### Requirement: 变更文件上下文降级 git diff

Desktop run 的 executor / evaluator prompt 组装 SHALL 以 git diff（工作区变更面）提供变更文件上下文（承接 file_log 在插件流程中「让 agent 识别变更文件」的既有用途）。Desktop run MUST NOT 写入 file_log：run 全程 workflow.json 的 `file_log` SHALL 零新增条目（不提取转录写路径、不调 `change-files`）。既有 skill 路径产生的 file_log 数据 SHALL 不受影响，其流程图节点挂载与图外面板显示规则不变。

#### Scenario: prompt 含变更文件上下文

- **WHEN** executor 会话发起前工作区存在未提交变更
- **THEN** 组装 prompt 含 git diff 变更文件上下文，executor 无需 file_log 即可识别在改文件

#### Scenario: run 不产生 file_log 条目

- **WHEN** 一次 run 走完若干相位后检查 workflow.json
- **THEN** `file_log` 相对 run 发起前零新增条目；run 之前由 skill 路径写入的既有条目原样保留且视图显示不变

### Requirement: static-check spawn 步门禁

implement / test-gen 相位的 executor 收口后，walker SHALL 必经 static-check 步（infra spawn 执行，承接原 SubagentStop hook 的门禁职责）。static-check 失败 SHALL 走**定向反馈边**：将捕获的诊断反馈注入同一 executor 会话（Continue）修复，独立计数上限 5 次（沿原 hook `loop_limit` 语义）；超限 SHALL 升格为相位 fail（walker 以桌面代写的 fail checklist 经写面 `phase_log` 落账，不跑 evaluator，进入重试 / 决策路径并消耗相位 retry 预算）。MUST NOT 将反馈边重试直接计入相位 retry 预算。非 implement / test-gen 相位 MUST NOT 触发 static-check。

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

Evaluator 会话 SHALL 依 prompt 约定在最终消息输出 checklist JSON（不自调 MCP `phase_log`）。Walker SHALL 解析该 JSON 并代调写面 `phase_log`（进程内）落 eval 记录；解析失败 SHALL 将运行停给用户（呈现原始输出与失败原因），MUST NOT 臆测 verdict、MUST NOT 静默跳过落账。checklist JSON 结构 SHALL 以插件既有 eval-checklist 形态为准（phase / attempt 信封 + 逐项 pass / evidence）。

#### Scenario: verdict 代写落账

- **WHEN** evaluator 最终消息输出合法 checklist JSON（verdict=pass）
- **THEN** walker 以该 checklist 经写面 `phase_log` 落 eval 记录（含 phase / attempt 信封），`phase_next` 随后推进

#### Scenario: 解析失败停给用户

- **WHEN** evaluator 最终消息不含可解析的 checklist JSON（结构漂移或空输出）
- **THEN** run 停止并呈现 evaluator 原始输出与解析失败原因，workflow.json 无新增 eval 记录，用户可显式重试该 attempt

### Requirement: 决策协议

写面 `phase_next` 返回重试上限（fail 且需跨相位跳转）时，walker SHALL 唤起决策 agent（compose_turn 新会话）；retry 预算内 SHALL NOT 唤起（walker 自走重试）。决策 agent 输入 SHALL 有界且来自 workflow.json / 写面响应：fail checklist（fail 项 + evidence）、`allowed_backtrack_phases`（写面 `phase_next` 白名单）、候选相位最近一次 eval report。决策输出 SHALL 为封闭集：

- `{ action: "backtrack", backtrack_to ∈ 白名单, reason ≤500 }` — 自治，walker 经写面 `backtrack` 执行；
- `{ action: "retry" }` — 同相位重试；
- `{ action: "stop", reason }` — 终止 run；
- `{ action: "ask", question, options[] }` — 无法裁决，UI 中断提问。

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验（工具侧兜底）：越权目标 SHALL 被拒绝且 run 停给用户，MUST NOT 写入越权回跳。决策输出解析失败按「verdict 解析失败」同款停给用户处理。

#### Scenario: 白名单内自治回跳

- **WHEN** test-execution#1 fail 达重试上限且决策 agent 输出 `{action:"backtrack", backtrack_to:"test-gen", reason:...}`（test-gen 在白名单内）
- **THEN** walker 经写面 `backtrack` 落回跳记录，`phase_next` 随后路由至 test-gen 重做，reason 随边标签可读

#### Scenario: 越权决议被兜底拦截

- **WHEN** 决策 agent 输出的 backtrack_to 不在 allowed_backtrack_phases 内
- **THEN** 写面 `backtrack` 二次校验拒绝（不写 workflow.json），run 停给用户并呈现越权详情

#### Scenario: ask 中断提问

- **WHEN** 决策 agent 输出 `{action:"ask", question:..., options:[...]}`
- **THEN** run 进入中断等待态，UI 呈现问题与选项；用户应答后 run 按应答继续（应答经运行控制命令面回流 walker）

### Requirement: 运行控制命令面

命令面 SHALL 提供 change run 的控制入口，命令体收拢为编排运行时公共 API 的薄包装（沿三件事纪律）：

- **发起**：按 change 名发起 run；沿 agent 执行先例提前 resolve（run 记录进入运行态后即返回），执行经运行状态 Channel 以同构状态部件流出；
- **停止**：终止当前 WorkerAgent 会话并收敛 run 为受控终态；对非运行态目标幂等忽略，MUST NOT 报错或误改既有终态；
- **应答**：ask 中断态下回传用户应答（选项或自由文本），驱动 run 继续；
- **确认**：phase 间停等点的用户确认（继续 / 终止）。

运行状态 Channel SHALL 与 agent 执行流同构（执行流通道例外，不属轮询取数）。发起前置校验 SHALL 覆盖：目标 change 存在且 workflow.json 可解析、无同 change 并行 run（CLI 可发现校验随子进程写通道消失）。

#### Scenario: 发起提前 resolve 与状态流

- **WHEN** 前端发起某 change 的 run
- **THEN** invoke 在 run 进入运行态后即 resolve，节点 / 相位状态变化随后经 Channel 持续流出，终态以同构部件收尾

#### Scenario: 停止幂等

- **WHEN** 对已收口（completed / stopped / failed）的 run 调停止命令
- **THEN** 幂等忽略：状态不变、不报错、不产生新事件

#### Scenario: 发起前置校验

- **WHEN** 对 workflow.json 损坏（unparsable）的 change 发起 run，或该 change 已有运行中的 run
- **THEN** 发起命令显式 `Err`（呈现损坏警示或并行冲突），不进入运行态

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **引擎恒 CLI**：change 执行会话恒走 claude code CLI 引擎（插件 dev-team 天然加载，protect-files 护栏过渡期依赖）；SDK 引擎 MUST NOT 承载 change 执行（无插件加载机制）。
2. **工作流类型**：仅 requirement 工作流全链；bug-fix / test-only 相位表后续独立 change。
3. **单图硬编码**：walker 只硬编码 walk 本相位循环图；MUST NOT 实现通用图引擎或图定义文件格式。
4. **file_log 不记录**：desktop run 不产生 file_log 条目（git diff 供变更上下文）为刻意边界；既有 skill 路径 file_log 数据显示不动。
5. **protect-files 过渡依赖**：步前实时护栏过渡期依赖插件 hook；插件真下线前需独立 change 补桌面原生护栏（内核 permission 层方向），本期仅留痕。

（原「sessionAnchors 精度损失」「sweep-phase 缺口」两条边界随写面进程内锚点复活而消失，不再留痕。）

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考，后续变更无需重新论证是否知情

#### Scenario: 引擎边界生效

- **WHEN** 审查 walker 发起 WorkerAgent 会话的引擎解析路径
- **THEN** 恒解析为 CLI 引擎（默认 agent 解析），无 SDK 分支；change run 发起不依赖 SDK 连接配置

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.3.15` 升级为 `0.4.0`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`：插件版本保持 `2.10.44`（工作树内 2.10.45 bump 与 walker-* 子命令面全部回退），三类交付产物（`claude-plugins/` / `cursor-plugins/` / `cursor-home-image/`）回退后 rebuild 刷新至与 2.10.44 源码一致。

#### Scenario: 版本号升级与插件回退

- **WHEN** 本变更实现完成
- **THEN** desktop version 为 0.4.0 且无 `src-tauri/Cargo.toml` 版本随动；dev-team 插件版本为 2.10.44，dist 产物中无 walker-* 痕迹

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration`（运行时域，tokio） | 薄图 walker：相位循环、三类节点执行、Gate 分支 | 红线：零相位转移规则（每步过渡问写面 phase-next）；ToolStep 节点化（不内联）；节点状态纯派生（workflow.json × provenance 反查）；模块内零进程 spawn（static-check spawn 经 port 缝下沉 infra）；依赖方向 orchestration → workflow |
| `crates/core/orchestration/src/port.rs` | 「相位机 + 工具步」进程内缝（语义换血） | ToolStepPort 进程内直调 core/workflow 写面；ToolCommand 封闭集收缩（ChangeFiles 出局）；WorkerAgentPort 契约不变；测试假引擎 + 假写面双缝直驱 |
| `crates/core/orchestration/src/verdict.rs` + `decision.rs` | 结构化输出解析 | 封闭 schema（checklist 信封 + 决策四动作封闭集）；解析失败显式失败停给用户，不臆测 |
| `crates/core/orchestration/src/prompt.rs` | prompt 组装与运行时插值 | 模板取自 core/workflow 相位表单源；运行时插值 git diff 变更文件上下文；executor 角色要点来自模板 |
| `crates/core/orchestration/src/transcript.rs`（砍半） | 会话转录消费 | 仅保留 `final_assistant_text`（verdict 提取）；`extract_write_paths` 出局 |
| `crates/infra/agent/src/worker.rs` | WorkerAgentPort 实现 + static-check spawn 缝 | compose_turn 新会话 + StopRegistry 终止 + 密封转录；provenance `source="change"`；spawn 不进 core |
| `src/commands/` change_flow 命令组 | 运行控制 IPC 面 | 发起（提前 resolve）/ 停止（幂等）/ 应答 / 确认 / 状态 Channel；三件事纪律薄包装；`Result<T, String>` 模板；前置校验无 CLI 可发现项 |
| `crates/core/workflow`（写面 + 读面，见 desktop-workflow-write-face） | workflow.json 域权威 | 读写同屋檐；walker 经进程内直调消费，不复制路由语义 |
| `crates/infra/store`（复用，不改） | 会话记录与反查 | `agentSessions` 按 source / sourceRef 过滤派生节点状态；不建 flow_runs 表 |
| `crates/infra/devteam` | **（删除）** | 整体删除（discover / runner），零遗留 |
| 既有 hooks（复用，不改） | 步前护栏与无外围账 | protect-files 原样生效（过渡期）；record-files / sweep-phase 未绑定 no-op（fail-open 刻意设计）；static-check 桌面模式由 spawn 步承接 |
