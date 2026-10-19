# desktop-workflow-write-face Specification

## Purpose

core/workflow 保持 change 流程状态的 Rust 单一权威（读 + 写），持久化载体为 workspace 库（desktop-change-state-store，落库经 port 缝）；相位表与 prompt 模板单源驻 core/workflow，walker 经进程内直调消费，desktop 全链不读不写 workflow.json（双向墙）。

## Requirements

### Requirement: workflow.json 写面进程内单权威

core/workflow SHALL 保持 change 流程状态的 Rust 单一权威（读 + 写），持久化载体 SHALL 由 workflow.json 重定义为 workspace 库（desktop-change-state-store；落库经 port 缝由壳层装配 store 实现，core/workflow 零 infra 依赖）。写面 SHALL 提供：

- `phase_next`：相位路由状态机——返回 next_phase、已插值的 executor / evaluator prompt、`allowed_backtrack_phases` 白名单、重试上限判定；`sessionAnchors` 以进程内状态复活（mid-phase interruption 分支可达，重入时中断相位可感知并标定）；
- `phase_start`：开启阶段并 attempt 计时；
- `phase_log`：eval 记录落账（checklist 信封 + skipped + 会话槽位，见「eval 条目会话槽位落账」），单事务原子落库；
- `backtrack`：回跳落账（stale 标记 + 传播 + reason ≤500）；
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

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验：`backtrack_to` 不在 `allowed_backtrack_phases` 内 SHALL 被拒绝且不落任何账（越权决议损坏不了状态，状态库零变更）；校验通过 SHALL 落回跳记录、对被跳过相位的 PhaseRecord 打 stale 标记并传播（与回跳落账同一事务语义），`reason` 超过 500 字符 SHALL 被拒绝。`backtrack_to` 与发起相位相同（自环）SHALL 在白名单含该相位时正常受理——这是「重做本相位」的唯一载体（白名单自含规则见「相位定义有序 step 序列与 artifact 清单」）：落账语义为改写最新条目 `backtrack_to` + 目标相位下游 stale 传播，`phase_next` 随后以新 attempt 路由回该相位；白名单不含自身时（零 agent 步相位）自环 SHALL 被拒且状态库零变更。该口径为桌面写面自有裁定（插件侧 `pipeline-backtrack` 的「目标索引必须小于当前相位」约束不适用于桌面路径，插件零改动、两份 spec 各管其面）。

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

- **WHEN** 以超过 500 字符的 reason 调用写面 backtrack
- **THEN** 调用显式失败，状态库零变更

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

`PhaseAgentSpec` SHALL 由 `{ agent_type: String, prompt: String, model_level }` 收敛为 `{ prompt: &'static str, model_level }`：`agent_type` 字段随 `__CALL_AGENT:<role>__` 令牌退役删除；prompt SHALL 为编译期静态文本（`include_str!("prompts/<role>.md")` 产物，见 desktop-phase-prompts）。写面 SHALL 删除 `interpolate`（`<change>` / `<phase>` 占位符插值）；`phase_next` 的 `build_phase_response` SHALL 以静态 prompt + 上下文头 `change: <name>`（append）+ 回溯原因行（append）组装，MUST NOT 做模板替换。desktop core（workflow + orchestration）SHALL 零残留 `__CALL_AGENT` / `interpolate` / `strip_call_agent` / `role_brief` / `<change>` / `<phase>`。

#### Scenario: 结构收敛与令牌退役

- **WHEN** 审查 `PhaseAgentSpec` 定义与 `phase_table` 表条目
- **THEN** 结构仅含 `prompt: &'static str` 与 `model_level`；无 `agent_type` 字段；无 `__CALL_AGENT` 字样
- **AND** 相位序 / model_level / 白名单 / 前置依赖语义不变

#### Scenario: 无模板替换

- **WHEN** `phase_next` 下发某相位 executor / evaluator prompt
- **THEN** prompt 主体为静态 md 文本，仅 append 上下文头 `change: <name>` 与（backtrack 时）回溯原因行
- **AND** 无 `<change>` / `<phase>` 替换行为

#### Scenario: 删除面零残留

- **WHEN** 全仓扫描 desktop core（workflow / orchestration）源码
- **THEN** 无 `__CALL_AGENT` / `interpolate` / `strip_call_agent` / `role_brief` / `<change>` / `<phase>` 残留

### Requirement: 相位定义有序 step 序列与 artifact 清单

`PhaseDefinition` SHALL 扩展为计划图节点与产物清单的单源载体：

