# 测试设计: test-execution-zero-agent-decision

> **变更**: test-execution-zero-agent-decision
> **日期**: 2026-10-18
> **基于**: `proposal.md`（验收标准 AC-1 ~ AC-10）、`design.md`（架构组件 / 关键设计决策 D1 ~ D10 / 修改文件清单 / 公共函数与类型定义）
> **被测模块来源**: `design.md`「变更清单 - 修改文件」（8 个实现文件；「新增文件」为空；「修改文件（测试模块 —— 编译面机械随动）」的四个文件本身即测试文件，不另设 per-file 章节，其断言改写已由对应源文件章节承载）+ `proposal.md`「测试文件」表的 `src/views/changes/flow/graph.test.ts`（原 311 / 402 用例改写为投影锚；`graph.ts` 本身按 design D10 零改动，仍以「源文件 -> 测试文件」映射对入册）

用例表读法：`路径类型` ∈ {正向, 异常, 边界}；`迭代类型` ∈ {单值, 参数化, 表驱动, 遍历枚举}。测试框架沿用仓库既有约定：Rust 侧 `#[test]` / `#[tokio::test]` 内嵌于同目录 `*_test.rs` 兄弟模块，断言用标准 `assert!` / `assert_eq!`；前端 `packages/desktop` 用 `vite-plus/test`（vitest 风格 API，`describe` / `it` / `expect`），测试文件为 colocated `*.test.ts`。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|---------|----------|
| AC-1 | `allowed_backtrack_phases(TEST_ONLY_TABLE/REQUIREMENT_TABLE, "test-execution")` 不含自身；`(…, "implement")` 含自身 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs` |
| AC-2 | test-execution 首败（`conclusion=Fail`）：零 `WorkerRole::Executor` / `Evaluator` 请求、恰一条 `verdict=Fail` PhaseRecord、`TestExecution` 节点 failed、余下步不执行 | `packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` |
| AC-3 | 首败后走决策分叉：决策会话恰一次、`ModelLevel::High`；决策前零 `WhitelistGate` 节点 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` |
| AC-4 | 决策输出 `{"action":"retry"}` → 解析失败走软拒一次（无 `WhitelistGate` 节点、漂移 +1、同会话续注，`continue_session` 断言）→ 第二轮合法 `backtrack` 通过白名单门落账 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/decision_test.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs` |
| AC-5 | 决策 `backtrack` 目标 = 失败相位且该相位零 agent 步 → `WhitelistGate` failed 节点 + 自环文案；连续 3 次自环（漂移计数超 `DECISION_DRIFT_LIMIT = 2`）→ `Terminal::failed("决策漂移")`；前两次驳回均同会话续注纠正 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/decision_test.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs` |
| AC-6 | 组合链：fail → 决策 backtrack → implement → test-execution 复用门因 mtime 变化失效 → 真重跑 → pass；重入的 test-execution 入口边为 `backtrack`（非 `retry`） | `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` |
| AC-7 | `fail_decided` 回归：backtrack 重入后新 fail 条目照常分叉；同一条目不重复分叉（不触发第二次决策） | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs` |
| AC-8 | 查询面：库中含 `retry` 边的旧 run → detail 投影后 edges 不含该边，其余边序不变；图面投影锚：输入含 `retry` 边 → 输出图不含该边 | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs`、`packages/desktop/src/views/changes/flow/graph.test.ts` |
| AC-9 | 升格落账 report 携完整明细（长 findings 用例 > 200 字仍落在 ≤2000 内），decision prompt 含「失败相位报告」段与读报告指引 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/decision_test.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs` |
| AC-10 | 回归锚不变：initialize 基线两步 `on_fail=Terminal`、implement / test-gen 的 static-check 反馈边、verdict 解析失败反馈边、archive 11 步、`RunEdgeKind` 四值往返、`retry_gate` 线词冻结 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs`、`packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` |

> 范围边界：`packages/desktop/src/views/changes/flow/graph.test.ts` 属 proposal「测试文件」表明列的断言改写面（原 311 / 402 用例改写为「输入含 `retry` 边 → 输出图不含该边」的投影锚），design D10 同时声明 `graph.ts` 本身零改动——故本章节按「源文件 -> 测试文件」映射对入册，`待测功能` 只标注「design.md 未声明该文件的公共 API 变更」，用例只锚投影结果、不要求任何实现演进。`crates/infra/store` / `crates/infra/checks` / `port.rs` 字段面 / `src/types/generated/bindings.ts` 同属只读红线，不纳入被测范围（proposal「不要修改」清单）。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

<!-- design.md「公共函数 / API」子表未声明该文件的公共 API 变更；以下待测功能取自「类型定义」子表（OnFail 三变体 / RunEdgeKind 四值保留），不额外推导源文件导出面。 -->

