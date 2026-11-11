# desktop-workflow-write-face Specification

## Purpose

core/workflow 保持 change 流程状态的 Rust 单一权威（读 + 写），持久化载体为 workspace 库（desktop-change-state-store，落库经 port 缝）；相位表与 prompt 模板单源驻 core/workflow，walker 经进程内直调消费，desktop 全链不读不写 workflow.json（双向墙）。

## Requirements

### Requirement: workflow.json 写面进程内单权威

core/workflow SHALL 保持 change 流程状态的 Rust 单一权威（读 + 写），持久化载体 SHALL 由 workflow.json 重定义为 workspace 库（desktop-change-state-store；落库经 port 缝由壳层装配 store 实现，core/workflow 零 infra 依赖）。写面 SHALL 提供：

- `phase_next`：相位路由状态机——返回 next_phase、已插值的 executor / evaluator prompt、`allowed_backtrack_phases` 白名单（项 = 相位 id + 表内特性描述 + 该相位声明的可操作产物清单，`backtrack_options` 自相位表 `description` / `artifact_manifest` 随行下发；评估 checklist 非声明产物，恒不在清单内）、重试上限判定；**决策分叉错误响应 SHALL 同样携该失败相位的 `backtrack_options` 白名单**（与正常路由同一推导单点；MUST NOT 留空等消费方自备，MUST NOT 依赖消费方跨轮缓存——run 首轮即分叉时无「上一站」可缓存）；`sessionAnchors` 以进程内状态复活（mid-phase interruption 分支可达，重入时中断相位可感知并标定）；
- `phase_start`：开启阶段并 attempt 计时；
- `phase_log`：eval 记录落账（checklist 信封 + skipped + 会话槽位，见「eval 条目会话槽位落账」），单事务原子落库；
- `backtrack`：回跳落账（stale 标记 + 传播 + reason ≤1000）；
- `archive`：归档双写（db status 翻转 + 目录改名，见 desktop-change-state-store「归档双写」）。

状态读取 SHALL 单源自 workspace 库（ChangeRecord / PhaseRecord / ChecklistItemRecord）；desktop MUST NOT 读或写任何 workflow.json。字段演进 SHALL 经 native_model 版本机制治理，MUST NOT 再依赖 JSON 文件形状保形（`persist.rs` raw Value 保形改写随载体退役）。写面 SHALL 保持 sync（store 为同步操作面，写面 MUST NOT 引入 tokio / async runtime）。V1 仅移植 requirement 工作流相位表；bug-fix / test-only 相位表后续独立变更。

#### Scenario: 写面全操作进程内可达

- **WHEN** 命令层不经 walker、不起任何子进程，直接调用写面的 `phase_next` / `phase_start` / `phase_log` / `backtrack` / `archive`
- **THEN** 各操作在进程内完成状态变更并落 workspace 库，无 CLI 子进程、无网络调用、无 workflow.json 读写

#### Scenario: 写后状态库一致可读

- **WHEN** 写面完成一次 `phase_start` + `phase_log` + `backtrack` 序列后经查询层读取该 change
- **THEN** PhaseRecord / ChecklistItemRecord / active_phase / stale 标记与写面返回语义逐项一致，状态完全来自 db

#### Scenario: 落库经 port 缝零 infra 依赖

- **WHEN** 审查 `crates/core/workflow/Cargo.toml` 与写面落库调用点
- **THEN** 零 infra/store 直接依赖；落库经 port 缝（trait）由 desktop-app 装配的 store 实现承载

#### Scenario: sync 边界保持

- **WHEN** 审查 core/workflow 写面的依赖引入
- **THEN** 无 tokio / async runtime 依赖，写面函数为同步签名；既有读路径消费方不因写面引入运行时重编译

### Requirement: eval 条目会话槽位落账

`phase_log` 写操作 SHALL 支持随行落会话槽位：落账输入（`PhaseLogInput`）SHALL 增加可选的 executor / evaluator / decision 会话 id 槽位；落账时 SHALL 仅将显式在位的槽位写入该 PhaseRecord 的三槽位列（缺省槽位列以 None 呈现，沿磁盘模型可选字段先例的显式在位纪律），verdict 条目随行携带 executor / evaluator 槽位、fail 升格条目同构处理。槽位值由调用方（walker）从 `WorkerTurnOutcome.session_id` 取值传入，写面 MUST NOT 自行解释或改写槽位值。decision 槽位的写入时机（decision 会话产生于 fail 条目落账之后：backtrack 写挂 / 显式 amend 写面操作 / 随下一 attempt 条目）由 design 定稿，本需求约束的是槽位落库的载体与显式在位纪律，不预设时机方案。槽位列 SHALL 与 `SessionRecord` 同库（join 一等查询，见 desktop-change-state-store）。

#### Scenario: 槽位随行落库

- **WHEN** walker 以携带 executor / evaluator session id 的 `PhaseLogInput` 调用写面 `phase_log`
- **THEN** 落库的 PhaseRecord 三槽位列携带对应会话 id，attempt 推导与 verdict 推导语义不受影响

#### Scenario: 缺省槽位列 None

- **WHEN** 以不含会话槽位的 `PhaseLogInput` 调用写面 `phase_log`（或调用方未传）
- **THEN** 落库条目对应槽位列为 None，查询层以 null 透出，不虚构值

#### Scenario: decision 槽位在案可查

- **WHEN** decision 会话按 design 定稿的时机落挂后查询该 phase 的 PhaseRecord
- **THEN** 对应条目的 decision 槽位列可查得该会话 id，且与其 executor / evaluator 槽位同驻记录（同 attempt），库内可 join 会话转录

### Requirement: backtrack 白名单二次校验与 stale 语义

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验：`backtrack_to` 不在 `allowed_backtrack_phases` 内 SHALL 被拒绝且不落任何账（越权决议损坏不了状态，状态库零变更）；校验通过 SHALL 落回跳记录、对被跳过相位的 PhaseRecord 打 stale 标记并传播（与回跳落账同一事务语义），`reason` 超过 1000 字符 SHALL 被拒绝。`backtrack_to` 与发起相位相同（自环）SHALL 在白名单含该相位时正常受理——这是「重做本相位」的唯一载体（白名单自含规则见「相位定义有序 step 序列与 artifact 清单」）：落账语义为改写最新条目 `backtrack_to` + 目标相位下游 stale 传播，`phase_next` 随后以新 attempt 路由回该相位；白名单不含自身时（零 agent 步相位）自环 SHALL 被拒且状态库零变更。该口径为桌面写面自有裁定：MUST NOT 沿用「目标索引必须小于当前相位」类旧约束——承载该约束的对偶 spec 已随插件退役整目录删除，桌面路径只受本条白名单规则约束，回跳合法性的唯一判据是 `allowed_backtrack_phases`。

