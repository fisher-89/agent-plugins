# 按 step 粒度运行的流程编排（desktop 的 exec/agent/gate 三步抽象）

日期: 2026-10 · 状态: 探索中（源码调研完成，方向待选）

## 主题与背景

`packages/desktop` 把 change 流程的编排从插件的「phase 生命周期 + 技能驱动 agent」进一步
抽象为：**每个 phase 下挂一串有序 step，step 分 exec / agent / gate 三类，全流程按 step
顺序推进，中断 / 继续 / 回退也落到 step 粒度**。本笔记记录这次抽象在真实代码里的落点、
三类 step 的封闭词汇、硬编码与声明式的边界，以及「step 粒度」在中断/继续/回退上的真实
语义（哪些真按 step、哪些仍是 phase 载荷）。

## 代码地图

| 关注点 | 位置 |
|--------|------|
| 步词汇 10 类（`ChangeStepKind`）与步状态 | `crates/core/orchestration/src/state.rs` |
| 状态词汇（`RunStepKind` 5 类 / `RunStatus` 等） | `crates/core/workflow/src/state.rs` |
| 相位表（每相位 executor/evaluator spec，8 相位） | `crates/core/workflow/src/write/phase_table.rs` |
| 路由状态机（phase-next / 重试预算 / 会话锚点） | `crates/core/workflow/src/write/phase_next.rs` |
| **walk 主循环（按 step 顺序执行的硬编码骨架）** | `crates/core/orchestration/src/walker.rs` |
| 运行控制注册表（中断/继续/停等/步累积器） | `crates/core/orchestration/src/control.rs` |
| 工具步执行（exec 类落点，直调写面） | `crates/core/orchestration/src/steps.rs` |
| 落库过滤（10→5 单点） | `crates/core/orchestration/src/run_history.rs` |
| 命令入口（start/stop/answer/confirm/watch） | `src/commands/change_flow/mod.rs` |
| 前端三类节点分组 | `src/views/changes/flow/run-state.ts`（`groupOf`） |
| 设计契约 | `openspec/specs/desktop-change-orchestration/spec.md` |

## 抽象层次：phase → 有序 step

相位表仍是「相位」这一层（8 个路由相位），但**相位内部被 walker 展开成 step 序列**。
`ChangeStepKind` 是 10 值封闭集，注释直接点名「三类节点可辨」：

```
ChangeStepKind（10 值封闭集）
├─ WorkerAgent 三角色（agent）   Executor / Evaluator / Decision
├─ 相位机/工具步（exec）          PhaseStart / StaticCheck / TestExecution / PhaseLog
└─ Gate 三门（gate）              VerdictGate / RetryGate / WhitelistGate
```

前端 `run-state.ts#groupOf` 用同样的三分法映射到 `RunStepGroup`：

```ts
executor/evaluator/decision              → 'workerAgent'
verdictGate/retryGate/whitelistGate      → 'gate'
其余（phaseStart/staticCheck/...）        → 'toolStep'
```

即：用户说的 exec / agent / gate = 代码里的 ToolStep / WorkerAgent / Gate（spec 用词是
「三类节点（WorkerAgent / ToolStep / Gate）统一可观测」）。

## 全流程按 step 顺序执行（drive() 骨架）

`walker.rs#drive` 是一个单图硬编码循环，每相位依次：

```
loop {
  check_cancel ──► (置位则 stopped)
  ① phase-next（路由权威，只读）──► done? 收口 completed
                                    ├─ max_retries_exceeded? → ⑦ 决策分叉
  ② phase-start（开相位 + attempt 计时）
  ├─ test-execution 相位：确定性门禁循环（TestExecution step + 修复反馈边）
  └─ 其余相位：
       ③ executor 会话（agent step；若 executor: Some）
       ④ static-check（implement/test-gen 才触发；失败走修复反馈边 ≤5）
       ⑤ evaluator 会话（agent step）
       ⑥ verdict 解析门（VerdictGate）→ 桌面代写 phase-log（PhaseLog step）
  [auto_next_phase=false] ConfirmWait ──► 停等确认 / 取消则 stopped
  回 ①（pass 推进 / fail 重试 / 上限 → 决策）
}
```

**关键**：step 顺序本身是 `drive()` 里**硬编码**的，不是「每个 phase 声明一份 step 表」。
相位间的差异由三个分散来源决定（见下节），这是当前抽象与「声明式 per-phase step 表」之间
的真实距离。

## 相位内 step 序列的三个变化源

「哪些 phase 跑哪些 step」并非一张表说清，而是三处叠加：

1. **`phase_table.rs` 的 `PhaseDefinition.executor: Option<…>`** —— code-review / acceptance
   两相位 executor 为 `None`，于是 ③ executor step 被跳过（只跑 evaluator）。
2. **`walker.rs` 的布局常量** —— `STATIC_CHECK_PHASES = ["implement","test-gen"]`、
   `TEST_EXECUTION_PHASES = ["test-execution"]`，决定 ④ static-check 与确定性门禁是否插入。
