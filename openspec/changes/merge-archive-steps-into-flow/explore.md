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

---

## 第三轮：方向修正——归档视为普通 step，之前的 C2/C3/C4 裂缝自然消解

用户两条定稿：**① 归档视为普通 step，`auto_next_phase` 补充「按指定 phase 停止」
能力；② 跳过语义——直接跳过 step（计划层不进序列）不落节点，step 内判断（执行了
发现无事可做）保留记录，符合 step 原本设计（不扩 `RunStepStatus`）。**

这两条推翻了第二轮的一个前提假设（「归档保持独立手动执行，archive_flow::drive
独立，不进 walker 分派」）。归档从「第三面命令组」降为「walker 第 10 相位的 step
序列」，连锁效果是**第二轮的 C2 / C3 / C4 裂缝全部自然消解**：

```
C2 run_id 从哪来       消解：归档节点就是当前 run 的节点，run_id 天然有，
                        落进 RunNodeRecord 走当前 run 的 nodes 整包，零伪 run
C3 身份串唯一性        消解：归档 phase 若也走 phase_start 开相，attempt 由
                        max+1 分配，archive 的 attempt 与 9 站同源不撞号
C4 边                  消解：六段串行 forward 边，复用 EdgeKind::Forward，零新边 kind
```

剩 C1（RunStepStatus 无 skipped）由用户决策 ② 收口：**跳过因走 `detail` 携带、
status 恒 `passed`，不扩词汇**——这正好是第二轮 C1c 的方向，且更干净（不是「语义
扭曲」，而是「step 内判断 = passed + 记因」本就与 run step 的
`passed(detail)` 语义同构）。

### 归档六段的「跳过」全部分类为 step 内判断

```
preflight   校验不过 → failed；过 → passed                        无跳过
commit      vcs.dirty 探测 false → 干净跳过（step 内判断）        保留记录
merge       vcs.branch_merged true → 已合入跳过（step 内判断）     保留记录
specSync    无 delta / 用户选择跳过（step 内判断）                 保留记录
seal        双写收口，无跳过                                      无跳过
finalize    vcs.dirty false → 已落盘跳过（step 内判断）            保留记录
```

五类跳过因全部是「step 执行了、内部判定无事可做」，按决策 ② 全部保留节点，
`status=passed` + `detail` 携跳过因（干净 / 已合入 / 无 delta / 用户选择 / 已落盘）。
「直接跳过 step」的形态只剩 phase 定义的 steps 序列差异（如 worktree=None 时序列
不含 commit/merge——但现状实现是「六段恒定 + step 内跳过」，故 V1 可能根本没有
「直接跳过」形态，六段恒定声明、逐段内部判断）。

### 方向修正带来的四个新裂缝（按依赖序）

#### N1 归档 phase 与 phase_next 路由的关系

`phase_next` 的 done 判定 = `REQUIREMENT_TABLE.iter().all(|def| has_phase_passed(...))`，
`has_phase_passed` 看 PhaseRecord（verdict pass / skipped）。归档不落 PhaseRecord，
它落 `ChangeRecord.status=archived` + 目录改名。归档并入 walker 后出现两条路：

```
N1a  归档进 REQUIREMENT_TABLE（9 相位）
     ├─ done 判定自然把 archive 算进去（8 相位 pass 后 next=archive）
     ├─ 但 has_phase_passed 需要 archive 的 PhaseRecord 条目——归档完成要落
     │  verdict=pass 的 PhaseRecord（新写面），或 phase_next 特判 archive 相位
     │  读 ChangeRecord.status
     └─ 代价：把「归档完成 = status=archived」与「相位完成 = PhaseRecord pass」
        两套完成语义耦合进同一个路由权威

N1b  归档不进 REQUIREMENT_TABLE，作为 walker 在 done 后的尾部序列
     ├─ phase_next 的 done 判定不变（8 相位），done 后 drive 不走 Completed
     │  收口，而是继续执行 archive 序列，archive 完成后才 Completed
     ├─ 归档保持「非路由相位」地位（同 code-analyze 的纯展示列语义）
     └─ 代价：walker 的 drive 主循环要加「done 后尾部序列」分支，归档不再
        是「普通 step」而是「普通相位之后的特殊尾段」——与用户「视为普通
        step」的字面有张力
```

N1a 更符合「普通 step」，但撞「归档完成语义 ≠ PhaseRecord」的既有事实；N1b 尊重
既有完成语义，但归档成了半特殊序列。这是方向修正后最先要定的分叉。

#### N2 git 操作进 walker 工具步

归档的 commit/merge/finalize 是 git 操作（`ArchiveVcsPort`），而 walker 的
`ToolStepPort` 封闭集是 phase_next / phase_start / phase_log / backtrack /
decision_log / static_check / test_execution——**零 git 变体**。要把归档六段变成
walker step，git 段需要接入 walker 的工具步执行：

```
选项 a  扩 ToolCommand（加 ArchiveCommit / ArchiveMerge / ArchiveFinalize /
        ArchiveSeal / ArchivePreflight 变体）+ ToolStepOutput 对应变体
        └─ git 操作经 ToolStepPort 走进程内直调，与 phase_start 同性质
选项 b  walker 新开 git 步执行缝（与 ArchiveVcsPort 直连，不走 ToolStepPort）
        └─ 保留 ToolStepPort 的「相位机 + 检查」定位，git 是第三类执行器
```