- `steps: [StepDefinition]`：相位内有序 step 序列；`StepDefinition` SHALL 携 `kind`（`exec` | `agent` | `gate` | `git` 四类封闭集，经 `StepNode::group()` 派生）、`role`（agent 步角色）、`prompt_ref`（指向 `include_str!` 静态 prompt 的引用键，非 agent 步可缺省）、`on_fail` 语义三变体（`FeedbackLoop { limit }` 定向反馈循环、`Terminal` 终态失败停给用户、`Escalate` 零会话落账升格交决策）；
- `prerequisites: &'static [&'static str]`：相位直接前置依赖（表内自描述，stale 传播与 allowed_backtrack 推导的唯一事实源；MUST NOT 存在第二份独立 match）；
- `backtrackable: bool`：相位回溯资格（表内自描述，沿 `prerequisites` 先例；`allowed_backtrack_phases` SHALL 按本属性过滤——`backtrackable=false` 的相位 MUST NOT 出现在任何相位的回溯白名单中，表头 initialize 恒 false（基线检查不可作为回溯目标），过滤 MUST NOT 以「跳过 idx==0」类位置推导实现——表结构变化即漂移）；白名单 SHALL 另按「可重做」裁定自身：`allowed_backtrack_phases` 中「当前相位自身在列」⟺ 该相位可重做（其 `steps` 含 `role()` 为 `Some` 的 agent 步）；零 agent 步的相位（test-execution / archive）自身 MUST NOT 入白名单——决策 agent 上下文里不存在「重做本相位」这个选项，判断 MUST NOT 以相位名硬编码；
- `artifact_manifest: [ArtifactSpec]`：相位声明产物清单；archive 相位 SHALL 无 `artifact_manifest`（不产文档）；requirement 表 test-design SHALL 声明 `markdown-doc·test-design.md`、implement SHALL 声明 `tasks-progress·tasks.md`（实现期进度视图，服务 phase_next 下发面——发现面同路径按首命中去重，tasks.md 出列为 dev-design 的 markdown-doc 单候选）；test-only 表 code-analyze SHALL 声明 `markdown-doc·design.md`（源码分析）；
- `executor` / `evaluator: Option<PhaseAgentSpec>`：archive 与 initialize 相位 SHALL 为 `None` / `None`（确定性相位）。

