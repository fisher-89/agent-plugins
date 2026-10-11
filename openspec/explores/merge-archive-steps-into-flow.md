# 归档步骤并入 phase → step 模型并合入流程图

日期: 2026-10 · 状态: 探索中（源码调研完成，方向待选）

## 主题与背景

`packages/desktop` 的 change 详情页目前有两套并行的执行模型：

1. **run 流程**已走完「phase → step」抽象——9 站流水线，每站下挂 step 序列
   （`RunStepKind` 十值全词汇），节点 + 边落库（`RunNodeRecord` / `RunEdgeRecord`），
   流程图常驻渲染。
2. **归档流程**仍是独立的六段阶段机（`ArchiveStage`：preflight / commit / merge /
   specSync / seal / finalize），进程内注册表、**不落库**，UI 是独立面板的六段清单
   （`ArchivePanel` + `StageList`），不在流程图中。

用户方向三句话：① 将归档步骤也并入 phase → step 模型；② 保持独立手动执行；
③ UI 层也合入流程图中。本笔记记录两套模型在真实代码里的落点、它们之间的语义裂缝，
以及「并入」需要先对齐的几个设计分叉。

## 代码地图

| 关注点 | 位置 |
|--------|------|
| run 步词汇十值（`RunStepKind`）与步状态 | `crates/core/workflow/src/state.rs` |
| 9 站流水线（`PIPELINE_PHASES`） | `crates/core/workflow/src/queries/detail.rs` + 前端 `flow/layout.ts` |
| run 节点/边落库（`RunNodeRecord`/`RunEdgeRecord`） | `crates/infra/store/src/model.rs` |
| run 步 → 图节点转换（`runStepNodes`/`unifiedRunNodes`） | `src/views/changes/flow/run-state.ts` |
| 图组装（`buildFlowGraph`） | `src/views/changes/flow/graph.ts` |
| 归档六段阶段机（`ArchiveStage` + `drive()`） | `crates/core/orchestration/src/archive_flow.rs` |
| 归档链进程内注册表（`ArchiveControl`，不落库） | 同上（`ArchiveControl`/`ArchiveGuard`） |
| 归档命令面（五命令） | `src/commands/archive_flow/mod.rs` |
| 归档前端 reducer（`ARCHIVE_STAGES`/`applyArchiveUpdate`） | `src/views/changes/flow/archive-state.ts` |
| 归档 UI（`ArchivePanel` 六段清单） | `src/views/changes/flow/archive-panel.tsx` |
| 归档 hook（`useArchiveFlow`） | `src/views/changes/hooks/use-archive-flow.ts` |
| 归档写面双写（改名 + status 翻转） | `crates/core/workflow/src/write/archive.rs` |
| 详情页拼缝（两套模型装配点） | `src/views/changes/change-detail-view.tsx` |

## 现状：两套并行执行模型

```
run 流程（已 phase → step 化，落库，常驻流程图）
  9 站 PIPELINE_PHASES × 每站 step 序列（RunStepKind 十值）
  ├─ 落库：RunNodeRecord / RunEdgeRecord，稳定身份键 phase:attempt:step:instanceNo
  ├─ 边四 kind：forward / feedback / retry / backtrack
  ├─ 三类节点：WorkerAgent / ToolStep / Gate
  └─ UI：react-flow 图（ChangeFlowGraph），常驻 + 在飞 run 活面叠加

归档流程（独立链，六段阶段机，进程内不落库）
  六段 ArchiveStage：preflight / commit / merge / specSync / seal / finalize
  ├─ 状态四值：running / passed / skipped / failed（注意 skipped，run step 无此态）
  ├─ 不落库：ArchiveControl 进程内注册表，终态除名，由 ChangeRecord.status=archived 承载
  ├─ 独立命令面：archive_flow_preflight / start / stop / state / watch
  ├─ 与 run 双向互斥：run 在案拒归档，归档在案拒 run
  └─ UI：ArchivePanel 独立面板（StageList 六段清单），不在流程图
```

两套模型的语义裂缝（并入时需正面处理）：

1. **状态词汇不对齐**：归档段有 `skipped`（五类跳过因：干净 / 已合入 / 无 delta /
   legacy 无 worktree / 已落盘），run step 是 `stopped`。归档的 `skipped` 是幂等语义
   （重试时跳过已完成半截），run step 没有对应词。
2. **身份键不对齐**：run 节点有 `(phase, attempt, step, instanceNo)` 四段稳定键（attempt
   跨 run max+1、instanceNo 区分 feedback 重跑）；归档链是一次性的，无 attempt /
   instanceNo 概念，重试 = 重新发起整链（幂等续走，不是 attempt 递增）。