#### 待测功能

- `OnFail`（类型定义）：三变体封闭集 `FeedbackLoop { limit }` / `Terminal` / `Escalate`，`escalate` 线词往返，缺省 `None` 语义不变
- `RunEdgeKind`（类型定义）：四值线面 `forward` / `feedback` / `retry` / `backtrack` 保留（`retry` 仅注释改「存量解码专用」，值域零改）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `OnFail::Escalate` 序列化 | 正向 | 序列化为 `"escalate"`（`rename_all = "camelCase"` 单元变体线词逐字） | 单值 |
| `OnFail` 三变体往返 | 正向 | `Escalate` / `Terminal` / `FeedbackLoop { limit: 5 }` 三变体 serialize → deserialize 等值还原 | 表驱动（三变体） |
| `OnFail` 非法词 | 异常 | `"escalateLoop"` / `"ESCALATE"` / `"Escalate"` / `""` / `null` 反序列化均 Err（封闭集防线不被放宽） | 参数化（5 值） |
| `OnFail::FeedbackLoop` 计数边界 | 边界 | `limit` = 0 / 1 / `u32::MAX` 往返无损（u32 边界；`Escalate` 无载荷不参与计数） | 参数化（3 值） |
| `StepDefinition.on_fail` 缺省形态 | 边界 | `None` 与 `Some(OnFail::Escalate)` 可辨；`Copy` / `PartialEq` 派生保留（既有结构体字面量零改动可构造） | 单值 |
| `RunEdgeKind` 四值线面 | 边界 | `forward` / `feedback` / `retry` / `backtrack` snake_case 逐字往返，值域仍四值（`retry` 变体存在性即解码兼容契约） | 遍历枚举 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 纯 serde 派生与类型面断言，零进程边界依赖 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能

- `allowed_backtrack_phases()`：白名单自含 ⟺ 可重做（`redoable` 自排除）；零 agent 步相位自排除；表序 / 缺省 / 表头早退语义不变
- `TEST_EXECUTION_STEPS`（表常量，来自「修改文件」表修改项 ①②）：两步 `on_fail` 由 `FeedbackLoop { 5 }` 改 `Escalate`；`Escalate` 归属守卫

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `allowed_backtrack_phases` 自排除 | 正向 | `TEST_ONLY_TABLE` / `REQUIREMENT_TABLE` 各一例：`"test-execution"` 结果不含 `"test-execution"` | 表驱动（两表） |
| `allowed_backtrack_phases` 自含 | 正向 | 两表 `"implement"` 结果含 `"implement"`（agent 步相位可重做） | 表驱动（两表） |
| 零 agent 步相位枚举 | 正向 | `"archive"` 在两表中均不含自身（第二个确定性门禁相位）；同表内 `test-gen` 含自身、`test-execution` 不含自身形成对照 | 表驱动（3 例） |
| 可重做相位全枚举 | 正向 | `proposal` / `dev-design` / `test-design` / `test-gen` / `code-analyze` / `code-review` / `acceptance` 两表各自在列 | 遍历枚举（7 相位 × 2 表） |
| 白名单序与长度 | 正向 | 结果为该相位前序中 `backtrackable` 相位按表序、自排除后长度减 1；`initialize` 前序零项 | 单值 |
| 未知 / 非法 `current_phase` | 异常 | 不存在的相位名 / `""` / 1200 字符超长 / `"🚀\n"` / 首尾空白变体 → 空 `Vec` 不臆测 | 参数化（5 值） |
| 表形态边界 | 边界 | 空表 `&[]` → 空 `Vec`；单元素表（仅 `initialize`）任意相位 → 空；`backtrackable = false` 的相位不进任何白名单 | 表驱动（3 形态） |
| `TEST_EXECUTION_STEPS` 两步声明 | 正向 | `TestExecution` / `MutationTest` 两步 `on_fail == Some(OnFail::Escalate)`；`PhaseStart` / `PhaseLog` 为 `None` | 表驱动（四步） |
| `Escalate` 归属守卫 | 正向 | 全表遍历：`Escalate` 仅允许出现在 `TestExecution` / `MutationTest` 步（其余步出现即断言失败——四个执行体只是否读 `on_fail`，误声明会静默失效） | 遍历枚举（全表所有步） |
| `on_fail` 回归锚（AC-10） | 边界 | `initialize` 的 `StaticCheck` / `TestExecution` 仍 `Terminal`；`implement` / `test-gen` 的 `StaticCheck` 仍 `FeedbackLoop { 5 }`；`archive` 11 步 `on_fail` 全 `None`；`FEEDBACK_LOOP_LIMIT == 5` | 表驱动（4 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 静态 `const` 相位表与纯函数直取，零注入 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs

<!-- design.md「公共函数 / API」子表未声明该文件的公共 API 变更；以下待测功能取自「修改文件」表（`fail_decided` 注释改写、`PhaseNextError` 两变体去 `eval_only`、`MaxRetriesExceeded` doc）与「类型定义」子表。 -->

#### 待测功能

- `fail_decided` 幂等闸：判据零改动，语义 = 该 fail 条目已进入过决策、不重复分叉（AC-7）
- `PhaseNextError`：两变体去 `eval_only` 字段；`MaxRetriesExceeded` 文案载体为变体 doc（「本相位已连续失败 N 次」）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `fail_decided` 新条目分叉 | 正向 | 最新 fail 条目 `decision_session_id = None` → `error = Some(VerdictFail { phase, round })`、`steps` 空 | 单值 |
| `fail_decided` 已决放行 | 正向 | 最新 fail 条目挂 `decision_session_id = Some(..)` → 不再报错，返回可执行 `steps` + `retry_feedback` | 单值 |
| `fail_decided` 回溯重入 | 正向 | 模拟 backtrack 重入后新 attempt 的 fail 条目（`decision_session_id = None`）→ 再次 `VerdictFail` 分叉 | 单值 |
| 重试窗口上限 | 异常 | `window_fails >= MAX_RETRY_TIMES` 且未已决 → `MaxRetriesExceeded { phase, round }`（变体无 `eval_only` 字段）；已决时达上限亦放行 | 表驱动（未决 / 已决） |
| 无 fail 条目 | 边界 | entries 全 pass / entries 为空 → `error = None`，直接出可执步序列 | 表驱动（2 形态） |
| 最新条目为 pass | 边界 | 最新条目 `verdict = Pass` 但携 `decision_session_id` → 不算 `fail_decided`（判据只看最新 fail 条目） | 单值 |
| fail 条目非最新 | 边界 | 早先 fail、其后 pass 条目 → 不触发决策分叉 | 单值 |
| `PhaseNextError` 字段面边界 | 边界 | `phase` 为空串 / 8 字符、`round` = 0 与 `u32::MAX` 均可构造且 `PartialEq` 可辨（两变体各一例） | 参数化（4 值） |
| 路由前置校验回归 | 异常 | `run_id` 空串 / 未知 change id → 既有 `Err` 语义不变（判据收缩不改前置校验） | 表驱动（2 例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ChangeStateStore` | 测试内假实现（预置 phase records / run 记录，可注入读 `Err`）——状态库属进程边界 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

<!-- design.md「公共函数 / API」子表未声明该文件的公共 API 变更；以下待测功能取自「修改文件」表（edges 投影前 `.filter(edge_kind != RunEdgeKind::Retry)`，读面唯一屏蔽点、边序不变）。 -->

#### 待测功能

- `change_detail()` 的 edges 投影面：投影前单点屏蔽 `RunEdgeKind::Retry`，其余边按 `edge_seq` 原序出线；nodes / runs 投影零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| edges 四 kind 混合投影 | 正向 | 库内 run 含 `forward` / `feedback` / `retry` / `backtrack` 四边 → 投影后恰 3 条，`retry` 缺席，其余按 `edge_seq` 升序 | 单值 |
| edges 全 `retry` | 正向 | 库内 run 边全为 `retry` → 投影后 `edges` 为空 `Vec`，查询 `Ok` 不报错 | 单值 |
| edges 集形态边界 | 边界 | 空边集 / 单条 `retry` / 单条 `forward` / `retry` 位于首、中、尾三种位置 → 屏蔽生效且其余边相对序不变 | 表驱动（6 形态） |
| 多 run 混合投影 | 边界 | run A 含 `retry`、run B 不含 → 各 run 独立投影，`runs` 数不变 | 单值 |
| nodes / runs 面零扰动 | 边界 | 同一 run 的 nodes 集与 runs 集数量、内容与库内一致（过滤只作用于 edges） | 单值 |
| edges 读失败透传 | 异常 | `list_run_edges` 注入 `StoreFault` → `change_detail` 整份 `Err`（过滤链不吞错、零降级空数组） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ChangeStateStore` | 测试内假实现（`run_edges` 直插含 `retry` 的行；`arm_edges_fault` 注入读 `Err`）——状态库属进程边界 | 全部用例 |
| 文件系统 | `TempRoot` 临时 change 目录（workspace 定位面）——运行环境隔离 | 全部用例 |

---

### packages/desktop/src/views/changes/flow/graph.ts -> packages/desktop/src/views/changes/flow/graph.test.ts