一次成功回溯 SHALL 恰好产出一个**回溯窗口记录**，由既有条目派生、零新表 / 零新列 / 零新时钟：记录形状 = { `from` = 标记行自身相位（发起相位，泳道位置靠后，区间上界）、`to` = 标记行 `backtrack_to`（目标相位，泳道位置靠前，区间下界）、`reason` = 标记行 `backtrack_reason` 原文逐字、`order` = 标记行 `PhaseStateRecord.id`（落账序）}。标记**不是新增行**，而是改写「发起相位最新条目」的 `backtrack_to` / `backtrack_reason` ⇒ 窗口不引入持久化载体，MUST NOT 为此新增列（如决策时刻列 / 窗口号列）或另建窗口表。`order` 取 `id` 的充分性由发起相位恒为「刚失败的相位」保证：其 fail 条目是决策前最后一次落账，故标记行 `id` 在每次回溯时都是全库最新，多个标记天然按决策先后递增。

#### Scenario: 白名单内回跳落账

- **WHEN** 以 `backtrack_to="test-gen"`（白名单内）、合法 reason 调用写面 backtrack
- **THEN** 回跳记录落库、目标相位可被 `phase_next` 路由至、被跳过相位的 PhaseRecord 带 stale 标记

#### Scenario: 目标为自身即重做本相位

- **WHEN** 以 `backtrack_to` 等于发起相位（如 `phase="implement"` / `to="implement"`，白名单含自身）调用写面 backtrack
- **THEN** 回跳落库成功、`phase_next` 随后路由回 implement 的新 attempt（重做本相位）；以 `phase="test-execution"` / `to="test-execution"`（白名单不含自身）调用时显式失败且状态库零变更

#### Scenario: 越权目标被拒绝

- **WHEN** 以 `backtrack_to` 不在 `allowed_backtrack_phases` 内调用写面 backtrack
- **THEN** 调用显式失败（错误抵达调用方），状态库零变更（无新行、无 stale 翻转、active_phase 不动）

#### Scenario: reason 超长被拒绝

- **WHEN** 以超过 1000 字符的 reason 调用写面 backtrack
- **THEN** 调用显式失败，状态库零变更

#### Scenario: 自环裁定单源自持

- **WHEN** 审查桌面写面 `backtrack` 的合法性判据
- **THEN** 判据只有 `allowed_backtrack_phases` 白名单（含自环三态：白名单内受理 / 不含自身拒绝 / 越权拒绝零落账）
- **AND** 无「与插件侧 spec 各管其面 / 插件零改动」类对偶条款或注释（对偶 spec 已删除）

#### Scenario: 窗口记录零持久化载体

- **WHEN** 对一次成功回溯审查落库形态与 `PhaseStateRecord` 字段面
- **THEN** 状态库新增的仅为「发起相位最新条目的 `backtrack_to` / `backtrack_reason` 改写 + stale 翻转」，无新表、无新列、无窗口号字段；窗口的四元 {`from`, `to`, `reason`, `order`} 全部可从既有条目直接读出

#### Scenario: 多个标记按决策先后递增

- **WHEN** 先后两次回溯（发起相位不同）落账后比较两条标记行的 `id`
- **THEN** 后一次回溯的标记行 `id` 更大（发起相位是各自时刻的刚失败相位，其 fail 条目为决策前最后一次落账）

### Requirement: 相位表与 prompt 模板单源收敛

工作流相位表（相位序、executor / evaluator prompt 模板、**有序 step 序列、artifact 清单、前置依赖、回溯资格**）SHALL 单源驻 core/workflow：prompt 模板 SHALL 为 `core/workflow/src/prompts/` 全静态角色知识 md，经 `phase_table.rs` 编译期 `include_str!("prompts/<role>.md")` 显式字面路径装配，MUST NOT 按 `<role>` 动态读取；step 序列与 artifact 清单 SHALL 以 `PhaseDefinition.steps` / `PhaseDefinition.artifact_manifest` 编译期 const 表承载，MUST NOT 落库、MUST NOT 经 port 缝运行时加载。`phase_table(workflow_type)` SHALL 按类型分派：`"requirement"` → `REQUIREMENT_TABLE`（10 个路由相位：initialize / proposal / dev-design / test-design / implement / test-gen / test-execution / code-review / acceptance / archive）、`"test-only"` → `TEST_ONLY_TABLE`（7 个路由相位：initialize / proposal / code-analyze / test-design / test-gen / test-execution / archive），其余（含空白 / 未知 / bug-fix）→ `None`（**不退化**）。initialize 为两表共有的**表头一次性基线检查相位**（确定性相位：`executor=None` / `evaluator=None`，`INITIALIZE_STEPS` 见「相位定义有序 step 序列与 artifact 清单」，执行语义见 desktop-change-orchestration「initialize 基线检查相位执行体」）；archive 为两表共有的确定性末相位（`PhaseDefinition.executor=None` / `evaluator=None`，11 步序列见「相位定义有序 step 序列与 artifact 清单」）。walker MUST NOT 自持相位转移规则（每步过渡问写面 `phase_next`）；`phase_next` SHALL 覆盖各表末相位（requirement 9 个前序路由相位（initialize + 8）pass 后 next=archive，test-only 6 个前序路由相位（initialize + 5）pass 后 next=archive，archive 空 checklist 落账后 done）。`detail.rs` 相位列保持纯布局身份、列序随 `phase_table` 出线（见 desktop-change-queries），MUST NOT 上位为路由权威；TS 侧相位表冻结，不再是桌面路径依赖。

#### Scenario: walker 不自持转移规则

- **WHEN** 审查 orchestration walker 源码对相位先后关系的编码
- **THEN** 零硬编码相位转移表；walker 每步过渡均以写面 `phase_next` 的返回为准（含 initialize 表头路由与 archive → done）

#### Scenario: 相位表单一来源

- **WHEN** 同一 workflow_type 的相位序、step 序列、artifact 清单与前置依赖在 core/workflow 与 walker 两侧分别查阅
- **THEN** 仅 core/workflow 相位表定义相位序、step 序列、artifact 清单与前置依赖（requirement 10 相位、test-only 7 相位均在案，两表表头均为一次性 initialize）；walker 侧无第二份定义
- **AND** prompt 模板单源 = `prompts/*.md` 静态文件，经 `include_str!` 装配，无第二份 prompt 文本

#### Scenario: 两值分派与未知类型不退化

