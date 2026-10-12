# desktop 工作流类型区分与 test-only 流程探索

日期:2026-10-08 · 状态:调研中

## 背景

packages/desktop 目前只有一种工作流（`requirement`）。需求是给 change 工作流引入类型区分、新增 `test-only` 流程：
不同工作流类型的 **phase 组合（有序列表）不同**，但 **phase → step 映射不受类型影响**（同一 phase 无论在哪个类型里出现，跑的 step 序列一致）。

本笔记记录对现有架构的调研结论与落点清单。

## 核心结论（TL;DR）

`workflow_type` 字段**已经存在于后端**（`ChangeStateRecord.workflow_type: String`），但：
1. 建档时**硬编码**为 `"requirement"`（create.rs:205）；
2. `phase_table(workflow_type)` 目前**只认 `"requirement"`**，其余一律 `None`（phase_table.rs:523）；
3. `code-analyze` 目前只是 10 列布局里的**占位列**——没有 PhaseDefinition、没有 step 序列、没有 prompt；
4. 前端 **完全不知道** workflow_type（`ChangeDetail` / `ChangeSummary` DTO 都不透出该字段），列布局靠前端写死的 `PIPELINE_PHASES`（10 站）。

所以「引入类型区分 + test-only」不是从零造新概念，而是把已经埋好的 `workflow_type` 缝从「单值硬编码」打通成「两值分派」，并补上 `code-analyze` 这个缺位的 phase 定义。

## 当前架构全景

### 双层权威（路由 vs 布局）

```
                    ┌──────────────────────────────────────────┐
                    │  phase_table(workflow_type)               │
                    │  (write/phase_table.rs)                   │
                    │  路由唯一权威：workflow_type → [PhaseDef]  │
                    │  V1 只认 "requirement"                     │
                    └──────────────┬───────────────────────────┘
                                   │ Some(REQUIREMENT_TABLE)
                                   ▼
   REQUIREMENT_TABLE = [ proposal, dev-design, test-design, implement,
                         test-gen, test-execution, code-review,
                         acceptance, archive ]   ← 9 个路由相位

                    ┌──────────────────────────────────────────┐
                    │  PIPELINE_PHASES (10 站)                  │
                    │  queries/detail.rs  +  flow/layout.ts     │
                    │  纯布局身份，MUST NOT 上位为路由权威       │
                    └──────────────┬───────────────────────────┘
                                   │
                                   ▼
   [ proposal, dev-design, test-design, implement, test-gen,
     test-execution, code-review, acceptance, code-analyze, archive ]
                          ↑ 第 9 列是 code-analyze 占位列（requirement 表里没有它）
```

关键点（desktop-workflow-write-face spec 已明文）：`PIPELINE_PHASES` 是**布局身份**，不是路由权威。requirement 表是 9 相位（不含 code-analyze），但布局是 10 列（含 code-analyze 占位）。

### PhaseDefinition = phase→step 映射的载体

每个 `PhaseDefinition`（phase_table.rs:17）同时声明：

```
PhaseDefinition {
  id: &'static str,
  steps: &'static [StepDefinition],        ← phase → step 映射（有序）
  artifact_manifest: &'static [ArtifactSpec],
  executor: Option<PhaseAgentSpec>,        ← prompt + model_level
  evaluator: Option<PhaseAgentSpec>,
}
```

**step 序列已经是独立的 static 常量**（关键观察，直接印证「phase→step 不受类型影响」的自然落点）：

```
PROPOSAL_STEPS        → [PhaseStart, Executor, Evaluator, VerdictGate, PhaseLog]
DEV_DESIGN_STEPS      → 同上形
TEST_DESIGN_STEPS     → 同上形
IMPLEMENT_STEPS       → [PhaseStart, Executor, StaticCheck, Evaluator, VerdictGate, PhaseLog]
TEST_GEN_STEPS        → [PhaseStart, Executor, StaticCheck, Evaluator, VerdictGate, PhaseLog]
TEST_EXECUTION_STEPS  → [PhaseStart, TestExecution, PhaseLog]  (确定性，零 agent)
CODE_REVIEW_STEPS     → [PhaseStart, Evaluator, VerdictGate, PhaseLog]  (eval-only)
ACCEPTANCE_STEPS      → 同 code-review
ARCHIVE_STEPS         → [PhaseStart, Preflight, Commit, Rebase, ResolveConflicts,
                         Merge, SpecSync, Seal, Finalize, PhaseLog]  (10 步确定性)
```