<!-- design.md「公共函数 / API」子表未声明该文件的公共 API 变更：`graph.ts` 按 design D10 零前端改动（`retry` 落默认样式、端点缺失边本就 `continue`），故 `#### 待测功能` 只以一条锚点条目说明「无 API 变更、零实现改动」，不反推设计外条目。
    该章节入册依据 = proposal.md「测试文件」表：`src/views/changes/flow/graph.test.ts`（原 311 / 402）处置为「输入含 `retry` 边 → 输出图不含该边」的投影锚改写；design.md「变更清单」注释与 tasks.md 阶段边界均明示该断言层改动归测试相位。 -->

#### 待测功能

- `buildFlowGraph()`: 图边投影结果契约——输入含 `retry` 边 → 输出图不含该边（design.md「公共函数 / API」子表未声明该文件变更，函数签名与实现零改动，本条目只锚既有导出面的结果契约）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `buildFlowGraph` 图边投影锚（AC-8 前端侧） | 正向 | 输入边表含 `retry` 行（模拟读面投影前的存量库行）且该行端点不在节点集 → 输出图 `edges` 零 `kind` = `retry`，`forward` / `feedback` / `backtrack` 三类按输入序全量出线（原 311 用例改写：期望值不再含 `retry`） | 单值 |
| `buildFlowGraph` 图边投影锚（AC-8 前端侧） | 边界 | `retry` 行位于输入边表首 / 中 / 尾三位置各一例 → 输出图仍零 `retry`，其余边相对序与输入序一致（`edge:<index>:…` 序号不因跳边错位到别的 kind 上） | 表驱动（3 位置） |
| `buildFlowGraph` 图边投影锚（AC-8 前端侧） | 边界 | 输入边表仅 `retry` 行（端点全悬空）→ `edges` 为空数组、不抛错；输入空边表 → `edges` 为空数组（读面投影后「无 retry」形态的正向对照） | 表驱动（2 形态） |
| `buildFlowGraph` 图边投影锚（AC-8 前端侧） | 异常 | `retry` 行一端悬空（`ghost` 身份串，原 402 用例）→ 该边跳过、不炸图，同批 `feedback` / `forward` 边照常出线 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 纯函数直测：`detail` / `runNode` / `edgeRow` fixture 构造器内联，真实组合 `layout`（`detail.pipeline` 列序）与 `run-state`（`nodeIdentityKey`），零 mock | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

<!-- design.md「公共函数 / API」子表未声明该文件的公共 API 变更；本文件在「修改文件」表中仅为纯注释面改动（`ChangeStepKind::RetryGate` doc 改「决策动作兑现门」、`MutationTest` doc 去「独立反馈边②」），变体名与 `as_str` 线词二十一值封闭集零改动，故不设用例。回归锚为既有 state_test 的线词封闭集 / 分组角色派生用例。 -->

#### 待测功能

（无。纯 doc 注释改动：`retry_gate` 与 `mutation_test` 线词、二十一值封闭集、`as_str` 输出均不变。）

#### 用例

（无。行为面零变更，无可断言新增；既有线词封闭集用例继续充当 AC-10 回归锚。）

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 无行为变更，不涉及注入 | — |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

<!-- design.md「公共函数 / API」子表未声明该文件的公共 API 变更（改动函数均为模块私有：step_test_execution / step_test_execution_escalate / step_mutation_test / step_mutation_test_escalate / step_*_upgraded_phase_log / feedback_limit / decision_session / build_decision_input / truncate_report / mutation_report_refs）。以下待测功能逐条对应「修改文件」表 ①~⑦ 修改项，全部经公开入口 `walk_run` 直驱假引擎 + 假写面验证。跨模块组合用例（fail → 决策 → backtrack → 重跑）以 walker 为链路发起方挂靠本章节。 -->

#### 待测功能