这呼应第二轮的 B（exec/git 两类）：若 git 操作进 ToolStepPort（选项 a），那 git
段在图上自然归入 Exec 类；若独立缝（选项 b），则 StepGroup 要加 git 分类。merge
段的「git + 条件 agent 冲突解算」仍是单节点装两种执行器的混合难题。

#### N3 preflight 数据面与 syncSpecs 选择的 walker 形态

现在归档的确认对话（完成度警告 / delta specs 清单 / merge 目标 / run_active 拒绝）
是独立命令 `archive_flow_preflight` 的读面，syncSpecs 由 `archive_flow_start` 参数
传入。并入 walker 后这两个面要重新安放：

```
preflight 数据面（完成度警告 + delta 清单 + merge 目标）
    ├─ 变成 archive phase 的 Preflight step（step detail 携带警告/清单，上图）
    └─ 或变成 archive 前的 confirm 停等点（停等时经快照/读面呈现数据）

syncSpecs 选择（是否跳过 delta specs 同步）
    ├─ 成为 RunRequest 字段（run 发起时一次传入，specSync step 内判断）
    └─ 或变成 archive phase 内的 ask 中断（AskPayload 选项化——「同步 / 跳过」）
```

关键观察：现有 `confirm` 停等是 `proceed: bool` 二选一，装不下 syncSpecs 的「同步
/ 跳过」选择；现有 `ask` 中断（`AskPayload { question, options }`）能装。所以归档前
的停等点若带 syncSpecs 选择，形态更接近 ask 而非 confirm——这会反过来影响
`auto_next_phase` 的「按指定 phase 停止」到底停在 confirm 还是 ask。

#### N4 auto_next_phase 的扩展形态

现在 `RunRequest.auto_next_phase: bool`，drive 主循环 `if !auto_next_phase { ConfirmWait }`
——要么全程停等、要么全程自动。要「按指定 phase 停止」，扩展形态：

```
选项 a  RunRequest 增 stop_before: Option<String>（或 Vec<String>）
        drive 停等条件 = !auto_next_phase || phase == stop_before
        └─ 最小改动；归档 = stop_before 的固定值
选项 b  把 auto_next_phase 换成确认策略枚举（Auto / StopAt(Vec) / Manual）
        └─ 更泛化，但前端 RunActions 的 checkbox 也要跟着换 UI
选项 c  归档 phase 恒停等（特殊硬编码），其余 9 相位受 auto_next_phase 支配
        └─ 最省，但把「指定 phase 停止」收窄成「归档特例」，失去通用性
```

前端 `RunActions` 的 checkbox 文案现在是「自动确认：仅跳过相位间停等；ask 中断、
失败与终态收口仍停下等待，**不自动归档**」——归档并入后这句「不自动归档」要改，
且 UI 要新增「归档前停下」的表达。

### 归档并入后的收口语义变化

```
现在    run 收口 = 8 相位全 pass → Completed（"Ready for archiving"，停等用户）
        归档 = 独立链（archive_flow_start 另起）

并入后  run 收口 = 8 相位全 pass + 归档相位完成 → Completed
        walker 的 ALL_PHASES_PASSED 文案与收口时机要改（归档完成才 Completed）
```

这意味着 `ChangeRunStatus::Completed` 的语义从「9 站评估完」扩成「9 站评估完 +
归档落盘完」，`ALL_PHASES_PASSED`（"Ready for archiving"）文案整体退役或改写。

### 归档并入后的第三面命令组去向

若归档进 walker，`archive_flow` 五命令（preflight / start / stop / state / watch）+
`ArchiveControl` 注册表 + `ArchiveSink` 桥可能整体退役，收编进 `change_flow` 命令面
与 `ChangeFlowControl`——这带来一个收尾问题：

```
收编面  归档的 stop（终止归档链）→ 复用 change_flow_stop（归档是 run 的 phase，
        停止 run 自然停归档）
归档的重试（幂等续走）→ 归档段失败后，walker 的 retry 语义（phase fail 重跑）
        是否覆盖「归档半完成重试」（现状是 archive_flow 的幂等续走分支）？
```

归档的「半完成重试」（改名成功 + db 翻转失败 → 续半边）是 archive_flow 特有的
幂等逻辑，walker 的 phase retry 是「整个 phase 重跑」。若归档段失败后 walker 重跑
整个 archive phase，commit 段的幂等（干净跳过）会兜住前半段——这恰好印证决策 ②
「step 内判断保留记录」的价值：幂等重试靠 step 内判断自我短路，而非靠第三面链的
续半边分支。

### 值得先对齐的两个分叉

1. **N1：归档进 REQUIREMENT_TABLE 还是 walker 尾部序列**——这决定「普通 step」的
   纯度。进表（N1a）最纯但撞「归档完成 ≠ PhaseRecord」；尾部序列（N1b）尊重完成
   语义但归档半特殊。
2. **N4：auto_next_phase 的「按指定 phase 停止」是通用能力还是归档特例**——通用
   （选项 a/b）改动面大但语义干净；归档特例（选项 c）最省但把能力收窄。且要连带
   定 N3 的「归档前停等是 confirm 还是 ask」（syncSpecs 选择决定了停等形态）。