`REQUIREMENT_TABLE` 逐相位引用上述 static；`steps` 已被抽成单点。所以给 test-only 建表时，**直接复用** `PROPOSAL_STEPS / TEST_DESIGN_STEPS / TEST_GEN_STEPS / TEST_EXECUTION_STEPS / ARCHIVE_STEPS`，只需**新增 `CODE_ANALYZE_STEPS`**。

### 缺失面：code-analyze

`code-analyze` 目前：
- 在 `PIPELINE_PHASES`（第 9 列）占位；
- **无** `CODE_ANALYZE_STEPS`；
- **无** prompt（`prompts/` 目录里没有 `code-analyze-*.md`；插件的 `code-analyze-planner.md` 是 dev-team agent，非 desktop 角色 prompt）；
- **无** `artifact_manifest`（大概率空）。

按 `pge-workflow-engine` spec，code-analyze 是 `DESIGN P→E`（executor + evaluator），即 step 形态与 proposal/dev-design/test-design 相同：
`[PhaseStart, Executor, Evaluator, VerdictGate, PhaseLog]`。

### 前置依赖（prerequisites）目前是写死的 requirement 专属

`phase_table.rs:550` 的 `prerequisites(phase)` 是硬编码 match（requirement 依赖图），`dependents(table, phase)`（stale 传播用）调用它。test-only 的前置依赖不同（见 pge spec）：

```
test-only prerequisites:
  proposal → []
  code-analyze → [proposal]
  test-design → [proposal, code-analyze]
  test-gen → [test-design]
  test-execution → [test-gen]
```

所以 `prerequisites` / `dependents` 需要**类型感知**（当前 `dependents(table, phase)` 只收 table，不收 workflow_type）。

## 落点清单（按模块）

### 后端 core/workflow

| 位置 | 现状 | 需要 |
|------|------|------|
| `write/phase_table.rs:523` `phase_table()` | 只认 `requirement` | 增加 `test-only` → `TEST_ONLY_TABLE` |
| `write/phase_table.rs` step statics | 缺 `CODE_ANALYZE_STEPS` | 新增 code-analyze 的 step 序列 |
| `write/phase_table.rs:550` `prerequisites()` | requirement 写死 | 类型感知（或迁到每表/每 phase 自带 prereq） |
| `prompts/` | 无 code-analyze prompt | 新增 `code-analyze-planner.md` / `code-analyze-evaluator.md`（或复用） |
| `write/create.rs:205` | 硬编码 `"requirement"` | 接受 workflow_type 入参 |
| `queries/detail.rs:17` `PIPELINE_PHASES` | 固定 10 站 | 由「布局常量」改为「随 workflow_type 出线的列序」（或 detail.pipeline 直接承载列序，前端不再自己排） |
| `queries/detail.rs` `change_detail()` | pipeline 用固定 `PIPELINE_PHASES` 建空站 | 改用 `phase_table(workflow_type)` 的相位序 |

### 后端 orchestration / commands

| 位置 | 现状 | 需要 |
|------|------|------|
| `commands/change_flow/mod.rs:103` | 拒绝非 requirement | test-only 进表后此门自然放行（文案要改） |
| `commands/changes/mod.rs:168` `create_change` | 三参（root/name/goal） | 透出 workflow_type 入参（IPC 面变化 → 重新生成 bindings） |
| `write/phase_start.rs` / `phase_log.rs` / `phase_next.rs` / `backtrack.rs` | 错误文案「V1 仅 requirement」 | 文案随类型面更新；backtrack 的 stale 传播要类型感知 |

### 前端 packages/desktop

