# 提案: run-workflow-by-step

> **变更**: run-workflow-by-step
> **日期**: 2026-10
> **状态**: draft
> **探索**: `openspec/changes/run-workflow-by-step/explore.md`（五轮探索已收敛：计划图=源码 / 执行图=db / 边表 / 相对路径 artifact 清单）

---

## 问题

desktop 后端把 change 流程编排抽象为「phase 下挂有序 step，step 分 exec / agent / gate 三类」，但真实代码里 step 序列**硬编码在 `walker.rs#drive()`**，相位差异散在 `phase_table.rs#executor: Option`、`walker.rs` 的 `STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 两个常量三处叠加。结果「按 step 粒度运行」目前只有概念层三类节点统一 + 运行态统一可观测，步序列不是数据驱动。四条结构性裂缝：

1. **计划没有单一载体**：workflow/phase/step 定义散在 `phase_table.rs`（8 相位 + executor/evaluator spec）、`walker.rs`（门禁布局常量）、`detail.rs`（9 站 `PIPELINE_PHASES` 展示列，含无路由的 `code-analyze`）、前端 `layout.ts`（同 9 站，注释要求双侧同步）四处，全是编译期硬编码。
2. **执行图只有扁平列表、没有边**：`RunStepRecord` 用 `seq`（emit 序）盖戳 + `run_history.rs#persisted_step` 做 10→5 过滤（phase_start / phase_log / 三门 gate 弃落）；前端 `graph.ts#deriveEdges` 用列差 Δ 派生 forward / retry / backtrack 三种边，**phase 内部 feedback 边（gate 失败回到 agent 修复）被粗粒度压平**，图上看不到这段反复。
3. **artifact 归属在前端硬编码**：`attachments.ts#docColumn(source)` 用字符串→列静态映射决定 proposal.md / specs/** 挂 proposal 列、design.md / tasks.md 挂 dev-design 列；后端 `discover_artifacts` 全树扫描 + 静态 matcher 插件，与 phase 定义无因果链，`enumerate_candidates` 全树遍历是「黑名单式发现」。
4. **stdout 记录散在 checks 层**：static-check stdout 全量即丢（落库只 ≤500 摘要），test-execution 落盘 `report_dir` 引用，agent 会话是 `SessionEventRecord` 引用——「step 保存 agent 会话记录 + 脚本 stdout」的诉求缺少统一输出记录口径。

explore 五轮已把目标模型收敛（四条决策定稿，见 explore 第四、五轮）：

- **决策①**：定义载体 = 源码（与 prompt 同落 `include_str!` + const 表）；db 仅存执行记录。prompt 静态化与 core/workflow 零 infra 两条红线**不翻案**。
- **决策②**：边表单独落；反馈边重跑 = 新节点；执行图节点稳定身份键 `(run_id, phase, attempt, step, instance_no)`；边 kind 扩为 forward / feedback / retry / backtrack 四值；边表两向查询 + `run_id` / `from_node_key` 两二级索引。
- **决策③**：不考虑历史兼容，重设计运行记录 model（`RunNodeRecord` 全词汇 + `RunEdgeRecord` 边表取代 `RunStepRecord.seq` + `persisted_step` 10→5 过滤）；artifact 关联键 = 相对路径，归属 = 源码 phase 清单（不落库、不重建 inventory 字段）。
- **决策④**：会话回放已在 infra/agent（`transcript(session_id)` 重放唯一来源，零改动）；stdout 记录收敛到 infra/exec，形成 `ExecOutput::Transient` / `ExecOutput::Recorded { exec_id }` 两种输出模式。

翻案红线仅一条：**「单图硬编码，MUST NOT 实现通用图引擎或图定义文件格式」**——`PhaseDefinition` 从「executor/evaluator 两个 agent spec」扩展成「有序 step 序列 + 依赖边 + artifact 清单」，drive() 硬编码骨架下沉为 plan 图结构。prompt 全静态零占位、core/workflow 零 infra 两条不翻案。

## 提案

把「编译期硬编码计划 + 内存/派生执行史」升级为「源码声明式计划图 + db 持久化执行图 + phase 声明 artifact 清单 + exec 层统一输出记录」：

1. **计划图落源码**：`PhaseDefinition` 扩展 `steps: [StepDefinition]`（kind = exec | agent | gate，携 role / prompt_ref / on_fail 语义）与 `artifact_manifest: [ArtifactSpec]`（kind + 相对路径模式 + title）；`prerequisites` 保持依赖边形态（stale 传播源）。定义仍为编译期 `include_str!` + const 表（`phase_table.rs`），不落库、不经 port 缝、无运行时文件 IO。`workflow_type` 保持 V1 恒 `"requirement"`。
2. **walker 改为 plan-step 通用执行**：`drive()` 不再硬编码 ①→⑦，改为迭代相位定义的 step 序列；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 两个常量退役（门禁由 step 定义声明）。三处特殊控制流以 step 级语义表达，不走 walker 内 step 类型特判：static-check / test-execution 的 `on_fail → feedback loop`（独立计数 ≤5、超限升格相位 fail）、verdict 解析失败为终态停给用户、phase 间 confirm 停等仍为 run 级 `auto_next_phase` 开关。路由权威仍驻写面 `phase_next`（walker 零自持转移规则红线不变）。
3. **执行图落库重设计**：`RunRecord` 保留 run 主行；`RunStepRecord`（`seq` + 5 词汇）退役，`RunNodeRecord` 承载全词汇节点（含 phase_start / phase_log / 三门 gate），稳定身份键 `(run_id, phase, attempt, step, instance_no)`，feedback 重跑为 `instance_no` 递增的新节点；`RunEdgeRecord` 边表单独落，`edge_kind ∈ { forward, feedback, retry, backtrack }`，附 `run_id` / `from_node_key` 两二级索引。`run_history.rs#persisted_step` 10→5 过滤整个退役。每 run 两写时序保留（run 行 start 建 / finish 终态 + 节点整包 + 边整包单事务）。
4. **artifact 相对路径 + 源码清单驱动**：`discover_artifacts(change_dir, phases)` 改为「读 phase 定义清单 → 按相对路径枚举匹配 → 插件 parse」；`enumerate_candidates` 全树扫描退役；前端 `docColumn(source)` 静态映射退役（phase 归属直接来自清单）。eval-checklist 保持 PhaseRecord 投影（`phase + attempt` 关联），不走相对路径框架。
5. **stdout 统一到 infra/exec**：`SystemCommand` 之外新增输出记录模式 `ExecOutput::Transient`（spawn 即得、反馈边消费、丢弃）与 `ExecOutput::Recorded { exec_id }`（stdout 落库/落盘、可回查）；static-check 由「临时」升级为可选「有 id 可查」，test-execution 的 `report_dir` 与新 `exec_id` 统一。会话记录继续走 `SessionEventRecord` 重放（`session_id` 引用），不内嵌、不与转录单表不变量冲突。
6. **前端边与节点渲染对齐后端**：`deriveEdges` Δ 派生退役、改读边表；`runStepNodes` 槽位配对 + `:seq` 堆叠退役、节点按后端稳定身份键渲染；边 kind 四值渲染（feedback 为新增的 phase 内回边）。`PIPELINE_PHASES` / `code-analyze` 保持纯布局身份（不因本变更上位路由权威）。
7. **版本交付**：`packages/desktop` 0.4.34 → 0.4.35；`plugins/dev-team` 零改动。

## 能力

### 新增能力

- 无（本变更修改八个既有能力）。

### 修改的能力

- **desktop-change-orchestration** — 翻案「单图硬编码」红线；`PhaseDefinition.steps[]` + `artifact_manifest[]` 计划图；walker 改为 plan-step 通用执行（`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 常量退役）；执行图节点稳定身份键与边 kind 四值；版本交付 0.4.35。
- **desktop-workflow-write-face** — `PhaseDefinition` 扩展有序 step 序列与 artifact 清单（仍编译期静态）；相位表成为 step 序列与产物清单的单源（prompt 静态化不翻案）。
- **desktop-change-state-store** — 运行记录 model 重设计：`RunNodeRecord`（全词汇 + 稳定实例键）与 `RunEdgeRecord`（边表 + 两二级索引）取代 `RunStepRecord`（`seq` + 10→5 过滤）；每 run 两写时序保留。
- **desktop-change-queries** — detail 聚合出线 nodes + edges（全词汇 + 稳定实例键 + 边 kind 四值）取代 `runs[].steps`（五词汇 + seq）；产物发现改 phase 清单驱动。
- **desktop-change-flow-view** — 前端 `deriveEdges` Δ 派生、`docColumn` 静态映射、`runStepNodes` 槽位配对三处退役；节点按后端稳定身份键渲染、边读边表、feedback 四值边呈现。
- **desktop-artifact-plugins** — `enumerate_candidates` 全树扫描退役；discovery 改读 phase 定义清单 + 相对路径枚举 + 插件 parse；eval-checklist 保留 PhaseRecord 投影分支；markdown-doc / tasks-progress 改清单驱动。
- **desktop-checks-domain** — stdout 记录收敛到 infra/exec：`ExecOutput::Transient` / `Recorded { exec_id }`；static-check 输出记录升级、test-execution `report_dir` 与 `exec_id` 统一。
- **desktop-corpus-regression** — 语料与 golden 覆盖 nodes / edges 投影（全词汇、feedback 边、稳定实例键）、artifact 清单驱动发现的样本。

### 引用沿用（零 delta）

- desktop-phase-prompts — 14 个 md 全静态零占位红线不翻案，`PhaseDefinition` 扩展不改变 prompt 静态化装配。
- desktop-agent-execution — 会话转录单表 + `transcript(session_id)` 重放唯一来源复用，零改动。
- desktop-change-worktree — 双 root 组合与 exec root 解析不变（artifact 相对路径与 phase 清单在 worktree 内照常经 id → 记录 → name/worktree 定位）。
- desktop-workspace-store — workspace 库模型组 additive 注册与格式版本作废重建惯例沿用（执行图 model 重设计经 version bump 整体重建、零迁移）。
- desktop-ipc-type-bindings — nodes / edges DTO 演进经 bindings 再生成，一致性守卫拦截漂移。

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` — `PhaseDefinition` 扩展 `steps: [StepDefinition]` 与 `artifact_manifest: [ArtifactSpec]`（const 静态表 + `include_str!` 装配不变）；`prerequisites` 依赖边保留
- `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` — step 词汇 / `StepDefinition` / `ArtifactSpec` / `artifact_manifest` 模型（计划图侧类型，同步保持）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` — 计划步序列下发（next 相位 steps + manifest 随 `phase_next` 响应供给 walker；路由权威不变）
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` — `drive()` 改 plan-step 通用迭代；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 常量退役；反馈边与 gate 分支按 step 级语义（on_fail）执行
- `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` — 步节点稳定身份键（`instance_no`）与边 kind 四值词汇；`ChangeStepKind` 全词汇统一
- `packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs` — `persisted_step` 10→5 过滤退役；节点 + 边整包投影（`RunNodeRecord` / `RunEdgeRecord` 载荷）
- `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` — `RunEntry` 累积器升级为节点 + 边累积（emit 序保留、`instance_no` 分配）；头注释口径同步
- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` — `RunNodeRecord` / `RunEdgeRecord` 新模型（native_model 版本治理）；`RunStepRecord` 退役
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` — run 域操作面重设计（`run_start` / `run_finish` 节点 + 边整包 / 节点与边查询 / 二级索引）
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` — `ChangeDetail` 出线 nodes + edges（全词汇、稳定实例键、边 kind 四值）取代 steps（golden 显式重写）
- `packages/desktop/src-tauri/crates/core/workflow/src/artifacts/registry.rs` — `discover_artifacts` 改 phase 清单驱动；`enumerate_candidates` / 全树扫描退役
- `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` — `ExecOutput::Transient` / `Recorded { exec_id }` 输出记录模式（spawn 捕获 + 落库/落盘接口）
- `packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` + `testexec/` — 输出捕获改经 infra/exec 记录模式；test-execution `report_dir` 与 `exec_id` 统一
- `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` + `changes/mod.rs` — nodes / edges DTO 装配与通知桥随 shape 演进
- `packages/desktop/src/views/changes/flow/graph.ts` — `deriveEdges` Δ 派生退役、改读边表（边 kind 四值）
- `packages/desktop/src/views/changes/flow/attachments.ts` — `docColumn` 静态映射退役、改读清单 phase 归属
- `packages/desktop/src/views/changes/flow/run-state.ts` — `runStepNodes` 槽位配对 + `:seq` 堆叠退役、节点按稳定身份键渲染
- `packages/desktop/src/views/changes/flow/types.ts` — 节点 / 边 / 步类型随 DTO 演进
- `packages/desktop/src/types/generated/bindings.ts` — 随 DTO 演进再生成
- `packages/desktop/package.json` — `version` 0.4.34 → 0.4.35

### 测试文件

- `crates/core/workflow/src/write/phase_table_test.rs` — step 序列 / artifact 清单声明与下发、静态装配不翻案
- `crates/core/workflow/src/artifacts/registry_test.rs` + `crates/core/workflow/src/artifacts/*_test.rs` — 清单驱动发现（相对路径命中 / 未命中 / eval-checklist 分支）、全树扫描退役
- `crates/core/orchestration/src/walker_test.rs` — plan-step 通用迭代、feedback 边重跑新节点、四边 kind、门禁相位限定
- `crates/core/orchestration/src/run_history_test.rs` / `control_test.rs` — 节点 + 边累积、`instance_no` 分配、10→5 过滤退役、control 零 IO
- `crates/infra/store/src/model_test.rs` / `store_test.rs` — `RunNodeRecord` / `RunEdgeRecord` 回环、边表二级索引、存量库 additive 打开与 version bump 重建
- `crates/core/workflow/src/queries/detail_test.rs` — nodes / edges 出线、稳定实例键、边 kind 四值、golden 显式重写
- `crates/infra/exec/src/lib_test.rs` — `ExecOutput` 两模式（Transient 丢弃 / Recorded 回查）
- `crates/infra/checks/src/**` — static-check / test-execution 输出记录升级（`exec_id` 回查 / `report_dir` 统一）
- `packages/desktop/src/views/changes/flow/graph.test.ts` / `attachments.test.ts` / `run-state.test.ts` — 边读边表、清单归属、稳定键渲染（data-testid 挂钩）
- `crates/core/workflow/tests/fixtures/` + `corpus_golden_test.rs` — nodes / edges / feedback 边 / artifact 清单样本 + `DESKTOP_GOLDEN_REWRITE=1` 显式重写

### 删除文件

- 无整文件删除（`run_history.rs` / `registry.rs` / `run-state.ts` / `attachments.ts` / `graph.ts` 分支收缩，文件保留）。

### 不要修改

- `plugins/dev-team` 全部（版本 2.10.44、三类交付产物）
- `core/workflow/src/prompts/` 14 个静态 md（prompt 全静态零占位红线不翻案）
- 相位机写面语义（phase_next 路由权威 / phase_log / backtrack 白名单与 stale 传播——仅 phase_next 增发 step 序列与 artifact 清单）
- 会话转录单表与 `transcript(session_id)` 重放唯一来源（复用，零改动）
- stop / confirm / answer 控制命令语义（请求-应答不变）
- `StepRecord` 审计职责与词汇（与执行图节点史职责分立，不做合并）
- `PIPELINE_PHASES` / `code-analyze` 纯布局身份（不因本变更上位路由权威）
- `openspec/specs/**` 既有基线 spec（本变更只写 `openspec/changes/run-workflow-by-step/specs/**` delta，归档时合并）

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 计划图单源与静态化 | `PhaseDefinition` 含 `steps: [StepDefinition]` 与 `artifact_manifest: [ArtifactSpec]`；步序列 / 产物清单仅驻 `phase_table.rs`（const + `include_str!`），无 db 定义表、无 port 缝定义读、无运行时文件 IO；prompt 仍全静态零占位（扫描无 `<role>` 间接读取） |
| AC-2 | walker 计划步通用执行 | 假引擎 + 假写面驱动：`drive()` 迭代相位 step 序列，`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 常量删除；walker 零相位转移规则（过渡仍问 `phase_next`）；feedback / verdict 解析失败 / confirm 停等语义不变 |
| AC-3 | 执行图节点与边 | 一次 run 覆盖全步词汇后查询：`RunNodeRecord` 全词汇在案（含 phase_start / phase_log / 三门 gate），身份键 `(run_id, phase, attempt, step, instance_no)` 稳定唯一；`RunEdgeRecord` 边表 forward / feedback / retry / backtrack 四 kind 齐备；`persisted_step` 10→5 过滤删除 |
| AC-4 | feedback 重跑新节点 | static-check 失败 → executor 修复 → 重跑：图上 executor 首次 / 修复各为独立节点（`instance_no` 递增），边为 feedback（phase 内回边，区别于 retry）；前端读边表渲染，`deriveEdges` Δ 派生退役 |
| AC-5 | 每 run 两写与边整包 | 假引擎一相位 run：库写事务恰两次（start 建 running 行 / finish 终态 + 节点整包 + 边整包单事务），无每步写事务；任一环节失败整体回滚（无半包节点 / 半包边） |
| AC-6 | 出线与 golden | `ChangeDetail` 出线 `runs[].nodes` / `runs[].edges`（稳定实例键、边 kind 四值、全词汇）取代 `steps`；时间戳 ISO 串 + null 口径不变、纯 derive；golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕；bindings 一致性守卫绿 |
| AC-7 | artifact 清单驱动 | `discover_artifacts` 按 phase 清单相对路径枚举命中（proposal.md / specs/** → proposal、design.md / tasks.md → dev-design、test-reports/** → test-execution）；`enumerate_candidates` 全树扫描与前端 `docColumn` 静态映射删除；eval-checklist 仍经 PhaseRecord 投影（phase + attempt） |
| AC-8 | stdout 统一记录 | static-check 以 `ExecOutput::Recorded { exec_id }` 捕获可回查（或 `Transient` 丢弃）；test-execution `report_dir` 与 `exec_id` 统一口径；infra/exec 输出记录单点，checks 层无私有 stdout 记录分支 |
| AC-9 | 前端稳定身份渲染 | 前端 `runStepNodes` 槽位配对 + `:seq` 堆叠删除，节点按后端 `(phase, attempt, step, instance_no)` 渲染；同键多实例不串扰；图边读边表四 kind 呈现（feedback 虚线可辨） |
| AC-10 | 语料与管线交付 | 语料含 nodes / edges / feedback 边 / artifact 清单样本，golden 覆盖新投影；`vp test` / `client:check` / knip / bindings 全绿；desktop version 0.4.35；`plugins/dev-team` 零改动 |

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 红线翻案涟漪：「单图硬编码」翻案与三处既有 spec 冲突（orchestration V1 边界第 3 条、walk 相位循环 ①→⑦ 描述、state-store run 运行史五词汇口径） | spec 自相矛盾、实现无所适从 | 高（确定发生） | 本变更 delta 逐条显式 MODIFIED 留痕（orchestration / state-store / queries / flow-view）；翻案理由锚定 explore 五轮决策定稿；其余红线（prompt 静态化 / core 零 infra）不翻案显式声明 |
| R2 步控制流表达力不足：仅 `steps: Vec<StepDefinition>` 线性表达不了 feedback loop / verdict 分叉 / confirm 停等 | 又退化成 walker step 类型特判，配置驱动半途而废 | 中 | step 定义引入 `on_fail` 级语义（feedback 循环 / 终态 / 升格），门禁与 gate 由 step 声明而非 walker 特判；残余表达力缺口（若有）显式留痕为 V1 边界，不隐式吸收 |
| R3 执行图模型重设计回归面大：`RunStepRecord.seq` + 10→5 过滤 + 前端三处派生/配对同时退役 | 图渲染 / 回放拼接回归 | 中 | 语料 golden 覆盖 nodes / edges 全投影；AC-3/4/6/9 逐面断言；`native_db` version bump 整体重建零迁移 |
| R4 反馈边节点身份与会话归属张力：feedback 多节点同会话（executor Continue 同一 session_id），provenance 只能到 `(phase, role, attempt)` | 节点→会话联动歧义（多实例同会话） | 低 | 会话寻址沿用槽位 id 直查 / sourceRef 反查（不变）；`instance_no` 仅为节点身份，不进 provenance、不虚构会话；多实例点击同一会话转录为可接受语义 |
| R5 stdout 全量落库写放大：static-check 高频 spawn 若默认 `Recorded` 全量落库 | db 体积与写放大 | 中 | 默认 `Transient`、按需 `Recorded`（反馈边消费 Transient 即够）；`Recorded` 全量落盘 + 引用，MUST NOT 全文进 `RunNodeRecord` detail（引用式与截断式口径不变） |
| R6 artifact 双源去重：清单驱动命中 vs 既有插件 matcher 扩展名命中可能重复 | 重复挂载 / 权威源不清 | 低 | 清单为唯一候选来源，插件 matcher 只在清单命中的候选项内判定 kind；`enumerate_candidates` 全树扫描退役，无第二来源 |

### 待决问题（design 相位定稿，不阻塞本 proposal）

- `StepDefinition` 的 `on_fail` 语义形态（feedback loop 计数归属 / verdict 解析失败终态 / 升格相位 fail 的表达），以及 feedback 边 kind 命名。
- eval-checklist 的「phase + attempt 关联」与相对路径 artifact 清单的统一表达（保留独立分支 vs 并入清单结构）。
- `RunNodeRecord` / `RunEdgeRecord` 主键与二级索引的 native_db 复合键打包形态（沿 `hash64(a)<<64|b` 先例，`from_node_key` 后继查询索引实现）。
- `ExecOutput::Recorded { exec_id }` 的落库载体（独立 exec 输出表 vs 落盘文件 + 引用）与 test-execution `report_dir` 的统一存储决策。