3. **`drive()` 骨架本身** —— 通用顺序（phase-next → phase-start → … → phase-log → confirm）。

含义：所谓「进一步抽象」目前是**概念层三类节点统一 + 运行态统一可观测**，但**步序列不是
数据驱动的**（spec 明确「单图硬编码，MUST NOT 实现通用图引擎或图定义文件格式」）。

另一个可注意的差异：`PIPELINE_PHASES`（`queries/detail.rs` 与前端 `layout.ts`）是 **9 站**
（末尾多 `code-analyze`），而 `phase_table.rs` 路由表只有 **8 相位**。`code-analyze` 是纯
布局列（display-only），永不被路由（路由权威在 `phase_next`，`PIPELINE_PHASES` 只是列身份，
spec 红线：「MUST NOT 上位为路由权威」）。

## 中断 / 继续 / 回退的 step 粒度语义

「按 step 粒度」在三件事上落地程度不一，需区分：

**中断（stop）—— step 感知，观察点落在 step 边界。**
- `change_flow_stop` → `control.request_stop`（cancel watch 置位）+ 若当前有 WorkerAgent
  会话则经 StopRegistry 请求终止。
- walker 在 **每轮循环顶** `check_cancel` 观察；agent step 运行中可被中途终止，收口后该
  step 被 emit 为 `Stopped` 态（`run_worker` 里 `AgentRunStatus::Stopped → ChangeStepStatus::Stopped`）。
- 工具步（exec 类）是确定性同步调用，cancel 只能在步与步之间被看到——所以「中断」对
  agent step 是中途打断，对 exec/gate step 是步间生效。

**继续（continue）—— 从停等点原样恢复，本质按 step 挂起。**
- 两类停等：`ConfirmWait`（phase 间，`change_flow_confirm` 应答）与 `Ask`（决策 ask 中断，
  `change_flow_answer` 应答）。
- 都是 oneshot 单次应答通道，`wait_confirm`/`wait_answer` 挂起在具体 step 之后的停等点；
  应答回流后从原地继续，不是重头。

**回退（backtrack）—— 载荷是 phase 粒度，但被 step 化包裹。**
- 决策分叉输出 `{action:"backtrack", backtrack_to∈白名单}`，`backtrack_to` 是**相位名**，
  stale 闭包 = `dependents` BFS 按相位传播（`phase_table.rs#prerequisites/dependents`）。
- 但回退执行在决策分叉里被拆成 step 节点：`WhitelistGate`（越权预校验）→ `RetryGate` →
  工具步 `Backtrack` 命令（写面 `backtrack` 落账）→ 回 phase-next 重路由。
- 即：**回退的语义单位是 phase，回退的过程被表达成 gate + exec step 序列**。

## 运行态 vs 持久化：步累积器（10 类）→ 落库（5 类）

- 内存面（`RunEntry.steps`）：全词汇 10 类，emit 序追加，`RunGuard::emit` 间达；重挂快照
  `ChangeRunSnapshot.steps` 直接携带（重挂不恒空）。
- 落库面：`run_history.rs#persisted_step` 是 **10→5 唯一过滤点**——落库只留
  executor/evaluator/decision/static_check/test_execution；phase_start/phase_log/三 gate
  弃（纯 Rust 分支无会话无独立产物，attempt 级评估史已由 PhaseRecord 承载）。
- `seq` 用全词汇 emit 序盖戳，被过滤步占号产生库内空洞（排序键语义合法）。

## 与插件模型的对比（为何是「进一步抽象」）

插件侧（`openspec/specs/phase-lifecycle` / `workflow-orchestration`）：技能驱动，协议是
`phase_next → phase_start → executor → phase_log`，状态落在 `workflow.json` 的
`active_phase`，由 agent 自己按 SKILL.md 文本执行——**步是隐式在 prompt 里**的。

desktop 侧：把这段协议**从 agent 手里收回到 walker**，显式变成 10 类 step 节点 + 统一状态
机 + 三类可观测；`phase_log`/`phase_start` 不再靠 agent 调 MCP，而是 walker 进程内直调写面
（「零 CLI 子进程写通道」红线）。所以抽象的核心不是「多了 step 表」，而是：**同一套步状态
机覆盖 agent 会话、确定性工具步、纯 Rust gate 三类执行，全部可观测、可停止、可落库重算**。

## 值得追问的线索

1. **声明式 step 表方向**：当前步序列硬编码在 `drive()`，相位差异散在三处
   （executor Option / STATIC_CHECK_PHASES / TEST_EXECUTION_PHASES）。是否有动机把它收敛成
   每相位一份 `&[StepKind]` 或 `PhaseDefinition.steps`？spec 当前明确「单图硬编码」是 V1 边界，
   但三处分散源已是不小的隐性耦合。
2. **code-analyze 列**：9 站布局里 `code-analyze` 无路由无 spec，是纯占位列还是预留？
   如果后续要接入，路由表 8→9 与列身份 9 的对齐关系会变成新的问题面。