- **WHEN** 分别以 `"requirement"` / `"test-only"` / `"bug-fix"` / `""` 调用 `phase_table`
- **THEN** 前两者返回对应 `Some` 表（10 / 7 相位），后两者返回 `None`，零 fallback、零 requirement 退化

### Requirement: phase_log 回跳后重评条目修复

写面 `phase_log` SHALL 修复插件侧已知缺陷（backtrack 后同相位重评时 eval 条目被跳过丢失）：backtrack 回跳后的同相位每一次重评 SHALL 全部落账，不因该相位已有 pass 记录而跳过；插件侧 TS 实现冻结不修。

#### Scenario: 回跳后重评条目齐全

- **WHEN** test-gen backtrack 回跳后，同相位完成第二次评估并以写面 `phase_log` 落账
- **THEN** 第二次 eval 条目完整落账（含 checklist 与时间戳），与首次条目并存，eval 序列无丢失

### Requirement: 等价性对照验收口径

写面以**对照功能验收**为准：逐项对照既往 desktop 直跑形态下 `phase_next` / `phase_start` / `phase_log` / `backtrack` 的行为语义验收（路由结果、重试上限、白名单下发、attempt 计时、eval 落账、stale 传播）；MUST NOT 建差分 oracle（双跑自动比对）。长期单实现，接受微小行为漂移。workflow.json schema 兼容命题随双向墙退役（desktop 与插件工作流不再共写共享文件）；状态语义演进以本 spec 与 desktop-change-state-store 为准。

#### Scenario: 对照验收清单逐项通过

- **WHEN** 以同一 change 状态分别经既往 desktop 形态与本写面推进等价相位序列
- **THEN** 对照清单逐项核验一致（active_phase 演进、eval 条目语义、白名单内容、stale 标记），差异仅限既知可接受面（时间戳值）

#### Scenario: 状态语义漂移显式捕获

- **WHEN** 写面重构后重放既有相位机行为测试集（路由 / 重试 / 白名单 / stale）
- **THEN** 测试全绿；语义变化必须以 spec delta 显式提出而非实现静默漂移

### Requirement: 写面独立可达

写面 SHALL 为命令层可达的独立能力：不经 walker run 即可直调（单次 `backtrack` / `phase_log` / `phase_start` 调用不要求拉起编排循环），为后续 UI「手动 backtrack / 重试」留口。

#### Scenario: 免编排单点调用

- **WHEN** 命令层直接调用写面完成一次 backtrack（无 walker run 在场）
- **THEN** 回跳落账成功，无需创建编排会话或 run 状态

### Requirement: PhaseAgentSpec 静态化与通配符退役

`PhaseAgentSpec` SHALL 由 `{ agent_type: String, prompt: String, model_level }` 收敛为 `{ prompt: &'static str, model_level }`：`agent_type` 字段随 `__CALL_AGENT:<role>__` 令牌退役删除；prompt SHALL 为编译期静态文本（`include_str!("prompts/<role>.md")` 产物，见 desktop-phase-prompts）。写面 SHALL 删除 `interpolate`（`<change>` / `<phase>` 占位符插值）；`phase_next` 的 `build_phase_response` SHALL 以静态 prompt + 上下文头 `change: <name>`（append）+ 回溯重做窗口段（append，见「回溯重做窗口推导与 prompt 携带」）组装，MUST NOT 做模板替换。回溯场景 MUST NOT 再以「单行回溯原因」形式追加（该行已由窗口段完全取代，单点渲染）。desktop core（workflow + orchestration）SHALL 零残留 `__CALL_AGENT` / `interpolate` / `strip_call_agent` / `role_brief` / `<change>` / `<phase>`。

#### Scenario: 结构收敛与令牌退役

- **WHEN** 审查 `PhaseAgentSpec` 定义与 `phase_table` 表条目
- **THEN** 结构仅含 `prompt: &'static str` 与 `model_level`；无 `agent_type` 字段；无 `__CALL_AGENT` 字样
- **AND** 相位序 / model_level / 白名单 / 前置依赖语义不变

#### Scenario: 无模板替换

- **WHEN** `phase_next` 下发某相位 executor / evaluator prompt
- **THEN** prompt 主体为静态 md 文本，仅 append 上下文头 `change: <name>` 与（窗口命中时）回溯重做窗口段
- **AND** 无 `<change>` / `<phase>` 替换行为

#### Scenario: 删除面零残留

- **WHEN** 全仓扫描 desktop core（workflow / orchestration）源码
- **THEN** 无 `__CALL_AGENT` / `interpolate` / `strip_call_agent` / `role_brief` / `<change>` / `<phase>` 残留

#### Scenario: 单行回溯原因行零残留

- **WHEN** 全仓扫描 desktop core（workflow / orchestration）产品源码中的回溯原因渲染点
- **THEN** 无 `⚠️ 回溯原因` 字样、无 `reason_suffix` 函数；`build_phase_response` 亦无 `backtrack_reason` 一次性入参（窗口由响应内的窗口集合统一供给，回溯分支与正常分支入参收敛为同一形）

### Requirement: 相位定义有序 step 序列与 artifact 清单

`PhaseDefinition` SHALL 扩展为计划图节点与产物清单的单源载体：

- `steps: [StepDefinition]`：相位内有序 step 序列；`StepDefinition` SHALL 携 `kind`（`exec` | `agent` | `gate` | `git` 四类封闭集，经 `StepNode::group()` 派生）、`role`（agent 步角色）、`prompt_ref`（指向 `include_str!` 静态 prompt 的引用键，非 agent 步可缺省）、`on_fail` 语义三变体（`FeedbackLoop { limit }` 定向反馈循环、`Terminal` 终态失败停给用户、`Escalate` 零会话落账升格交决策）；
- `prerequisites: &'static [&'static str]`：相位直接前置依赖（表内自描述，stale 传播与 allowed_backtrack 推导的唯一事实源；MUST NOT 存在第二份独立 match）；
- `backtrackable: bool`：相位回溯资格（表内自描述，沿 `prerequisites` 先例；`allowed_backtrack_phases` SHALL 按本属性过滤——`backtrackable=false` 的相位 MUST NOT 出现在任何相位的回溯白名单中，表头 initialize 恒 false（基线检查不可作为回溯目标），过滤 MUST NOT 以「跳过 idx==0」类位置推导实现——表结构变化即漂移）；白名单 SHALL 另按「可重做」裁定自身：`allowed_backtrack_phases` 中「当前相位自身在列」⟺ 该相位可重做（其 `steps` 含 `role()` 为 `Some` 的 agent 步）；零 agent 步的相位（test-execution / archive）自身 MUST NOT 入白名单——决策 agent 上下文里不存在「重做本相位」这个选项，判断 MUST NOT 以相位名硬编码；
- `artifact_manifest: [ArtifactSpec]`：相位声明产物清单；archive 相位 SHALL 无 `artifact_manifest`（不产文档）；requirement 表 test-design SHALL 声明 `markdown-doc·test-design.md`、implement SHALL 声明 `tasks-progress·tasks.md`（实现期进度视图，服务 phase_next 下发面——发现面同路径按首命中去重，tasks.md 出列为 dev-design 的 markdown-doc 单候选）；test-only 表 code-analyze SHALL 声明 `markdown-doc·design.md`（源码分析）；
- `executor` / `evaluator: Option<PhaseAgentSpec>`：initialize / test-execution / archive 三个确定性相位 SHALL 为 `None` / `None`。test-execution 的角色 spec 缺席后，其「首败即交决策」语义 SHALL 由门禁步声明 `OnFail::Escalate` 承载——`phase_next` 的决策分叉判据 SHALL 取「`evaluator.is_some()` 或 `steps` 含 `OnFail::Escalate` 步」，MUST NOT 以相位名硬编码，且在两表上须与原 `evaluator.is_some()` 单判据的相位集合等价。

