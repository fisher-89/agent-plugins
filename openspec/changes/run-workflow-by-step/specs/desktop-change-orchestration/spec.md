# desktop-change-orchestration Specification (Delta)

## MODIFIED Requirements

### Requirement: 薄图 walker 相位循环与路由红线

Desktop 后端 SHALL 提供编排运行时（walker，core/orchestration），对 change run 执行**计划图驱动的 step 循环**：每相位依序迭代其 `PhaseDefinition.steps`（`StepDefinition` 序列，kind = exec | agent | gate，定义见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」），不再在 `drive()` 内硬编码 ①→⑦ 步序。每步执行后 SHALL 问写面 `phase_next` 决定推进 / 重试 / 决策（路由权威不变）；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 两个门禁布局常量 SHALL 退役（门禁步由 step 定义声明，见「static-check spawn 步门禁」「计划图与执行图对齐与步粒度回退」）。

相位内 step 序列的三类特殊控制流 SHALL 以 step 级语义表达，MUST NOT 以 walker 内 step 类型特判实现：

- **feedback 边**（static-check / test-execution 失败 → 同一 agent step 修复 → 重跑）：失败步 `on_fail` 声明的定向反馈循环，独立计数 ≤5，超限升格相位 fail；每次重跑为 `instance_no` 递增的**新节点**，图上以 `feedback` 边（phase 内回边）表达；
- **verdict 解析失败**：终态失败停给用户，MUST NOT 臆测 verdict、MUST NOT 静默跳过；
- **phase 间 confirm 停等**：仍为 run 级 `auto_next_phase` 开关（phase 间自动、phase 内自动不变）。

红线保持：walker MUST NOT 持有任何相位转移规则——每步过渡 SHALL 问写面 `phase_next`；`PIPELINE_PHASES` 与后端 `detail.rs` 相位列 SHALL 保持纯布局身份，MUST NOT 上位为路由权威。重启 run SHALL 自 active_phase 续走（active_phase 持久化于 db `ChangeRecord`，已 pass 相位 MUST NOT 重头执行，续走由 `phase_next` 依 eval 历史重算）。写面全部落库载体为 workspace 库（desktop-workflow-write-face / desktop-change-state-store），walker 对载体无感知。

#### Scenario: 相位内计划步全循环自动走完

- **WHEN** 以假引擎（预录 AgentEvent 序列）+ 假写面（fake port，预录 phase-next / phase-start / phase-log 响应与 step 序列）驱动某相位 run
- **THEN** walker 依该相位 `PhaseDefinition.steps` 序逐 step 执行（exec / agent / gate 按声明），pass 后经写面 `phase_next` 推进下一相位，全程无人工干预

#### Scenario: test-execution 相位确定性承接

- **WHEN** walker 走到 test-execution 相位且 runner 结论 pass
- **THEN** 相位以 runner 门禁步承接（零 executor / evaluator 会话），机械 checklist 代写 `phase_log` 落账 pass 后推进；fail / error 走 feedback 边语义（见 desktop-test-execution）

#### Scenario: fail 重试与预算上限

- **WHEN** evaluator verdict 为 fail 且重试未超预算
- **THEN** walker 不唤决策 agent 自走重试同相位；连续 fail 达 5 次后停止自走并按决策协议分叉

#### Scenario: 中断续走不重头

- **WHEN** walker 于 implement#2 运行中被停止（或桌面重启）后重新发起该 change 的 run
- **THEN** walker 自 db `ChangeRecord.active_phase`=implement 的下一 attempt 续走，已 pass 的 proposal / dev-design 等相位不重新执行；中断相位经进程内锚点（sessionAnchors 复活）可感知并被标定

### Requirement: 三类节点统一 walker 与可观测均一

Walker SHALL 将每相位循环表达为三类节点（step 定义声明 kind）：

- **WorkerAgent 节点**（executor / evaluator / decision）：经 compose_turn 开新会话，转录可观测、可停止；
- **ToolStep 节点**（phase-start / phase-log / backtrack 相位机步进程内直调写面 + static-check / test-execution 检查域 spawn 步落 checks 边界）：无智能；
- **Gate 节点**（verdict 解析 / retry 计数 / 白名单校验）：纯 Rust 分支。

ToolStep MUST NOT 以命令式内联实现：其执行结果 SHALL 作为图上节点状态可观测（失败 = 可见失败态，与 WorkerAgent 失败同等呈现）。节点状态 SHALL 为派生视图 = **库读史（`RunRecord` / `RunNodeRecord` / `RunEdgeRecord`，desktop-change-state-store「run 执行图落库」）∪ 进程内注册表在飞 run（`RunEntry` 节点 + 边累积器）** × 按 provenance 反查的 session 集；执行图节点 SHALL 以稳定身份键 `(run_id, phase, attempt, step, instance_no)` 寻址（feedback 重跑为 `instance_no` 递增的新节点），边 SHALL 以 `RunEdgeRecord` 四值 `edge_kind ∈ { forward, feedback, retry, backtrack }` 显式落库（desktop-change-state-store），前端 `deriveEdges` 列差 Δ 派生 SHALL 退役、改读边表。步词汇 SHALL 为全词汇封闭集（agent 阶段 + 脚本阶段 + 流程面步骤 + 三门 gate），落库与回显同一份词汇；`RunStepRecord.seq` 与 `persisted_step` 10→5 过滤 SHALL 退役（desktop-change-state-store）。已收口 run 的任一节点状态与边 SHALL 可由库读史单独重算得出（重启 / 重挂后不依赖进程内存或 run 私有状态文件）。