3. **回退的 step 化 vs phase 化**：backtrack 载荷仍是 phase 名，stale 传播按相位闭包；但
   过程被 gate 化。若真要做到「回退按 step 粒度」（回退到某个 step 而非整个 phase），现有
   PhaseRecord/attempt 的粒度（相位 × attempt）装不下，需要 step 级 checkpoint——这是目前
   「step 粒度」描述与实际数据模型之间最大的张力。
4. **中断恢复的落点**：中断靠 `active_phase`（相位 × attempt）续走，不靠 step；重启后从
   相位开头重跑（已 pass 相位不重头，但中断的那个相位从 phase_start 重来）。「中断按 step
   粒度恢复」在现状里不存在——会话锚点只标定窗口，不提供 step 级 resume。若要 step 级
   续走，需要把 `RunStepRecord`（已有 seq/phase/attempt/step/status）升级为恢复依据而非
   纯图史。
5. **`ask` 与 `confirm` 两类停等的统一**：`RunEntry` 里 confirm_tx / answer_tx 是两个独立
   槽位 + `RunUpdate::Ask`/`ConfirmWait` 两变体。若 step 表未来引入更多停等类型（如
   `WaitGate` step），这两槽位会否泛化成统一的「挂起 step 应答通道」值得观察。

---

## 第二轮：收敛到「配置驱动 workflow→phase→step + artifact 挂 phase + step 存会话/输出」的差距

用户方向三句话：① 收敛到一处、改为配置驱动的 workflow→phase→step 模型；② artifact 挂在
phase 上；③ step 保存 agent 会话记录与脚本 stdout。逐一对照真实代码，评估「现状 → 目标」的
落点与裂缝。

### 现状：配置散在 4 处，全是编译期硬编码

```
workflow_type: String（ChangeRecord 字段，V1 恒 "requirement"，只是相位表键，无 workflow 级定义）

配置的四个物理落点
├─ phase_table.rs      REQUIREMENT_TABLE &[PhaseDefinition]（8 相位 + executor/evaluator
│                        spec + include_str! 14 个 prompts/*.md；const 静态表）
├─ walker.rs           STATIC_CHECK_PHASES / TEST_EXECUTION_PHASES 两个常量（门禁布局）
├─ detail.rs           PIPELINE_PHASES [&str;9]（展示列，含 code-analyze 无路由站）
└─ 前端 layout.ts      PIPELINE_PHASES（同 9 站，注释「与 detail.rs 同序，改动需双侧同步」）
   phase_table.rs      prerequisites()（依赖表，stale 闭包传播源）
```

`phase_table(workflow_type)` 的消费点共 6 处：`phase_next` / `phase_start` / `phase_log` /
`backtrack`（写面）+ `archive_flow`（完成度核算）+ `change_flow` 命令（发起前置校验）。
「收敛到一处」目前不存在——光相位表与门禁布局就跨 `workflow` / `orchestration` 两个 crate。

`PhaseDefinition` 现有形状（step 化的起点）：

```rust
PhaseDefinition {
    id, description,
    executor: Option<PhaseAgentSpec>,   // prompt + model_level；code-review/acceptance 为 None
    evaluator: Option<PhaseAgentSpec>,
}
```

它**没有 step 列表**——step 序列是 `walker.rs#drive()` 硬编码 + 两个门禁常量补丁叠加的。

### 目标 ① 的深层分叉：step 序列配置化后，控制流怎么表达

drive() 的骨架不是线性 step，而是带三处特殊控制流：

```
phase-next ─► phase-start ─► [executor] ─► [static-check◄─修复反馈边 loop] ─► evaluator
        ▲                                                                            │
        │                                  verdict-gate ─► phase-log ─► confirm ─────┘
        └── fail 重试（≤5）/ 上限 → 决策分叉（backtrack/retry/stop/ask）
```

- **static-check / test-execution 的「定向反馈边」**：失败 → 同会话续注修复 → 再跑，独立
  计数 ≤5，超限升格相位 fail。这不是 step 表能线性表达的东西，需要 `on_fail → feedback loop`
  之类的控制流原语，否则又退化成 walker 里 step 类型特判。
- **verdict-gate 的解析失败**：不是「下一步」，是终态失败停给用户。
- **phase 间 confirm 停等**：auto_next_phase 决定是否跳过，是 run 级开关而非 step 属性。

结论：如果只把 `steps: Vec<StepKind>` 塞进 PhaseDefinition，能消掉 STATIC_CHECK_PHASES /
TEST_EXECUTION_PHASES 两个常量，但「反馈边 / 分叉 / 停等」仍得在 walker 特判 step 类型——
配置驱动只完成一半。要真正数据驱动，得设计一套能表达 loop / on_fail 分支的 step DSL，这直接
撞 spec 红线「单图硬编码，MUST NOT 实现通用图引擎或图定义文件格式」（desktop-change-
orchestration「V1 范围与边界留痕」第 3 条），需要显式翻案。

### 目标 ② 现状：artifact 是「发现式」，后端模型无 phase 归属