`StepNode` 词汇 SHALL 为二十二值（`executor` / `evaluator` / `decision` / `static_check` / `test_execution` / `mutation_test` / `archi_check` / `clean` / `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` / `preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `cleanup` / `seal` / `finalize`），与 `RunStepKind` / `ChangeStepKind` 同词面 1:1 全映射（`mutation_test`、`cleanup`、`clean` 与 `archi_check` 线词入封闭集，18→19、19→20、20→21 与 21→22 均为白名单式扩展）；code-analyze 复用既有 `Executor` / `Evaluator` / `VerdictGate` / `PhaseStart` / `PhaseLog` 步词，MUST NOT 新增步词。`StepGroup` SHALL 保持四分类（`Exec` / `Agent` / `Gate` / `Git`），`mutation_test` 与 `archi_check` 归 `Exec`、`cleanup` 与 `clean` 归 `Git`。两表表头 SHALL 声明共享 `INITIALIZE_STEPS`：`PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`（一次性基线检查相位——Clean 为 vcs 直调基线恢复步、StaticCheck / TestExecution 步 `on_fail=Terminal`（失败即停，无反馈边——基线的债不属于本次变更）、`PhaseStart` / `Clean` / `PhaseLog` `on_fail=None`；零 `artifact_manifest`——基线报告树非文档素材，回查经节点 `output_ref`）。两表共享的 `TEST_EXECUTION_STEPS` SHALL 为 `PhaseStart → TestExecution → MutationTest → PhaseLog` 四步：TestExecution / MutationTest 两步 `on_fail=Escalate`（零 WorkerAgent 会话——首败即落账后回 `phase_next` 交决策面，见 desktop-test-execution「失败即升格交决策（零 agent 修复）」与 desktop-mutation-testing「失败升格交决策（报告指引式信息面）」），`PhaseStart` / `PhaseLog` `on_fail=None`。requirement 表的 implement `PhaseDefinition` SHALL 声明 7 步序列：`PhaseStart → Executor → StaticCheck(FeedbackLoop{5}) → ArchiCheck(FeedbackLoop{3}) → Evaluator → VerdictGate(FeedbackLoop{5}) → PhaseLog`——`ArchiCheck` 步 MUST 位于 static-check 通过之后、evaluator 之前（先修机器能判的，再让评估面看已过门禁的产物），其 `on_fail` SHALL 为 `FeedbackLoop { limit: 3 }`（架构违例的反馈边上限独立于 static-check 的 5 次与 verdict 的 5 次，三段计数互不占用；见 desktop-change-orchestration「ArchiCheck 步接线与反馈边」）。同一步词在不同表项语义不同（initialize 表项的 TestExecution 为 `Terminal`、test-execution 表项为 `Escalate`），语义由表项声明承载，MUST NOT 由相位名推导；`Escalate` 归属 SHALL 由表测试守卫（只允许出现在 TestExecution / MutationTest 步上——四个执行体只按 `on_fail` 分派，误声明会静默失效）。`REQUIREMENT_TABLE` 的 archive `PhaseDefinition` SHALL 声明 11 步序列：`PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize → PhaseLog`（`ResolveConflicts` 为条件执行步，`Cleanup` 为尽力步——清理失败仅 step detail 警告、步恒 `passed`，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」；其余步 `on_fail=None`——步失败即 Terminal）。`TEST_ONLY_TABLE` SHALL 复用 requirement 同名 step 静态（proposal / test-design / test-gen / test-execution / archive），并新增 `CODE_ANALYZE_STEPS`：`PhaseStart → Executor → Evaluator → VerdictGate → PhaseLog`（P→E，`on_fail=Terminal` 于 VerdictGate）；`ArchiCheck` 步 MUST NOT 出现在 `TEST_ONLY_TABLE` 的任何相位（该表无 implement 相位，代码改动落在 code-analyze；且测试文件被架构扫描全排除，挂上只会空转）。

`steps` SHALL 全静态声明（const 表内联，`prompt_ref` 经编译期 `include_str!` 映射回静态 prompt 表）；MUST NOT 将 prompt 文本收进 db 定义表、MUST NOT 经 port 缝做运行时定义加载。`artifact_manifest` 为「相对路径 → phase」归属的权威来源，前端 `docColumn` 静态映射退役后 MUST NOT 在 core/workflow 之外再保留第二份 phase 归属表。

#### Scenario: 步序列全静态声明

- **WHEN** 审查 `phase_table.rs` 的 `PhaseDefinition` 表条目（含 initialize、implement、archive、test-execution 与 code-analyze）
- **THEN** 每相位 `steps` 为编译期 const 有序序列（kind / role / prompt_ref / on_fail 齐备），initialize 相位 5 步在案（表头 `INITIALIZE_STEPS` 两表共享）且 `executor=None` / `evaluator=None` / `backtrackable=false`，implement 相位 7 步在案（`ArchiCheck` 位于 StaticCheck 与 Evaluator 之间、`on_fail=FeedbackLoop{3}`），archive 相位 11 步在案且 `executor=None` / `evaluator=None`，test-execution 4 步在案（TestExecution / MutationTest 两步 `on_fail=Escalate`），code-analyze 5 步在案（P→E）；无 db 定义表、无 port 缝定义读、无运行时文件 IO

#### Scenario: StepNode 二十二值与四分类分组

- **WHEN** 审查 `StepNode` / `RunStepKind` / `StepGroup` 定义
- **THEN** 词汇二十二值全映射（9 个归档步词（含 `cleanup`）、`mutation_test`、`archi_check` 与 initialize 基线步词 `clean` 在案，code-analyze 零新步词）；`StepGroup` 含 Git，commit / rebase / merge / cleanup / finalize / clean 归 Git、preflight / seal / mutation_test / archi_check 归 Exec、spec_sync / resolve_conflicts 归 Agent

#### Scenario: 架构门禁步只在 implement

- **WHEN** 遍历 requirement 表与 test-only 表全部相位的 `steps`
- **THEN** `ArchiCheck` 步恰在 requirement 表的 implement 相位出现一次（序在其 StaticCheck 之后、Evaluator 之前）；acceptance / initialize / archive / test-gen / test-execution / test-only 全表零 `ArchiCheck` 步

#### Scenario: 回溯白名单按 backtrackable 过滤

- **WHEN** 对两表任一非表头相位调用 `allowed_backtrack_phases`
- **THEN** 白名单为其前序相位（含自身）中 `backtrackable=true` 者的集合，initialize 恒不在白名单（`backtrackable=false`）；过滤依据为表内显式属性，无「跳过 idx==0」位置推导；自身是否在列另由「可重做」裁定（见下条 scenario）

#### Scenario: 白名单自含即重做、零 agent 步自排除

- **WHEN** 分别对 implement 与 test-execution 调用 `allowed_backtrack_phases`
- **THEN** implement 的白名单含 implement 自身（`steps` 含 agent 步 ⇒ 可重做，「重做本相位」经回溯到自身表达）；test-execution 的白名单不含自身（`steps` 全为确定性步 ⇒ 不可重做，决策上下文无「重做本相位」选项），其余前序 `backtrackable` 相位照常在列

#### Scenario: 决策分叉响应携白名单

- **WHEN** `phase_next` 以 `VerdictFail` / `MaxRetriesExceeded` 返回决策分叉错误，且该轮是本 run 的第一次调用（无任何前置正常路由响应）
- **THEN** 错误响应的 `allowed_backtrack_phases` 为该失败相位的 `backtrack_options`（非空、与正常路由同一推导单点，含「可重做」自排除语义）；消费方据此组装的决策输入白名单与写面二次校验载荷均非空，backtrack 决议可正常兑现——MUST NOT 返回空集要求消费方自行缓存前序响应

#### Scenario: artifact 清单权威归属

- **WHEN** 查阅 requirement 与 test-only 工作流各相位的 `artifact_manifest`
- **THEN** proposal.md / specs/** 属 proposal、design.md / tasks.md 属 dev-design、test-design.md 属 test-design、test-reports/** 属 test-execution（以清单声明为准）；implement 声明 `tasks-progress·tasks.md`（下发面，发现面同路径首命中去重不重复出列）；test-only 下 code-analyze 声明 design.md、test-design 声明 test-design.md；initialize 与 archive 相位零 artifact_manifest；清单之外文档不挂 phase 列

#### Scenario: PhaseAgentSpec 静态化不翻案

- **WHEN** 审查 `PhaseDefinition` 扩展后的 executor / evaluator spec
- **THEN** 仍为 `{ prompt: &'static str, model_level }`（`include_str!` 产物），无 `agent_type` / `__CALL_AGENT__` / `interpolate` 残留；initialize / test-execution / archive 三确定性相位 executor / evaluator 均 None

### Requirement: test-only 相位表与前置依赖

`TEST_ONLY_TABLE` SHALL 为 7 个路由相位，序固定：`initialize → proposal → code-analyze → test-design → test-gen → test-execution → archive`。每相位的前置依赖 SHALL 为：

| phase | prerequisites |
|-------|---------------|
| initialize | [] |
| proposal | [] |
| code-analyze | [proposal] |
| test-design | [proposal, code-analyze] |
| test-gen | [test-design] |
| test-execution | [test-gen] |
| archive | [] |

step 复用 SHALL 为：initialize 复用 requirement 表头 `INITIALIZE_STEPS`（两表共享同一 static），proposal / test-design / test-gen 复用 requirement 同名 `*_STEPS`（含同名 executor / evaluator prompt），test-execution 复用 `TEST_EXECUTION_STEPS`（确定性门禁 `[PhaseStart, TestExecution, MutationTest, PhaseLog]`——拆步：测试 / 覆盖率修绿后才跑突变，TestExecution / MutationTest 两步 `on_fail=Escalate`（零 agent、首败落账交决策），不照搬插件侧 Executor→E），archive 复用 `ARCHIVE_STEPS`，code-analyze 用新增 `CODE_ANALYZE_STEPS`。test-only 的 `proposal` 描述 SHALL 为「测试向提案」（description 字段语义，prompt 主体暂复用 requirement proposal prompt，是否拆分独立 prompt 留待看效果）。

#### Scenario: test-only 七相位序与复用

- **WHEN** 读取 `phase_table("test-only")` 的相位序与各相位 `steps`
- **THEN** 序恰为 initialize / proposal / code-analyze / test-design / test-gen / test-execution / archive；initialize / proposal / test-design / test-gen / test-execution / archive 的 `steps` 指针与 requirement 同名 static 一致，code-analyze 为 `CODE_ANALYZE_STEPS`

#### Scenario: 前置依赖表内自描述

- **WHEN** 读取 `TEST_ONLY_TABLE` 各 `PhaseDefinition.prerequisites`
- **THEN** initialize=[]、code-analyze=[proposal]、test-design=[proposal, code-analyze]、test-gen=[test-design]、test-execution=[test-gen]、proposal/archive=[]；`dependents(TEST_ONLY_TABLE, "proposal")` 含 code-analyze 与 test-design

### Requirement: 写面严格类型分派与未知类型显式拒绝

`phase_next` / `phase_start` / `phase_log` / `backtrack` SHALL 以 `phase_table(record.workflow_type)` 的返回为唯一分派依据：返回 `None`（未知 / 空白 / bug-fix 等不受支持类型）SHALL 显式 `Err`（错误文案通吃 `{requirement, test-only}`，MUST NOT 再写死「V1 仅 requirement」），状态库零变更。写面 MUST NOT 对未知类型做 requirement 退化。存量 `ChangeRecord.workflow_type` 恒为 `"requirement"`（写面 V1 起硬编码），严格分派后存量记录天然合法，零迁移。

#### Scenario: 未知类型显式拒绝零副作用

- **WHEN** 对 `workflow_type="bug-fix"`（或空白）的 change 调用 `phase_next` / `phase_start` / `phase_log` / `backtrack`
- **THEN** 每次均显式 `Err`（文案含 `{requirement, test-only}` 支持面），状态库零变更、零 fallback requirement

#### Scenario: test-only 分派可走通

- **WHEN** 对 `workflow_type="test-only"` 的 change 依次走 `phase_next`（proposal → code-analyze → … → archive → done）
- **THEN** 相位推进严格按 `TEST_ONLY_TABLE` 序，backtrack stale 传播按表内 `prerequisites` 正确、与 requirement 表互不串扰

### Requirement: 回溯重做窗口推导与 prompt 携带

`phase_next` SHALL 从既有评估条目派生「回溯窗口」，并随 `PhaseNextOutcome` 下发，使回溯窗口内**每个相位的每个 agent 步**（executor 与 evaluator 同等）的 prompt 均携带该背景，直至窗口闭环。窗口闭环判据与边界 MUST NOT 依赖运行期状态或 run 锚点。

窗口记录的字段面 SHALL 为 `BacktrackWindowEntry`：

- `from_phase: String`（发起相位，区间上界）与 `from_index: usize`（其相位表序下标）；
- `to_phase: String`（目标相位，区间下界）与 `to_index: usize`（其相位表序下标）；
- `reason: String`（`backtrack_reason` 原文逐字）。

`PhaseNextOutcome` SHALL 增 `backtrack_window: Vec<BacktrackWindowEntry>`：正常响应填「覆盖 `next_phase` 的开放窗口」，decision 分叉的 error 响应填「覆盖失败相位的开放窗口」；两处 SHALL 共用同一推导函数，MUST NOT 各持一份判据。空集合即「无窗口」（MUST NOT 用 `Option` 表达同一语义）。

三条判据 SHALL 为：

1. **是否拼接** `connect(P, W) ⟺ to_index ≤ col(P) ≤ from_index`，其中 `col` = 该相位在相位表中的下标（与 `detail.pipeline` 下标、前端 `colIndex` 同源，MUST NOT 另引第二份相位序）；
2. **是否未闭环** `open(W) ⟺ 发起相位不存在 id 大于标记行 id 的条目`；
3. **堆叠与裁剪** `filter(open) → filter(connect) → 按 order 升序 → take_last(MAX_BACKTRACK_WINDOWS)`，`MAX_BACKTRACK_WINDOWS = 2` 与 `MAX_REASON_CHARS` 同驻写面常量区，prompt 文案与测试断言 MUST NOT 写死该数字。

上限 SHALL 写成双分支，MUST NOT 只写其中一支：命中窗口数 **≤ `MAX_BACKTRACK_WINDOWS`（0 / 1 / 2 层）时全量并集**（按创建序升序、最早在前，多窗互不暂停互不覆盖）；**超出上限时取创建序末位两层**，被裁的恒为创建序最早那条——它同时是该组中 `from_index` 最大者（同时开放窗口按创建序 `from_index` 严格递减，故「创建序最早 ≡ `take_last` 首先丢弃 ≡ `from_index` 最大」；explore 口语化的「最旧最宽」只是该判据的常见后果，MUST NOT 反过来以「区间最宽」定义裁剪位置）。并集语义（A2）在上限内完整保留，超限则降级为末位两层：这是 A2 与上限的唯一共存点，两处 delta（本能力与 desktop-change-orchestration）都须如实写到该双分支。

判据 MUST NOT 读取 `run_id` / `SessionAnchors`（跨 run 继承由此天然成立）。边界 SHALL 零特判：含源头相位（`P = from` 落在区间上界，最需要知道上次因何而挂）、自环回溯（`to == from` 退化为单点区间，与常规回溯同一条代码，MUST NOT 写 `if to == from` 特判）、嵌套回跳（多窗各自独立开放，下游相位可同时命中；内层回跳到更上游后外层当前相位落到区间外即不再拼，回放进区间即再拼——由区间判据自愈，MUST NOT 另设暂停 / 恢复状态）。

文案 SHALL 由单点常量承载（经 `write/mod.rs` 再导出供 orchestration 复用，MUST NOT 有第二份副本）：标题 `## 回溯重做窗口（背景信息）` + 一段纯陈述说明（按发生先后排列、最上面最早、保留原始换行与标点、未做截断或改写、属背景不构成额外验收条件）+ 逐条以 `<backtrack reason>` / `</backtrack>` 定界的**原因原文**。标签行 SHALL 零属性（`from` / `to` / 顺序号一律不渲染）；渲染 MUST NOT 取自 `reason` 之外的任何字段，MUST NOT 出现任何相位 id 字面量。原因 MUST NOT 被截断、压平或 trim（含首尾换行与标点逐字保留）。

位置 SHALL 为契约而非排版偏好：executor / evaluator 的窗口段 SHALL 原地取代被删除的单行回溯原因（静态主体之后、失败反馈段**之前**）；evaluator 的最终序 SHALL 为 `上下文头 → 静态主体 → 窗口段 → 失败反馈段 → 输出协议附录`。MUST NOT 置于失败反馈段之后（反馈段是「本相位这次的失败清单」，窗口段是「为什么这个相位被叫回来」，后者是前者的上位语境）。

体积 SHALL 确定有界：单相位窗口段 ≤ `MAX_BACKTRACK_WINDOWS` ×（标签与说明行 + `MAX_REASON_CHARS`）≈ 2.1K 字符。实现 MUST NOT 因体积顾虑再加截断。

#### Scenario: 窗口内每个相位每个 agent 步都携带背景

- **WHEN** 决策回溯 `to=dev-design` / `reason=R` 后流程经 dev-design → test-design → implement → test-gen 依次推进
- **THEN** 四个相位的 executor 与 evaluator prompt 均含 `## 回溯重做窗口` 段与 R（无一相位丢失），窗口外相位（如 code-review）零该段

#### Scenario: 含源头相位

- **WHEN** 流程推进到发起相位（如 test-gen）本身并请求其 prompt
- **THEN** 该相位 prompt 仍含窗口段（`to_index ≤ col(P) = from_index`）；该相位落账新条目后（`id` 大于标记行）窗口即闭环，其后续 prompt 零该段

#### Scenario: 自环回溯退化为单点且零特判

- **WHEN** 以 `backtrack_to` 等于发起相位（如 `to = from = implement`）回溯
- **THEN** 窗口区间退化为单点，只有 implement 自身 prompt 携带窗口段，其落账即闭环；源码审查确认判据零 `if to == from` 特判分支

#### Scenario: 嵌套双窗按创建序并集

- **WHEN** 两个窗口同时开放（`from=test-gen → to=dev-design` 与 `from=test-design → to=proposal`）且下游相位同时落在两区间内
- **THEN** 该相位窗口段按创建序渲染两条原因（早者在前）；只落在较窄区间的相位只渲染对应那一条

#### Scenario: 内层回跳不改外层判据

- **WHEN** 外层窗口 `[dev-design..test-gen]` 开放期间发生内层回溯 `from=implement → to=proposal`，流程被打回 proposal 后逐相位重推
- **THEN** proposal 相位只渲染内层原因（外层 `to_index` 之上，区间不命中），dev-design 相位两条同时命中（外层仍在区间内、内层区间下界命中），test-gen 相位只渲染外层原因——全程零暂停 / 恢复状态，判据由区间自愈

#### Scenario: 层级上限取末位两层

- **WHEN** 同一相位命中三个同时开放的窗口（创建序 Win1 / Win2 / Win3，`from_index` 依次为 5 / 4 / 3）
- **THEN** 渲染集合为末位两层（Win2 与 Win3，按创建序升序），创建序最早的 Win1 被裁
- **AND** 被裁的 Win1 恒为该组中 `from_index` **最大**者（同时开放窗口按创建序 `from_index` 严格递减 ⇒ 创建序最早 ≡ `take_last` 首先丢弃 ≡ `from_index` 最大；「最旧最宽」仅为其常见口语后果，不作判据）；出线的是 `from_index` 最小的两条
- **AND** 窗口段零「另有 N 条未展开」类元信息

#### Scenario: 求值顺序固定

- **WHEN** 构造三窗用例：输入条目序与创建序不同（条目不按 `id` 升序排列），且创建序最早的窗口 `from_index` 最大、最后创建者最小（符合结构性性质）
- **THEN** 结果与 `filter(open) → filter(connect) → 按 order 升序 → take_last(MAX_BACKTRACK_WINDOWS)` 逐条相等（即渲染 `from_index` 最小的两条，恰为最近创建的两次回溯）
- **AND** 把顺序写成「先 `take_last` 再 `sort`」的实现取到的是输入末位两条而非创建序末位两条，在该用例下判红

#### Scenario: 跨 run 继承

- **WHEN** 回溯窗口尚未闭环时停止 run 并以新 `run_id` 重新发起（新 `SessionAnchors` 实例、`round` 归 1）
- **THEN** 目标相位的 prompt 仍含窗口段（判据不读 run 锚点）；源码审查确认窗口判据的入参不含 `run_id` / `SessionAnchors`

#### Scenario: error 响应同样携窗口

- **WHEN** 失败相位落在某开放窗口内并触发决策分叉（`VerdictFail` 或 `MaxRetriesExceeded`）
- **THEN** 该 error 响应的 `backtrack_window` 为覆盖失败相位的开放窗口集合（与正常响应同一推导函数），供决策输入直接消费

#### Scenario: 渲染零元信息零相位标识

- **WHEN** 审查窗口段渲染函数的取值与产物
- **THEN** 每条只取 `reason` 一个字段，标签行零属性；断言 prompt 正文不出现 `from_phase` / `to_phase` 的任何字面量；原因原文（含换行 / 首尾空白 / 标点 / 1000 字符长文）逐字保留

#### Scenario: 段位置先于反馈段

- **WHEN** 一个落在窗口内的相位同时带 fail 重试反馈段
- **THEN** prompt 中窗口段索引小于失败反馈段索引；evaluator 的输出协议附录仍在最后

### Requirement: 回溯窗口与轮次预算的口径分层

写面的「本轮」口径 SHALL 显式分两层且 MUST NOT 混用：

- **预算层**：`round` / `MAX_ROUNDS` / 窗口 fail 计数按**单次 run 窗口** `entries[anchor..]` 计算，语义 = 单次运行的循环预算，重启换新预算（既有语义，逐字不变）；
- **背景层**：回溯窗口按 **db 事件序全量 entries** 计算，语义 = 一次回跳的语义窗口，跨 run 直到闭环。

两层的差异 SHALL 是有意设计而非疏漏：把背景层判据写成读 `anchor` 会把跨 run 的未闭环窗口误判为已闭环（正是本能力要修的现象）。实现 MUST NOT 以任何形式的「统一口径」把两层合并。

#### Scenario: 两层口径并存且互不侵蚀

- **WHEN** 同一 change 在新 run 中路由某相位（`entries[anchor..]` 为空 ⇒ `round = 1`）
- **THEN** 预算层按新 run 窗口计数（`round = 1`、窗口 fail 数从 0 起），背景层仍按全量条目命中未闭环窗口；两者取值不同却同时对同一响应的字段面成立

#### Scenario: 判据不读锚点

- **WHEN** 审查窗口推导函数的签名与函数体
- **THEN** 入参只有条目切片、相位表与目标相位 id；无 `SessionAnchors` / `run_id` / `anchor` 参与，零 `entries[anchor..]` 切片

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/` 写面模块 | change 流程状态域权威写面（db 载体） | `phase_next`（路由 + prompt 插值 + 白名单 + sessionAnchors 进程内锚点）/ `phase_start` / `phase_log`（checklist 信封 + skipped + 会话槽位，单事务原子）/ `backtrack`（stale + 传播 + 白名单二次校验 + reason ≤1000；合法性判据单源，本 spec）/ `archive`（双写）；落库经 port 缝；sync 无 tokio；`persist.rs` raw Value 保形改写退役 |
| 落库 port 缝（新） | core/workflow → store 的进程内缝 | trait 定义落位 design 定稿；desktop-app 装配 store 实现；core/workflow 零 infra 依赖 |
| core/workflow 相位表（含 prompt 模板，不改） | requirement 工作流相位序单源 | executor / evaluator prompt 模板驻此；TS 侧 `lib/workflow.ts` 冻结；`PIPELINE_PHASES` 保持布局身份 |
| `crates/core/orchestration/src/prompt.rs`（不改） | prompt 组装 | 模板取自 core/workflow 相位表单源 |
| `crates/core/orchestration`（消费方，不改面） | walker 经 port 缝进程内直调写面 | 依赖方向 orchestration → workflow；walker 零自持转移规则；LocalToolSteps 直调面签名不变（载体在写面内换血） |
| `crates/infra/store`（workspace 库实例） | 状态落库载体 | ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord（见 desktop-workspace-store / desktop-change-state-store） |
| `crates/core/workflow/src/state.rs`（StepNode / RunStepKind） | 二十一值词面单点 | `Clean` / `clean` 白名单式入封闭集（20→21）；`StepNode::group()` 将 clean 派生 `StepGroup::Git`、`role()` 为 None；与 `RunStepKind` / `ChangeStepKind` 同词面 1:1 |
| `crates/core/workflow/src/write/phase_table.rs`（ARCHIVE_STEPS） | 11 步序列静态 | SpecSync 与 Seal 之间插 `Cleanup`（`prompt_ref=None`、尽力步语义驻 walker 执行体）；`ARCHIVE_STEP_ORDER` 断言 10 → 11 |
| `crates/core/workflow/src/write/phase_table.rs` | 表结构单源 | `PhaseDefinition` 增 `backtrackable: bool`（表内自描述）；`INITIALIZE_STEPS` 静态五步（`PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`，StaticCheck / TestExecution `on_fail=Terminal`）+ 两表表头 `INITIALIZE` 表项（`executor=None` / `evaluator=None` / `prerequisites=[]` / `backtrackable=false` / 零 artifact_manifest）；`allowed_backtrack_phases` 按 `backtrackable` 过滤（无位置推导跳过）；`REQUIREMENT_TABLE` 10 相位 / `TEST_ONLY_TABLE` 7 相位 |
| `crates/core/workflow/src/write/phase_next.rs`（零改动） | 路由 | 表驱动「首个未 pass 相位」天然含 initialize 表头，零特判、零新规则 |
| `crates/core/workflow/src/state.rs` | `OnFail` 变体面 | 增 `Escalate`（无载荷）；三变体 `FeedbackLoop { limit }` / `Terminal` / `Escalate`，与缺省 `None` 各自语义见 `StepDefinition.on_fail` doc |
| `crates/core/workflow/src/write/phase_table.rs`（`TEST_EXECUTION_STEPS`） | 步声明单源 | TestExecution / MutationTest 两步 `on_fail: Some(OnFail::Escalate)`（原 `FeedbackLoop { limit: 5 }`） |
| `crates/core/workflow/src/write/phase_table.rs`（`backtrack_options` / `allowed_backtrack_phases`） | 白名单推导 | `redoable = phase.steps.iter().any(|step| step.node.role().is_some())`；`table[..=idx]` 过滤 `backtrackable` 后 `redoable \|\| phase.id != current_phase` 自排除；`backtrack_options` 每项携 `PhaseDefinition.description`（决策上下文特性文案单源）与 `PhaseDefinition.artifact_manifest`（该相位可操作产物清单单源；清单零 `eval-checklist` 项——评估 checklist 由 PhaseRecord 投影而来、非相位声明产物），`allowed_backtrack_phases` 为其 id 投影（过滤逻辑零副本） |
| `crates/core/workflow/src/write/backtrack.rs` | 白名单二次校验 | `to == phase` 在白名单内时受理（重做本相位），否则拒绝且零写 |
| `crates/core/workflow/src/write/phase_next.rs` | 路由与幂等闸 | `fail_decided`（decision_session_id 在案 ⇒ 不再分叉）语义注释改写；删 `eval_only` 派生，`PhaseNextError` 两变体去 `eval_only` 字段 |
| `crates/core/workflow/src/write/phase_table_test.rs` | 声明守卫 | `Escalate` 仅允许出现在 TestExecution / MutationTest 步；两步 `on_fail` 期望值断言 |
| `crates/core/workflow/src/state.rs`（`StepNode` / `RunStepKind` / `ChangeStepKind`） | 词面单点 | `ArchiCheck` / `archi_check` 白名单式入封闭集（21→22，三枚举同词面 1:1）；`StepNode::group()` 派生 `StepGroup::Exec`、`role()` 为 `None` |
| `crates/core/workflow/src/write/phase_table.rs`（`IMPLEMENT_STEPS`） | 计划图声明 | 7 步：`PhaseStart → Executor → StaticCheck{5} → ArchiCheck{3} → Evaluator → VerdictGate{5} → PhaseLog`；`ArchiCheck` 唯一的步词出处 |
| `crates/core/workflow/src/write/phase_table.rs`（`TEST_ONLY_TABLE`） | 表项守卫 | 零 `ArchiCheck` 步（该表无 implement 相位）；表测试断言两条表全相位扫描 |
| `crates/core/workflow/src/write/phase_next.rs` | 窗口推导与 prompt 携带单点 | `pub struct BacktrackWindowEntry { from_phase: String, from_index: usize, to_phase: String, to_index: usize, reason: String }`（`Debug` / `Clone` / `PartialEq` / `Eq`；非 `Serialize`——不出 bindings）；`PhaseNextOutcome.backtrack_window: Vec<BacktrackWindowEntry>`；窗口推导 fn（入参 `(&[PhaseStateRecord], &'static [PhaseDefinition], phase_id)`，输出按创建序升序；≤ `MAX_BACKTRACK_WINDOWS` 全量并集，超限取创建序末位两层、裁掉 `from_index` 最大那条）；窗口段渲染 fn（入参窗口切片，输出 `String`，只取 `reason`）；`build_phase_response` 删 `backtrack_reason` 入参、窗口段插在原单行原因处；`error_outcome` 填覆盖失败相位的窗口 |
| `crates/core/workflow/src/write/phase_table.rs` | 窗口层级上限常量 | `pub(crate) const MAX_BACKTRACK_WINDOWS: usize = 2;`（与 `MAX_REASON_CHARS` / `MAX_REPORT_CHARS` / `MAX_ROUNDS` 同区；prompt 文案与测试断言引用常量，零写死数字） |
| `crates/core/workflow/src/write/mod.rs` | 出口面 | `pub use phase_next::{…, BacktrackWindowEntry}` 与窗口段渲染 fn 的再导出（供 orchestration 复用同一文案单点，零第二份副本） |
| `crates/core/workflow/src/write/backtrack.rs`（零改动） | 落账裁定 | 白名单二次校验 / 表位校验 / `reason ≤ 1000` / stale 传播逐字不变；窗口是读面派生，不改变写面裁定 |
| `crates/core/workflow/src/write/phase_next_test.rs` | 窗口判据语料 | 四态（目标 / 中间 / 源头 / 窗外）、自环单点、嵌套双窗并集、三层末位裁剪、内层回跳、跨 run 继承、error 响应窗口、原因原文逐字保真、渲染零相位 id、求值顺序、S2 断言改写 |
| `crates/core/workflow/src/queries/detail.rs`（零改动） | 泳道序真值源 | `pipeline` 仍按 `phase_table` 序投影（`col` 的取值来源）；本能力只消费该序，不改其构造 |
| `crates/infra/store/**`（零改动） | 落库载体 | `PhaseStateRecord` 字段面不变；`list_phase_records` 的 id 升序语义即创建序来源；零 schema / 零迁移 |