#### Scenario: ToolStep 失败同等可见

- **WHEN** static-check 或 test-execution spawn 步以非零退出 / 显式 Err 收场（或写面相位机步返回错误）
- **THEN** 图上该节点呈失败态（与 executor 会话失败同等的可观测性），run 不静默越过该节点

#### Scenario: 节点与边两源派生可重算

- **WHEN** 审查编排运行时的持久化面与读路
- **THEN** runs / run_nodes / run_edges 三表在案（desktop-change-state-store）、在飞 run 节点与边驻进程内注册表（RunEntry）；已收口 run 的任一节点状态与边均可由库读史重算得出，不依赖进程内存

#### Scenario: feedback 边四值可辨

- **WHEN** 一次 run 含 phase 内 feedback（static-check 失败 → 修复 → 重跑）、phase fail 重试与跨站 backtrack
- **THEN** 执行图边表出现 feedback / retry / backtrack / forward 四 kind，各边显式落库；前端读边表呈现（feedback 为 phase 内回边、retry 为同 phase 重试、backtrack 为跨站回跳）

### Requirement: static-check spawn 步门禁

implement / test-gen 相位的 executor 收口后，walker SHALL 必经 static-check 步（checks 边界 `crates/infra/checks` spawn 执行，承接原 SubagentStop hook 的门禁职责；该步由相位 step 定义声明，非 `STATIC_CHECK_PHASES` 常量注入）。static-check 失败 SHALL 走**定向反馈边**：将捕获的诊断反馈注入同一 executor 会话（Continue）修复，独立计数上限 5 次（沿原 hook `loop_limit` 语义）；每次修复重跑为 `instance_no` 递增的新节点、图上以 `feedback` 边表达。超限 SHALL 升格为相位 fail（walker 以桌面代写的 fail checklist 经写面 `phase_log` 落账，不跑 evaluator，进入重试 / 决策路径并消耗相位 retry 预算）。MUST NOT 将反馈边重试直接计入相位 retry 预算。非 implement / test-gen 相位 MUST NOT 触发 static-check（其 step 序列不含 static-check 步）。

#### Scenario: 修复反馈边

- **WHEN** executor 收口后 static-check 报 lint / fmt 错误
- **THEN** walker 将诊断文本注入同一 executor 会话（Continue）重试修复，反馈边计数 +1，新修复节点 `instance_no` 递增；修复后 static-check 通过则相位继续 evaluator 步

#### Scenario: 反馈边超限升格

- **WHEN** 同一相位内定向反馈边已达 5 次仍不通过
- **THEN** walker 以代写 fail checklist 经写面 `phase_log` 落账（不跑 evaluator），相位按 fail 收场进入重试 / 决策路径，walker 不再注入反馈边

#### Scenario: 门禁相位限定

- **WHEN** proposal / dev-design 等非 implement / test-gen 相位 executor 收口
- **THEN** walker 不发起 static-check spawn（该相位 step 序列不含 static-check 步），直接进入 evaluator 步

### Requirement: run 落库写缝与累积器驻服务端

run 落库写缝 SHALL 收在 walker 侧单点：`walk_run` 终态出口（`guard.finish` 处）一次完成终态更新 + 节点整包 + 边整包 + active_phase 处置（同事务，desktop-change-state-store「run 执行图落库」「启动标定与 active_phase 悬挂处置」）；`ChangeFlowControl` SHALL 保持进程内零 IO 不变量（不持 store 句柄，store 写经 walker 侧 port / 命令层装配注入）。累积器 SHALL 移驻服务端并升级为**节点 + 边双累积**：`RunEntry`（注册表条目）自持节点列表（全词汇、emit 序、`instance_no` 分配）与边列表（feedback / retry / backtrack 显式落边），walker 经 `RunGuard::emit` 间达零变化；重挂快照 SHALL 含 nodes + edges（重挂恢复不再恒空）。启动装配 SHALL 执行启动标定（残留 running → interrupted，操作面见 desktop-change-state-store）。停止 / 会话失败 / verdict 解析失败等一切 run 死亡路径 SHALL 汇聚同一 finish 落包出口（终态出口唯一既有不变量）。

#### Scenario: 写缝单点与 control 零 IO

- **WHEN** 审查 walker 终态路径与 control 源码
- **THEN** 落包仅出现在 walk_run 终态出口一处；control 无 store 句柄、无 IO 调用（进程内纯状态不变量保持）

#### Scenario: 全死亡路径同一出口

- **WHEN** run 分别以 completed / stopped（用户停止）/ failed（会话失败、verdict 解析失败）收口
- **THEN** 三路径均经同一 finish 落包出口：终态 + reason + 节点整包 + 边整包 + active_phase 处置单事务落库，库读史与内存面终态一致