3. **持久化不对齐**：run 节点落库常驻；归档不落库，收口后 `ArchiveControl` 除名、
   快照返回 `None`，六段进度随之蒸发，详情页 refresh 只回落「已归档」形态（按钮消失）。
4. **执行器不对齐**：归档六段只有 specSync 与 merge（冲突解算）涉及 agent 会话，其余
   四段是纯 Rust 内联操作（git 提交/合入、写面 archive、落盘提交），类似 ToolStep 但
   比 run 的 ToolStep 更「编排内联」（run 的 ToolStep 是 static_check / test_execution
   等进程外工具）。

## 需求拆解（三句话 → 三个约束面）

```
① 归档步骤并入 phase → step 模型
   └─ 归档六段成为「某个 phase 下的 step 序列」，可观测、可停止、可上图

② 保持独立手动执行
   └─ 归档不进入 walker 的自动推进链；仍有独立确认对话（preflight）、start/stop；
      与 run 双向互斥保持

③ UI 层合入流程图
   └─ ArchivePanel 的六段清单（StageList）退役，六段作为流程图中的节点呈现
```

三句话之间有一个天然张力：**①③ 要求归档可上图且常驻（否则刷新即蒸发），② 要求它
不是 run**。这直接指向最关键的持久化分叉（见下 C 分叉）。

## 关键设计分叉

### A. 归档在 phase 模型中的位置

归档链不属于现有 9 站中的任何一站（它在 run 全部收口之后才发起）。三种落法：

```
A1  归档作为第 10 列（phase）加入 PIPELINE_PHASES
    ├─ 后端 detail.rs + 前端 layout.ts 双侧同步（既有红线「改动需双侧同步」）
    ├─ 语义最干净：归档就是一个 phase，六段是其 step 序列
    └─ 代价：PIPELINE_PHASES 是「路由相位 + 展示列」混合体，加归档列会拉大
       两者距离（code-analyze 已是「无路由纯占位列」先例）

A2  归档复用 code-analyze 列
    ├─ code-analyze 当前是纯占位列（无路由无 spec，探索 run-workflow-by-step
      已点名它是「占位还是预留」）
    ├─ 把归档六段挂到 code-analyze 列下，省一列
    └─ 但 code-analyze 语义上是「代码分析」，与归档无关，语义污染

A3  归档作为独立「归档泳道」，不进 PIPELINE_PHASES，但进 FlowGraph.columns
    ├─ 流程图层追加一列，路由层完全不动
    └─ 与「并入 phase → step 模型」字面有张力：它成了图上的特殊列而非真 phase
```

### B. 归档六段 → step 词汇

归档六段与 run 十值 `RunStepKind` 零重合。三种映射：

```
B1  扩展 RunStepKind（十值 → 十六值），归档六段并入同词表
    ├─ 优点：一张词表、一个节点类型、一条渲染路径
    └─ 缺点：run 与归档的语义边界混在一个 enum，落库过滤/查询/边语义都要特判

B2  新增独立 ArchiveStepKind（六值），归档节点类型独立
    ├─ 与 run 的 RuntimeFlowNode 并列一个新的归档节点 kind
    ├─ 归档段状态四值（running/passed/skipped/failed）独立于 RunStepStatus
    └─ 代价：FlowNode 联合类型 + 渲染 + 转换都多一套分支

B3  归档六段映射到现有三分类（不新增词汇）
    ├─ preflight/commit/seal/finalize → ToolStep；specSync → WorkerAgent；
      merge（冲突解算）→ WorkerAgent
    └─ 但丢失「这是归档的哪一段」的语义（六段名不可见，只剩三分类徽章）
```

B1/B2 之间还有一个子问题：归档段状态四值的 `skipped` 与 run 的 `stopped` 不兼容。
若 B1 复用 `RunStepStatus`（running/passed/failed/stopped），归档的 `skipped` 无处安放；
若 B2 独立 `ArchiveStepStatus`，则四值自然对齐（running/passed/skipped/failed）。

### C. 归档是否落库（常驻渲染的关键）

这是最硬的分叉，直接决定「刷新后归档节点还在不在」：