| 位置 | 现状 | 需要 |
|------|------|------|
| `flow/layout.ts:16` `PIPELINE_PHASES` | 写死 10 站 | 列序改由 detail（后端出线）供给，或按 workflow_type 分派 |
| `flow/graph.ts:71` `buildFlowGraph` | 用固定 `PIPELINE_PHASES` 建列 | 用 detail 出线的相位序建列 |
| `flow/run-state.ts:150` | `PIPELINE_PHASES.indexOf(phase)` 算 colIndex | 同上，用出线列序 |
| `flow/attachments.ts:11` | `PIPELINE_PHASES.indexOf` 校验归属 | 同上 |
| `types/generated/bindings.ts` | `ChangeDetail` 无 workflow_type | DTO 透出 workflow_type（或直接透出相位列序） |
| `change-create-dialog.tsx` | 无类型选择 | 增加类型选择（requirement / test-only） |

## 需要澄清的开放问题（多线方向）

1. **archive 是否进 test-only 表**：
   - `test-only-workflow/spec.md` 说 5 个 phase（**不含 archive**）；
   - `README.md` 的 test-only 流程是 6 段（**含 archive**，`/openspec-archive-change`）；
   - desktop 的 requirement 表是 9 相位（**archive 是表内第 9 相位**，由 `stop_before=archive` 手动门驱动）。
   - 所以 desktop 的 test-only 大概率是 6 相位（5 路由 + archive），但需与 spec 对齐确认。

2. **phase → step 复用粒度**：test-only 的 `proposal` 是否与 requirement 的 `proposal` 用**同一个** PROPOSAL_STEPS（含同一 executor prompt）？「test-only 提案」语义是否要独立 prompt（测试向提案 vs 需求提案）？`test-only-workflow/spec.md` 里 proposal description 是「测试向提案」，暗示 prompt 可能不同，但 step 结构（P→E）相同——这正是「step 映射不受类型影响、prompt 受类型影响」的边界，需要确认。

3. **`prerequisites` 的承载方式**：是给每个 `PhaseDefinition` 加 `prerequisites: &[&str]` 字段（表内自描述），还是保留独立函数做类型分派？当前 `prerequisites()` 是独立 match，`dependents()` 依赖它。若 test-only 也要 backtrack stale 传播，两种方式都可行，倾向表内自描述（每表自带依赖，杜绝第二份 match）。

4. **列序的权威归属**：现在 `PIPELINE_PHASES` 后端/前端双侧同步（layout.ts 注释「改动需双侧同步」）。若列序随 workflow_type 变化，前端不该再自持一份静态表——应由 `ChangeDetail.pipeline`（PhaseEntry[] 已按相位序出线）直接承载列序，前端 `PIPELINE_PHASES` 退役为「测试夹具用常量」或彻底删除。这与 desktop-workflow-write-face spec 里「`PIPELINE_PHASES` 保持纯布局身份、MUST NOT 上位为路由权威」的既有约束一致，但进一步要求它**不再是前端列序来源**。

5. **DTO 透出形态**：`ChangeDetail` 是透出裸 `workflow_type: String`，还是透出派生好的列序？若只透 workflow_type，前端还得自持一张 type→phases 表（又回到双侧同步）；若透出列序（或让 `pipeline` 直接就是权威列序），前端零映射。倾向后者（`detail.pipeline` 已按序出线，前端直接用 `pipeline.map(p => p.phase)` 建列）。

## 决策定稿（2026-10-08）

| # | 方向 | 定稿 |
|---|------|------|
| 1 | archive 是否进 test-only 表 | **进**：TEST_ONLY_TABLE = 6 相位 = 5 路由 + archive |
| 2 | phase→step 复用粒度 | **先复用**：test-only 的 proposal / test-design / test-gen / test-execution 直接复用 requirement 的同名 static（step 序列 + prompt 全复用）；code-analyze 无存量 → **新增** `CODE_ANALYZE_STEPS` + `code-analyze-planner.md` / `code-analyze-evaluator.md`。是否拆独立 test-proposal prompt **留待看效果** |
| 3 | prerequisites 承载 | **表内自描述**：`PhaseDefinition` 加 `prerequisites: &'static [&'static str]` 字段，删独立 `prerequisites()` match；`dependents(table, phase)` 改为遍历 `candidate.prerequisites.contains(&phase)` |
| 4 | 列序权威归属 | **detail.pipeline 出线承载**：前端 `PIPELINE_PHASES` 退役，列序 = `detail.pipeline.map(p => p.phase)` |