- 首败升格执行体（TestExecution 步）：`step_test_execution` 三分派 + `step_test_execution_escalate` 单发（AC-2）
- 首败升格执行体（MutationTest 步）：`step_mutation_test` 二分派 + `step_mutation_test_escalate` 单发（runner `Err → Terminal` 保留）
- 升格落账：`step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log`（report 携完整明细、checklist 恰一条、三会话槽位恒 `None`、去 `fix_session`）
- `truncate_report()`：≤2000 原样 / 超长留 1 字符余量 + `…` / 空明细兜底（D2）
- `mutation_report_refs()`：`report_paths` 原文拼接 / 空清单占位（D3）
- `feedback_limit()`：增 `Escalate` 漂移 arm（D1）
- `decision_session()`：软拒环 + `DECISION_DRIFT_LIMIT = 2` + 每轮 `WhitelistGate` 实例 + ask 不计漂移（D6、AC-3 / AC-4 / AC-5）
- `build_decision_input()`：`fail_report` 取失败相位最新 attempt 原文、去 `eval_only`（D7、AC-9）
- `resolve_deadlock()`：删门节点/门校验/`Retry` 分支，兑现面只剩 `backtrack` / `stop`（D8、AC-6）
- `drive()`：`StepFlow::Upgraded` 截断余步、backtrack 入口边优先级（AC-6 组合链）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `step_test_execution`（Escalate） | 正向 | `conclusion=Fail` 首败：恰 1 条 `verdict=Fail` PhaseRecord（`decision_session_id=None`）、`TestExecution` 节点 failed、`WorkerRole::Executor` 与 `Evaluator` 请求数均为 0、`MutationTest` 与 `PhaseLog` 步未执行、`StepFlow::Upgraded` 回 phase-next | 单值 |
| `step_test_execution`（Escalate） | 正向 | `conclusion=Pass`：挂 `cursor.test_outcome` 并 Proceed，`MutationTest` 照序执行、零升格落账 | 单值 |
| `step_test_execution`（Escalate） | 异常 | `conclusion=Error`：同式升格——零会话、恰 1 条 fail 条目、余下步不执行 | 单值 |
| `step_test_execution`（Escalate） | 异常 | `run_tool(TestExecution)` 返回 `Err`（`fail_on("test-execution")`）：`Terminal` 收敛，零 fail 条目、零升格 PhaseLog 节点（环境问题不进决策） | 单值 |
| `step_test_execution`（分派） | 边界 | `on_fail = Some(OnFail::Terminal)`（initialize 基线绿门）走既有 terminal 执行体且零决策请求；`on_fail = None` → 漂移 `Err` | 表驱动（2 形态） |
| `step_test_execution`（分派） | 边界 | `on_fail = Some(OnFail::FeedbackLoop { limit: 5 })` 误声明到 test-execution 步：显式漂移 `Err`，零 `run_worker`、零 loop（不得静默退化为「限 0」禁用） | 单值 |
| `step_test_execution`（升格实例面） | 边界 | 同一 attempt 内 `TestExecution` 节点 `instance_no` 恰 1（零重跑）、升格 `PhaseLog` 节点恰 1 个 | 单值 |
| `step_mutation_test`（Escalate） | 正向 | `conclusion=Fail` 首败：恰 1 条 fail 条目、零 Executor / Evaluator 请求、`MutationTest` 节点 failed、下游 `PhaseLog` 不执行、`Upgraded` | 单值 |
| `step_mutation_test`（Escalate） | 正向 | `conclusion=Pass`（含 `skip` 跳过面）：挂 `cursor.mutation_outcome` 并 Proceed，PhaseLog 落 pass 条目 | 单值 |
| `step_mutation_test`（Escalate） | 异常 | `run_tool(MutationTest)` 返回 `Err`：`MutationTest` 节点 failed + `Terminal`，零升格条目、零决策请求（runner `Err → Terminal` 分支原样保留） | 单值 |
| `step_mutation_test`（Escalate） | 异常 | `conclusion=Error`：同式升格，report 取自 `feedback_text` | 单值 |
| `step_mutation_test`（分派） | 边界 | 非 `Escalate` 声明（`None` / `Terminal` / `FeedbackLoop`）落在 MutationTest 步：漂移 `Err`（该步无语义合法 `Terminal`） | 表驱动（3 值） |
| `truncate_report` | 正向 | 明细 500 字符：report 逐字原文落账（无省略号） | 单值 |
| `truncate_report` | 边界 | 明细长度 0 / 1 / 1999 / 2000 / 2001 / 2500 字符：≤2000 原样、2001 取前 1999 + `…`（总长恰 2000、写面不拒）、2500 同式截断 | 参数化（6 值） |
| `truncate_report` | 边界 | 多字节内容（CJK / emoji / `\n` / `\0`）：按 `chars()` 计数截断、不 panic、不切坏字符边界 | 参数化（4 形态） |
| `truncate_report` | 边界 | 明细空串（fail 态理论不达）：兜底 `conclusion=…；报告目录 …`（test 步）/ `conclusion=…；报告路径 …`（mutation 步） | 表驱动（两处） |
| 升格落账（test） | 正向 | checklist 恰 1 条 `{item:"测试执行", pass:false}`，evidence = `diagnose_brief(findings_brief)` + `；` + `report_dir`；`executor_session_id` / `evaluator_session_id` / `decision_session_id` 三槽位全 `None` | 单值 |
| 升格落账（mutation） | 正向 | checklist 恰 1 条 `{item:"突变测试", pass:false}`，evidence = brief + `；` + `report_paths.join("、")`；三会话槽位全 `None` | 单值 |
| `mutation_report_refs` | 正向 | 两元素 `report_paths` → `"a、b"` 原文拼接、零改写 | 单值 |
| `mutation_report_refs` | 边界 | 空清单 → `"（无报告产物）"`；单元素 → 原文；超大列表（64 条）→ 全量拼接不 panic | 参数化（3 形态） |
| `mutation_report_refs` | 边界 | 单路径超长（1200 字符）：拼入 evidence 后由 `diagnose_brief` 折叠截断（evidence 200 字上限） | 单值 |
| `feedback_limit` | 异常 | `Some(OnFail::Escalate)` → `Err(Terminal::failed("计划图漂移：…不得声明 OnFail::Escalate"))` | 单值 |
| `feedback_limit` | 边界 | `FeedbackLoop { 0 }` / `{ 1 }` / `{ u32::MAX }` → `Ok(n)` 原值透传；`Terminal` / `None` → 既有漂移 `Err` 文案（回归） | 参数化（5 值） |
| `decision_session` | 正向 | 首轮合法 backtrack（to 在白名单）：`WorkerRole::Decision` 请求恰 1 次且 `model_level == ModelLevel::High`；返回前 `WhitelistGate` passed 节点恰 1 个 | 单值 |
| `decision_session` | 正向 | 决策前节点集零 `WhitelistGate`（门已迁入会话，`resolve_deadlock` 不再 emit 门节点） | 单值 |
| `decision_session` | 异常 | 第 1 轮输出 `{"action":"retry"}`：解析 `Err` → 漂移 1、**零** `WhitelistGate` 节点、第 2 轮请求 `continue_session == Some(第 1 轮 session_id)` 且 prompt 为 `decision_fix_prompt` 纠正文案；第 2 轮合法 backtrack 经门 passed 后落账 | 单值 |
| `decision_session` | 异常 | 第 1 / 2 轮均自环（`to == "test-execution"` 且不在白名单）：各 emit 1 个 failed `WhitelistGate`（detail 含自环文案）、漂移 1 / 2 均续注同会话；第 3 轮漂移 3 > `DECISION_DRIFT_LIMIT` → `Terminal::failed("决策漂移…")` | 单值 |
| `decision_session` | 异常 | 第 1 / 2 轮未知名动作（`{"action":"explode"}` / `{"action":"Retry"}`）：两次软拒（零门节点）+ 同会话续注；第 3 轮合法 stop → 门 passed → 正常收敛 | 表驱动（2 个非法词 × 3 轮序列） |
| `decision_session` | 异常 | 越权非自环（`to = "code-analyze"`，白名单外）：failed 门节点 detail 携含 `allowed` 列表的通用文案；漂移 1 后同会话续注 | 单值 |
| `decision_session` | 边界 | 漂移计数边界：漂移 2（第 2 次驳回）仍续注、漂移 3 才 `Terminal`；混合序列（解析 `Err` → 门 failed → 解析 `Err`）连续累加不分流 | 表驱动（3 序列） |
| `decision_session` | 边界 | ask 停等：emit `RunUpdate::Ask` → 应答后续问（漂移不变、`continue_session` 更新）；连续 3 轮 ask 后漂移仍 0；`wait_answer` 返回 `None` → `Terminal::stopped("ask 停等被终止")` | 参数化（1 轮 / 3 轮 / 终止） |
| `decision_session` | 边界 | 每轮（含 ask 轮与驳回轮）`ToolCommand::DecisionLog` 恰 1 次且 phase / session_id 与当轮请求一致 | 单值 |
| `build_decision_input` | 正向 | `fail_report` = 失败相位最新 attempt 的 `report` 原文（500 字符长文逐字保留、不二次拼装）；`fail_checklist` 仅失败项；`allowed` 原样；`candidates` 排除失败相位自身 | 单值 |
| `build_decision_input` | 边界 | checklist 全 pass 配 fail verdict → 全量随行不虚减；station 缺席 → `attempt = 0` 且 checklist 空；`attempt` = 0 与 u32 上限均可承载 | 表驱动（3 形态） |
| `build_decision_input` | 异常 | 快照 `detail` 读取 `Err` → `Terminal::failed("快照读取失败…")`，零决策请求；`last_result` 缺席 → `Terminal`（无法组装决策输入） | 单值 |
| `resolve_deadlock`（兑现面） | 正向 | backtrack 兑现：`ToolCommand::Backtrack` 恰 1 次、`trace.incoming == Some(EdgeKind::Backtrack)`、`RetryGate` 节点 passed 恰 1 个、run 边零 `retry` 产出 | 单值 |
| `resolve_deadlock`（兑现面） | 正向 | stop：门 passed → `Terminal::stopped`，零 `Backtrack` 写面命令、零相位重跑 | 单值 |
| `resolve_deadlock`（兑现面） | 边界 | `PhaseNextError` 解构去 `eval_only` 后两变体（`VerdictFail` / `MaxRetriesExceeded`）均能组装同形决策输入并进入软拒环 | 表驱动（两变体） |
| 组合链（fail → 决策 → backtrack → 重跑） | 正向 | fail → 决策 backtrack 到 `implement` → `implement` 执行 → `test-execution` 重入（复用门因 mtime 变化失效）→ 真重跑 → pass；重入入口边 kind = `backtrack`（非 `retry`），全程零 `retry` 边 | 单值 |
| 组合链（backtrack 到自身） | 边界 | backtrack 到自身（可重做相位 `implement`，白名单自含）→ 门 passed + `Backtrack` 落账 → 下一轮 phase_next 回溯分支先手接管（`fail_decided` 放行、不二次决策） | 单值 |
| 回归锚（AC-10） | 边界 | `implement` / `test-gen` 的 static-check 反馈边仍 5 次循环 + 超限升格；verdict 解析失败反馈边仍续注修复；`initialize` 基线两步 `Terminal` 停给用户；`archive` 11 步零升格 | 表驱动（4 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `WorkerAgentPort`（agent 子进程会话） | 假引擎 `FakeWorker`：预录 decision / evaluator 产出队列、捕获 `WorkerTurnRequest`（角色、`model_level`、`continue_session`）与会话序，可编程 `Err` / `Stopped` 收敛 | 决策软拒环、漂移上限、升格零会话、回归反馈边 |
| `ToolStepPort`（工具 / 写面命令通道） | 假工具 `FakeTools`：预录 `TestExecutionOutcome` / `MutationTestOutcome` 序列、捕获 `ToolCommand`（PhaseLog / Backtrack / DecisionLog / TestExecution / MutationTest），`fail_on(label)` 注入 `Err` | 首败升格、截断落账、backtrack 兑现、错误分支 |
| `WorkflowSnapshotPort` | 假快照实现（`ChangeDetail` 由真实 `Store` 种子数据装配、可注入读 `Err`）——快照读取属进程边界 | `build_decision_input` 与快照失败用例 |
| 状态库 / 文件系统 | 真实 `Store` + `TempRoot` 临时 change 目录（不 mock：写面读面真实组合） | 全部端到端驱动用例 |
| `decision.rs` / `prompt.rs` / `verdict.rs` / `phase_next.rs` | 不 mock（真实组合，最小 mock 原则） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/decision_test.rs