```
discover_artifacts(change_dir, phases)
  └─ 扫描文件树（相对路径）+ eval 条目 → 静态 matcher 插件（tasks-progress /
     eval-checklist / markdown-doc）→ ArtifactDescriptor { kind, source, title }
     source = 相对 POSIX 路径（"design.md"）或 eval 序号串（"3"）——无 phase 字段
```

phase 归属现在全在前端 `attachments.ts#docColumn(source)` 静态映射：

```
proposal.md / specs/**  → col:proposal
design.md / tasks.md    → col:dev-design
test-reports/**         → col:test-execution
eval-checklist          → 靠 payload.phase + attempt 挂节点（隐式携带，非后端赋值）
```

所以「artifact 挂在 phase 上」要动的是：给 artifact 模型加显式 phase 归属（或 phase 定义里
声明产物清单），取代前端 docColumn 的字符串→列硬编码。这牵出一个语义问题：markdown 产物
（proposal.md / design.md / tasks.md）是 agent 写盘的文件，归属是「phase 定义声明产出路径」
还是「执行时记录」？两种选法的存储形态不同（见下方问题 D）。

### 目标 ③ 现状：step 只存「引用 + 截断摘要」，全量在别处

```
RunStepRecord（run 步节点史行，5 类词汇）
  { step_key, run_id, phase, attempt, step, status,
    session_id: Option<String>,   // ← 会话仅引用，不内嵌
    detail: Option<String>,       // ≤200 字 diagnose_brief 口径
    timestamp }

StepRecord（审计行，7 类词汇）
  { ..., summary: String(≤500 截断), reference: Option<String>(报告目录/会话 id) }

全量载体现状
├─ agent 会话记录   → SessionEventRecord 独立单表（按 session_id 重放）
│                     归属靠 provenance source_ref=<id>/<phase>/<role>/<attempt> 反查
├─ static-check stdout → 内存 diagnostics 字段（反馈边注入原料）→ 落库只有 summary；全量即丢
└─ test-execution stdout → 磁盘 change_test_reports/<name>/（summary.json + plan/report.json）
                          落库只有 report_dir 引用
```

「step 保存 agent 会话记录 + 脚本 stdout」的实质 = 把上述「引用式 + 截断式」改成「step 内联
或强关联」。这里有个存储决策要选：stdout 全量进 db（blob/text，RunStepRecord 加字段）还是
继续磁盘文件 + 引用升级为「step 可直接展开读」？会话记录是 join SessionEventRecord（已有表）
还是 RunStepRecord 复制内嵌（与「转录单表 = 全史重放唯一来源」的既有不变量冲突）？

### 值得先对齐的四个分叉（按依赖序）

1. **workflow 定义载体**：db 表（WorkflowDefinition/PhaseDefinition/StepDefinition 三张）还是
   磁盘定义文件（注意 spec 红线「零 workflow.json 读写」针对的是 change **状态**载体，定义
   模板是另一回事，插件时代 workflow_file_inventory 是「定义+状态」混合体，desktop 当初拆成
   「定义硬编码 + 状态落 db」）？这决定「配置驱动」到底是运行时加载还是编译期重组。
2. **step 控制流表达力**：仅 `Vec<StepKind>`（线性）就能满足，还是需要 loop/on_fail 原语
   （否则 static-check/test-execution 反馈边、verdict 分叉仍留在 walker 特判）？这决定要不要
   翻案「单图硬编码」红线。
3. **artifact 的 phase 归属来源**：定义声明（phase → 产物路径清单，替换前端 docColumn）还是
   执行记录（step 落库时登记产出）？前者是配置问题，后者是运行态数据问题。
4. **会话/stdout 的存储形态**：内联 vs 引用。若内联 stdout，RunStepRecord 体积与 db 写放大
   要评估；若内联会话，与 SessionEventRecord 转录单表的「全史重放唯一来源」不变量如何共存。

---

## 第三轮：四条决策落地验证（db 定义 / 计划图+执行图 / artifact 清单+isPhaseArtifact / 会话回放+stdout 记录）

用户给出的四条方向，逐一对照真实代码的支点与裂缝。

### 决策① 定义载体 = db 表 —— 支点成立，但撞「prompt 静态化」红线

**支点**：workspace 库模型组是 additive 注册零迁移（`store.rs#workspace_models`，当前 11 模型）。
加 `WorkflowDefinition` / `PhaseDefinition` / `StepDefinition` 三张表纯 additive，无历史迁移负担。

```
workspace_models() 现有 11 模型
  AgentRunRecord / SessionRecord / SessionEventRecord / ExploreRecord
  ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord
  RunRecord / RunStepRecord / StoreMetaRecord
  +（拟）WorkflowDefinition / PhaseDefinition / StepDefinition / 依赖边表
```

**裂缝 A——prompt 文本进不进 db？** `desktop-phase-prompts` spec 有硬红线：「14 个 md 全静态
零占位，编译期 `include_str!` 显式字面路径装配，无 `<role>` 间接读取」。若定义表把 prompt
文本也收进 db，「全静态零占位」即被翻案；折中方案是**结构进 db、prompt 仍编译期静态**——
PhaseDefinition 表存 `prompt_ref: "proposal-planner"` 之类引用键，运行时再映射回 include_str!
静态表。这保留了「静态文本」红线，只把「结构」（节点 + 边 + artifact 清单）落库。