---

## 第四轮：N1 定稿 = 进 REQUIREMENT_TABLE —— 归档是「确定性相位」模式的第二例

用户定稿 N1：归档进 `REQUIREMENT_TABLE`（9 相位）。这个决策在真实代码里有一个
**现成先例**——`test-execution` 相位已经是「确定性相位」模式（绿跑零 agent），归档
phase 就是它的第二例，且更纯粹。对照如下：

```
评估相位（proposal / dev-design / test-design / implement / test-gen /
          code-review / acceptance 七站）
  steps = PhaseStart → Executor → (StaticCheck) → Evaluator → VerdictGate → PhaseLog
  PhaseDefinition.executor = Some（真用）  evaluator = Some（真用）
  └─ 完成 = Evaluator 产出 verdict，VerdictGate 解析，PhaseLog 落 PhaseRecord

确定性相位（test-execution，先例）
  steps = PhaseStart → TestExecution → PhaseLog（机械 checklist 落账 verdict=pass）
  PhaseDefinition.executor = Some（仅 model_level 供反馈边修复会话用，prompt 不用）
  PhaseDefinition.evaluator = Some（**纯 dummy**，steps 里无 Evaluator/VerdictGate 步）
  └─ 完成 = 机械 checklist 全 pass 推导 verdict=pass，PhaseLog 落 PhaseRecord

归档相位（拟）
  steps = PhaseStart → Preflight → Commit → Merge → SpecSync → Seal → Finalize → PhaseLog
  PhaseDefinition.executor = None（归档无 executor 会话）
  PhaseDefinition.evaluator = None（归档无 evaluator 会话）
  └─ 完成 = Seal 落 status=archived + PhaseLog 机械 checklist 落 PhaseRecord(pass)
```

关键洞察：**归档 phase 的「完成」是双事实**——`Seal` step 做 status=archived +
目录改名（归档的物理事实），末尾 `PhaseLog` step 落一条 verdict=pass 的 PhaseRecord
（路由的「相位已过」事实，让 `has_phase_passed` 识别）。这完全对称于 test-execution
相位的机械 checklist 落账，不是「归档完成语义 ≠ PhaseRecord」的污染——现有 8 个
评估相位本来也是「eval 节点（PhaseRecord）+ step 节点（执行图）」双轨呈现。

### 裂缝 A（最硬）：drive 主循环的 `next.evaluator.is_none()` 硬检查

`walker.rs#drive` 里有一条硬检查（第 185-188 行）：

```rust
if next.evaluator.is_none() {
    return Terminal::failed(format!("phase \"{phase}\" 无 evaluator 定义，无法评估落账"))
        .into_pair();
}
```

归档 phase 的 `evaluator=None` 会撞。这条检查的语义是「每个 phase 都要评估落账」，
但 test-execution 相位已经用「机械 checklist 落账」绕过了它——**靠的是给
`PhaseDefinition.evaluator` 填一个 dummy（`TEST_EXECUTION_EVALUATOR_PROMPT`）**，
steps 里根本不用。

所以归档进表会暴露这个既有技术债：

```
A1  归档也填 dummy evaluator（延续 hack）
    ├─ 与 test-execution 同型，改动最小
    └─ 但归档真的不需要评估，dummy 字段是纯噪音

A2  收窄检查为「steps 含 Evaluator/VerdictGate 步才要求 evaluator 定义」
    ├─ 诚实：归档 evaluator=None、test-execution 的 dummy evaluator 也能清理
    └─ 代价：把「机械落账相位」显式化为一种正式形态（不再是隐式 hack）
```

A2 的正确性更强，且顺带清理 test-execution 的 dummy。但要注意：`PhaseCursor::new`
读 `next.executor.as_ref().map(|spec| spec.model_level)` 作反馈边修复档位，所以
test-execution 的 executor 定义（model_level=Low）是**真用的**，只有 evaluator 是
纯 dummy——收窄检查只动 evaluator，不动 executor。

### 裂缝 B：walker 需要第三执行缝（vcs / git）

归档的 git 步（commit/merge/finalize）现在由 `ArchiveVcsPort` 承载（`ProcessArchiveVcs`
进程 spawn），而 walker 只有两缝：

```
walker 现有执行缝
  worker: WorkerAgentPort   ── agent 会话（executor/evaluator/decision）
  tools:  ToolStepPort      ── 进程内直调写面 + 检查 spawn（phase_next/phase_start/
                                phase_log/backtrack/decision_log/static_check/
                                test_execution）

归档六段需要的执行器
  SpecSync                → worker 缝（复用 run_worker，role=Executor）
  Preflight / Seal        → 读 store + 调 write 面（tools 缝可装，但 Preflight 还需 vcs 探测）
  Commit / Merge / Finalize → git 进程 spawn（ArchiveVcsPort，walker 现无此缝）
```

结论：归档进表后，git 段必须引入 vcs 缝。两个走向（呼应第二轮 N2 / B 决策）：

