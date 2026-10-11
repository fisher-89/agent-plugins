# 测试设计: run-workflow-by-step

> **日期**: 2026-10
> **依据**: proposal.md（AC-1–AC-10）+ design.md（D1–D6、数据模型、公共函数 / API、类型定义）

---

## 测试框架与路径约定（框架识别结论）

- **Rust（src-tauri 各 crate + shell 命令层）**：cargo 内建测试框架，测试文件与源文件同目录同名的 `*_test.rs` 模块（既有惯例，如 `write/phase_table_test.rs`、`walker_test.rs`、`run_history_test.rs`）；跨 crate 语料集成测试驻 `crates/core/workflow/tests/`（`corpus_golden_test.rs`）。断言面为标准 `assert_eq!` / `assert!` / `assert_ne!` / `matches!` + `serde_json` 线面比对；db 边界一律 tempfile 真件库真实组合。
- **前端（packages/desktop/src）**：vitest（`vite-plus/test`）+ @testing-library/react，测试文件与源文件同目录 `*.test.ts(x)`；`describe` / `it` / `expect` 自 `vite-plus/test` 导入；`data-testid` 为唯一查询挂钩；进程边界 mock 统一走 `vi.mock('@tauri-apps/api/core')`（invoke 分发 + 可编程 Channel，既有装置沿用）。
- **Mock 纪律**（沿仓库既有）：Rust 侧零框架级 mock——db 边界一律 tempfile 真件库，进程内假件仅限「作为被测 API 显式入参 / 注入依赖」的 trait 假件（入参例外）；前端仅 mock 进程边界（invoke / Channel / 计时器），内部纯函数模块（graph / attachments / run-state 等）真实组合。
- 以下源文件路径均相对项目根；Rust 路径前缀 `packages/desktop/src-tauri/`，前端路径前缀 `packages/desktop/`。

---

## 验收范围