#### 待测功能

- `parse_decision()`：`DecisionAction` 收缩为三动作封闭集后，`{"action":"retry"}` 与未知名同走结构漂移 `Err`（软拒路径入口）
- `ensure_backtrack_allowed()`：增第二参 `failed_phase`；自环与通用越权两类文案单点；非 backtrack 恒 `Ok`
- `DecisionAction`（类型定义）：三变体 `Backtrack` / `Stop` / `Ask`，`Retry` 删除
- `DecisionInput`（类型定义）：删 `eval_only`、增 `fail_report: String`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `parse_decision` 三动作解析 | 正向 | `backtrack`（含 to + reason）/ `stop` / `ask`（含 options 列表）三变体逐变体解析成功 | 表驱动（三变体） |
| `parse_decision` 载体提取 | 正向 | 围栏代码块包裹与前后混杂叙述 → 提取解析成功（既有行为回归） | 单值 |
| `parse_decision` 已退役动作 | 异常 | `{"action":"retry"}` → `Err`（结构漂移，不产出变体；软拒环由此计数） | 单值 |
| `parse_decision` 非法结构 | 异常 | `{"action":"explode"}` / `{"action":"Retry"}` / 缺 `action` / `""` / 纯空白 → 均 `Err` | 参数化（5 值） |
| `parse_decision` reason 长度 | 边界 | backtrack reason 501 / 2000 字符 → 解析层原样承接不拒（≤500 归写面 `backtrack` 单点拒绝） | 参数化（2 值） |
| `parse_decision` 字段缺失/冗余 | 边界 | backtrack 缺 `reason` → `Err`；`to` 为空串 → 解析成功交由门判定；含多余字段 → 解析成功 | 表驱动（3 形态） |
| `ensure_backtrack_allowed` 放行 | 正向 | `to` 在白名单 → `Ok`；`Stop` / `Ask` 动作恒 `Ok`（不看白名单） | 表驱动（3 动作） |
| `ensure_backtrack_allowed` 自环 | 正向 | `to == failed_phase` 且不在白名单 → `Err`，文案含「确定性门禁相位（零 agent 步）」与 `stop` / `ask` 出路 | 单值 |
| `ensure_backtrack_allowed` 越权 | 异常 | `to` 在白名单外且 ≠ `failed_phase` → `Err`，文案携 `allowed` 列表原文 | 单值 |
| `ensure_backtrack_allowed` 空白名单 | 异常 | `allowed = []` 时 backtrack 任意目标 → `Err`（自环与越权两类文案各一例，无静默放行出口） | 表驱动（2 例） |
| `ensure_backtrack_allowed` 自含即重做 | 边界 | `to == failed_phase` 且**在**白名单（可重做相位）→ `Ok`（白名单自含 = 重做本相位） | 单值 |
| `ensure_backtrack_allowed` 匹配边界 | 边界 | `allowed` 含重复项 / 50 项超大列表 / `failed_phase` 空串 / 大小写差异（`"Implement"` ≠ `"implement"`）→ 判定不误放行 | 参数化（4 形态） |
| `DecisionInput.fail_report` 字段面 | 正向 | 构造含 `fail_report` 的输入：空串 / 2000 字符 / 含 emoji 与换行 → `PartialEq` 等值可辨、字段独立于 `candidates` | 参数化（3 形态） |
| `DecisionAction` 封闭集边界 | 边界 | `Backtrack` 的 `to` / `reason` 取空串与 1200 字符超长值可构造且不 panic（长度门在写面与门层，不在类型层） | 参数化（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 纯解析 / 校验函数与类型面，零注入 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs

#### 待测功能

- `decision_prompt()`：三动作协议（backtrack / stop / ask）、「失败相位报告」段渲染 `fail_report`、白名单自注记「（= 重做本相位）」、读报告文件指引（AC-9、D4）
- `decision_fix_prompt()`（新增）：驳回原因 + 三动作重申 + 白名单一行（软拒环纠正文案单点，D6）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `decision_prompt` 段落结构 | 正向 | 失败清单 / 可回溯白名单 / 候选报告 / 失败相位报告 / 报告文件五段在场；协议段恰列 backtrack / stop / ask 三动作，且不含 `{"action": "retry"}` 行 | 单值 |
| `decision_prompt` 失败报告段 | 正向 | 「失败相位报告」段渲染 `input.fail_report` 原文（500 字符长文逐字在内）；「报告文件」段含 `read` 与 `reports/test/summary.json`、突变段级 / 归并报告指引 | 单值 |
| `decision_prompt` 白名单自注记 | 正向 | `allowed` 含 `phase` 自身时该条目追加「（= 重做本相位）」，其余条目零注记（两两对照） | 表驱动（自含 / 不含自身两形态） |
| `decision_prompt` 空/超长报告 | 边界 | `fail_report` 空串 → 「（无报告文本）」；恰 2000 字符原样；直构 3000 字符异常输入 → 原样插值不 panic（截断在 walker 侧单点） | 参数化（3 形态） |
| `decision_prompt` 集合面边界 | 边界 | `allowed` 空 / 单元素 / 50 项；`candidates` 空 / verdict 全 `None` → 段落结构稳定、占位形态一致 | 表驱动（5 形态） |
| `decision_prompt` checklist 边界 | 边界 | `fail_checklist` 空 / 单条 / 含换行、emoji、`\0` 的 evidence → 段落不被破坏、条目逐条出线 | 参数化（3 形态） |
| `decision_prompt` 静态与确定性 | 异常 | 组装结果零模板占位残留（`{phase}` 等花括号不泄漏）；同输入重复组装逐字节一致 | 单值 |
| `decision_fix_prompt` 组装 | 正向 | 含驳回原因原文 + 三动作重申 + 白名单一行（`a / b` 形式） | 单值 |
| `decision_fix_prompt` 边界 | 边界 | `error` 空串 / 500 字符 / 含换行；`allowed` 空（白名单行空占位）→ 组装不 panic 且段结构稳定 | 参数化（4 形态） |
| `decision_fix_prompt` 协议一致 | 异常 | 三动作清单与红线句与 `decision_prompt` 同源，不含退役 `retry` 动作 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 纯字符串组装函数，入参为结构体值，零注入 | 全部用例 |

---

## 不可测试项

无。`design.md`「新增文件」为空，「修改文件」八个实现文件均在既有测试布局中有对应兄弟测试模块（`*_test.rs`），测试路径可解析；`packages/desktop/src/views/changes/flow/graph.test.ts`（proposal「测试文件」表的 `retry` fixture 改写项）按 colocated `*.test.ts` 布局解析为 `graph.ts -> graph.test.ts` 映射对，已入册并给出投影锚用例；`packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` 虽可解析测试路径，但改动为纯 doc 注释面（词面冻结），已在上文以章节框架标注「无用例」，不计入不可测项。