```
B1  扩 ToolCommand（git 变体进 ToolStepPort）
    ├─ 所有非 agent 步统一走 tools 缝，LocalToolSteps 注入 ArchiveVcsPort
    └─ 但 ToolStepPort 的定位「进程内直调写面（sync 零 IO）」与 git 的「进程 spawn
       + 有副作用」性质不同，会稀释这个缝的定位

B2  walker 新增独立 vcs 缝（Arc<dyn ArchiveVcsPort>）
    ├─ worker / tools / vcs 三缝并存，git 步在 run_step 里直调 vcs
    ├─ Preflight 的 vcs 探测（current_branch / branch_merged）也走 vcs 缝
    └─ 与第二轮的「exec/git 两类」呼应：git 是第三类执行器，StepGroup 加 git 分类
```

merge 段的「git rebase + 条件 agent 冲突解算」仍是单节点装两种执行器的混合体：git
主体走 vcs 缝，冲突时唤起 agent 子分支（复用 run_worker，role=Executor + 既有
`merge_conflict_prompt` 搬入 walker）。

### 裂缝 C：归档落账的机械 checklist 形态

归档 phase 末尾的 PhaseLog 需要机械 checklist（仿 `test_execution_phase_log_input`）。
归档的 checklist 条目可设计为三段全 pass：

```
archive_phase_log_input（拟）
  checklist = [
    "worktree 提交与合入完成"（commit/merge 段 step 内跳过或执行通过）
    "delta specs 同步完成"    （specSync 段：synced / skipped 均通过）
    "归档落盘与收口完成"      （seal/finalize 段）
  ]
  report = 归档摘要（archived_dir / specs 状态 / warnings）
  verdict = 全 pass 推导 pass
```

但注意一个张力：归档的 `skipped`（用户选择跳过 specs 同步）在归档语义里是「完成」，
在 `phase_log` 写面的 `skipped=true` 语义里也是「verdict=pass」。所以归档 checklist
的「specs 同步」条目恒 pass（无论 synced 还是用户跳过），`detail` 里区分两种形态
——这与决策 ②「step 内判断保留记录」一致。

### 裂缝 D：归档失败的 phase retry 幂等性

归档 step 失败（commit 失败 / merge 冲突无法裁决 lean / seal 半完成）后，walker 的
phase fail → retry 语义重跑整个 archive phase。幂等性靠 step 内判断短路：

```
归档段失败 → 重跑 archive phase
  commit    step 内跳过（worktree 干净 / 目录已缺失）          兜住前半
  merge     step 内跳过（branch_merged=true 已合入）           兜住
  seal      workflow::write::archive 自带续半边分支（改名成功 + 翻转失败 → 只补翻转）
  finalize  step 内跳过（dirty=false 已落盘）                  兜住
```

这印证决策 ② 的深层价值：**archive_flow 的「半完成续半边」逻辑，在 step 化后由
「step 内判断 + write 面幂等」自然承载**，不再需要第三面链的特殊续走分支。但要
单独审视一个形态：merge 冲突「无法裁决」（lean）失败后重跑，worktree 已被
`rebase --abort` 恢复干净，重跑 merge 段会重新 rebase——幂等性成立，只是冲突解算
的 agent 会话会重来一次（可接受，与 run 的 phase retry 语义同构）。

### 进表后的 route / done 全景

```
phase_next 路由（REQUIREMENT_TABLE 9 相位）
  proposal → dev-design → ... → acceptance → archive → done
  has_phase_passed(archive) = archive 列 PhaseRecord 有 pass/skipped 且非 stale
  归档落 PhaseLog（verdict=pass）后 → has_phase_passed(archive)=true → done

run 收口
  drive 主循环 next.done=true → Completed
  此时 status=archived（Seal 已翻转）+ archive 列 PhaseRecord(pass) 已落
  → run Completed 与 change archived 同步发生（不再是「Ready for archiving」停等）
```

`ALL_PHASES_PASSED`（"Ready for archiving"）文案整体退役——run 的 Completed 语义从
「9 站评估完」扩成「9 站评估完 + 归档落盘完」。

### 值得先对齐的次生分叉

1. **裂缝 A 走 A1（dummy evaluator）还是 A2（收窄检查）**——A2 顺带清理
   test-execution 的 dummy，但把「机械落账相位」显式化；A1 最小改动但延续 hack。
   这决定归档 phase 的 PhaseDefinition 形状。
2. **裂缝 B 走 B1（git 进 ToolStepPort）还是 B2（独立 vcs 缝）**——B2 与「exec/git
   两类」呼应更一致，且 Preflight 的 vcs 探测天然归 vcs 缝；B1 统一非 agent 步但
   稀释 ToolStepPort 定位。
3. **归档的 StepNode 词汇扩展形状**：6 个新词（Preflight/Commit/Merge/SpecSync/Seal/
   Finalize）进 StepNode 与 RunStepKind，`group()` 派生里 git 段（commit/merge/
   finalize）归哪类——这依赖裂缝 B 的走向（B2 则加 git 分类）。

---

## 第五轮：A/B/C 定稿的连锁验证

用户三条定稿：**A 移除 `evaluator.is_none()` 检查；B 走 B2 独立 vcs 缝 + rebase
与冲突解算拆两步（按条件执行解冲突）；C 归档无 evaluator、无需 checklist。**

三条里 C 的连锁最大——它直接推翻了第四轮「末尾 PhaseLog 落机械 checklist」的假设，
逼出一个新的完成判定裂缝。逐条验证如下。