### 决策 1 落点

`TEST_ONLY_TABLE`（6 相位，序固定）：

```
proposal → code-analyze → test-design → test-gen → test-execution → archive
```

与 `test-only-workflow/spec.md` 的 5 相位差别仅在**追加 archive 表内末相位**（desktop 的 archive 是 `stop_before=archive` 手动门 + 确定性相位，requirement 同理）。pge spec 是插件侧叙述，desktop 侧 archive 进表不冲突。

### 决策 2 落点（复用矩阵）

| phase | step 序列 | executor prompt | evaluator prompt |
|-------|-----------|-----------------|------------------|
| proposal | 复用 `PROPOSAL_STEPS` | 复用 `PROPOSAL_EXECUTOR_PROMPT` | 复用 `PROPOSAL_EVALUATOR_PROMPT` |
| code-analyze | **新增** `CODE_ANALYZE_STEPS`（P→E，与 proposal/dev-design 同形） | **新增** `code-analyze-planner.md` | **新增** `code-analyze-evaluator.md` |
| test-design | 复用 `TEST_DESIGN_STEPS` + 复用 prompt | 同左 | 同左 |
| test-gen | 复用 `TEST_GEN_STEPS` + 复用 prompt | 同左 | 同左 |
| test-execution | 复用 `TEST_EXECUTION_STEPS`（确定性，零 agent） | 复用 | 复用 |
| archive | 复用 `ARCHIVE_STEPS`（executor/evaluator 均 None） | — | — |

**注意一个既有差异**：pge spec 里 test-execution pattern 是 `EXEC Executor→E`（有 executor），而 desktop 的 `TEST_EXECUTION_STEPS` 是确定性门禁（`[PhaseStart, TestExecution, PhaseLog]`，零 agent，`PhaseDefinition.executor/evaluator` 的 prompt 虽 Some 但 walker 不消费——steps 里无 Executor/Evaluator 步）。决策 2「复用」= 沿用 desktop 确定性格，不照搬插件侧 Executor→E。此为 desktop 与插件工作流的**既有**差异，非本次引入。

### 决策 3 落点

`PhaseDefinition` 加字段后，`dependents` 类型感知天然成立（table 已按 workflow_type 分派）：

```rust
pub struct PhaseDefinition {
    // ... 既有字段
    pub prerequisites: &'static [&'static str],   // 新增
}

pub(crate) fn dependents(table: &[PhaseDefinition], phase: &str) -> Vec<String> {
    table.iter()
        .filter(|c| c.prerequisites.contains(&phase))
        .map(|c| c.id.to_owned())
        .collect()
}
```

prerequisites 数据源：requirement 照搬 phase_table.rs:550 现 match；test-only 照 pge spec（proposal=[]，code-analyze=[proposal]，test-design=[proposal,code-analyze]，test-gen=[test-design]，test-execution=[test-gen]，archive=[]）。

### 决策 4 落点（前端四处消费面）

| 文件:行 | 现状 | 改为 |
|---------|------|------|
| `flow/graph.ts:71` | `PIPELINE_PHASES.map` 建列 | `detail.pipeline.map(p => p.phase)` 建列（colIndex = 数组下标） |
| `flow/run-state.ts:150` | `PIPELINE_PHASES.indexOf(step.phase)` | 由 pipeline 出线的 phase→colIndex Map 反查 |
| `flow/attachments.ts:11` | `PIPELINE_PHASES.indexOf` 校验归属 | 同上（或改收 pipeline 相位集） |
| `flow/layout.ts:16` | `PIPELINE_PHASES` 常量 | 退役为测试夹具常量，或彻底删除 |

`buildFlowGraph` 的 `runStepNodes` 目前是纯函数（`run-state.ts` 不接收 pipeline），要改成带列序入参，或在 `buildFlowGraph` 内二次重算 colIndex（建议前者：`runStepNodes(steps, phaseOrder)` 显式传序，测试直接断言）。