**裂缝 B——core/workflow 零 infra + 写面 sync 边界。** `desktop-workflow-write-face` 要求
core/workflow 零 infra 依赖、写面 sync（无 tokio）。定义表若成为路由权威（替代
`phase_table()`），读面要么经 `ChangeStateStore` port 扩展（定义读接口），要么定义加载在
壳层完成、core 只收「已加载的定义」——前者扩大 port 面，后者把定义加载从 core 挪出去。这
直接决定「配置驱动」的边界划在哪。

### 决策② 计划图 + 执行图，门禁失败/backtrack 产生倒序 —— 边语义已存在但未落库

**现状拆解**（图的两半目前形态不对称）：

```
计划图（plan）  = 编译期散落四处，无落库：
                  REQUIREMENT_TABLE（节点）+ prerequisites（依赖边）
                  + walker drive() 硬编码骨架 + STATIC_CHECK/TEST_EXECUTION_PHASES 常量

执行图（execution）= RunStepRecord 扁平列表（seq 升序，无「边」字段）
                    + 前端 deriveEdges 按 colIndex 差 Δ 派生边：
                      Δ>0 forward / Δ=0 retry / Δ<0 backtrack
```

**关键支点**：前端 `graph.ts#deriveEdges` 已经把「倒序边」的语义写出来了——`backtrack` 就是
Δ<0 的倒序边，「门禁失败/回溯产生倒序」在前端展示层已成立。缺的是：**边没有落库、没有成为
运行时控制依据**（walker 靠 `phase_next` 的 stale/backtrack_to 重算路由，不看执行图）。

**裂缝 C——反馈边不是图边。** static-check / test-execution 的「定向反馈边」目前是
「同会话续注（Continue）+ 重跑」的进程内 loop，不产生图上节点或回边。若执行图要把「门禁
失败」表达为倒序边，反馈边需显式化为节点 + 回边（`static-check → (fail) → executor 修复 →
static-check`），否则执行图里看不到这段反复。

**裂缝 D——执行图节点的身份粒度。** RunStepRecord 键 = `(run_id, seq)`，seq 是全词汇 emit 序，
不是稳定的「计划 step 实例 id」。要让执行图节点能指回计划图节点（哪个 step 的哪次 attempt
执行），需要显式的 `(phase, attempt, step, retry_no)` 类实例键，而非靠 seq 推导。

### 决策③ phase 定义 artifact 清单 + step 完善/检查 + isPhaseArtifact

**现状**：artifact 是「发现式」——`discover_artifacts` 扫文件树，phase 归属靠前端
`attachments.ts#docColumn(source)` 静态映射：

```
proposal.md / specs/**  → col:proposal
design.md / tasks.md    → col:dev-design
test-reports/**         → col:test-execution
```

**决策③的实质** = 把 `docColumn` 这张映射表从「前端硬编码」上移到「phase 定义」里，成为
PhaseDefinition 的 `artifact_manifest: [ArtifactSpec]` 字段；step 定义声明「产出/校验哪些
artifact」。`isPhaseArtifact` 是完整性约束：step 执行登记的产出必须命中本 phase 清单，否则
判为「无关联产出」拒绝挂载或告警。

**裂缝 E——发现式 vs 登记式两套信息源。** 现在 artifact 靠「事后扫文件树」发现，与 step 执行
无因果链；「step 负责完善/检查 artifact」要求 step 执行时**登记产出**（执行时写登记，而非事后
扫描）。两者并存会双源冲突：同一 markdown 产物既被扫描发现、又被 step 登记，去重与权威源要
定（建议登记式为权威、扫描降级为兜底/校验）。

**裂缝 F——「step 检查 artifact」与 evaluator 职责的重叠。** evaluator 现在已输出 checklist
（checklist 表项 = item/pass/evidence），这本身是一种「对 phase 产物的检查」。「step 检查
artifact」若另起一套检查语义，会与 evaluator 的 verdict/checklist 职责边界不清——要么明确
evaluator 就是「检查 artifact 的 agent step」，要么 artifact 检查是独立于 evaluator 的 gate。

### 决策④ 会话回放 infra/agent（已落地）+ stdout 交 infra/exec（现状是空壳）

**会话回放——已确认在 infra/agent，方向成立**：
```
transcript(session_id) → store.list_session_events(session_id)   // 重放唯一来源
ResumeTranscript 装载缝（lib.rs）→ SDK 引擎 start() 时重建对话史
CLI 引擎 Continue（compose begin SessionRef::Continue）同样经转录装载
```

**stdout 记录——infra/exec 目前只是 spawn 封装，无记录职责**：
`infra/exec/src/lib.rs` 只提供 `SystemCommand`（shell 包装 / platform flags / kill_tree），
不落任何输出。stdout 记录现在散落在 checks 层，且形态恰好对应你说的「临时 / 有 id 可查」两分：