> 表中「被测文件或模块」列填源文件（Rust 源文件相对 `packages/desktop/src-tauri/`、前端源文件相对 `packages/desktop/`，沿 design.md 变更清单口径），对应测试文件见「单元测试」各节 `->` 标题。再导出 / 生成物 / 配置 / 无独立测试路径的类型面不在本表（见「不可测试项」）。

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 计划图单源与静态化：`PhaseDefinition` 含 `steps: [StepDefinition]` 与 `artifact_manifest: [ArtifactSpec]`；步序列 / 产物清单仅驻 `phase_table.rs`（const + `include_str!`），无 db 定义表、无 port 缝定义读、无运行时文件 IO；prompt 仍全静态零占位 | `crates/core/workflow/src/write/phase_table.rs`、`crates/core/workflow/src/state.rs`、`crates/core/workflow/src/write/phase_next.rs` |
| AC-2 | walker 计划步通用执行：`drive()` 迭代相位 `steps` 序列，`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 常量删除；walker 零相位转移规则（过渡仍问 `phase_next`）；feedback / verdict 解析失败 / confirm 停等语义不变 | `crates/core/orchestration/src/walker.rs`、`crates/core/workflow/src/write/phase_next.rs` |
| AC-3 | 执行图节点与边：一次 run 覆盖全步词汇后查询，`RunNodeRecord` 全词汇在案（含 phase_start / phase_log / 三门 gate），身份键 `(run_id, phase, attempt, step, instance_no)` 稳定唯一；`RunEdgeRecord` 边表 forward / feedback / retry / backtrack 四 kind 齐备；`persisted_step` 10→5 过滤删除 | `crates/core/workflow/src/state.rs`、`crates/core/orchestration/src/state.rs`、`crates/core/orchestration/src/run_history.rs`、`crates/infra/store/src/model.rs`、`crates/infra/store/src/store.rs`、`crates/infra/store/src/change_port.rs` |
| AC-4 | feedback 重跑新节点：static-check 失败 → executor 修复 → 重跑，图上 executor 首次 / 修复各为独立节点（`instance_no` 递增），边为 feedback（phase 内回边，区别于 retry）；前端读边表渲染，`deriveEdges` Δ 派生退役 | `crates/core/orchestration/src/walker.rs`、`crates/core/orchestration/src/control.rs`、`src/views/changes/flow/graph.ts` |
| AC-5 | 每 run 两写与边整包：假引擎一相位 run，库写事务恰两次（start 建 running 行 / finish 终态 + 节点整包 + 边整包单事务），无每步写事务；任一环节失败整体回滚（无半包节点 / 半包边） | `crates/core/orchestration/src/walker.rs`、`crates/core/orchestration/src/run_history.rs`、`crates/core/orchestration/src/control.rs`、`crates/infra/store/src/store.rs` |
| AC-6 | 出线与 golden：`ChangeDetail` 出线 `runs[].nodes` / `runs[].edges`（稳定实例键、边 kind 四值、全词汇）取代 `steps`；时间戳 ISO 串 + null 口径不变、纯 derive；golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕；bindings 一致性守卫绿 | `crates/core/workflow/src/queries/detail.rs`、`src/commands/changes/mod.rs`、`src/commands/change_flow/mod.rs` |
| AC-7 | artifact 清单驱动：`discover_artifacts` 按 phase 清单相对路径枚举命中（proposal.md / specs/** → proposal、design.md / tasks.md → dev-design、test-reports/** → test-execution）；`enumerate_candidates` 全树扫描与前端 `docColumn` 静态映射删除；eval-checklist 仍经 PhaseRecord 投影（phase + attempt） | `crates/core/workflow/src/artifacts/registry.rs`、`crates/core/workflow/src/artifacts/markdown_doc.rs`、`crates/core/workflow/src/artifacts/tasks_progress.rs`、`src/views/changes/flow/attachments.ts` |
| AC-8 | stdout 统一记录：static-check 以 `ExecOutput::Recorded { exec_id }` 捕获可回查（或 `Transient` 丢弃）；test-execution `report_dir` 与 `exec_id` 统一口径；infra/exec 输出记录单点，checks 层无私有 stdout 记录分支 | `crates/core/orchestration/src/port.rs`、`crates/infra/exec/src/lib.rs`、`crates/infra/checks/src/static_check.rs`、`crates/infra/checks/src/testexec/mod.rs` |
| AC-9 | 前端稳定身份渲染：`runStepNodes` 槽位配对 + `:seq` 堆叠删除，节点按后端 `(phase, attempt, step, instanceNo)` 渲染；同键多实例不串扰；图边读边表四 kind 呈现（feedback 虚线可辨） | `crates/core/orchestration/src/state.rs`、`src/views/changes/flow/graph.ts`、`src/views/changes/flow/run-state.ts`、`src/commands/change_flow/mod.rs` |
| AC-10 | 语料与管线交付：语料含 nodes / edges / feedback 边 / artifact 清单样本，golden 覆盖新投影；`vp test` / `client:check` / knip / bindings 全绿；desktop version 0.4.35；`plugins/dev-team` 零改动 | `crates/core/workflow/src/queries/detail.rs`、`crates/infra/store/src/store.rs` |

> 交叉承载注记：AC-5 的整包原子回滚断言由 `store_test.rs`（finish 单事务回滚）与 `walker_test.rs`（两写计数）交叉承载；AC-3 的「persisted_step 退役」由 `run_history_test.rs` 断言全词汇零过滤；AC-7 的 `ArtifactDescriptor.phase` 类型面由 `registry_test.rs` / `eval_checklist_test.rs` 交叉承载；AC-6/AC-10 的 golden 由 `corpus_golden_test.rs` 承载（挂靠 detail.rs golden 集成投影面）。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能

- phase_table(): 计划图单源（返回项 `PhaseDefinition` 增 `steps: [StepDefinition]` 与 `artifact_manifest: [ArtifactSpec]`；V1 仍仅 `requirement`；const + `include_str!` 静态装配不翻案）
- allowed_backtrack_phases(): 白名单语义不变（表序前置集含自身；未知相位空集）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 计划图单源 steps 声明 | 正向 | `phase_table("requirement")` 返回 8 相位，逐相位 `steps` 序列与 design 数据模型表一致：proposal/dev-design/test-design = `PhaseStart → Executor → Evaluator → VerdictGate(on_fail=Terminal) → PhaseLog`；implement/test-gen 在 Executor 后插 `StaticCheck(exec, on_fail=FeedbackLoop{5})`；test-execution = `PhaseStart → TestExecution(on_fail=FeedbackLoop{5}) → PhaseLog`；code-review/acceptance 无 Executor 步（AC-1） | 新增 |
| 计划图单源 steps 声明 | 边界 | 空相位（无 steps 声明相位）不存在——8 相位均至少含 `PhaseStart`；VerdictGate 仅出现于「有 evaluator 步」相位，code-review/acceptance 无 StaticCheck/TestExecution（门禁由 step 声明、相位限定，AC-2 门禁相位限定面） | 新增 |
| 计划图单源 steps 声明 | 异常 | `phase_table("bug-fix")` / `phase_table("test-only")` / `phase_table("refactor")` / `phase_table("")` → None（V1 仅 requirement，不臆测非 V1 workflow_type，AC-1） | 新增 |
| 计划图单源 artifact_manifest | 正向 | 逐相位 `artifact_manifest` 声明与 design 数据模型表一致：proposal 含 `markdown-doc·proposal.md` / `markdown-doc·specs/**`；dev-design 含 `markdown-doc·design.md` / `tasks-progress·tasks.md`；test-execution 含 `markdown-doc·test-reports/**`（AC-1/AC-7 归属权威源） | 新增 |
| 计划图单源 artifact_manifest | 边界 | 无产物相位（implement / test-gen / code-review / acceptance）→ `artifact_manifest` 空数组（清单零占位不虚设，AC-7 清单为唯一候选来源） | 新增 |
| 计划图单源 artifact_manifest | 异常 | 清单各 `path` 均为相对 POSIX 路径模式且无越界（绝对路径 / `../` 前缀 / 空 `path`）；`kind` 与清单绑定一致（proposal 清单不声明 tasks-progress kind）→ 相位表无此形态（负向断言，AC-7 清单为唯一候选来源、R6 双源去重） | 新增 |
| 计划图单源 prompt 静态化 | 正向 | 全部 agent 步的 `prompt_ref` 均为 `&'static str` 且非空（`include_str!` 编译期装配）；全表扫描无 `<role>` 间接读取（AC-1 零占位红线） | 新增 |
| 计划图单源 prompt 静态化 | 边界 | 非 agent 步（PhaseStart / StaticCheck / TestExecution / PhaseLog / 三门 gate）`prompt_ref` 恒 None（仅 agent 步携 prompt_ref，AC-1） | 新增 |
| 计划图单源 prompt 静态化 | 异常 | `prompt_ref` 指向的 `include_str!` 静态文本含运行时占位（`<role>` / `{{…}}`）或空串 → 相位表无此形态（零占位红线负向断言，AC-1） | 新增 |
| 计划图单源 on_fail 语义 | 正向 | `OnFail` 三态穷举可构造且与相位绑对：StaticCheck/TestExecution 携 `FeedbackLoop { limit: 5 }`、VerdictGate 携 `Terminal`、Executor/Evaluator 携 None（AC-2 语义前置） | 新增 |
| 计划图单源 on_fail 语义 | 边界 | `FeedbackLoop` 计数上限为 5（u32 边界：limit=0 / limit=1 / limit=u32::MAX 可构造不 panic，语义面由 walker 消费，AC-2/AC-4） | 新增 |
| 计划图单源 on_fail 语义 | 异常 | 除 implement/test-gen 的 StaticCheck 与 test-execution 的 TestExecution 外，任何步声明 `FeedbackLoop` / 任何 executor 步声明 `Terminal` → 相位表无此形态（门禁/终态仅与特定步绑定，负向断言） | 新增 |
| 计划图单源类型收敛 | 正向 | `PhaseDefinition` 结构收敛（穷尽解构含 `steps` / `artifact_manifest` 新字段；`executor` / `evaluator` 两角色齐备性既有回归保绿，AC-1） | 新增 |
| 计划图单源类型收敛 | 异常 | 序列化往返逐字段可辨：`PhaseDefinition` 反序列化后 steps / artifact_manifest 与 const 表逐项等值（防 const 表漂移） | 新增 |
| allowed_backtrack_phases 回归 | 正向 | 中段相位（test-gen）→ 表序前置集含自身；末相位（acceptance）→ 全表（白名单语义不变，AC 零改动锚） | 新增 |
| allowed_backtrack_phases 回归 | 边界 | 首相位（proposal）→ 空白名单；未知相位 / 空串 → 空 Vec 不臆测（AC 零改动锚） | 新增 |
| allowed_backtrack_phases 回归 | 异常 | 传入空表（`&[]`）+ 任意 current_phase → 空 Vec（无表不臆测，边界守门） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯 const 表直测，零依赖零 mock；`include_str!` 装配为编译期产物 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

#### 待测功能

- StepGroup / AgentRole / StepNode / OnFail / StepDefinition / ArtifactSpec: 计划图侧类型封闭集与线面（StepNode 10 值全词汇、`group()` / `role()` 派生访问器、OnFail 两变体、`prompt_ref` 静态引用）
- RunStepKind: 5 值 → 10 值全词汇（增 PhaseStart / PhaseLog / VerdictGate / RetryGate / WhitelistGate，snake_case 线面）
- RunNodeStateRecord / RunEdgeStateRecord / RunNodeEntry / RunEdgeEntry: 节点 + 边中性类型序列化往返（`RunStepStateRecord` / `RunStepEntry` 退役）
- RunFinishCommand: `steps` → `nodes` + `edges`（终态收口载荷换形）
- ChangeStateStore: `list_run_steps` 删除、增 `list_run_nodes` / `list_run_edges`（trait 编译面）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 计划图侧类型线面 | 正向 | `StepNode` 10 值全词汇 snake_case 逐字出线（executor / evaluator / decision / phase_start / static_check / test_execution / phase_log / verdict_gate / retry_gate / whitelist_gate）；`group()` 三分类（Exec/Agent/Gate）与 `role()`（Executor/Evaluator/Decision）派生访问器一致（AC-1/AC-3 全词汇前置） | 新增 |
| 计划图侧类型线面 | 边界 | `OnFail` 两变体序列化往返（`FeedbackLoop { limit }` 与 `Terminal`）可辨；`StepDefinition` 三字段（node / prompt_ref / on_fail）两态构造（prompt_ref None/Some、on_fail None/Some）（AC-1） | 新增 |
| 计划图侧类型线面 | 异常 | 非法词反序列化拒绝（`"exec"` / `"gate"` / `"unknown"` 等非 10 词汇）——封闭集防线零触点持衡（AC-1/AC-3） | 新增 |
| RunStepKind 全词汇 | 正向 | `RunStepKind` 10 值全词汇 snake_case 逐字出线（含 PhaseStart / PhaseLog / VerdictGate / RetryGate / WhitelistGate）；与 `ChangeStepKind`（orchestration 侧）1:1 全映射（AC-3 零过滤前置） | 新增 |
| RunStepKind 全词汇 | 异常 | 5 值时代旧词（若残留）不再出线；非法词反序列化拒绝（AC-3 退役面） | 新增 |
| RunNode/RunEdge 中性类型 | 正向 | `RunNodeStateRecord` / `RunEdgeStateRecord` 全字段序列化往返（节点身份键 phase/attempt/step/instance_no、status/session_id/detail/output_ref/timestamp；边 from_node_key/to_node_key/edge_kind/timestamp）逐字段可辨（AC-3/AC-6 线面前置） | 新增 |
| RunNode/RunEdge 中性类型 | 边界 | `output_ref` / `session_id` / `detail` None 与 Some 两态；`instance_no` 0 / 1 / u32::MAX 三态可构造（AC-3/AC-6 null 口径与身份键边界） | 新增 |
| RunNode/RunEdge 中性类型 | 异常 | `RunStepStateRecord` / `RunStepEntry` 已从公开出口移除（退役类型编译面缺席；引用旧类型不编译，AC-3） | 新增 |
| RunFinishCommand 载荷 | 正向 | `RunFinishCommand` 现携 `nodes: Vec<RunNodeEntry>` + `edges: Vec<RunEdgeEntry>`；`steps` 字段移除（AC-3/AC-5 收口载荷换形） | 新增 |
| RunFinishCommand 载荷 | 边界 | 空 nodes / 空 edges → 空包合法（零步 run 收口，AC-5） | 新增 |
| RunFinishCommand 载荷 | 异常 | nodes 内 `step` 非法词 / edges 内 `edge_kind` 非法词反序列化拒绝（封闭集防线）；旧 `steps: Vec<RunStepEntry>` 载荷反序列化拒绝或忽略（旧字段不复现，AC-3 退役面） | 新增 |
| ChangeStateStore trait | 正向 | trait 面含 `list_run_nodes(run_id)` / `list_run_edges(run_id)`、无 `list_run_steps`；最小假件实现后可调（AC-3 trait 编译面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯类型 serde 直测 + 纯内存等值面，零依赖零 mock | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs

#### 待测功能

- phase_next(): `PhaseNextOutcome` 增 `steps: Vec<StepDefinition>` + `artifact_manifest: Vec<ArtifactSpec>` 下发（next 相位计划步序列与产物清单随行；路由权威不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 计划步序列下发 | 正向 | next 相位（如 proposal → dev-design）→ `PhaseNextOutcome.steps` 与 `phase_table` 该相位 steps 声明一致；`artifact_manifest` 随行与该相位清单一致（AC-1 下发半边） | 新增 |
| 计划步序列下发 | 正向 | 路由权威不变：既有 phase_next 路由 / 白名单 / 重试上限用例保绿（仅响应结构扩字段，语义零改写，AC-2） | 新增 |
| 计划步序列下发 | 边界 | 相位无 artifact 清单（implement / test-gen 等）→ `artifact_manifest` 空 Vec 下发（零占位不虚设） | 新增 |
| 计划步序列下发 | 边界 | 末相位（acceptance）无 next → 既有语义不变（无 steps / manifest 下发） | 新增 |
| 计划步序列下发 | 异常 | store 返回 Err / 未建档 change → `Err(String)` 透传（既有无序改写） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（入参例外） | 既有进程内假件（可编程 read + Err 注入），作为被测函数显式入参 | 「store Err 透传」「路由回归」 |
| 无（真实组合） | 其余纯函数直测（phase_table 真件只读） | 「steps 下发」「空清单」 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

#### 待测功能

- change_detail(): `ChangeDetail` 出线 `runs[].nodes`（全词汇 + 稳定身份键 + outputRef）+ `runs[].edges`（四 kind）取代 `runs[].steps`；产物发现改清单驱动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| nodes/edges 出线 | 正向 | 建档 change 含一次覆盖 10 步词汇的 run → `runs[].nodes` 全词汇在案（含 phaseStart / phaseLog / verdictGate / retryGate / whitelistGate），每条 `phase/attempt/step/instanceNo/status/sessionId/detail/outputRef` 逐字段投影（AC-3/AC-6） | 新增 |
| nodes/edges 出线 | 正向 | 同 run 边整包出线：`runs[].edges` 四 kind（forward / feedback / retry / backtrack）齐备，`fromNodeKey` / `toNodeKey` 指向节点身份串（AC-3/AC-6） | 新增 |
| nodes/edges 出线 | 正向 | 稳定实例键出线：feedback 重跑两节点同 `(phase, attempt, step)` 而 `instanceNo` 递增可辨（AC-4/AC-6） | 新增 |
| nodes/edges 出线 | 正向 | 时间戳 ISO 串口径不变（与既有字段同式）；`outputRef` / `sessionId` / `detail` 各 None 与 Some 两态出线（AC-6 null 口径） | 新增 |
| nodes/edges 出线 | 边界 | 零 run change → `runs` 空数组；单节点零边 run → `edges` 空数组（miss 非错误） | 新增 |
| nodes/edges 出线 | 边界 | 文档形态 change（db 缺记录磁盘目录在场）→ `runs` 恒空数组、聚合不报错（AC-6） | 新增 |
| nodes/edges 出线 | 异常 | store `list_run_nodes` / `list_run_edges` 返回 Err → 该 run nodes/edges 降级空数组、详情其余面不受阻断（沿 list_phase_records unwrap_or_default 同口径） | 新增 |
| 既有随动 | 边界 | 既有 `ChangeDetail` 断言基设随 nodes/edges 键演进（构造器补 nodes/edges 字段；golden 显式重写后线面钉死，见 corpus 节，AC-6） | 新增 |
| corpus golden 集成投影（组合链路，链路发起方 = change_detail） | 正向 | 一次覆盖全词汇 + 四边 kind（含 feedback 边）的 run 样本 → golden 含 runs[].nodes 全词汇 + runs[].edges 四 kind，与库内记录逐字一致（AC-6/AC-10） | 新增 |
| corpus golden 集成投影 | 正向 | feedback 边样本：同 `(phase, attempt, step)` 两节点 instanceNo 递增、边端点指向节点身份串；artifact 清单样本（proposal.md / specs/** / design.md / tasks.md / test-reports/** 归属 phase）→ golden 含 `ArtifactDescriptor.phase` 投影（AC-4/AC-7/AC-10） | 新增 |
| corpus golden 集成投影 | 边界 | 既有 detail golden 全量增 nodes / edges 键（存量零 run 样本 → 空数组）——`DESKTOP_GOLDEN_REWRITE=1` 覆写模式 + 人工确认 diff 预期范围留痕后快照回归钉死（AC-6） | 新增 |
| corpus golden 集成投影 | 异常 | 种子含半包形态（节点与边数目不一致 / 边端点悬空指向不存在节点）→ golden 不产出或显式拒绝（整包不变量防半包 golden 入仓，AC-5/AC-10） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（入参例外） | 既有 DetailStore 假件扩展 run 半边：`list_run_nodes` / `list_run_edges` 可编程序列（含 Err 可编程），写半边 unimplemented panic（纯读面越权即暴露） | 本节全部用例 |
| 磁盘目录树（进程边界） | 既有 Env tempdir 真实目录（产物文件 + 定位面） | 「文档形态恒空 runs」 |
| 无（真实组合） | golden 集成投影经真件 store run 域操作面种子（语料构造器），真实 detail 读链 + serde_json 线面比对 | 「corpus golden 集成投影」 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/artifacts/registry.rs -> packages/desktop/src-tauri/crates/core/workflow/src/artifacts/registry_test.rs

#### 待测功能

- discover_artifacts(): 清单驱动枚举（读 phase 清单 → 相对路径匹配 → 插件 parse；`ArtifactDescriptor` 增 `phase` 归属；`enumerate_candidates` / `collect_files` 全树扫描退役）
- read_artifact(): 按 kind + source 解析单信封（source 校验口径不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 清单驱动发现 | 正向 | 清单命中 `proposal.md` / `specs/**` / `design.md` / `tasks.md` / `test-reports/**` 各自枚举为候选，`ArtifactDescriptor.phase` 归属随清单（proposal / dev-design / test-execution，AC-7） | 新增 |
| 清单驱动发现 | 正向 | 文件类候选经插件 parse 判 kind（markdown-doc / tasks-progress）；eval-checklist 走 `(phase, attempt)` PhaseRecord 投影分支，与文件类在出口汇合同一 `Vec<ArtifactDescriptor>`（AC-7 D3） | 新增 |
| 清单驱动发现 | 正向 | `specs/**` 前缀 glob 深路径命中（specs/x/y/spec.md）——相对路径枚举面（AC-7） | 新增 |
| 清单驱动发现 | 边界 | manifest 空 → 零文件候选；phases 空 → 零 eval-checklist 候选（两空态零伪候选） | 新增 |
| 清单驱动发现 | 边界 | 精确路径与 `**` 前缀 glob 双重命中同一文件 → 去重（候选唯一，无重复挂载，R6） | 新增 |
| 清单驱动发现 | 异常 | 清单未命中的全树文件（explore.md / 旧代际 phases/** / 任意路径）零入列——白名单取代全树扫描（AC-7 退役面） | 新增 |
| 清单驱动发现 | 异常 | 插件 parse 失败（markdown-doc / tasks-progress 内容不可解析）→ 该候选跳过、其余候选不受阻断（best-effort） | 新增 |
| read_artifact 回归 | 正向 | 按 kind + source 命中 → 解析单信封 Some；未知 source → None（source 校验口径不变，AC-7） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 磁盘目录树（进程边界） | 既有 tempdir 真实目录装置（产物文件 + eval-checklist PhaseRecord 种子），真实枚举 + 真实插件 parse | 本节全部用例 |
| 无（真实组合） | `manifest` 以 `phase_table` 真件只读供给；插件 matcher 真实组合（清单为唯一候选来源，零 mock） | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/artifacts/markdown_doc.rs -> packages/desktop/src-tauri/crates/core/workflow/src/artifacts/markdown_doc_test.rs

#### 待测功能

- MarkdownDocPlugin（KIND = "markdown-doc"）: `matches` 改「清单命中的 markdown-doc 候选项」而非「扩展名 == md」；title/rank 名单逻辑保留

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 清单类型判定 | 正向 | 清单命中的 markdown-doc 候选（proposal.md / design.md / specs/**）→ plugin 收录并 parse；title/rank 代际名单逻辑保留（AC-7） | 新增 |
| 清单类型判定 | 边界 | 清单未命中但扩展名 == .md 的任意文件（explore.md）→ 不再入列（「扩展名 == md」旧判据退役，AC-7） | 新增 |
| 清单类型判定 | 异常 | 清单命中的非 md 内容（tasks.md 属 tasks-progress 清单）→ markdown-doc 不收（kind 由清单绑定，跨插件不串扰，AC-7） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 磁盘目录树（进程边界） | 既有 tempdir 真实目录（.md 文件 + 清单命中的 markdown-doc 候选） | 本节全部用例 |
| 无（真实组合） | 插件 matcher 真实组合；`manifest` 由 registry 入参显式供给 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/artifacts/tasks_progress.rs -> packages/desktop/src-tauri/crates/core/workflow/src/artifacts/tasks_progress_test.rs

#### 待测功能

- TasksProgressPlugin（KIND = "tasks-progress"）: `matches` 改「清单命中的 tasks.md 候选项」；勾选计数 parser 保留

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 清单类型判定 | 正向 | 清单命中的 tasks.md 候选 → plugin 收录并解析勾选计数；parser 语义保留（AC-7） | 新增 |
| 清单类型判定 | 边界 | 清单未命中但名为 tasks.md 的任意文件 → 不入列（旧判据退役，AC-7） | 新增 |
| 清单类型判定 | 异常 | 清单命中但 tasks.md 勾选语法不可解析 → 该候选跳过 / 空进度，其余候选不受阻断（best-effort，AC-7） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 磁盘目录树（进程边界） | 既有 tempdir 真实目录（tasks.md + 清单命中的候选） | 本节全部用例 |
| 无（真实组合） | 插件 matcher 真实组合；`manifest` 由 registry 入参显式供给 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

- new_run_id(): 铸造 `run-<millis>`（不变）
- walk_run(): 计划步通用执行——`drive()` 改迭代相位 `steps`（按 `StepNode.group()` 分派 exec/agent/gate）；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 常量退役；`on_fail` 表达 feedback / terminal / confirm；feedback 重跑 = 新节点；每 run 两写时序保留
- 组合链路（挂靠本节，链路发起方 = walk_run）: walk_run → RunHistoryPort → StoreRunHistory → 真件 store 全链落库

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 计划步通用迭代 | 正向 | 假引擎 + 假写面驱动 proposal 相位（PhaseStart → Executor → Evaluator → VerdictGate → PhaseLog）按 steps 序执行；transition 仍问 `phase_next`（walker 零相位转移规则，AC-2） | 新增 |
| 计划步通用迭代 | 正向 | `STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 常量退役：implement/test-gen 的 static-check 由 `StepDefinition` 声明携 `FeedbackLoop{5}`，test-execution 由 `TestExecution` 步声明（walker 无 step 类型特判，AC-2） | 新增 |
| 计划步通用迭代 | 异常 | 假引擎 agent 步返回会话失败 → 按 `on_fail` 语义收场（executor 无 on_fail → 相位 fail；VerdictGate Terminal → 终态），不因 step 类型特判走错分支（AC-2） | 新增 |
| feedback 重跑新节点 | 正向 | static-check 失败 → executor 修复 → 重跑：executor 首次 / 修复各为独立节点（`instance_no` 递增），边 kind = feedback（区别于 retry，AC-4） | 新增 |
| feedback 重跑新节点 | 边界 | FeedbackLoop 独立计数 ≤ limit（5）；超限升格相位 fail（AC-2/AC-4 边界） | 新增 |
| feedback 重跑新节点 | 异常 | FeedbackLoop 修复过程中 executor 会话失败（修复步自身失败）→ 计数递增并继续 feedback 循环 / 超限升格（不静默吞失败，AC-2/AC-4） | 新增 |
| verdict 解析失败终态 | 正向 | `VerdictGate(on_fail=Terminal)` verdict 解析失败 → 终态停给用户（AC-2） | 新增 |
| verdict 解析失败终态 | 异常 | verdict 解析失败但 gate 步未声明 Terminal（若存在）→ 不吞为隐式 fail（on_fail 语义为唯一终态来源，AC-2） | 新增 |
| confirm 停等语义 | 正向 | phase 间 confirm 停等仍为 run 级 `auto_next_phase` 开关（非 step 属性，AC-2 语义不变） | 新增 |
| confirm 停等语义 | 边界 | 末相位（acceptance）confirm 停等后无 next → 停等收口（无悬空停等，AC-2） | 新增 |
| confirm 停等语义 | 异常 | `auto_next_phase` 关闭且 confirm 未应答 / 应答否定 → 不推进 next（停等不破，AC-2）；confirm 停等与 step 级 `on_fail`（FeedbackLoop/Terminal）不混淆（run 级开关独立于 step 属性，AC-2） | 新增 |
| 四边 kind | 正向 | 一次 run 覆盖四边 kind emit：forward（计划序推进）/ feedback（gate 失败回 agent）/ retry（相位 fail 同相重跑）/ backtrack（跨相位回跳）各自落边（AC-3/AC-4） | 新增 |
| 四边 kind | 异常 | 边端点悬空（from/to 节点不在本 run 节点集）→ 不落边 / 组合失败暴露（整包不变量，AC-3） | 新增 |
| 每 run 两写时序 | 正向 | 假引擎一相位绿跑（多步 emit）→ 计数假件断言恰两次调用（run_started 1 次 + run_finished 1 次）、run 中零额外 run 域写（AC-5） | 新增 |
| 每 run 两写时序 | 正向 | 真件组合（StoreRunHistory + TestDb）：多相位 run 走完 → `list_run_nodes` / `list_run_edges` 回读节点 + 边整包与 emit 序逐条一致（AC-3/AC-5） | 新增 |
| 每 run 两写时序 | 正向 | 三死亡路径同一出口：completed / stopped / failed 三形态各自完成第二写且终态与 reason 各异（AC-5） | 新增 |
| 每 run 两写时序 | 异常 | run_started Err（可编程假件注入）→ fail-fast：工具步 / 会话零调用、run 以 failed 收口且注册表除名 | 新增 |
| 每 run 两写时序 | 异常 | run_finished Err（可编程假件注入）→ best-effort 静默：run 照常返回终态、不重试不报错 | 新增 |
| new_run_id | 边界 | 返回形如 `run-<millis>`；两次铸造 id 可辨（毫秒级唯一铸造面） | 新增 |
| new_run_id | 异常 | 时钟回拨 / 同毫秒两次铸造 → id 仍唯一可辨（不依赖全局单调计数，铸造面不 panic） | 新增 |
| 既有用例基设（签名随动） | 边界 | 既有 walk_run 用例机械随动：加 history 参（真件走 StoreRunHistory / 计数走假件），断言不弱化 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| RunHistoryPort（入参例外） | 进程内假件：Mutex 记录 start / finish 调用序与命令、可编程 Err 注入（start 失败 / finish 失败两形态）——作为 walk_run 显式入参 | 「恰两次调用」「fail-fast」「best-effort」 |
| db 文件（进程边界） | 真件 TestDb（tempfile workspace 库）+ StoreRunHistory 真实委派（既有装置沿用，组合轮不注入假件） | 「真件组合回验」 |
| 假引擎（既有装置） | 沿用既有 scripted WorkerAgentPort / ToolStepPort 假件（被测入参，既有先例） | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

#### 待测功能

- ChangeStepState: 增 `instance_no: u32` + `output_ref: Option<String>`（步节点稳定身份键 + 输出引用）
- EdgeKind / ChangeStepEdge: 边四值词汇（Forward / Feedback / Retry / Backtrack）+ `NodeKey = (String, u32, ChangeStepKind, u32)` 端点
- ChangeRunSnapshot: 增 `edges: Vec<ChangeStepEdge>`
- RunUpdate / RunNotice: 增 `Edge { edge: ChangeStepEdge }` 变体

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ChangeStepState 身份键 | 正向 | `ChangeStepState` 增 `instance_no` / `output_ref` 字段序列化往返；同 `(phase, attempt, step)` 多实例经 `instance_no` 递增可辨（AC-3/AC-4/AC-9） | 新增 |
| ChangeStepState 身份键 | 边界 | `output_ref` None / Some(exec_id) 两态；`instance_no` 0 / u32::MAX 边界（AC-6/AC-8 null 口径） | 新增 |
| ChangeStepState 身份键 | 异常 | 同 `(phase, attempt, step, instance_no)` 两节点身份串相同 → 唯一性断言暴露（碰撞防线，AC-3）；`output_ref` 空串 / 非 exec_id 形态与合法 exec_id 可辨（AC-8） | 新增 |
| EdgeKind 四值 | 正向 | `EdgeKind` 四值 camelCase 线面逐字出线（forward / feedback / retry / backtrack）；`ChangeStepEdge` 三字段（from / to / kind）往返（AC-3/AC-6/AC-9） | 新增 |
| EdgeKind 四值 | 异常 | 非法边 kind 词反序列化拒绝（封闭集防线，AC-3） | 新增 |
| RunUpdate/RunNotice Edge 变体 | 正向 | `RunUpdate::Edge` / `RunNotice::Edge` 变体构造与 `kind()` 访问器一致（kind-only 信封，AC-6/AC-9 通知桥前置） | 新增 |
| RunUpdate/RunNotice Edge 变体 | 边界 | 快照含 `edges` 空数组（零边 run）；多边快照序稳定（emit 序，AC-5/AC-9） | 新增 |
| RunUpdate/RunNotice Edge 变体 | 异常 | Edge 变体端点悬空（from/to 身份串不在本快照节点集）→ 组合不变量暴露 / 拒绝（半包防线，AC-3/AC-5）；`RunNotice::Edge` 越权携带查询载荷（非 kind-only）→ 信封形状防线（AC-6/AC-9） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯类型 serde 直测 + 纯内存等值面，零依赖零 mock | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/run_history_test.rs

#### 待测功能

- persisted_step(): 删除（退役）——10→5 过滤整个退役；`ChangeStepKind` 与 `RunStepKind` 改 1:1 全映射（全词汇零过滤）
- finish_command(): 组装节点 + 边整包（`RunNodeEntry` 全词汇直映射 + `RunEdgeEntry` 四 kind 显式落边）
- StoreRunHistory::new(): store 适配器（持 `Arc<dyn ChangeStateStore>` 委派 run_started / run_finished）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| persisted_step 退役 | 边界 | `persisted_step` 已删除：全词汇直映射路径无过滤谓词（10 词汇 `ChangeStepKind` 与 `RunStepKind` 1:1 全映射，编译面锚——phaseStart/phaseLog 与三门不再被弃落，AC-3） | 新增 |
| persisted_step 退役 | 异常 | 引用 `persisted_step` 不编译（退役符号缺席；残留调用编译失败即证明过滤路径已摘除，AC-3） | 新增 |
| finish_command 组装 | 正向 | 全词汇 10 步 emit 序列输入 → `RunFinishCommand.nodes` 恰 10 条全词汇在案（含 phaseStart / phaseLog / verdictGate / retryGate / whitelistGate），零过滤（AC-3） | 新增 |
| finish_command 组装 | 正向 | 节点 `instance_no` 分配：同 `(phase, attempt, step)` feedback 重跑 → `instance_no` 递增稳定（AC-4 身份键） | 新增 |
| finish_command 组装 | 正向 | `edges` 四 kind 显式落边：`from_node_key` / `to_node_key` 端点指向节点身份串 `"{phase}:{attempt}:{step}:{instance_no}"`（AC-3/AC-4） | 新增 |
| finish_command 组装 | 正向 | run_id / change_id / status / reason 自 RunRequest 与收口参数透传等值；`output_ref` / `session_id` / `detail` 两态透传 | 新增 |
| finish_command 组装 | 边界 | 空 nodes / 空 edges → 空包（零步 run 收口合法，AC-5 整包含空集） | 新增 |
| finish_command 组装 | 异常 | 边端点引用不存在的节点身份串 → 组装仍透传 / 上层整包校验拒绝（不静默丢弃悬空边，AC-3/AC-5） | 新增 |
| StoreRunHistory 委派 | 正向 | 真件组合：run_started 落 running 行、run_finished nodes+edges 整包落库——`list_run_nodes` / `list_run_edges` 回读与命令逐字段一致（AC-5 第二写实装） | 新增 |
| StoreRunHistory 委派 | 异常 | 委派 Err（未建档 change / 重复 run_id 冲突）→ `Err(String)` 串语义透传且记因语境保留（walker fail-fast / best-effort 分流输入面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯函数直测（finish_command）+ tempfile 真件库（沿 walker_test TestDb 装置）承载 StoreRunHistory 委派 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/control.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs

#### 待测功能

- ChangeFlowControl::publish() / RunGuard: `RunEntry.steps` → `nodes` + `edges`（节点 `instance_no` 分配、边四 kind 显式落边）；`RunGuard::steps()` → `nodes()` / `edges()`；快照含 nodes + edges（进程内零 IO 不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 节点 + 边累积 | 正向 | `publish` 步节点 + 边双累积：`RunUpdate::Edge` 落边四 kind 显式、步节点 `instance_no` 递增；快照含 nodes + edges 且序稳定（AC-3/AC-4） | 新增 |
| 节点 + 边累积 | 边界 | 零边 run（单节点）→ 快照 `edges` 空数组；多实例同 `(phase, attempt, step)` 经 `instance_no` 递增不串扰（AC-4/AC-9） | 新增 |
| 节点 + 边累积 | 异常 | 未 begin_run 就 publish → 注册表无此 run（除名 / 无快照，不 panic 不残留半包状态） | 新增 |
| RunGuard 面 | 正向 | `RunGuard::steps()` → `nodes()` / `edges()` 拆面；`finish` 消费 self 后终态除名、无残留可改写面（AC-5 收口唯一出口） | 新增 |
| RunGuard 面 | 异常 | finish 后重复 publish / emit → 终态除名后无可写面（幂等收口断言） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `#[tokio::test]` 进程内 tokio 原语（broadcast / oneshot / watch / Mutex）真实组合；多任务挂起/唤醒以 `tokio::spawn` + `yield_now` 轮询握手 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

#### 待测功能

- StaticCheckOutcome: 增 `output_ref: Option<String>`（`ExecOutput::Recorded` 的 `exec_id` 引用）
- TestExecutionOutcome: 增 `exec_id: String`（与既有 `report_dir` 统一口径）
- ToolStepOutput: 随动透传

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 输出引用随 step 产出透传 | 正向 | `StaticCheckOutcome.output_ref`（None / Some(exec_id)）与 `TestExecutionOutcome.exec_id` 随 step 产出透传至 walker → run_history → RunNodeEntry.output_ref 全链一致（AC-8） | 新增 |
| 输出引用随 step 产出透传 | 边界 | `output_ref` None（Transient 即弃）/ Some（Recorded 可回查）两态；`exec_id` 与 `report_dir` 同口径（AC-8） | 新增 |
| 输出引用随 step 产出透传 | 异常 | `ToolStepOutput` 反序列化缺 `output_ref` / `exec_id` 字段 → 拒绝 / 缺省 None（线面契约守卫，AC-8） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯类型 serde 直测 + 既有五假 port 装置（WorkerAgentPort / ToolStepPort / StaticCheckRunner / TestExecutionRunner / RunEventSink 假件为被测入参） | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

- RunNodeRecord / RunEdgeRecord: 执行图模型（`node_key` / `edge_key` 主键、稳定身份键字段、四 kind、`output_ref`）
- RunStepRecord: 退役（native_model id 14 退役、新增节点 / 边两模型 id）
- WORKSPACE_STORE_FORMAT_VERSION: 2 → 3（version bump 重建零迁移）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| RunNode/RunEdge 模型 | 正向 | `RunNodeRecord` / `RunEdgeRecord` 落库回环（native_model 编解码逐字段一致：node_key/edge_key 主键打包、phase/attempt/step/instance_no 身份键、edge_kind 四值、output_ref）（AC-3） | 新增 |
| RunNode/RunEdge 模型 | 边界 | `node_key = hash64(run_id)<<64 拼接 node_seq` 与 `edge_key` 复合主键打包无碰撞；`from_node_key` 后继查询经 String 二级索引命中（AC-3 D4） | 新增 |
| RunNode/RunEdge 模型 | 异常 | 非法 edge_kind 词反序列化拒绝（封闭集防线）；同 `(phase, attempt, step, instance_no)` 两行主键冲突 → 拒绝（AC-3） | 新增 |
| RunStepRecord 退役 | 正向 | `RunStepRecord` 从 native_model 注册摘除（id 14 退役、新增节点/边模型 id）；引用旧模型不编译（AC-3 退役面） | 新增 |
| 版本治理 | 正向 | `WORKSPACE_STORE_FORMAT_VERSION` 2 → 3；存量库 additive 打开成功、version bump 整体重建零迁移（AC-3/AC-10） | 新增 |
| 版本治理 | 边界 | version=3 空库（零 run / 零 change）打开成功且注册表含 nodes / edges 两新模型、不含已退役 RunStepRecord（AC-3/AC-10） | 新增 |
| 版本治理 | 异常 | version=2 存量库（含 RunStepRecord）打开 → 触发整体重建而非 additive 迁移（零迁移链）；version=3 注册表残留已退役 RunStepRecord → 拒绝 / 重建（AC-3/AC-10） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | tempfile 真件库（native_db / native_model 编解码真实往返） | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

- start_change_run(): run 起始第一写（不变）
- finish_change_run(): 终态 + 节点整包 + 边整包 + active_phase 清位单事务
- list_change_runs(): run 清单（不变）
- list_run_nodes(): 取代 `list_run_steps`（`run_id` 序）
- list_run_edges(): 按 run 全量读边（`from_node_key` 后继查询另经二级索引）
- calibrate_interrupted_runs(): 启动标定（不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| run 域操作面-finish | 正向 | finish_change_run 节点 + 边整包单事务：全词汇节点（10 词汇、session_id/detail/output_ref 两态）+ 四 kind 边一次落，`list_run_nodes` / `list_run_edges` 回读逐字段一致（AC-3/AC-5） | 新增 |
| run 域操作面-finish | 正向 | finish 同事务 active_phase 清位：预置 active_phase=Some 的 change 收口后读回 None（AC-5） | 新增 |
| run 域操作面-finish | 异常 | 节点整包撞键注入（同 `(phase, attempt, step, instance_no)` 两行）→ 整事务回滚：run 行仍 running、`RunNodeRecord` 零行、`RunEdgeRecord` 零行、active_phase 原值保留（AC-5 整包原子回滚） | 新增 |
| run 域操作面-finish | 异常 | finish 目标 run 不在案 → NotFound；run 已终态再 finish → Err（非 running 不可再收口） | 新增 |
| run 域操作面-start | 异常 | 同 run_id 二次 start_change_run → Conflict（AC-5 start 冲突前置） | 新增 |
| run 域读面 | 正向 | list_run_nodes 按 run_id 圈定、身份键序稳定（同 phase/attempt/step 下 instance_no 递增）；list_run_edges 按 run 全量读、`from_node_key` 二级索引后继查询命中（AC-3） | 新增 |
| run 域读面 | 边界 | 无节点 / 无边 → 空数组（miss 非错误）；seq / instance_no 空洞序合法（AC-3） | 新增 |
| run 域读面 | 异常 | list_run_nodes / list_run_edges 传空串 / 未知 run_id → 空数组（不 panic，miss 非错误；读面幂等无副作用） | 新增 |
| run 域读面-list_change_runs | 正向 | list_change_runs 按 change_id 圈定既有回归保绿（running / 终态两态 run 均在案、序稳定，AC 零改动锚） | 新增 |
| 存量库与版本治理 | 正向 | 存量库 additive 打开：既有模型形态库文件（零 run 写入）重开成功、新 `RunNodeRecord` / `RunEdgeRecord` 表首轮读写正常；`WORKSPACE_STORE_FORMAT_VERSION` 2→3 version bump 重建零迁移（AC-3/AC-10） | 新增 |
| 启动标定 | 正向 | calibrate_interrupted_runs 既有回归保绿（残留 running → interrupted + active_phase 清位、幂等，AC 零改动锚） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | tempfile 真件库真实事务；「中途死亡」形态以构造残留（start 后不 finish）模拟，零 mock | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/infra/store/src/change_port.rs -> packages/desktop/src-tauri/crates/infra/store/src/change_port_test.rs

#### 待测功能

- impl ChangeStateStore for Store: `list_run_steps` → `list_run_nodes` / `list_run_edges`（委托同名 store 操作面）、`run_finish` 载荷换 nodes + edges、`StoreError` → `StoreFault` 映射单点不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| trait 委托等值 | 正向 | `&dyn ChangeStateStore` / `Arc<dyn>` 类型擦除驱动：`list_run_nodes` / `list_run_edges` 委托同名操作面、结果与直调 store 逐字段一致；`run_finish` 载荷 nodes + edges 整包落库（AC-3/AC-5） | 新增 |
| trait 委托等值 | 边界 | 无节点 / 无边 run → 空数组委托；`run_start` / `run_finish` 载荷空 nodes/edges 合法（AC-5） | 新增 |
| trait 委托等值 | 异常 | `StoreError` → `StoreFault` 三分支翻译不变（未建档 / 冲突 / 不可写映射单点，AC 零改动锚） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | tempfile 真实 Store 上以 trait object 类型擦除驱动（被测主体即真件，trait object 仅强转），零假件 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/infra/exec/src/lib.rs -> packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs

#### 待测功能

- SystemCommand()（公开方法不变）: bare / script / script_raw / script_with / arg / args / into_std / into_tokio（spawn 封装零改动）
- record_exec_output(): `ExecOutput::Recorded` 落盘半边（exec_id 铸造 + 落盘，返回 `exec_id` 引用）
- read_exec_output(): `ExecOutput::Recorded` 回查半边

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ExecOutput 落盘 | 正向 | record_exec_output 写入 change 报告目录树（`<report_dir>/exec-outputs/<exec_id>.log`），返回非空 `exec_id`；read_exec_output 回查 `Some(Vec<u8>)` 且与写入字节一致（AC-8） | 新增 |
| ExecOutput 落盘 | 正向 | 两次 record_exec_output → exec_id 铸造互异（引用唯一面） | 新增 |
| ExecOutput 落盘 | 边界 | stdout / stderr 全空 → 落盘空内容仍可回查（`Some(vec![])`，AC-8 空输出口径） | 新增 |
| ExecOutput 落盘 | 边界 | 超长 stdout / 含 \n \0 emoji 字节 → 落盘全量字节保真（引用式不截断，AC-8/R5） | 新增 |
| ExecOutput 回查 | 异常 | read_exec_output 未知 exec_id → None（miss 非错误） | 新增 |
| ExecOutput 落盘 | 异常 | 落盘目录不可写（报告树不可建）→ `Err(String)` 串语义 | 新增 |
| SystemCommand 回归 | 正向 | 既有 SystemCommand 组装形状用例保绿（script / script_raw / arg 壳包装与 argv 形态零改动锚） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统（进程边界） | tempfile 真实目录树（报告目录落盘 + 回查真实往返） | 本节全部用例 |
| 无（真实组合） | SystemCommand 纯组装直测（零 spawn） | 「SystemCommand 回归」 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs -> packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs

#### 待测功能

- ProcessStaticCheck: static-check 输出捕获改经 `ExecOutput`（默认 `Transient` / 可选 `Recorded`）；checks 层零私有 stdout 捕获 / 落盘分支

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 输出记录模式 | 正向 | 默认 `Transient`：spawn 即得、反馈边消费、即弃（无 id、全量不进库不落盘引用）；可选 `Recorded`：经 `record_exec_output` 落盘并回填 `output_ref = Some(exec_id)`（AC-8） | 新增 |
| 输出记录模式 | 边界 | `Recorded` 模式下 stdout 空 / 超长 / 特殊字节保真；`output_ref` None 与 Some 两态（AC-8） | 新增 |
| 输出记录模式 | 异常 | 落盘失败（报告树不可写）→ `Recorded` 回退 `Transient` 或 `Err` 透传（不因落盘失败阻断 static-check 主流程，AC-8） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 进程环境（进程边界） | 既有 `TEST_ENV_LOCK` 环境锁 + tempfile workspace 根（真实 spawn / 落盘 / 回查） | 本节全部用例 |
| infra/exec（注入依赖） | `record_exec_output` / `read_exec_output` 真件直调（输出记录单点，零 mock） | 「Recorded 落盘回填」 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs

#### 待测功能

- ProcessTestExecution: test-execution `report_dir` 与 `exec_id` 统一口径（输出记录经 infra/exec 单点；`report_dir` 复用既有 `change_test_reports/<name>/`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| report_dir 与 exec_id 统一 | 正向 | 一次 test-execution 绿跑 → `TestExecutionOutcome.exec_id` 与既有 `report_dir` 同口径；`exec_id` 经 `record_exec_output` 落盘、`read_exec_output` 回查一致（AC-8） | 新增 |
| report_dir 与 exec_id 统一 | 边界 | 报告目录已有残留 → 既有清理语义保留；`exec_id` 多次运行互异（AC-8） | 新增 |
| report_dir 与 exec_id 统一 | 异常 | 报告目录不可写 / 汇总写盘失败 → `Err(String)` 串语义透传（不静默吞失败，AC-8） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 进程环境（进程边界） | 既有 `TEST_ENV_LOCK` 环境锁 + tempfile workspace 根（真实 report_dir + 落盘 / 回查） | 本节全部用例 |
| infra/exec（注入依赖） | `record_exec_output` / `read_exec_output` 真件直调（输出记录单点，零 mock） | 「exec_id 统一回查」 |

---

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

#### 待测功能

- get_change_detail()（IPC）: 详情聚合统一视图（`ChangeDetailUnified`：`detail.runs[].nodes` / `detail.runs[].edges` 取代 steps；`ActiveRunView` 增 `edges`；时间戳 ISO 串口径不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 统一查询装配 | 正向 | 无运行 run：返回 `detail.runs[].nodes` / `detail.runs[].edges` 全史 + `active_run=null`——单命令一次返回（AC-6） | 新增 |
| 统一查询装配 | 正向 | 运行中 run（已 emit 多步 + 多边）→ `activeRun.edges` 活面 + nodes/edges 全词汇 emit 序出线（AC-6/AC-9 通知桥消费面） | 新增 |
| 统一查询装配 | 正向 | 时间戳 ISO 串口径与既有字段同式（转换单点在本命令层，AC-6） | 新增 |
| 统一查询装配 | 边界 | blank root / 开库失败 → None（既有早退语义回归）；文档形态 change → 文档形态 + active_run=null | 新增 |
| 统一查询装配 | 异常 | 未知 change 名（record 与定位双缺）→ None（既有语义不变） | 新增 |
| 既有读命令回归 | 正向 | list_changes / read_artifact / create_change / archive_change 既有用例保绿（本组零语义改动锚） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| tauri 运行环境（进程边界） | 既有装置：`tauri::test::mock_app()` + 真实 WorkspaceStores（tempdir 数据根）托管，`app.state::<…>()` 直调（沿本文件既有 Env / app_with 先例） | 本节全部用例 |
| 注册表活面 | 真实 `Arc<ChangeFlowControl>` 托管进 mock app；活面种子经 begin_run + publish（进程内真实组合，零 mock） | 「运行中 run」「终态退场」 |

---

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

#### 待测功能

- change_flow_start() / change_flow_stop() / change_flow_answer() / change_flow_confirm() / change_flow_watch()（IPC）: 请求-应答语义不变；仅组合根随 `ExecOutput` 记录器装配；通知桥随 `RunNotice::Edge` 演进

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 控制面回归 | 正向 | start / stop / confirm / answer / watch 既有用例保绿（签名与请求-应答零改动锚，AC-2/AC-5 命令面） | 新增 |
| 组合根装配 | 正向 | change_flow_start 组合根装配 `ExecOutput` 记录器（默认 Transient）→ walker 输出记录经 infra/exec 单点（AC-8） | 新增 |
| 组合根装配 | 异常 | 组合根装配缺 ExecOutput 记录器（注入失败）→ 启动早退 None / Err（不进入 walker，AC-8） | 新增 |
| 通知桥演进 | 正向 | `RunNotice::Edge` 变体随 shape 演进出线（kind-only 信封，客户端收通知重查统一视图，AC-6/AC-9） | 新增 |
| 通知桥演进 | 边界 | 零边 run → Edge 通知零次 / 空边集（无空边通知风暴，AC-9） | 新增 |
| 通知桥演进 | 异常 | Edge 通知丢失 / 乱序 → 客户端重查统一视图兜底（查询结果权威、通知仅为触发信号，AC-6/AC-9） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| tauri 运行环境（进程边界） | 既有装置：mock_app + 真实 WorkspaceStores / Arc<ChangeFlowControl> / Arc<StopRegistry> 托管（沿本文件 Env / app_with 先例） | 本节全部用例 |
| Channel（进程边界） | 沿既有 Channel 捕获装置驱动假引擎背景 run | 「RunNotice::Edge 出线」 |

---

### packages/desktop/src/views/changes/flow/graph.ts -> packages/desktop/src/views/changes/flow/graph.test.ts

#### 待测功能

- buildFlowGraph(): `deriveEdges` Δ 派生退役、改读 `runs[].edges`（四 kind）；边端点按节点稳定身份键寻址；feedback 边虚线呈现

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 边表直读 | 正向 | 后端返回 `runs[].edges`（四 kind）→ `FlowGraph.edges` 直读边表（不再经 Δ 派生）；`deriveEdges` 移除（AC-4/AC-9） | 新增 |
| 边表直读 | 正向 | 边端点按节点稳定身份键寻址（fromNodeKey / toNodeKey → `(phase, attempt, step, instanceNo)`），feedback 边以虚线呈现可辨（AC-9） | 新增 |
| 边表直读 | 边界 | 零边 run → `edges` 空数组；feedback 多实例同 `(phase, attempt, step)` 经 instanceNo 递增不串扰（AC-4/AC-9） | 新增 |
| 边表直读 | 异常 | 边端点指向不存在节点（半包 / 数据缺口）→ 不 panic，降级跳过 / 悬空边不渲染（AC-9 稳健面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `buildFlowGraph` 纯函数直测（内存构造 `ChangeDetail` fixture，零进程边界依赖） | 本节全部用例 |

---

### packages/desktop/src/views/changes/flow/attachments.ts -> packages/desktop/src/views/changes/flow/attachments.test.ts

#### 待测功能

- mountMaterials(): `docColumn` 静态映射退役、改读 `detail.artifacts[].phase` 清单归属；eval-checklist 走 PhaseRecord 投影分支

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 清单归属 | 正向 | `detail.artifacts[].phase` 归属驱动挂载（proposal / dev-design / test-execution 列）；`docColumn` 静态映射移除（AC-7） | 新增 |
| 清单归属 | 边界 | 无 artifact 的 change → 空挂载（无 docColumn 兜底）；eval-checklist 走 PhaseRecord 投影分支挂载（AC-7 D3） | 新增 |
| 清单归属 | 异常 | `detail.artifacts` 缺 phase 字段 / phase 未知 → 不 panic，跳过该 artifact（稳健面，AC-7） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `mountMaterials` 纯函数直测（内存构造 `ChangeDetail` / `ArtifactDescriptor` fixture，零进程边界依赖） | 本节全部用例 |

---

### packages/desktop/src/views/changes/flow/run-state.ts -> packages/desktop/src/views/changes/flow/run-state.test.ts

#### 待测功能

- unifiedRunSteps(): 改读 `runs[].nodes`（取代 `steps`）
- runStepNodes(): `runStepNodes` 槽位配对 + `:seq` 堆叠退役、节点按后端 `(phase, attempt, step, instanceNo)` 直渲染；feedback 多实例不串扰

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 稳定身份渲染 | 正向 | `runStepNodes` 节点按 `(phase, attempt, step, instanceNo)` 直渲染；`:seq` 堆叠移除；`unifiedRunSteps` 改读 `runs[].nodes`（AC-9） | 新增 |
| 稳定身份渲染 | 正向 | 同 `(phase, attempt, step)` 多实例（feedback 重跑）经 instanceNo 递增渲染为独立节点不串扰（AC-4/AC-9） | 新增 |
| 稳定身份渲染 | 边界 | 零节点 run → 空节点列表；单节点零边 → 无反馈边渲染（miss 非错误，AC-9） | 新增 |
| 稳定身份渲染 | 异常 | 节点缺 instanceNo / 未知 step 词汇 → 不 panic，降级处理（不渲染该节点或默认分组，AC-9 稳健面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `runStepNodes` / `unifiedRunSteps` 纯函数直测（内存构造 DTO fixture，零 react / 零进程边界依赖） | 本节全部用例 |

---

## 不可测试项

- **再导出出口面（`crates/core/workflow/src/write/mod.rs` / `crates/core/orchestration/src/lib.rs` / `crates/core/workflow/src/artifacts/mod.rs`）** — 零逻辑再导出（`StepDefinition` / `EdgeKind` / `ArtifactDescriptor.phase` 等出口面跟随），正确性由消费侧测试以导出可导入性编译面承载，无独立测试路径；不新增导出供测试调用（knip 纪律）。
- **类型 / 模型治理面（`crates/core/workflow/src/artifacts/envelope.rs` 的 `ArtifactDescriptor.phase`、`src/views/changes/flow/types.ts` 的 `FlowEdgeKind`/`RuntimeFlowNode`/`FlowEdge`）** — 无同名测试文件（envelope_test.rs / types.test.ts 不在设计范围），类型字段增改由消费侧测试（registry_test / eval_checklist_test / graph.test / run-state.test / attachments.test）以构造与投影交叉承载。
- **bindings 再生成一致性（`packages/desktop/src/types/generated/bindings.ts`）** — 生成物零手改、由 bindings 一致性守卫（管线命令）拦截漂移，非单测面；前端类型消费正确性由各前端测试以新类型 fixture 承载。
- **`package.json` version bump（0.4.34 → 0.4.35）** — 静态配置核对项，归实现阶段守线与验收阶段承载。
- **AC-1「无 db 定义表 / 无 port 缝定义读 / 无运行时文件 IO」「prompt 零占位」grep 面** — 源码静态审查约束（定义仅驻 `phase_table.rs` const + `include_str!`），由实现阶段红线自查 grep 与 code-review 承载，非进程内可断言；行为面（steps / artifact_manifest 声明一致、prompt 静态非空）已由 `phase_table_test.rs` 覆盖。
- **AC-2「walker 零相位转移规则」grep 面** — 「过渡仍问 phase_next」是 diff 面断言（walker 源码不出现转移规则常量 / 自持跳转）；行为面由 `walker_test.rs`（假写面驱动 transition 问 phase_next）+ `phase_next_test.rs`（路由权威回归）覆盖。
- **AC-3「persisted_step 10→5 过滤删除」「walker / control 源码零过滤逻辑」grep 面** — 过滤谓词删除是 diff 面断言；行为面（全词汇零过滤、三门与 phaseStart/phaseLog 均在案）已由 `run_history_test.rs` / `walker_test.rs` / `store_test.rs` / `corpus_golden_test.rs` 缺席断言覆盖。
- **AC-6 golden「人工确认留痕」** — 显式重写流程中的人工确认是流程性步骤，不可自动化断言；golden 等值回归本身由 `corpus_golden_test.rs` 覆盖（覆写模式仅在重写流程启用）。
- **AC-10 knip 零未用导出残留 / `vp test` / `client:check` / `server:check` 全绿 / desktop version 0.4.35 / `plugins/dev-team` 零改动** — 管线守线与静态核对项（版本号 / 插件产物零 diff），归实现阶段守线任务与验收阶段承载。