---

## 矛盾与边界发现（需 spec delta 或 design 定稿）

### A. 「10 列恒定」被决策 4 推翻

**定稿**：spec delta 随本次 change **一起做**（不拆独立 change）。

`desktop-change-flow-view/spec.md` 大量断言「10 列恒定」「code-analyze 占位列」「archive 恒第 10 列」「`PIPELINE_PHASES` 10 站双侧同步」。列序改由 `detail.pipeline` 出线后：

- **requirement：10 列 → 9 列**（code-analyze 占位列消失，archive 从第 10 列变第 9 列）；
- **test-only：6 列**（proposal/code-analyze/test-design/test-gen/test-execution/archive）。

即「列总数恒 10」的约束**失效**，改为「列总数 = 该 change 相位表长度」。这需要：
1. `desktop-change-flow-view/spec.md` 的「10 列」「code-analyze 占位」「archive 第 10 列」等 Scenario 改写为「随 workflow_type 出线」；
2. 后端 `queries/detail.rs` 的 `PIPELINE_PHASES` 常量与 `change_detail` 建站逻辑同步改；
3. 前端 `layout.test.ts` / `graph.test.ts` / `change-flow-graph.test.tsx` 里大量 `PIPELINE_PHASES.map` 夹具与「恒 10 列」断言全部重写。

### B. 未支持 workflow_type 的 pipeline 建站 fallback

`change_detail` 现在用固定 `PIPELINE_PHASES` 建空站（10 站），entries 按 phase 归位；`phase_table(workflow_type)` 仅在产物清单处 `unwrap_or_default()`。若 pipeline 建站也改为按 `phase_table` 序，则：

- legacy 记录 / 未知 workflow_type → `phase_table` 返回 `None`，pipeline 建站序来源缺失。
- **定稿（第三轮推翻第二轮）**：**读写都严格，不 fallback requirement**。存量数据统一设成 requirement（见第三轮「事实链」），因此正常形态下不存在未知类型 change；若真有脏值，读面不静默当 requirement 跑（具体呈现形态留 change），写面显式拒绝。`change_detail` 建站改为按 `phase_table(workflow_type)` 序，`None` 时的行为由 change 细化定稿（返回 None / 空 pipeline / 改 Result 签名三选一）。

### C. `change_flow_start` 前置校验（已定稿：读写都严格）

**定稿**：`change_flow_start` 前置校验保留「`phase_table` 返回 None → 显式拒绝」的门，仅把文案从「V1 仅 requirement」改为通吃 `{requirement, test-only}`（test-only 进表后被放行，此门只剩未知类型的拒绝面）。`write/{phase_start,phase_log,phase_next,backtrack}.rs` 的「V1 仅 requirement」文案同改。**不引入**「未知类型静默跑 requirement」的 fallback。

### D. 建当入口的 IPC 面变化

`create_change` 目前三参（root/name/goal）→ 需透出 workflow_type。`ChangeCreateDialog` 增加类型选择器；`create_change_with` 泛型缝加 workflow_type 形参；`ChangeStateRecord.workflow_type` 由硬编码改入参。**注意**：这是 IPC 签名变化 → 需重跑 specta 重新生成 `types/generated/bindings.ts`，且 `create_change` 的「三参签面不变」既有测试断言（mod_test.rs:734）会被打破。

---

## 决策补充（第二轮）

| # | 方向 | 定稿 |
|---|------|------|
| A | 「10 列恒定」spec delta 归属 | **随本次 change 一起做** |
| B | 未支持 workflow_type 的 fallback | ~~全部按 requirement 处理~~（**已被第三轮推翻**：改「读写都严格 + 存量设 requirement」） |
| 列序精确改法 | `runStepNodes` 签名、colIndex 透传等 | **交由 change 细化**，探索不再往下推 |
| 5 | explore→change 类型区分 | **explore agent 推算类型 + 「启动变更」按钮旁显示 + 可人工修改** |

---

## 决策补充（第三轮）