### A 定稿：移除 drive 的 `evaluator.is_none()` 是安全的（step 层各自兜底）

drive 主循环第 185-188 行的硬检查是**冗余的提前检查**——真正消费 evaluator 的
`step_evaluator`（walker.rs 803-808 行）有自己的 None 检查（`let Some(evaluator) =
&ctx.next.evaluator else { return Err("计划步漂移") }`），`step_executor` 同理。移除
drive 层检查后：

```
移除前  每 phase 强制要求 evaluator（drive 层）
移除后  phase 可无 evaluator（归档），但 steps 若声明 Evaluator/VerdictGate 步
        而 evaluator=None，仍被 step_evaluator 的漂移检查拦截
```

即「移除」不破坏任何安全不变量——「steps 声明了 Evaluator 步就必须有 evaluator
定义」这条约束下沉到 step 执行体，反而更精确（约束跟着消费点走）。test-execution
相位的 dummy evaluator 也因此失去存在意义，可顺带清理（但清理与否是独立取舍，A
本身只要求移除检查）。

### B 定稿：rebase 与冲突解算拆两步，ff_advance 位置成新模糊点

现状 `archive_flow.rs` 的 Merge 段是**单段内嵌循环**：`rebase_branch` → Rebased
（立即 ff_advance）或 Conflicted（while 循环 resolve_conflicts → ff_advance）。

拆两步后，`PhaseCursor` 需要新跨步产物「rebase 冲突清单」（仿 eval_outcome /
test_outcome 先例）：

```
Merge（rebase 重放）step，vcs 缝
  rebase_branch → Rebased             → passed，cursor 记「无冲突」
                → Conflicted(conflicts) → passed，cursor 记 conflicts（冲突态保留）
                → Err                 → failed（Terminal）

ResolveConflicts step，vcs 缝 + worker 缝（条件）
  cursor 无冲突 → passed + detail=无冲突（step 内判断，保留记录——决策②语义）
  cursor 有冲突 → resolve_conflicts agent 循环 → 解完 → passed
```

**新模糊点：`ff_advance`（主仓 ff-only 快进）落在哪一步？** 现状它在 Rebased 与
解完两条路径各执行一次。拆两步后有三种放法：

```
放法 1  Merge step 内：Rebased 路径立即 ff_advance；Conflicted 路径留给
        ResolveConflicts 解完再 ff_advance —— ff_advance 散在两处（现状同构）
放法 2  ResolveConflicts step 收尾统一 ff_advance：无冲突 → 跳过解算直接快进；
        有冲突 → 解完快进 —— ff_advance 单点，但 ResolveConflicts 名不副实
放法 3  拆成三段：Rebase step + ResolveConflicts step（条件）+ Merge step（ff_advance）
        —— 命名最清晰（rebase 重放 / 解冲突 / 主仓快进三语义分立），但多一段
```

放法 3 最贴合「rebase 和冲突分为两步」的字面，又把 ff_advance 独立出来；放法 2
最省段但名字要改。这是 B 定稿后剩的次生分叉。

### C 定稿：归档无 evaluator、无 checklist —— 逼出「完成判定」新裂缝

C 推翻了第四轮「归档末尾 PhaseLog 落机械 checklist」的假设。归档 phase 的 steps
不再含 Evaluator / VerdictGate / PhaseLog，也不落 PhaseRecord。连锁如下：

```
连锁 1  phase_next 完成判定活锁（最硬）
  done = table.iter().all(|def| has_phase_passed(&entries, def.id))
  has_phase_passed 只看 PhaseRecord（verdict pass / skipped 且非 stale）
  归档不落 PhaseRecord → has_phase_passed(archive) 恒 false
  → phase_next 永远路由到 archive → 活锁
  解法：phase_next 对 archive 相位特判，改读 ChangeRecord.status == Archived
        （Seal step 翻转 status 后即「已过」）

连锁 2  active_phase 清位
  phase_start(archive) 写 active_phase（供 attempt 分配 + 详情页「运行中」呈现）
  但 active_phase 清位目前唯一依赖 phase_log 落账单事务
  归档无 phase_log → Seal（status=archived）后 active_phase 残留 "archive"
  解法：Seal/Finalize 清 active_phase，或 archive 相位不走 phase_start

连锁 3  图上归档列无 eval 节点
  归档不落 PhaseRecord → pipeline[].attempts 无 archive 条目 → 归档列只有
  step 节点（无 EvalFlowNode）——这其实更符合「归档是 step 序列」的定位，
  比第四轮「双轨呈现」更干净

连锁 4（好消息）归档失败 = Terminal，不进决策分叉
  归档无 PhaseLog → step 失败是 Err(Terminal) → drive 直接 Failed 收口
  → 不进 phase retry 循环、不触发 MAX_RETRY_TIMES 决策分叉
  → 与现状 archive_flow 的 fail_stage 停链行为一致，allowed_backtrack_phases
    把 archive 暴露给决策 agent 的隐患自然消解（归档失败不唤起决策）
```

连锁 1 是 C 的核心代价：**「归档完成 = status=archived」这个物理事实，必须成为
phase_next 路由权威的第二判据**（PhaseRecord 之外）。这跟第四轮 N1a 预判的「两套
完成语义耦合」一致，但用户 C 选择了「不落 PhaseRecord」而非「落空 checklist」，
所以耦合点从「写面 archive 落 PhaseRecord」变成「phase_next 特判 status」。