```
「临时」（无 id，全量即丢）
  static-check：spawn → stdout+stderr 拼 diagnostics → 内存返回 → 落库只 summary(≤500 截断)
「有 id / 路径可查」
  test-execution：落盘 change_test_reports/<name>/（summary.json + plan/report.json）
                  StepRecord.reference 存 report_dir
  agent 会话：session_id 可查（SessionEventRecord 重放）
```

**决策④的实质** = 把「记录」职责从 checks 收敛到 infra/exec，形成两种输出模式：
`ExecOutput::Transient`（spawn 即得、反馈边消费、丢弃）与 `ExecOutput::Recorded { exec_id }`
（stdout 落库/落盘、可回查）。这会让 static-check 从「临时」升级为可选的「有 id 可查」，
而 test-execution 的 report_dir 与新的 exec_id 如何统一是存储决策点。

### 合起来看：两条既有 spec 红线需要显式翻案

四条决策合起来，是把「编译期硬编码计划 + 内存/派生执行史」升级为「db 持久化计划图 + db
持久化执行图 + phase 声明的 artifact 清单 + exec 层统一输出记录」。但有两处直接撞既有红线，
不能隐式吸收：

1. **「单图硬编码，MUST NOT 实现通用图引擎或图定义文件格式」**（desktop-change-orchestration
   V1 边界第 3 条）——计划图/执行图 graph 模型本身就是对这条的翻案，需要显式立项改 spec。
2. **「prompt 全静态零占位，编译期 include_str!」**（desktop-phase-prompts）——若 prompt 文本
   进 db 定义表则翻案；折中是结构进 db、prompt 仍静态引用。

### 建议下一轮追的点（按依赖序）

1. **定义表落在 core 还是 infra，port 面怎么扩**：定义读面经 `ChangeStateStore` port 扩展，
   还是壳层加载后注入 core？这决定 core/workflow「零 infra」红线要不要改。
2. **执行图边的存储形态**：边表（run_id + from_seq + to_seq + kind）单独落，还是节点内联
   next 指针？前端 deriveEdges 的派生逻辑要不要退役、改为读落库边。
3. **执行图节点稳定身份键**：`(phase, attempt, step, retry_no)` 实例键如何与现有
   RunStepRecord.seq 共存（seq 保留 emit 序，实例键用于指回计划图）。
4. **artifact 权威源**：登记式（step 产出登记）为权威、扫描式（discover_artifacts）降级为
   校验/兜底，双源去重规则。

---

## 第四轮：四条决策定稿后的目标模型（计划图=源码 / 执行图=db / 边表 / 登记 artifact）

用户四条定稿：① 纠正——db 仅存执行记录，定义同 prompt 落源码；② 边表单独落；③ 不考虑
历史兼容，重设计运行记录 model；④ 仅保留登记式 artifact。逐条落地后，目标模型可画出。

### 决策①纠正的连锁效果：上一轮两个红线裂缝都消失了

上一轮担心的两处翻案，这一纠正后**不再需要**：

- 「prompt 全静态零占位」红线：**不翻案**。定义仍走 `include_str!`（prompt）+ const 表（结构），
  与现状 `phase_table.rs` 完全同构，只是 `PhaseDefinition` 结构扩展。
- 「core/workflow 零 infra + 写面 sync」红线：**不翻案**。定义仍是 core/workflow 自持的编译期
  静态，不经 db、不经 port 缝、不经 tokio——上一轮「定义表落 core 还是 infra」的追问整个消失。

真正被翻案的只剩一条：**「单图硬编码，MUST NOT 实现通用图引擎或图定义文件格式」**。因为
`PhaseDefinition` 从「executor/evaluator 两个 agent spec」扩展成「有序 step 序列 + 依赖边 +
artifact 清单」，这本身就是把 drive() 硬编码骨架下沉为 plan 图结构。

### 目标模型：计划图（源码）与执行图（db）的对齐

```
┌─ 源码（core/workflow 编译期静态，const 表 + include_str!）────────────────┐
│ WorkflowDefinition（"requirement"）                                         │
│   ├─ phases: [PhaseDefinition]                                             │
│   │    ├─ id / description                                                 │
│   │    ├─ steps: [StepDefinition]          ← 计划图节点（exec/agent/gate） │
│   │    │    kind(exec|agent|gate) · role · prompt_ref · on_fail 语义        │
│   │    ├─ artifact_manifest: [ArtifactSpec] ← phase 声明产物清单           │
│   │    │    kind · source 模板 · title                                      │
│   │    └─ executor/evaluator: PhaseAgentSpec(prompt_ref + model_level)     │
│   └─ prerequisites（依赖边，plan 图形态，stale 传播源）                      │
└───────────────────────────────────────────────────────────────────────────┘
                          │ 运行期实例化（walker 按 step 序列执行）
                          ▼
┌─ db（仅执行记录；不计历史兼容 → 重设计）──────────────────────────────────┐
│ RunRecord            run 主行（run_id / status / started_at / finished_at）│
│ RunNodeRecord        执行图节点（重设计）                                   │
│   ├─ 稳定身份键 (run_id, phase, attempt, step_instance)                    │
│   ├─ step_kind 全词汇（含 phase_start / phase_log / gate 三门）            │
│   └─ status · session_id · detail · stdout/exec 引用                       │
│ RunEdgeRecord        边表（单独落）                                         │
│   └─ from_node_key → to_node_key · edge_kind(forward/retry/backtrack)      │
│ ArtifactRecord       登记式产物（run/节点归属）                             │
│   └─ run_id · node_key · kind · source · phase · title                     │
└───────────────────────────────────────────────────────────────────────────┘
```