| # | 方向 | 定稿 |
|---|------|------|
| 1 | 类型推算产出格式 | **title 同模式**：stance 约定笔记写标记行（与首行 `# <标题>` 同构），前端解析 → 回填 `ExploreRecord` |
| 2 | 未支持 workflow_type | **读写都严格**（不 fallback requirement）；**存量数据统一设成 requirement** |
| 3 | WorkflowTypeSelect | **不抽共享组件**：仅两选项的普通 select，promote 与 change_create 各自内联（架构边界优先，无单次使用抽象） |

### 「读写都严格 + 存量设 requirement」的事实链

关键事实：`ChangeRecord.workflow_type: String` 自 v3 就存在（`native_model(id=9, version=4)`），且 desktop 写面 `create.rs:205` 从 V1 起硬编码 `"requirement"`——**生产库存量 ChangeRecord 的 workflow_type 恒为 requirement**，严格校验生效后存量记录天然合法。

因此「存量设 requirement」的实际落点很轻：

- **ChangeRecord**：存量已是 requirement，**零迁移**；严格校验 = `phase_table` 只认 `{requirement, test-only}`，其余 `None`。若未来 db 出现脏值（非两合法值），需一次性规范化（store 打开库时防御扫描 or 显式迁移），把脏值 set 成 `"requirement"`。
- **ExploreRecord**（第 4 点 promote 路径新增类型字段）：v3 升级时存量 explore 记录 decode-only 升级**默认 `"requirement"`**——这才是真正有「存量设 requirement」迁移语义的地方。

### 「读写都严格」的语义

`phase_table(workflow_type)`：`"requirement"` → `REQUIREMENT_TABLE`，`"test-only"` → `TEST_ONLY_TABLE`，其余 → `None`（**不退化**）。

- **写面**（`phase_next` / `phase_start` / `phase_log` / `backtrack`）+ **发起面**（`change_flow_start`）：未知类型 → 显式 `Err`（沿用现有「不受支持」拒绝面）。
- **读面**（`change_detail` 建站 + 产物清单）：未知类型 → 不 fallback requirement。具体呈现形态留 change 细化（`change_detail` 当前返回 `Option<ChangeDetail>`，严格报错需定「返回 None 视为不可达」还是「改签名返回 Result」；两者都不是静默当 requirement 跑）。

---

## 探索→变更类型区分（promote 路径）

### 现状链路

```
探索详情页「启动变更」按钮（ExploreDetailHeader，explore-detail-view.tsx:117）
  → usePromoteAction → list.promote(name)
  → useExploreList.promote → commands.promoteExplore(root, name)
  → 后端 promote_explore（commands/explores/mod.rs:244）
      → promote_explore_with
         ① 读主仓笔记全文（goal）
         ② 读 ExploreRecord（title / promoted_to 校验）
         ③ spawn_blocking write::promote_explore
              → super::create(..., name, note, title)
                  └ create 硬编码 workflow_type: "requirement"（create.rs:205）
         ④ mark_explore_promoted 打标
```

### 类型推算的现状缺口

explore agent 目前**没有**「产出变更类型」的机制。它只：
1. 经 `buildExplorePrompt`（lib/explore-stance.ts）注入立场 prompt，要求笔记首行 `# <标题>`；
2. 产出笔记文件 `openspec/explores/<name>.md`；
3. 前端 watch → `extractMarkdownTitle`（lib/explore-title.ts）解析首行 → `updateExploreTitle` 回填 `ExploreRecord.title`。

类型推算需要新增一条**与 title 回填同构**的「产出 → 解析 → 回填」链路。

### 落点（promote 路径）