### 次生分叉：归档 phase 是否走 PhaseStart

C 连带引出「归档 phase 的 steps 序列第一段是什么」：

```
形态 X  归档走 PhaseStart（与 8 站同构）
        attempt 由 phase_start max+1 分配，详情页呈现「运行中 · archive · attempt N」
        代价：active_phase 残留需 Seal/Finalize 清位（连锁 2）

形态 Y  归档不走 PhaseStart，steps 从 Preflight 起步
        cursor.attempt 恒 0（单次 run 内唯一，归档一次性不撞号）
        active_phase 不写 archive（无清位负担），但详情页归档阶段无「运行中」呈现
        代价：归档 phase 与其余 8 站「必开相」的形状不一致
```

形态 X 更一致（复用 phase_start 的 attempt 分配 + 呈现），代价是补 active_phase
清位；形态 Y 更省，但归档 phase 成为「无 PhaseStart 的特例相位」。这取决于
「归档阶段详情页要不要呈现 active_phase」的产品判断。

### 本轮收束后的两个待选分叉

1. **B 的 ff_advance 位置**：放法 1（散两处）/ 放法 2（ResolveConflicts 收尾统一，
   改名）/ 放法 3（拆三段 Rebase/Resolve/Merge）。
2. **C 的 phase_next 特判形态 + 归档是否走 PhaseStart**：phase_next 对 archive 相位
   读 status 是必然的（连锁 1），但「active_phase 清位」（连锁 2）与「PhaseStart
   走不走」（形态 X/Y）是同一件事的两面，可合并为一个决策点。

---

## 第六轮：形态 X + 放法 3 定稿，「最小特殊性」约束纠正 C 的落账解读

用户定稿：**① ff_advance 走放法 3（拆三段 Rebase / ResolveConflicts / Merge）；②
形态 X（归档走 PhaseStart，与 8 站同构）；③ 新约束——归档尽量不增加 phase 和 step
的特殊场景。**

这条新约束直接暴露了第五轮 C 解读的一个偏差，需要先纠正。

### 关键转向：C「不需要 checklist」的正确落账形态是「空 checklist 落账」，不是「phase_next 特判」

第五轮我把 C 解读成「归档不落 PhaseRecord → phase_next 特判 status」。但
phase_next 特判 status（连锁 1）正是**最大的 phase 层特殊场景**，与「最小特殊性」
约束正面冲突。

重新审视 `phase_log` 写面的 `derive_verdict`（phase_log.rs 30-36 行）：

```rust
fn derive_verdict(checklist: &[ChecklistItem]) -> Verdict {
    if checklist.iter().all(|item| item.pass) {
        Verdict::Pass   // 空 checklist → all() 恒真 → Pass
    } else {
        Verdict::Fail
    }
}
```

**空 checklist 落账是写面早已支持的合法输入**（注释明确「空 checklist → pass」）。
所以「不需要 checklist」的归档，正确落账形态是：

```
归档末尾 PhaseLog step 落账
  PhaseLogInput {
    phase: "archive",
    report: 归档摘要（archived_dir / specs 状态 / warnings），   ← 摘要进 report
    checklist: vec![],                                            ← 空，非机械三条目
    skipped: false,
    三会话槽: None,
  }
  → derive_verdict([]) = Pass → PhaseRecord(verdict=pass) 落账
  → has_phase_passed(archive) 识别 → phase_next 零特判
  → active_phase 清位由 phase_log 单事务 → 零特判
```

两条落账路径的对比：

```
路径 P（空 checklist 落账，本纠正）
  phase_next 零特判 / active_phase 零特判 / PhaseLog 复用
  代价：archive 列多一个 pass 实心绿 eval 节点（checklist 空）
  └─ 「最小特殊性」正解

路径 Q（第五轮误读：不落 PhaseRecord）
  phase_next 特判 status / active_phase 特判清位（两个 phase 层特殊场景）
  好处：archive 列无 eval 节点（纯 step 呈现）
  └─ 违背「最小特殊性」约束
```

用户 C 说「不需要 checklist」，路径 P 的 checklist 是**空的**（不是机械三条目），
满足字面；用户本轮「尽量不增加特殊场景」，路径 P 的零特判满足精神。故 P 是正解，
Q 作废。

### 完整 steps 序列（形态 X + 放法 3 + 空落账）

```
ARCHIVE_STEPS（10 step，归档相位）
  PhaseStart        复用词汇：开相，attempt max+1 分配，active_phase=archive
  Preflight         新词：校验（store 读 + vcs 探测），内联只读
  Commit            新词：vcs.commit_all，step 内判断「干净」跳过
  Rebase            新词：vcs.rebase_branch，产出冲突清单 → cursor 跨步产物
  ResolveConflicts  新词：条件执行——无冲突 passed+detail；有冲突 agent 解算
  Merge             新词：vcs.ff_merge 主仓快进（放法 3 独立成段）
  SpecSync          新词：agent 会话（worker 缝，spec_sync_prompt）
  Seal              新词：workflow::write::archive（status=archived + 改名）
  Finalize          新词：vcs.commit_paths，step 内判断「已落盘」跳过
  PhaseLog          复用词汇：空 checklist 落账（report=归档摘要），清 active_phase
```