```
C1  落库到新表（ArchiveNodeRecord / ArchiveEdgeRecord）
    ├─ 归档史常驻流程图（与 run 的 RunNodeRecord 同构）
    ├─ 归档终态后 ChangeRecord.status=archived，但归档节点仍可读
    └─ 代价：新增 model + store 操作面 + ChangeStateStore trait 扩展 + 格式版本 bump

C2  复用 RunNodeRecord（归档当作一次特殊 run）
    ├─ 不新增表，但归档不是 run，会混淆 run 语义（run 五值状态、run_id 铸造等）
    └─ 归档链无 attempt/instanceNo，硬塞进 run 的身份键语义别扭

C3  不落库，仅运行期实时叠加（像 activeRun）
    ├─ 归档进行中六段上图，收口后节点随 ArchiveControl 除名消失
    ├─ 归档完成后详情页只剩「已归档」终态（现状），六段进度不可回溯
    └─ 最省改动，但「UI 合入流程图」的常驻性不成立——收口即蒸发
```

观察：run 的「常驻」是靠落库 + 全史叠加（`runs[].nodes`）实现的。归档若要同等常驻，
C1 是最对称的路径；若接受「归档只在进行中上图、终态回落 status=archived」，C3 可行
但语义打折。

### D. 归档的 attempt / instanceNo 从哪来

run 节点身份键四段 `(phase, attempt, step, instanceNo)` 是前端直读的稳定键。归档链：

- 无 attempt（一次性，重试 = 重新发起整链幂等续走，不是 attempt 递增）；
- 无 instanceNo（六段各自最多一次，merge 冲突解算的多次循环是「同段后写覆盖 detail」，
  不产生多实例节点）。

若并入 step 模型，归档节点需要一套身份键。最简：`(archive, 0, 段名, 0)` 固定 attempt=0 /
instanceNo=0；或归档列下的节点用「段名」单一维度，不套用 attempt/instanceNo。

### E. preflight 确认对话与流程图的边界

归档的「确认对话」数据面（完成度警告 / delta specs capability 清单 / merge 目标分支 /
run_active 拒绝卡）是归档特有，不属于 step 节点。并入流程图后：

- 确认对话（preflight 数据面）大概率保留为面板——它发生在链发起之前，不是执行进度；
- 只有「进行面」的六段（发起后的进度）转成流程图节点；
- 归档的会话转录（specSync / merge 冲突解算）应像 run step 那样挂到对应节点（现有
  `detail-drawer` 的转录联动已有 `RoleSessionRef` 反查机制，归档转录目前是面板内独立
  的 `ArchiveTranscript`）。

### F. 与 run 的双向互斥如何表达在图上

归档与 run 双向互斥（run 在案拒归档、归档在案拒 run）。若归档成了图上的第 10 列，图上
会出现「run 九站 + 归档列」并存的形态，但两者时间上互斥（归档只在 run 收口后发起）。
图是否需要表达这种互斥（如归档列在 run 收口前呈现空列/置灰），还是复用现有「空列恒定」
规则（9 站恒定，未走的站呈现空列）直接扩展到第 10 列——值得先定。

## 值得追问的线索

1. **归档列是否真进 `PIPELINE_PHASES`**：9 站现在是「路由相位（8）+ 展示占位（code-analyze）」
   的混合体，归档是「非路由、非 run、独立手动」的第三类。把归档塞进同一个 `PIPELINE_PHASES`
   常量，会进一步拉大「列身份」与「路由权威」的距离（spec 红线：PIPELINE_PHASES 只是列
   身份，MUST NOT 上位为路由权威）。A3（独立泳道）是否更尊重这条红线？
2. **`skipped` 语义要不要进入 step 状态词汇**：归档五类跳过因是幂等重试的关键；run step
   没有 skipped。若并入，是给 `RunStepStatus` 加 `skipped`（污染 run 语义），还是归档独立
   状态（B2）？
3. **归档落库的边界**：C1 新增 ArchiveNodeRecord 与 C3 零落库之间，有没有中间态——只落
   「归档终态摘要」（summary + 六段终态快照）到 ChangeRecord 或一张轻量表，六段常驻但
   不落边（归档链本质线性，边可派生）？这比 C1 轻、比 C3 常驻。
4. **归档转录的寻址**：现有 `RoleSessionRef` 反查靠 `sourceRef = <id>/<phase>/<role>/<attempt>`
   三段/四段定式；归档会话的 `sourceRef` 是 `<id>/archive/spec-sync`、`<id>/archive/merge-conflict`
   （archive_flow.rs 里写死）。并入流程图节点后，这两个 sourceRef 与「归档列 phase 名」如何
   对齐，决定转录能否复用现有抽屉联动。
5. **`useArchiveFlow` 与 `useChangeFlowRun` 的关系**：现在两 hook 并置（`useDetailViewData` 里
   分别装配）。若归档并入 step 模型，归档的实时流（`ArchiveUpdate`）是否要收敛到 run 的通知
   桥（`RunNotice`），还是保持第三面命令组（archive_flow_*）隔离？