计划图（源码）与执行图（db）经「step kind + phase」对齐：节点是计划的运行时实例，边显式落
库，前端 `deriveEdges` 按 colIndex 差 Δ 派生的逻辑退役、改读边表。

### 决策③的松绑面：run_history.rs 的 10→5 过滤整个退役

「不考虑历史兼容」直接解锁了当前最别扭的一处——`run_history.rs#persisted_step` 的 10→5 过滤
（phase_start / phase_log / verdict_gate / retry_gate / whitelist_gate 被弃，理由是「纯 Rust
分支无会话无独立产物，图史价值为零」）。执行图要完整表达「按 step 顺序执行 + 门禁失败/回溯
倒序」，这些流程面步骤与 gate **必须上图**，否则边表连不起来。所以：

```
退役：RunStepRecord.seq（emit 序盖戳）+ persisted_step 过滤 + 10→5 映射
取代：RunNodeRecord 全词汇 + 稳定实例键 (run_id, phase, attempt, step_instance)
      + RunEdgeRecord 边表
```

「不计兼容」还对齐了 store 既有惯例：`WORKSPACE_STORE_FORMAT_VERSION` 探测作废重建，旧形态
只有「不可达 / 整体丢弃」两条路，无第三条 decode/migrate 路径——新执行图模型走 version bump
整体重建，零迁移负担。

### 决策④的拆解：三个 artifact 插件命运不同

登记式把「候选来源」从「扫文件树 + eval 条目」换成「读登记记录」，但三个 plugin 不能一刀切：

```
eval-checklist   纯 PhaseRecord 投影，零文件扫描 → 天然「登记」（PhaseRecord 落账即登记），
                 保留，不改来源
markdown-doc     现扫全树 .md → 改读登记记录；title/rank 名单逻辑保留（标注），
                 matches 从「扩展名==md」改为「登记类型==markdown-doc」
tasks-progress   tasks.md 特化视图 → 同 markdown-doc 改来源；勾选计数 parser 保留
```

退役面：`registry.rs#enumerate_candidates`（文件树遍历 + eval 序号候选）整个退役；
前端 `attachments.ts#docColumn(source)` 的 phase 归属静态映射退役（phase 归属直接来自登记记录）。
`discover_artifacts` 签名从 `(change_dir, phases)` 变为「读登记表」。

### 剩余要凿实的三点（都是新模型内部的设计题，不再撞 spec 红线）

1. **artifact 登记挂哪**：独立 `ArtifactRecord` 表，还是挂在 `RunNodeRecord` 的 payload 里？
   「db 仅存执行记录」暗示登记是执行产物——若挂节点，ArtifactRecord 可以省；但 artifact 要
   跨 run 聚合（change 详情页按 phase 列展示全史产物），独立表 + run 归属更顺。倾向独立表。
2. **节点身份键的 step_instance 语义**：反馈边里 executor 会「同会话续注重跑」多次，这些
   重跑是同一 step_instance 的多个 attempt，还是各自独立节点？这决定 feedback 循环在图上是
   「回边指向同一节点」还是「新节点 + retry 边」。
3. **边表键与查向**：native_db 复合键打包先例（`hash64(a)<<64|b`）现成，但边表要支持
   「按 run 全量读」+「按 from_node 读后继」两向——是否加二级索引（`run_id` 已有先例，
   `from_node_key` 若要查后继需另立索引）。

---

## 第五轮：三点定稿 + 一个历史线索（inventory 曾退役，本轮是轻量找回）

用户三点定稿：① artifact 按相对 change 路径关联、phase 关联哪些由源码决定；② 反馈边重跑
= 新节点；③ 同意边表二级索引。逐条落地后，两个历史性发现值得记下。

### 历史线索：`inventory` 曾是「由 workflow 模型驱动的产物清单」，已退役

grep 命中了 `target/debug` 编译缓存里的历史诊断——`discover_artifacts(&change_dir, inventory,
workflow_model.as_ref())` 传了 3 参数（当前签名是 2 参数），说明**历史上 artifact 曾由
`workflow_model` 驱动**。而 `corpus_golden_test.rs` 现在断言 detail 投影「不含 `inventory` /
`fileLog` / `unparsable` 三退役键」：