8 个新 StepNode 词汇 + 2 个复用（PhaseStart/PhaseLog）。`PhaseCursor` 需加两个
跨步产物：`rebase_conflicts: Option<Vec<String>>`（Rebase 产、ResolveConflicts 消）与
`archive_summary`（Seal/SpecSync 产、PhaseLog 消）——仿 eval_outcome / test_outcome
先例，零新机制。

### 「最小特殊性」盘点：几乎为零

以「需要 walker / phase_next / 写面对 archive 这个词做特判」为特殊场景判据，盘点：

```
正常扩展（零特判）
  PIPELINE_PHASES + archive                        列身份常量加值
  REQUIREMENT_TABLE + archive PhaseDefinition      表加条目（executor=None/evaluator=None）
  StepNode / RunStepKind + 8 词                    词汇封闭集加值
  StepGroup + git 分类                             B2 决定的通用第三类执行器
  StepContext + vcs 缝                             B2 决定的通用执行缝
  PhaseCursor + 2 跨步产物字段                      test_outcome 先例
  ResolveConflicts 条件执行 = step 内判断           决策②语义，非控制流特判

消除既有冗余（减少特殊）
  移除 drive 的 evaluator.is_none() 检查            冗余提前检查下沉 step 层
  test-execution 的 dummy evaluator 可删            移除检查后失去存在意义

通用泛化（非归档专属）
  step_phase_log 第三形态（空 checklist 落账）      phase_log 写面早已支持空 checklist
  auto_next_phase 按指定 phase 停止（N4）           通用能力，非归档特例
```

真正的「特殊场景」残留：**零**。归档成功落空账、归档失败靠「on_fail=None 的步
失败即 Terminal」通用语义直接 Failed 收口（不进决策分叉，与现状 archive_flow 停链
一致），都不需要任何「if phase == archive」特判。

### 一个需要正视的代价：archive 列的 eval 节点

路径 P 让 archive 列多一个 pass 实心绿 eval 节点（checklist 空，代表「归档完成」）。
这是「零特判」换来的代价。三种消化方式：

```
代价 1  接受 eval 节点（零特判）——archive 列 = 1 eval 节点 + 10 step 节点，
        与 test-execution 列同构（pass eval + step 节点）
代价 2  前端对 archive 列隐 eval 节点（前端特判，违背最小特殊性）
代价 3  空 checklist 的 PhaseRecord 落账但 report 摘要已够读，eval 节点视觉从简
        （不改特判，只调渲染样式——可选）
```

建议走代价 1（接受），因为 archive 列有「归档完成」的 eval 节点语义上合理
（它就是「归档这个 phase 已 pass」的标记），且与 test-execution 列完全同构。

### 本轮待确认的一个点

**step_phase_log 的第三形态表达**：现在它是「test_outcome 机械 / verdict 解析」两
分流，归档落空账需第三条「空 checklist 落账」分支。这个分支是 PhaseLog step 的
通用泛化（空 checklist 是合法输入），不是归档特判。但要注意一个细节——归档的
report（归档摘要）与 checklall 空，与评估相位的「report=评估报告 + checklist=检查项」
语义不同，`step_phase_log` 的第三形态是否需要独立落账函数（仿
`test_execution_phase_log_input` 的 `archive_phase_log_input`），还是直接复用现有
`run_phase_log` + 空 checklist 载荷即可——这是实现层的最后一个小分叉。

---

## 第七轮（敲定）：复用 run_phase_log + 空 checklist，不新增 archive_phase_log_input

用户敲定上一轮待确认点：**复用 `run_phase_log` + 空 checklist 载荷**（弃「独立
`archive_phase_log_input` 落账函数」选项）。逐点对照代码确认可行且零写面改动。

### 代码可行性验证

```
run_phase_log（walker.rs:1012-1028）
  落账单点：run_tool(ToolCommand::PhaseLog { change_id, phase, input })
  ├─ 对 checklist 内容零约束——空 vec 原样透传
  └─ step_phase_log 同文件，可直接调用（零新增函数）

phase_log 写面（phase_log.rs:43-116）
  derive_verdict([]) = Pass        ← 注释明确「空 checklist → pass」
  skipped=false + verdict=Pass     ← 约束不冲突（原约束是「skipped 时 verdict 必须 pass」）
  表位前置                          ← archive 进 REQUIREMENT_TABLE 后自动命中（第六轮 N1）
  active_phase 匹配                 ← 形态 X（PhaseStart 开相）自动满足
  落 PhaseRecord + 清 active_phase   ← 单事务，零特判
```

结论：**归档落账 = step_phase_log 内新增一条 archive 分支，构造
`PhaseLogInput { phase: "archive", report: cursor.archive_summary, checklist: vec![], skipped: false, 三槽位 None }`，
直接 `run_phase_log(ctx.tools, ctx.request, input)`。** 零新增落账函数、零写面改动、
零 `if phase == archive` 特判。

### step_phase_log 第三分支的落点

现有两分流（walker.rs:914-996）：`test_outcome` 机械 → `verdict` 解析 → 否则 Err。
archive 分支插在 test_outcome 之后、verdict 之前：