---

## 附：两套模型的关键类型速查

```
RunStepKind（十值，snake_case 线格式）
  Executor / Evaluator / Decision            ── WorkerAgent 三角色
  StaticCheck / TestExecution                ── ToolStep（进程外工具）
  PhaseStart / PhaseLog                      ── ToolStep（相位机步）
  VerdictGate / RetryGate / WhitelistGate    ── Gate 三门

RunStepStatus（四值）：running / passed / failed / stopped
RunEdgeKind（四值）：forward / feedback / retry / backtrack

ArchiveStage（六值，camelCase 线格式）
  Preflight / Commit / Merge / SpecSync / Seal / Finalize
ArchiveStageStatus（四值）：running / passed / skipped / failed
ArchiveUpdate（三变体）：Stage / SessionEvent / Finished
```

归档六段中只有 `specSync` 与 `merge`（冲突解算分支）跑 agent 会话，其余四段是
`drive()` 里同步内联的 git / 写面调用——这段不对称在并入 step 模型时，会决定归档
节点是否也分「WorkerAgent / ToolStep / Gate」三分类，还是归档列节点统一一种样式。

---

## 第二轮：A=第10列 / B=扩展词汇的分类粒度 / C=复用 RunNodeRecord 的落点验证

用户定稿：**A** 归档进第 10 列；**B** 扩展 RunStepKind，但分类粒度待定（合并一类
vs exec/git 两类）；**C** 复用 `RunNodeRecord`。逐条对照真实代码验证落点与裂缝。

### A 确认：第 10 列的物理落点

`PIPELINE_PHASES` 有两处镜像，需双侧同步（既有红线「改动需双侧同步」）：

```
crates/core/workflow/src/queries/detail.rs   pub const PIPELINE_PHASES: [&str; 9]
src/views/changes/flow/layout.ts             export const PIPELINE_PHASES: readonly string[]
```

第 10 列追加 `"archive"` 后，`colIndexOf(phase)`（前端 `graph.ts` / `run-state.ts`）
与后端 `pipeline.iter_mut().find(...)` 自动把归档列纳入（两者都是 `indexOf` /
`find` 语义，不依赖 9 的硬数字）。真正的语义问题是：`phase_table(workflow_type)`
路由表仍是 8 相位（code-analyze 先例：无路由纯占位列），归档列同样走「无路由、
列身份」路线，不触碰 `phase_next` 路由权威——符合 spec 红线。

### B 的分类粒度：归档六段执行器精确拆解

StepGroup 三分类（`workflow/state.rs`）是**派生分组**（`StepNode::group()`），且
walker 的 `run_step` 实际是 `match step.node` 逐变体特判（每个 StepNode 一个分支，
不是按 group 分派）。前端 `run-state.ts#groupOf` 是渲染徽章分组。**两处都不是执行
权威**——执行体由 walker（run 侧）与 archive_flow::drive（归档侧）各自持有。

归档六段的真实执行器（读 `archive_flow.rs#drive`）：

```
preflight    store 读 + completion_of + vcs.current_branch/branch_merged   只读校验（零写）
commit       vcs.commit_all                                                 git 写
merge        vcs.rebase_branch + vcs.ff_merge；冲突时 worker.run(agent)     git 写 + 条件 agent
specSync     worker.run                                                     agent 会话
seal         fs::rename + db set_archived（workflow::write::archive）       内联写面（非 git 非 agent）
finalize     vcs.dirty + vcs.commit_paths                                   git 写
```

所以真实是**三类执行器**：agent（specSync + merge 冲突分支）、git（commit/merge/
finalize）、内联（preflight/seal）。与现有 StepGroup 三分类（Exec=进程内脚本步 /
Agent=会话 / Gate=分支门）对照：

| 归档段 | 执行器 | 现有 StepGroup 映射 |
|--------|--------|--------------------|
| preflight | 内联只读 | Exec（进程内直调，同 phase_start 性质） |
| commit | git 写 | **无对应分类**（Exec 语义是「相位机步+检查 spawn」，不是 git） |
| merge | git 写 + 条件 agent | **混合**：git 主体 + 冲突时 Agent 子分支 |
| specSync | agent | Agent（复用） |
| seal | 内联写面 | Exec（进程内直调） |
| finalize | git 写 | **无对应分类** |

两个选项的评估：