`StepNode` 词汇 SHALL 为二十一值（`executor` / `evaluator` / `decision` / `static_check` / `test_execution` / `mutation_test` / `clean` / `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` / `preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `cleanup` / `seal` / `finalize`），与 `RunStepKind` / `ChangeStepKind` 同词面 1:1 全映射（`mutation_test`、`cleanup` 与 `clean` 线词入封闭集，18→19、19→20 与 20→21 均为白名单式扩展）；code-analyze 复用既有 `Executor` / `Evaluator` / `VerdictGate` / `PhaseStart` / `PhaseLog` 步词，MUST NOT 新增步词。`StepGroup` SHALL 保持四分类（`Exec` / `Agent` / `Gate` / `Git`），`mutation_test` 归 `Exec`、`cleanup` 与 `clean` 归 `Git`。两表表头 SHALL 声明共享 `INITIALIZE_STEPS`：`PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`（一次性基线检查相位——Clean 为 vcs 直调基线恢复步、StaticCheck / TestExecution 步 `on_fail=Terminal`（失败即停，无反馈边——基线的债不属于本次变更）、`PhaseStart` / `Clean` / `PhaseLog` `on_fail=None`；零 `artifact_manifest`——基线报告树非文档素材，回查经节点 `output_ref`）。两表共享的 `TEST_EXECUTION_STEPS` SHALL 为 `PhaseStart → TestExecution → MutationTest → PhaseLog` 四步：TestExecution / MutationTest 两步 `on_fail=Escalate`（零 WorkerAgent 会话——首败即落账后回 `phase_next` 交决策面，见 desktop-test-execution「失败即升格交决策（零 agent 修复）」与 desktop-mutation-testing「失败升格交决策（报告指引式信息面）」），`PhaseStart` / `PhaseLog` `on_fail=None`。同一步词在不同表项语义不同（initialize 表项的 TestExecution 为 `Terminal`、test-execution 表项为 `Escalate`），语义由表项声明承载，MUST NOT 由相位名推导；`Escalate` 归属 SHALL 由表测试守卫（只允许出现在 TestExecution / MutationTest 步上——四个执行体只按 `on_fail` 分派，误声明会静默失效）。`REQUIREMENT_TABLE` 的 archive `PhaseDefinition` SHALL 声明 11 步序列：`PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize → PhaseLog`（`ResolveConflicts` 为条件执行步，`Cleanup` 为尽力步——清理失败仅 step detail 警告、步恒 `passed`，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」；其余步 `on_fail=None`——步失败即 Terminal）。`TEST_ONLY_TABLE` SHALL 复用 requirement 同名 step 静态（proposal / test-design / test-gen / test-execution / archive），并新增 `CODE_ANALYZE_STEPS`：`PhaseStart → Executor → Evaluator → VerdictGate → PhaseLog`（P→E，`on_fail=Terminal` 于 VerdictGate）。

`steps` SHALL 全静态声明（const 表内联，`prompt_ref` 经编译期 `include_str!` 映射回静态 prompt 表）；MUST NOT 将 prompt 文本收进 db 定义表、MUST NOT 经 port 缝做运行时定义加载。`artifact_manifest` 为「相对路径 → phase」归属的权威来源，前端 `docColumn` 静态映射退役后 MUST NOT 在 core/workflow 之外再保留第二份 phase 归属表。

#### Scenario: 步序列全静态声明

- **WHEN** 审查 `phase_table.rs` 的 `PhaseDefinition` 表条目（含 initialize、archive、test-execution 与 code-analyze）
- **THEN** 每相位 `steps` 为编译期 const 有序序列（kind / role / prompt_ref / on_fail 齐备），initialize 相位 5 步在案（表头 `INITIALIZE_STEPS` 两表共享）且 `executor=None` / `evaluator=None` / `backtrackable=false`，archive 相位 11 步在案且 `executor=None` / `evaluator=None`，test-execution 4 步在案（TestExecution / MutationTest 两步 `on_fail=Escalate`），code-analyze 5 步在案（P→E）；无 db 定义表、无 port 缝定义读、无运行时文件 IO

#### Scenario: StepNode 二十一值与四分类分组

- **WHEN** 审查 `StepNode` / `RunStepKind` / `StepGroup` 定义
- **THEN** 词汇二十一值全映射（9 个归档步词（含 `cleanup`）、`mutation_test` 与 initialize 基线步词 `clean` 在案，code-analyze 零新步词）；`StepGroup` 含 Git，commit / rebase / merge / cleanup / finalize / clean 归 Git、preflight / seal / mutation_test 归 Exec、spec_sync / resolve_conflicts 归 Agent

#### Scenario: 回溯白名单按 backtrackable 过滤

- **WHEN** 对两表任一非表头相位调用 `allowed_backtrack_phases`
- **THEN** 白名单为其前序相位（含自身）中 `backtrackable=true` 者的集合，initialize 恒不在白名单（`backtrackable=false`）；过滤依据为表内显式属性，无「跳过 idx==0」位置推导；自身是否在列另由「可重做」裁定（见下条 scenario）

#### Scenario: 白名单自含即重做、零 agent 步自排除

- **WHEN** 分别对 implement 与 test-execution 调用 `allowed_backtrack_phases`
- **THEN** implement 的白名单含 implement 自身（`steps` 含 agent 步 ⇒ 可重做，「重做本相位」经回溯到自身表达）；test-execution 的白名单不含自身（`steps` 全为确定性步 ⇒ 不可重做，决策上下文无「重做本相位」选项），其余前序 `backtrackable` 相位照常在列

#### Scenario: artifact 清单权威归属

- **WHEN** 查阅 requirement 与 test-only 工作流各相位的 `artifact_manifest`
- **THEN** proposal.md / specs/** 属 proposal、design.md / tasks.md 属 dev-design、test-design.md 属 test-design、test-reports/** 属 test-execution（以清单声明为准）；implement 声明 `tasks-progress·tasks.md`（下发面，发现面同路径首命中去重不重复出列）；test-only 下 code-analyze 声明 design.md、test-design 声明 test-design.md；initialize 与 archive 相位零 artifact_manifest；清单之外文档不挂 phase 列

#### Scenario: PhaseAgentSpec 静态化不翻案

- **WHEN** 审查 `PhaseDefinition` 扩展后的 executor / evaluator spec
- **THEN** 仍为 `{ prompt: &'static str, model_level }`（`include_str!` 产物），无 `agent_type` / `__CALL_AGENT__` / `interpolate` 残留；archive 与 initialize 相位 executor / evaluator 均 None

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/` 写面模块 | change 流程状态域权威写面（db 载体） | `phase_next`（路由 + prompt 插值 + 白名单 + sessionAnchors 进程内锚点）/ `phase_start` / `phase_log`（checklist 信封 + skipped + 会话槽位，单事务原子）/ `backtrack`（stale + 传播 + 白名单二次校验 + reason ≤500）/ `archive`（双写）；落库经 port 缝；sync 无 tokio；`persist.rs` raw Value 保形改写退役 |
| 落库 port 缝（新） | core/workflow → store 的进程内缝 | trait 定义落位 design 定稿；desktop-app 装配 store 实现；core/workflow 零 infra 依赖 |
| core/workflow 相位表（含 prompt 模板，不改） | requirement 工作流相位序单源 | executor / evaluator prompt 模板驻此；TS 侧 `lib/workflow.ts` 冻结；`PIPELINE_PHASES` 保持布局身份 |
| `crates/core/orchestration`（消费方，不改面） | walker 经 port 缝进程内直调写面 | 依赖方向 orchestration → workflow；walker 零自持转移规则；LocalToolSteps 直调面签名不变（载体在写面内换血） |
| 插件 MCP / TS 写路径（彻底分离，不改） | CLI 工作流自有写通道 | 双向墙：desktop 不读不写 workflow.json；插件零改动 |
| `crates/infra/store`（workspace 库实例） | 状态落库载体 | ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord（见 desktop-workspace-store / desktop-change-state-store） |
| `crates/core/workflow/src/state.rs`（StepNode / RunStepKind） | 二十一值词面单点 | `Clean` / `clean` 白名单式入封闭集（20→21）；`StepNode::group()` 将 clean 派生 `StepGroup::Git`、`role()` 为 None；与 `RunStepKind` / `ChangeStepKind` 同词面 1:1 |
| `crates/core/workflow/src/write/phase_table.rs`（ARCHIVE_STEPS） | 11 步序列静态 | SpecSync 与 Seal 之间插 `Cleanup`（`prompt_ref=None`、尽力步语义驻 walker 执行体）；`ARCHIVE_STEP_ORDER` 断言 10 → 11 |
| `crates/core/workflow/src/write/phase_table.rs` | 表结构单源 | `PhaseDefinition` 增 `backtrackable: bool`（表内自描述）；`INITIALIZE_STEPS` 静态五步（`PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`，StaticCheck / TestExecution `on_fail=Terminal`）+ 两表表头 `INITIALIZE` 表项（`executor=None` / `evaluator=None` / `prerequisites=[]` / `backtrackable=false` / 零 artifact_manifest）；`allowed_backtrack_phases` 按 `backtrackable` 过滤（无位置推导跳过）；`REQUIREMENT_TABLE` 10 相位 / `TEST_ONLY_TABLE` 7 相位 |
| `crates/core/workflow/src/write/phase_next.rs`（零改动） | 路由 | 表驱动「首个未 pass 相位」天然含 initialize 表头，零特判、零新规则 |
| `crates/core/workflow/src/state.rs` | `OnFail` 变体面 | 增 `Escalate`（无载荷）；三变体 `FeedbackLoop { limit }` / `Terminal` / `Escalate`，与缺省 `None` 各自语义见 `StepDefinition.on_fail` doc |
| `crates/core/workflow/src/write/phase_table.rs`（`TEST_EXECUTION_STEPS`） | 步声明单源 | TestExecution / MutationTest 两步 `on_fail: Some(OnFail::Escalate)`（原 `FeedbackLoop { limit: 5 }`） |
| `crates/core/workflow/src/write/phase_table.rs`（`allowed_backtrack_phases`） | 白名单推导 | `redoable = phase.steps.iter().any(|step| step.node.role().is_some())`；`table[..=idx]` 过滤 `backtrackable` 后 `redoable \|\| phase.id != current_phase` 自排除 |
| `crates/core/workflow/src/write/backtrack.rs` | 白名单二次校验 | `to == phase` 在白名单内时受理（重做本相位），否则拒绝且零写 |
| `crates/core/workflow/src/write/phase_next.rs` | 路由与幂等闸 | `fail_decided`（decision_session_id 在案 ⇒ 不再分叉）语义注释改写；删 `eval_only` 派生，`PhaseNextError` 两变体去 `eval_only` 字段 |
| `crates/core/workflow/src/write/phase_table_test.rs` | 声明守卫 | `Escalate` 仅允许出现在 TestExecution / MutationTest 步；两步 `on_fail` 期望值断言 |