```
历史：workflow.json 的 inventory 字段（产物清单）→ 落 detail 线面
退役：inventory / fileLog / unparsable 三字段整体删除
现状：discover_artifacts 扫文件树 + 静态插件 matcher（无定义驱动）
```

决策①的意义 = 在 inventory 退役之后，用**更轻的形态**找回「由定义决定 artifact 归属」：不落
库、不重建 inventory 字段，只让**源码 phase 定义声明「相对路径 → phase」的映射**。

### 决策①落地：artifact 关联键 = 相对路径（现状 source 已是），归属 = 源码 phase 清单

现状 `ArtifactDescriptor.source` **已经就是相对 POSIX 路径**（`encode_file_source`），所以
「相对路径关联」零新增概念——关联键复用现有 `source`。变化在「发现」这一半：

```
现状（黑名单/全量扫描）：扫全树 .md → 全部都是 markdown-doc → 前端 docColumn 决定挂哪列
                          （表外文档仍在 artifact 列表，只是不挂列）
目标（白名单/清单驱动）：phase 定义声明 ArtifactSpec { kind, path 模式, title }
                          → 按清单枚举匹配 change 目录文件 → 命中才成为该 phase 的 artifact
                          → 前端 docColumn 静态映射退役（上移源码）
```

`discover_artifacts(change_dir, phases)` 变为「读 phase 定义清单 + 按路径枚举 + 插件 parse」；
`enumerate_candidates` 的文件树遍历退役。三个插件里 eval-checklist 是**异类**——它是
PhaseRecord 投影而非文件，天然挂在 (phase, attempt) 上，不走「相对路径」框架，需单独保留
「phase+attempt 关联」分支（其余两插件改走相对路径清单）。

### 决策②落地：反馈边重跑 = 新节点，取代前端「槽位配对」机制

现在前端 `run-state.ts#runStepNodes` 用「槽位配对」处理同一 `(phase:attempt:step)` 的多次出现：
running 状态 push 一个 open slot，终态找最近 open slot 配对填充，同键多 running 用 `:seq` 后缀
堆叠。这套机制的**存在本身**就是因为后端节点没有稳定身份。

决策②「新节点」= 后端给每次执行一个稳定实例键：

```
节点身份键 = (run_id, phase, attempt, step, instance_no)   // 每次执行 instance_no 递增
```

于是前端 `runStepNodes` 的槽位配对 + `:seq` 堆叠整个退役，节点直接按后端身份渲染。这也让
「边」推导干净化：`from_instance → to_instance` 显式落库，不再靠 colIndex 差 Δ 派生。

**新观察——边 kind 从 3 种变 4 种。** 前端 `graph.ts#deriveEdges` 现在只有 forward(Δ>0) /
retry(Δ=0) / backtrack(Δ<0) 三种。但「step 粒度」暴露了 phase **内部**的反馈边：

```
executor(首次) → static-check(fail) → executor(修复#1) → static-check(重跑) → ...
        └──────── feedback（同 phase 内回到 executor，不是 phase 级 retry）────┘
```

feedback 是「gate 失败回到 agent step」的 step 级回边，与「phase fail 后整个相位重跑」的
retry 不是一回事。所以边 kind 需扩为 forward / feedback / retry / backtrack 四值——这是
「相位粒度 → step 粒度」自然暴露的第四种边，之前被「相位列 + Δ 差」的粗粒度压平了。

### 决策③落地：边表两向查询 + 二级索引

边表 `RunEdgeRecord` 用 native_db 复合键打包先例（`hash64(run_id)<<64|seq`）做主键，
另立两个二级索引：

```
run_id         二级索引（按 run 全量读边，先例已有）
from_node_key  二级索引（按 from_node 查后继，需新立）
```

### 至此的设计闭环（可进入 change 提案的完整图景）

```
源码（编译期静态）                    db（仅执行记录，重设计）
WorkflowDefinition("requirement")    RunRecord（run 主行）
 ├─ PhaseDefinition                  RunNodeRecord（全词汇节点 + 稳定实例键）
 │   ├─ steps[]（计划图节点）          RunEdgeRecord（边表：forward/feedback/retry/backtrack
 │   ├─ artifact_manifest[]（路径清单）           + run_id/from_node 两二级索引）
 │   └─ executor/evaluator spec      （artifact 不再落库——靠相对路径 + 源码清单关联）
 └─ prerequisites（依赖边）

退役清单：RunStepRecord.seq + persisted_step 10→5 过滤、前端 deriveEdges Δ 派生、
         前端 docColumn 静态映射、前端 runStepNodes 槽位配对、enumerate_candidates 全树扫描
翻案红线：仅「单图硬编码」一条（prompt 静态化 / core 零 infra 两条不翻案）
```

探索到此已经收敛：目标模型、存储形态、退役面、翻案红线都齐了，可以固化为 OpenSpec change
提案（proposal + spec delta）。剩下两个纯实现期的小题可留到 design 阶段定：eval-checklist 的
「相对路径 vs phase+attempt 关联」分支如何与其余插件统一表达、以及 feedback 边的 kind 命名。