```
B-合并一类  六段全归一个 group（全归 Exec，或新加 archive 组）
    ├─ 优点：改动最小，前端 groupOf 只需把六个新词落到一个分支
    ├─ 缺点：git / 内联 / agent 三类执行器在徽章上不可辨（但 step 名本身已可辨）
    └─ 关键：归档独立手动执行，分组不参与执行逻辑，纯渲染语义——代价有限

B-exec/git 两类  git 段（commit/merge/finalize）与内联段（preflight/seal）分两 group，
                specSync 复用 Agent
    ├─ 优点：执行器类型在图上可辨（「这段在动 git」vs「这段在内联收口」）
    ├─ 代价：StepGroup 三分类 → 四分类（Exec/Agent/Gate/Git），
    │        后端 group() 派生 + 前端 RunStepGroup + GROUP_LABEL 三处同步
    └─ merge 的「git + 条件 agent」仍是一个节点装两种执行器，分组只能取主导（git）
```

关键洞察：**因为归档保持独立手动执行（archive_flow::drive 独立，不进 walker 分派），
StepGroup 对归档六段来说纯属前端渲染徽章，不影响任何执行逻辑**。所以这个分类是
「可观测粒度」的产品选择，不是架构约束。若只想让流程图能跑、六段可辨，合并一类
（step 名区分）已够；若想让 git 操作在图上高亮可辨，分 exec/git 两类更诚实。

### C 复用 RunNodeRecord 的四个硬裂缝

`RunNodeRecord`（model.rs id=16）字段形状基本能装下归档六段（phase=archive、
attempt、step、instance_no、status、session_id、detail、output_ref），但有四处
语义裂缝：

```
裂缝 C1  RunStepStatus 无 skipped
    RunStepStatus 四值 = running / passed / failed / stopped
    归档五类跳过因（干净 / 已合入 / 无 delta / legacy 无 worktree / 已落盘）
    是幂等重试的核心语义，四值装不下。
    选项：a) 加 skipped（六值，run 侧从无跳过——语义污染）
          b) 跳过段不落节点（跳过 = 没执行 = 不上图，但幂等形态不可见）
          c) 跳过段落 passed + detail 携跳过因（零词汇改动，语义扭曲）

裂缝 C2  run_id 从哪来
    RunNodeRecord 主键 node_key = hash64(run_id)<<64 | node_seq，run_id 二级索引；
    它天然是「run 的子表」。归档不是 run，没有 run_id。
    detail 出线走 runs[]（ChangeRunEntry.nodes），归档节点要上图须挂进某个 run。
    选项：a) 归档铸造伪 run_id + 落 RunRecord 主行（status 五值无 archived）
          b) detail 独立出线 archive 字段（复用 RunNodeRecord 表结构，
             但不走 runs[] 的 run 语义）

裂缝 C3  身份串唯一性
    node_id = "{phase}:{attempt}:{step}:{instance_no}"，唯一性靠「attempt 跨 run
    max+1 分配」。归档 attempt 恒 0 → 两次归档的 archive:0:commit:0 撞号。
    但归档链线性无回边（无 feedback/retry/backtrack），若零边（见 C4），
    身份串撞号不致命——前端节点 id 可用 run_id 前缀消歧。

裂缝 C4  归档链可能不需要边
    六段串行无回边，RunEdgeRecord 的四 kind（forward/feedback/retry/backtrack）
    对归档无用武之地。复用 RunNodeRecord 但零 RunEdgeRecord（边派生或省略）是
    合理路径——这反过来减轻 C3 的身份串唯一性压力（无边端点引用）。
```

这四个裂缝的共同根因：**RunNodeRecord 是「run 的子表」**（run_id 归属 + node_key
打包 + node_id 身份串三处都锚定 run 语义），而归档是「非 run 的独立手动链」。复用
它 = 要么让归档「伪装成一次特殊 run」（C2a，需要给 RunRecord/RunStatus 加归档语义），
要么「复用表结构但 detail 独立出线」（C2b，RunNodeRecord 表复用、runs[] 语义不混）。

### 需要先对齐的两个决策点（比 B 的分类粒度更靠前）

1. **归档节点上图走哪条出线**：伪装成特殊 run 挂进 `runs[]`（复用 ChangeRunEntry
   / unifiedRunNodes 整条读面，但 RunStatus 要加 archived 语义），还是 `ChangeDetail`
   增独立 `archive` 字段（复用 RunNodeRecord 表、前端 buildFlowGraph 单独拼一列）。
   这决定 C2 的形态，比 B 的分组更早定。
2. **skipped 语义归宿**：归档五类跳过因是幂等重试的关键。若「跳过段也要常驻上图」
   （用户能回溯「上次归档 commit 段因干净而跳过」），RunStepStatus 必须扩 skipped
   或走 C1c 的 detail 携带；若「跳过段不上图」，则六段进度图在幂等重试时会缺段。