```
if let Some(outcome) = cursor.test_outcome.take() { ... return Ok(()) }   // 既有
if let Some(summary) = cursor.archive_summary.take() {                     // 新增
    run_phase_log(..., PhaseLogInput { report: summary, checklist: vec![], ... })
    cursor.failed = false;   // 归档成功 = pass
    return Ok(())
}
let Some(checklist) = cursor.verdict.take() else { Err(缺评估产物) }       // 既有
```

分流互不误判：归档 phase 无 TestExecution step（test_outcome 恒 None）、无
Evaluator/VerdictGate step（verdict 恒 None），只有 archive_summary 一个产物——
三分流按「产物在否」天然分离，无需按 phase 名判断。

### 一个实现层小提醒（非新分叉）

`archive_summary` 由 SpecSync 与 Seal 两段先后产出（SpecSync 记 specs 状态/warnings，
Seal 补 archived_dir 后收口），PhaseLog 消费的是最终拼好的摘要。这与
`test_outcome`（单步产出单次消费）稍有不同——archive_summary 是「两段累积」字段。
实现时要么 PhaseCursor 字段在 SpecSync/Seal 各写半边，要么 Seal 落账后统一铸出
summary 再走 PhaseLog。属字段组织细节，不改变「复用 run_phase_log + 空 checklist」
的结论。

---

## 第八轮（敲定）：summary 也不需要，archive 落账 = 空 report + 空 checklist

用户再敲定：**archive 当前不需要检查，连 summary 也不必提供。** 即归档落账的
`report` 也传空串。逐点对照代码验证，发现这是**净简化**——不仅可行，还连带删掉
第七轮的 `archive_summary` 跨步产物字段。

### report 空串的合法性验证

```
写面（phase_log.rs:55-60）
  report 校验只有长度门（> MAX_REPORT_CHARS 才 Err），零非空校验
  空串 0 chars 合法通过

落库（store.rs:1356 / model.rs:614）
  PhaseRecord.report: String，空串直存，零约束

前端渲染（两处，零崩坏）
  flow-event-node.tsx:24-37  EvalBody 只渲染 attempt + verdict 徽章 + skipped/stale，
                            根本不显示 report——空 report 在图上 eval 节点零视觉差异
  detail-drawer.tsx:130      {node.record.report} 空串渲染成空 div（whitespace-pre-wrap），
                            无异常；checklist 空走 InlineChecklist →「（清单为空）」

下游消费（零读 report 的路径命中 archive）
  has_phase_passed          只读 verdict/skipped/stale，不读 report
  build_decision_input      读 report——但 archive 失败即 Terminal 不进决策分叉（第五轮连锁4）
  fail_feedback_suffix      读 report——但 archive 不 fail 重试（失败即 Terminal）
  eval-checklist 产物       非空 checklist 才产出信封；空 checklist 零信封，不读 report
```

结论：**archive 落账 = `PhaseLogInput { phase: "archive", report: String::new(), checklist: vec![], skipped: false, 三槽位 None }`。**
零写面改动、零 UI 改动、零下游风险。

### 连锁简化：archive_summary 整个字段可删

第七轮记的 `archive_summary`（SpecSync 产 specs 状态/warnings、Seal 补 archived_dir
收口）是**专为 report 摘要服务的跨步产物**。report 空后它失去唯一消费者：

```
第七轮  PhaseCursor 加两个跨步产物：rebase_conflicts + archive_summary
第八轮  PhaseCursor 只加一个跨步产物：rebase_conflicts（Rebase 产、ResolveConflicts 消）
        archive_summary 删除——SpecSync/Seal 的产出不进跨步状态，step 节点 detail 已够
```

第七轮末尾「archive_summary 两段累积」的实现细节提醒**随之消解**，无需再定
SpecSync/Seal 各写半边还是 Seal 统一铸出。

### 最终归档落账形态（三轮敲定后）

```
ARCHIVE_STEPS（10 step）末尾 PhaseLog 分支
  run_phase_log(
    PhaseLogInput {
      phase: "archive",
      report: "",            ← 空（无 summary，无检查）
      checklist: vec![],     ← 空（无 evaluator，无检查）
      skipped: false,
      executor/evaluator/decision 三槽位: None,
    }
  )
  → derive_verdict([]) = Pass → PhaseRecord(verdict=pass, report="", checklist=[])
  → has_phase_passed(archive) → 零特判
  → active_phase 清位（phase_log 单事务）→ 零特判
```

归档列最终呈现 = 1 个 pass 实心 eval 节点（attempt + verdict 徽章，无 report 无
checklist）+ 10 个 step 节点——「归档完成」的纯标记，与第六轮代价 1 的预期一致，
且比预期更干净（无空 report 占位）。

### 一个顺带的观察（非分叉，仅记录）

`derive_verdict([]) = Pass` 在这里承载了「空 checklist = 无检查 = 无条件通过」的
语义。这与评估相位「checklist 是证据、verdict 由 evaluator 显式给出」的语义不同——
但这是**既有写面早已支持的口径**（phase_log.rs 注释明确），不是归档引入的新特判。
若未来要区分「无检查」与「检查了但全过」，那才是要动 verdict 显式化的更大话题
（第七轮之前曾开过这个方向，后随用户回退），与本轮的「复用空 checklist」决策
独立，不在本次范围。