| 层 | 现状 | 需要 |
|----|------|------|
| `ExploreRecord`（store/model.rs:213，v2） | title / promoted_to | 加 `workflow_type`（或 suggested_workflow_type）→ **v3**，decode-only 升级默认 `"requirement"` |
| `buildExplorePrompt`（explore-stance.ts） | 只约定首行标题 | 增加「推算变更类型」的约定输出标记（**title 同模式**：笔记写约定行，格式见下方定稿） |
| 前端解析（explore-title.ts 旁） | 只 `extractMarkdownTitle` | 增加 `extractWorkflowType` 类解析 |
| 回填命令 | 只有 `update_explore_title` | 增加 `update_explore_workflow_type`（或合并进同类写口） |
| `useExploreList.promote` | `promote(name)` | `promote(name, workflowType)` |
| `commands.promoteExplore` IPC | `(root, name)` | `(root, name, workflowType)` |
| `write::promote_explore` | 无类型 | 加 workflow_type → 透传 `create` |
| `write::create` | 硬编码 | 加 workflow_type 入参（与 change_create 手动入口共用） |
| `explore-detail-view.tsx` header | 只有「启动变更」按钮 | 按钮旁加类型选择器（默认 = 推算值，可改 requirement/test-only） |

### 开放问题（promote 路径）

1. **推算产出格式** —— **已定稿：title 同模式**。
   agent 在探索笔记里写一条**约定标记**（与首行 `# <标题>` 同构，如约定行 `workflow_type: test-only`），前端解析回填 `ExploreRecord`。独立 tool / 收敛事件路线（成本高）放弃。
   - 仍待 change 细化：标记的具体格式（约定行 vs front-matter）与首行标题的共存顺序；`explore-stance.ts` 里的措辞（怎么引导 agent 判断 requirement vs test-only 并可靠写出标记）。

2. **「建议值」与「最终值」的字段语义**：`ExploreRecord.workflow_type` 存 agent 推算值（可被人工覆盖），promote 入参 = 人工最终选择。是**单字段**（前端可改后回填 + promote 读它）还是**双字段**（suggested vs selected）？倾向单字段。

3. **人工修改是否持久化**：改类型后是回填落库（下次进详情仍是改后值），还是仅本次 promote 会话内临时？倾向**回填落库**（沿 title 模式：改即写）。

4. **promote 与 change_create 的入口汇合** —— **已定稿：不抽共享组件**。
   手动新建（change-create-dialog.tsx）也要类型选择器，两者默认值语义不同（手动默认 requirement；promote 默认 agent 推算值）。`WorkflowTypeSelect` 只是**两个选项的普通 select**，**MUST NOT 抽共享组件**——promote 与 change_create 各自内联一个普通 select（架构边界优先：跨入口复用一个两选项 select 属于 speculative abstraction，违背「无单次使用抽象」纪律）。两个入口仅共享 `workflow_type` 的合法值集（后端 `phase_table` 的 `{requirement, test-only}` 是唯一权威，前端零第二份值表）。

5. **类型推算的 prompt 引导**：stance 里怎么措辞让 agent 判断 requirement vs test-only？需给出判断标准（如「目标是补测试 / 生成测试用例 → test-only；否则 requirement」），否则 agent 自由发挥不可控。

---

## 附：相关 spec / 文件索引

- `openspec/specs/test-only-workflow/spec.md` —— test-only 5 相位产品叙述（无 acceptance / integration-test / unit-test）
- `openspec/specs/pge-workflow-engine/spec.md` —— phase 表与前置依赖规范性数据（requirement 8 相位 / test-only 5 / bug-fix 6）
- `openspec/specs/desktop-workflow-write-face/spec.md` —— `PhaseDefinition` 结构、step 序列单源、`PIPELINE_PHASES` 布局身份
- `openspec/specs/desktop-change-flow-view/spec.md` —— 前端 10 列布局与取数契约（列序目前固定，需随决策 4 改写）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` —— 相位表唯一权威
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` —— `PIPELINE_PHASES` + `change_detail`
- `packages/desktop/src/views/changes/flow/layout.ts` —— 前端 10 站镜像
- `packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs` —— promote 写面（create 复用点）
- `packages/desktop/src-tauri/src/commands/explores/mod.rs` —— promote_explore 命令层
- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` —— `ExploreRecord` v2（待升 v3）
- `packages/desktop/src/views/explores/explore-detail-view.tsx` —— 「启动变更」按钮落点
- `packages/desktop/src/lib/explore-stance.ts` / `explore-title.ts` —— stance prompt + 标题回填（类型回填的同构先例）