#### Scenario: 重挂恢复节点与边完整

- **WHEN** run 运行中（已 emit 多节点与 feedback 边）视图重挂后读取快照
- **THEN** 快照 nodes 与 edges 与已 emit 逐条一致（emit 序、`instance_no` 稳定），后续通知继续触发重读；不依赖前端累积

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **引擎恒 CLI**：change 执行会话恒走 claude code CLI 引擎（插件 dev-team 天然加载，protect-files 护栏过渡期依赖）；SDK 引擎 MUST NOT 承载 change 执行（无插件加载机制）。
2. **工作流类型**：仅 requirement 工作流全链；bug-fix / test-only 相位表后续独立 change。
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

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.34` 升级为 `0.4.35`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 `2.10.44` 与三类交付产物）。

#### Scenario: 版本号升级与插件零改动

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` 的 version 为 0.4.35；`plugins/dev-team` 版本与交付产物零改动

## ADDED Requirements

### Requirement: 计划图与执行图对齐与步粒度回退

计划图（源码，desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）与执行图（db，desktop-change-state-store「run 执行图落库」）SHALL 经 **step kind + phase + attempt + instance_no** 对齐：执行图节点是计划 step 的运行时实例，节点身份键 SHALL 为 `(run_id, phase, attempt, step, instance_no)`（`instance_no` 每次执行递增，feedback 重跑为独立新节点）；执行图边 SHALL 显式落 `RunEdgeRecord`，`edge_kind ∈ { forward, feedback, retry, backtrack }`——`forward` 计划序推进、`feedback` gate 失败回到同 phase agent 步的 step 级回边、`retry` phase fail 后同相位重跑、`backtrack` 跨相位回跳。计划图 MUST NOT 落库（定义恒为编译期静态）；执行图 MUST NOT 上位为路由权威（walker 过渡仍问 `phase_next`，不看执行图）。回退（backtrack）的语义单位 SHALL 保持 phase 粒度（`backtrack_to` 为相位名，stale 闭包按 `prerequisites` BFS 传播），其过程 SHALL 仍被表达为 gate + exec step 序列（WhitelistGate → RetryGate → Backtrack 命令）；本变更 MUST NOT 把回退载荷改为 step 粒度。

#### Scenario: 节点身份稳定且 feedback 独立

- **WHEN** 同 phase 同 attempt 内 static-check 失败触发 executor 修复重跑，随后 phase fail 重试该相位
- **THEN** executor 首次与修复各为 `instance_no` 递增的独立节点（同 phase / attempt / step），feedback 边指向前一节点；phase 重试产生新 attempt，retry 边指向新 attempt 首节点；各节点身份键可稳定反查计划 step

#### Scenario: 边四值与查询

- **WHEN** 审查一次含前进 / 反馈 / 重试 / 回跳的 run 执行图
- **THEN** `RunEdgeRecord` 四 kind 齐备且 `run_id` / `from_node_key` 两二级索引可达（按 run 全量读、按 from_node 读后继）；`backtrack` 边指向白名单内目标相位首节点

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/phase_table.rs` | 计划图单源 | `PhaseDefinition` 增 `steps: [StepDefinition]` 与 `artifact_manifest: [ArtifactSpec]`（const + `include_str!` 装配不变）；`prerequisites` 依赖边保留；prompt 全静态零占位红线不翻案 |
| `crates/core/workflow/src/write/phase_next.rs` | 计划步序列下发 | `phase_next` 响应携 next 相位 steps + manifest；路由权威不变 |
| `crates/core/orchestration/src/walker.rs` | 计划步通用执行 | `drive()` 迭代 phase steps；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 退役；feedback / verdict 分叉 / confirm 停等按 step 级语义；零自持转移规则 |
| `crates/core/orchestration/src/state.rs` | 步节点身份与边词汇 | 节点身份键 `(run_id, phase, attempt, step, instance_no)`；`edge_kind` 四值封闭集；`ChangeStepKind` 全词汇统一 |
| `crates/core/orchestration/src/run_history.rs` | 节点 + 边整包投影 | `persisted_step` 10→5 过滤退役；`RunNodeRecord` / `RunEdgeRecord` 载荷投影 |
| `crates/core/orchestration/src/control.rs` | 节点 + 边累积器 | `RunEntry` 自持 nodes + edges（emit 序、`instance_no` 分配）；快照含 nodes + edges；进程内零 IO 不变量保持 |
| `crates/infra/store` | 执行图落库 | `RunNodeRecord` / `RunEdgeRecord` + 边表二级索引（见 desktop-change-state-store） |
| `src/commands/change_flow/mod.rs` | 通知桥与 DTO 装配 | nodes / edges DTO 随 shape 演进；启动标定装配；stop / confirm / answer 请求-应答不变 |
| `crates/infra/exec` | 输出记录单点 | `ExecOutput::Transient` / `Recorded { exec_id }`（见 desktop-checks-domain）；spawn 捕获与回查统一 |
