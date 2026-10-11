# 测试设计: merge-archive-steps-into-flow

> **日期**: 2026-10
> **依据**: proposal.md（AC-1–AC-10）+ design.md（D1–D7、数据模型、变更清单、公共函数 / API、类型定义）

---

## 测试框架与路径约定（框架识别结论）

- **Rust（src-tauri 各 crate + shell 命令层）**：cargo 内建测试框架，测试文件与源文件同目录同名的 `*_test.rs` 模块（既有惯例：`write/phase_table_test.rs`、`walker_test.rs`、`control_test.rs`、`steps_test.rs` 等）；跨 crate 语料集成测试驻 `crates/core/workflow/tests/`（`corpus_golden_test.rs`）。断言面为标准 `assert_eq!` / `assert!` / `assert_ne!` / `matches!` + `serde_json` 线面比对；db 边界一律 tempfile 真件库真实组合（沿 `walker_test.rs` `TestDb` 先例）。
- **前端（packages/desktop/src）**：vitest（`vite-plus/test`）+ @testing-library/react，测试文件与源文件同目录 `*.test.ts(x)`；`describe` / `it` / `expect` 自 `vite-plus/test` 导入；`data-testid` 为唯一查询挂钩；进程边界 mock 统一走 `vi.mock('@tauri-apps/api/core')`（invoke 分发 + 可编程 Channel，既有装置沿用）。
- **Mock 纪律**（沿仓库既有）：Rust 侧零框架级 mock——db 边界一律 tempfile 真件库，进程内假件仅限「作为被测 API 显式入参 / 注入依赖」的 trait 假件（`WorkerAgentPort` / `ToolStepPort` / `ArchiveVcsPort` / `WorkflowSnapshotPort` / `RunHistoryPort` / `ChangeStateStore` / `StaticCheckRunner` / `TestExecutionRunner` 假件为被测入参例外）；前端仅 mock 进程边界（invoke / Channel），内部纯函数模块（layout / run-state / types 等）真实组合。
- 以下「被测文件或模块」与「单元测试」标题路径：Rust 前缀 `packages/desktop/src-tauri/`，前端前缀 `packages/desktop/`（沿 design.md 变更清单口径）。

---

## 验收范围

> 表中「被测文件或模块」列填源文件（Rust 相对 `packages/desktop/src-tauri/`、前端相对 `packages/desktop/`），对应测试文件见「单元测试」各节 `->` 标题。再导出 / 生成物 / 配置 / 无独立测试路径的类型面不在本表（见「不可测试项」）。归档链退役文件（`archive_flow.rs` / `commands/archive_flow/` / `archive-panel.tsx` / `archive-state.ts` / `use-archive-flow.ts`）为删除文件，不纳入被测范围，其「已删除」由 AC-9 / AC-10 的 diff 面断言承载。

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 第 10 相位与双侧同步：`REQUIREMENT_TABLE` 含 archive `PhaseDefinition`（`executor=None` / `evaluator=None`）；`PIPELINE_PHASES` 后端 `detail.rs` 与前端 `layout.ts` 均 10 列且含 `archive`；archive 列进 `pipeline` 投影 | `crates/core/workflow/src/write/phase_table.rs`、`crates/core/workflow/src/queries/detail.rs`、`src/views/changes/flow/layout.ts` |
| AC-2 | 10 步序列与词汇：archive 相位 steps = PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Seal → Finalize → PhaseLog；`StepNode` / `RunStepKind` / `ChangeStepKind` 三枚举各含 8 个新词（preflight / commit / rebase / resolve_conflicts / merge / spec_sync / seal / finalize），`StepGroup` 含 Git | `crates/core/workflow/src/state.rs`、`crates/core/workflow/src/write/phase_table.rs`、`crates/core/orchestration/src/state.rs`、`crates/core/orchestration/src/port.rs`、`src/views/changes/flow/run-state.ts`、`src/views/changes/flow/run-step-node.tsx` |
| AC-3 | walker 直驱归档相位：假引擎 + 假 vcs port + 假写面驱动 archive 相位，10 步按序执行，git 步经 vcs 缝直调（core 零 spawn），Rebase 产出 `rebase_conflicts`、ResolveConflicts 无冲突 passed+detail、有冲突 agent 解算，Merge 独立 ff_merge，Seal 经写面 archive 双写收口 | `crates/core/orchestration/src/walker.rs`、`crates/core/orchestration/src/steps.rs`、`crates/core/orchestration/src/port.rs`、`crates/core/orchestration/src/prompt.rs` |
| AC-4 | 空 checklist 落账：archive 相位末尾 PhaseLog 以 `report=""` + `checklist=[]` 落 `verdict=pass` 的 PhaseRecord；`derive_verdict([])=Pass` 识别；`phase_next` 随后路由 done；零 `if phase == archive` 特判 | `crates/core/orchestration/src/walker.rs`、`crates/core/workflow/src/write/phase_next.rs`、`crates/core/workflow/src/queries/detail.rs` |
| AC-5 | evaluator 检查移除：drive 主循环无 `next.evaluator.is_none()` 检查；声明 Evaluator/VerdictGate 步而 `evaluator=None` 的漂移仍被 `step_evaluator` 拦截；test-execution dummy evaluator 清理不影响确定性承接 | `crates/core/orchestration/src/walker.rs`、`crates/core/orchestration/src/steps.rs` |
| AC-6 | 持久化零新表：archive 相位一次 run 收口后查询，归档 8 个新步词节点 + PhaseStart / PhaseLog 全落 `RunNodeRecord`（身份键 `(phase, attempt, step, instance_no)` 稳定唯一），串行 forward 边落 `RunEdgeRecord`；refresh 后归档列全史常驻 | `crates/core/orchestration/src/walker.rs`、`crates/core/orchestration/src/control.rs`、`crates/core/workflow/src/queries/detail.rs`、`crates/core/workflow/src/state.rs` |
| AC-7 | 手动执行语义：run 走完 8 个路由相位后停在 archive 前（stop-before-archive），未确认不进 archive 相位；用户确认后 archive 相位执行、run 完成（archive 完成才 Completed）；auto 模式同样停在 archive 前 | `crates/core/orchestration/src/walker.rs`、`crates/core/orchestration/src/control.rs`、`src/commands/change_flow/mod.rs`、`src/views/changes/flow/run-control-panel.tsx` |
| AC-8 | 失败停链语义：archive 任一步失败 → `Terminal` → run 直接 `Failed` 收口，不唤决策 agent、不触发 phase retry；重试靠 step 内判断短路（干净跳过 / 已合入跳过 / 已落盘跳过 / 双写续半边） | `crates/core/orchestration/src/walker.rs`、`crates/core/workflow/src/write/phase_table.rs` |
| AC-9 | 前端合入流程图：`ArchivePanel` / `StageList` / `useArchiveFlow` / `archive-state.ts` 删除；归档 10 步节点上图（git 分类可辨），确认面对话由停等面 + Preflight step detail 承载；`layout.ts` 10 列、`run-state.ts` 加 git 分类 | `src/views/changes/flow/layout.ts`、`src/views/changes/flow/run-state.ts`、`src/views/changes/flow/run-step-node.tsx`、`src/views/changes/flow/run-control-panel.tsx`、`src/views/changes/change-detail-view.tsx` |
| AC-10 | 命令面收编与版本：`archive_flow` 五命令 + `ArchiveControl` + `ArchiveSink` 删除，归档发起 / 停止经 `change_flow` 命令面；`packages/desktop` version 0.4.37；`plugins/dev-team` 2.10.44 零改动；`vp test` / `client:check` / knip / `bindings:check` 全绿 | `src/commands/change_flow/mod.rs`、`src/commands/changes/mod.rs` |

> 交叉承载注记：AC-3 的「core 零 spawn」与 AC-5 的「`evaluator.is_none()` 检查删除」为 diff 面断言，行为面由 `walker_test.rs`（假 vcs 缝直调、无 spawn 分支、假写面驱动）覆盖，diff 面见「不可测试项」；AC-4 的空 checklist PhaseRecord 由 `walker_test.rs`（落账载荷）与 `phase_next_test.rs`（路由 done）+ `detail_test.rs`（archive 列 eval 空表投影）交叉承载；AC-6 的节点 / 边持久化由 `walker_test.rs`（真件 store 收口后查询）与 `detail_test.rs`（十八值投影）交叉承载。AC-2 的 `types.ts`（类型面）、AC-10 的 `commands/mod.rs` / `main.rs` / `lib.rs` / `bindings.ts` / `package.json`（命令登记 / 再导出 / 生成物 / 版本配置面）无独立测试路径，不在上表「被测文件或模块」列，承载见「不可测试项」。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

#### 待测功能

- StepNode: 十值 → 十八值（+`Preflight` / `Commit` / `Rebase` / `ResolveConflicts` / `Merge` / `SpecSync` / `Seal` / `Finalize`，snake_case 词）；`as_str()` / `group()` / `role()` 派生随动
- RunStepKind: 十值 → 十八值（同词面，snake_case）
- StepGroup: 三分类 → 四分类（+`Git`）；`group()` 派生 commit/rebase/merge/finalize→Git、preflight/seal→Exec、spec_sync/resolve_conflicts→Agent
- AgentRole: 三值 → 五值（+`SpecSync` / `ResolveConflicts`）；`role()` 派生 spec_sync→SpecSync、resolve_conflicts→ResolveConflicts

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| StepNode 十八值全词汇 | 正向 | `StepNode` 十八值 snake_case 逐字出线（十既有词 + preflight / commit / rebase / resolve_conflicts / merge / spec_sync / seal / finalize）；`as_str()` 与 `RunStepKind::as_str()` 逐字同源（AC-2） | 新增 |
| StepNode 分组派生 | 正向 | `group()` 四分类：commit/rebase/merge/finalize→`StepGroup::Git`、preflight/seal→`StepGroup::Exec`、spec_sync/resolve_conflicts→`StepGroup::Agent`、executor/evaluator/decision→Agent、三门 gate→Gate（AC-2） | 新增 |
| StepNode 角色派生 | 正向 | `role()`：spec_sync→`Some(AgentRole::SpecSync)`、resolve_conflicts→`Some(AgentRole::ResolveConflicts)`、其余非 agent 步→`None`（AC-2 / D4） | 新增 |
| StepNode 线面 | 边界 | 8 新词序列化往返（snake_case）与反序列化逐词可辨；`Preflight` / `Commit` / `Rebase` / `ResolveConflicts` / `Merge` / `SpecSync` / `Seal` / `Finalize` 八变体穷举（AC-2） | 新增 |
| StepNode 线面 | 异常 | 非法词（`"git"` / `"archive"` / `"spec-sync"` / `"resolveConflicts"`）反序列化拒绝——封闭集防线零触点持衡（AC-2） | 新增 |
| RunStepKind 十八值全词汇 | 正向 | `RunStepKind` 十八值 snake_case 逐字出线，与 `ChangeStepKind::as_str()` 同词面 1:1 全映射（AC-2 / AC-6 落库零过滤前置） | 新增 |
| RunStepKind 线面 | 异常 | 十值时代旧表不残留、非法词反序列化拒绝；`StepGroup` 新增 `Git` 词面 lowercase 出线（AC-2） | 新增 |
| AgentRole 五值 | 正向 | `AgentRole` 五值 lowercase 逐字出线（executor / evaluator / decision / spec_sync / resolve_conflicts），与 `WorkerRole::as_str()` 同源（AC-2 / D3） | 新增 |
| StepNode 分组全量穷举 | 边界 | 十八值逐节点 `group()` / `role()` 与 design 数据模型表一致（git 步 role=None、agent 步 group=Agent、内联步 group=Exec）（AC-2 / D4） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯类型 serde 直测 + 纯内存派生访问器，零依赖零 mock | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能

- phase_table(): 返回项 `REQUIREMENT_TABLE` 追加第 9 个路由相位 `archive`（`PhaseDefinition { id: "archive", steps: ARCHIVE_STEPS, artifact_manifest: &[], executor: None, evaluator: None }`）；`ARCHIVE_STEPS` 10 步序列声明（PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Seal → Finalize → PhaseLog，`prompt_ref` 恒 None、`on_fail` 恒 None）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| archive 相位声明 | 正向 | `phase_table("requirement")` 返回 9 路由相位，末位 `archive`：`executor=None` / `evaluator=None`、`artifact_manifest` 空数组（AC-1） | 新增 |
| archive 相位声明 | 正向 | archive `steps` 10 步序列与 design 数据模型表一致：PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Seal → Finalize → PhaseLog（AC-2） | 新增 |
| archive 步 on_fail 语义 | 正向 | archive 10 步 `on_fail` 恒 `None`（步失败即 Terminal，不唤决策、不进相位重试；AC-8） | 新增 |
| archive 步 prompt_ref 语义 | 边界 | archive 10 步 `prompt_ref` 恒 `None`（spec_sync / resolve_conflicts 的 prompt 由 orchestration 运行时组装——workflow 零 orchestration 依赖；AC-2 / 数据模型） | 新增 |
| phase_table 非 requirement | 异常 | `phase_table("bug-fix")` / `phase_table("test-only")` / `phase_table("")` → `None`（V1 仅 requirement 语义不变；AC-1 前置） | 回归 |
| archive 相位步分组可派生 | 正向 | archive 步经 `StepNode::group()` 分组：commit/rebase/merge/finalize→Git、preflight/seal→Exec、spec_sync/resolve_conflicts→Agent、phase_start/phase_log→Exec（AC-2 / AC-3 分派前置） | 新增 |
| ARCHIVE_STEPS 类型收敛 | 异常 | archive 相位步骤均自 `StepNode` 十八值封闭集取值（不出现非法词 / 不引用退役 `ArchiveStage` 词汇）；`ARCHIVE_STEPS` 长度恰 10（AC-2） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯 const 表直测，零依赖零 mock；`phase_table()` 只读返回 `&'static` 相位表 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs

#### 待测功能

- phase_next(): 路由经 `REQUIREMENT_TABLE` 自然覆盖 archive——8 路由相位 pass 后 `next=archive`、archive 空 checklist 落账（`verdict=pass`）后 `all_passed` → `done`；`has_phase_passed` / `allowed_backtrack_phases` 对 archive 零特判

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| archive 路由 | 正向 | 前 8 路由相位（proposal…acceptance）全部 `verdict=pass` 落账后 → `phase_next` 返回 `next_phase=Some("archive")`（AC-1 / AC-4） | 新增 |
| archive 路由 | 正向 | archive 以 `report=""` + 空 checklist 落 `verdict=pass` 后 → `phase_next` 返回 `done`（`all_passed` 真；AC-4 / AC-7） | 新增 |
| archive 路由 | 边界 | archive 未落账（或落 `verdict=fail`）→ `next_phase` 仍指 archive，未 done（archive 完成才 Completed；AC-7） | 新增 |
| 空 checklist 识别 | 正向 | `derive_verdict([])=Pass`——archive 空 checklist 落账被识别为 pass（AC-4） | 新增 |
| 零特判 | 边界 | `has_phase_passed("archive")` 与其余相位同式（读 archive 列 PhaseRecord 最新 pass 条目），无 `if phase == "archive"` 分支语义差异（AC-4） | 新增 |
| 回溯白名单 | 边界 | `allowed_backtrack_phases` 对 archive 仅前向（archive 为末相位，不在任何相位前置集内）；archive 不在 `dependents` / stale 传播闭包（AC-8 / 零改动锚） | 回归 |
| store Err | 异常 | store 读半返回 Err / 未建档 change → `Err(String)` 透传（既有语义不改写；AC-4） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（入参例外） | 既有进程内假件（可编程 read + Err 注入），作为被测函数显式入参 | 「store Err 透传」「路由回归」 |
| 无（真实组合） | 其余纯函数直测（`phase_table` 真件只读供给） | 「archive 路由」「空 checklist 识别」 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

#### 待测功能

- change_detail(): `PIPELINE_PHASES` 9 → 10 列（追加 `"archive"`）；`pipeline` 含 archive 列；`runs[].nodes` 十八值全词汇投影（零过滤）；golden 显式重写

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 10 列 pipeline | 正向 | `PIPELINE_PHASES` 10 列且含 `"archive"`（第 10 列、`code-analyze` 之后）；`change_detail` 的 `pipeline` 恒出 10 站（无 attempt 的 archive 站为空序列）（AC-1 / D5） | 新增 |
| archive 列投影 | 正向 | archive 空 checklist 落账后 → `pipeline` archive 站含一条 `verdict=pass` 的 AttemptRecord（report 空、checklist 空表、三槽位 null）（AC-4 / D6） | 新增 |
| nodes 十八值投影 | 正向 | 一次覆盖 archive 8 新步词 + PhaseStart / PhaseLog 的 run → `runs[].nodes` 十八值全词汇在案（preflight / commit / rebase / resolve_conflicts / merge / spec_sync / seal / finalize 逐词出线），零过滤（AC-6） | 新增 |
| nodes 十八值投影 | 正向 | 归档节点 `session_id` 仅 SpecSync / ResolveConflicts 在案、其余 None；`detail` 携跳过因（干净 / 已合入 / 无 delta / 用户选择 / 已落盘）（AC-6 / 数据模型） | 新增 |
| 边投影 | 正向 | 归档串行推进边全为 `Forward`（零 feedback / retry / backtrack）；端点指向节点身份串（AC-6） | 新增 |
| nodes/edges 边界 | 边界 | 零 run change → `runs` 空数组；archive 站零 attempt → 空序列（miss 非错误） | 回归 |
| nodes/edges 异常 | 异常 | store `list_run_nodes` / `list_run_edges` 返回 Err → 该 run nodes/edges 降级空数组、详情其余面不受阻断（沿 `list_phase_records` unwrap_or_default 同口径） | 回归 |
| golden 集成投影 | 正向 | 语料含 archive 列、8 归档步词节点、git 步分类、空 checklist PhaseRecord 样本 → golden 与库内记录逐字一致（`DESKTOP_GOLDEN_REWRITE=1` 显式重写后人工确认留痕；AC-6 / AC-10） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（入参例外） | 既有 DetailStore 假件扩展 run 半边：`list_run_nodes` / `list_run_edges` 可编程序列（含 Err 可编程），写半边 unimplemented panic（纯读面越权即暴露） | 本节全部用例 |
| 磁盘目录树（进程边界） | 既有 tempdir 真实目录（产物文件 + 定位面） | 「文档形态恒空 runs」 |
| 无（真实组合） | golden 集成投影经真件 store run 域操作面种子（语料构造器），真实 detail 读链 + serde_json 线面比对 | 「golden 集成投影」 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

#### 待测功能

- ChangeStepKind: 十值 → 十八值（+8 snake_case 词：preflight / commit / rebase / resolve_conflicts / merge / spec_sync / seal / finalize；`as_str()` 随动）
- ArchivePreflight: 新增 `{ name, completed, incomplete_phases, missing_artifacts, delta_specs, worktree, branch, merge_target }`（serde + specta）
- RunUpdate::ConfirmWait: `{ phase }` → `{ phase: String, archive: Option<ArchivePreflight> }`
- ChangeRunSnapshot: 增 `archive: Option<ArchivePreflight>`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ChangeStepKind 十八值 | 正向 | 十八值 camelCase 逐字出线（十既有词 + preflight / commit / rebase / resolveConflicts / merge / specSync / seal / finalize）；`as_str()` snake_case 与 `RunStepKind::as_str()` 同词面（AC-2） | 新增 |
| ChangeStepKind 线面 | 异常 | 非法词反序列化拒绝；`resolveConflicts` / `specSync` 与 snake 词 `resolve_conflicts` / `spec_sync` 逐字对应（AC-2） | 新增 |
| ArchivePreflight 形状 | 正向 | `ArchivePreflight` 八字段（name / completed / incomplete_phases / missing_artifacts / delta_specs / worktree / branch / merge_target）serde + specta 序列化往返逐字段可辨（AC-7 / D2 停等载荷） | 新增 |
| ArchivePreflight 边界 | 边界 | `completed` bool 三态（true / false 缺省）；`incomplete_phases` / `missing_artifacts` / `delta_specs` 空 Vec / 单元素 / 超长列表；`worktree` / `branch` / `merge_target` None 与 Some 两态（AC-7） | 新增 |
| ArchivePreflight 异常 | 异常 | 缺失必填字段（name / completed）反序列化拒绝；多余字段忽略或拒绝（AC-7 线面契约守卫） | 新增 |
| RunUpdate::ConfirmWait 换形 | 正向 | `ConfirmWait { phase, archive }`：archive=None（普通相位停等）与 archive=Some(preflight)（archive 前停等）两态；`kind()` 仍 `"confirmWait"`、`RunNotice::ConfirmWait` 零载荷投影不变（AC-7 / D2） | 新增 |
| RunUpdate::ConfirmWait 异常 | 异常 | 旧 `ConfirmWait { phase }` 载荷反序列化拒绝或缺省 archive=None（旧字段不复现）；`RunNotice` 六变体不变（零新 kind，停等载荷不出进程）（AC-7 / D2） | 新增 |
| ChangeRunSnapshot 出线 | 正向 | `ChangeRunSnapshot` 增 `archive: Option<ArchivePreflight>`；snapshot 投影沿 `ActiveRunView.archive` 出线（AC-7 / AC-10） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯类型 serde 直测 + 纯内存等值面，零依赖零 mock | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

#### 待测功能

- WorkerRole: 三值 → 五值（+`SpecSync` / `ResolveConflicts`，`as_str()` = `"spec_sync"` / `"resolve_conflicts"`）
- ToolCommand: 增 `ArchivePreflight { change_id }` / `ArchiveSeal { change_id }`
- ToolStepOutput: 增 `ArchivePreflight(ArchivePreflight)` / `ArchiveSeal(workflow::write::ArchiveOutcome)`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| WorkerRole 五值 | 正向 | `WorkerRole` 五值 `as_str()` 逐字出线（executor / evaluator / decision / spec_sync / resolve_conflicts）；与 `AgentRole::as_str()` 同源（AC-2 / D3） | 新增 |
| WorkerRole provenance | 正向 | provenance `source_ref` 组装含 spec_sync / resolve_conflicts 两词互异（同 phase 同 attempt 内唯一，无 `executor` 撞号；D3 / R4） | 新增 |
| ToolCommand 两新变体 | 正向 | `ToolCommand::ArchivePreflight { change_id }` / `ArchiveSeal { change_id }` 构造 + PartialEq 可辨（change_id 载荷归键；AC-3） | 新增 |
| ToolStepOutput 两新变体 | 正向 | `ToolStepOutput::ArchivePreflight(ArchivePreflight)` / `ArchiveSeal(ArchiveOutcome)` 封闭集可构造、载荷可透传（AC-3） | 新增 |
| ToolStepOutput 异常 | 异常 | 两新变体反序列化缺字段 / 载荷类型不匹配 → 拒绝（线面契约守卫；AC-3） | 新增 |
| ArchiveVcsPort 面 | 边界 | `ArchiveVcsPort` trait 面零改动（rebase_branch / ff_merge / commit_all / commit_paths / current_branch / branch_merged / dirty / worktree_snapshot / rebase_continue / rebase_abort 已齐备），作为 walker 第三缝显式入参编译面在案（AC-3 / R2） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯类型 serde 直测 + trait 编译面断言；`ArchiveVcsPort` / `WorkerAgentPort` / `ToolStepPort` 假件为被测入参 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs

#### 待测功能

- spec_sync_prompt(change: &str) -> String: spec 同步 prompt 单点（自 archive_flow.rs 搬移，文本零改动）
- merge_conflict_prompt(change: &str, conflicts: &[String]) -> String: 解冲突 prompt 单点（自 archive_flow.rs 搬移，文本零改动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| spec_sync_prompt 组装 | 正向 | 输入 change 名 → prompt 含 change 名插值 + spec 同步指令主体（AC-3） | 新增 |
| spec_sync_prompt 边界 | 边界 | change 空串 / 超长串（>1000 chars）/ 含特殊字符（`\n` / `\0` / emoji）→ 不 panic，插值原样保留（AC-3） | 新增 |
| spec_sync_prompt 静态红线 | 异常 | prompt 零 MCP / workflow.json / CLI skill 溯源、零 `<role>` 占位（零占位红线不翻案）（AC-3 / 引用沿用） | 新增 |
| merge_conflict_prompt 组装 | 正向 | 输入 change + conflicts 非空列表 → prompt 含 change 名与逐条冲突路径插值（AC-3） | 新增 |
| merge_conflict_prompt 边界 | 边界 | conflicts 空列表 / 单元素 / 超大列表；单条冲突路径超长 / 含特殊字符 → 不 panic、逐条可辨（AC-3） | 新增 |
| 搬移保真 | 回归 | 两函数文本与 `archive_flow.rs` 搬移前逐字节一致（零 CLI skill 溯源、零 MCP 依赖；AC-3 / 零改动沿用） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯字符串组装直测（静态模板 + 运行时插值），零依赖零 mock | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

#### 待测功能

- LocalToolSteps::new(anchors, static_check, test_execution, vcs, store, run_id) -> Self: 增 `vcs: Arc<dyn ArchiveVcsPort>` 注入
- run_archive_preflight(main_root: &Path, store: &dyn ChangeStateStore, id: &str, vcs: &dyn ArchiveVcsPort) -> Option<ArchivePreflight>: 前置只读载荷单点（completion_of / missing_artifacts / detect_delta_specs + vcs.current_branch / branch_merged）
- `execute` 增 `ToolCommand::ArchivePreflight` / `ToolCommand::ArchiveSeal` 两分支（不写 `StepRecord` 审计行）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ArchivePreflight 分支 | 正向 | `ToolCommand::ArchivePreflight { change_id }` → `ToolStepOutput::ArchivePreflight(ArchivePreflight)`：完成度（completed / incomplete_phases）自 PhaseRecord、delta specs 自磁盘目录扫描、branch / merge_target 自 vcs.current_branch / branch_merged（AC-3） | 新增 |
| ArchivePreflight 分支 | 边界 | 未建档 change → 返回 None 或显式 Err（不臆测）；`specs/` 目录缺席 → delta_specs 空（AC-3） | 新增 |
| ArchivePreflight 分支 | 异常 | vcs.current_branch 返回 Err（detached HEAD 空输出）→ merge_target None 或显式 Err（不 panic；AC-3） | 新增 |
| ArchiveSeal 分支 | 正向 | `ToolCommand::ArchiveSeal { change_id }` → `ToolStepOutput::ArchiveSeal(ArchiveOutcome)`：直调 `workflow::write::archive` 双写（AC-3） | 新增 |
| ArchiveSeal 分支 | 异常 | 双写写面 Err（目录改名 / status 翻转失败）→ `Err(String)` 透传（AC-8 失败停链前置） | 新增 |
| 审计行零触点 | 正向 | ArchivePreflight / ArchiveSeal 两分支不写 `StepRecord` 审计行（归档步为执行图节点，非相位机步骤；断言 append_step 未被调用）（AC-3 / 数据模型） | 新增 |
| LocalToolSteps::new vcs 注入 | 正向 | `new` 增 vcs 参数后构造并 `ToolStepPort::run` 可驱动 ArchivePreflight / ArchiveSeal 分支（AC-3） | 新增 |
| 既有四命令回归 | 回归 | PhaseNext / PhaseStart / PhaseLog / Backtrack / DecisionLog / StaticCheck / TestExecution 分支语义不变（AC-3 / 零改动沿用） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（入参例外） | 既有进程内假件（可编程 read + append_step 计数 + Err 注入） | 「未建档」「写面 Err」「审计行零触点」 |
| ArchiveVcsPort（入参例外） | 假 vcs（current_branch / branch_merged 可编程返回值 + Err 注入） | 「ArchivePreflight 分支」 |
| StaticCheckRunner / TestExecutionRunner（入参例外） | 既有假 runner（可编程产出） | 「既有四命令回归」 |
| 磁盘目录树（进程边界） | tempdir 真实目录（specs/ 文件 + change 目录） | 「delta_specs 扫描」「Seal 双写定位」 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/control.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs

#### 待测功能

- ChangeFlowControl::confirm() / RunGuard::wait_confirm(): `confirm` 增 `sync_specs: bool`（oneshot 载荷 `(bool, bool)`）；`wait_confirm` 返回 `(proceed, sync_specs)`
- ChangeFlowControl::publish(): `ConfirmWait` 分支落 `entry.archive`
- RunEntry: 增 `archive: Option<ArchivePreflight>`；`snapshot()` 出 `archive`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| confirm sync_specs 回流 | 正向 | `confirm(root, id, proceed, sync_specs)` → `wait_confirm()` 返回 `(proceed, sync_specs)` 二元组（四组合 `(true/false, true/false)` 逐组合回流；AC-7 / D2） | 新增 |
| confirm sync_specs 异常 | 异常 | 无运行 run / 无等待方 → `confirm` 返回 `Err`；取消信号置位 → `wait_confirm` 以 `(false, false)` 收敛（停止不必先应答；AC-7） | 新增 |
| publish ConfirmWait.archive | 正向 | `publish` `ConfirmWait { phase, archive: Some(preflight) }` → `RunEntry.archive` 落值、status 置 WaitingConfirm；`archive: None` → 普通相位停等 archive 槽清空（AC-7 / D2） | 新增 |
| snapshot archive 出线 | 正向 | `snapshot()` 含 `archive: Some(preflight)` / `None` 两态，与 `publish` 落值一致；终态除名后 snapshot 恒 None（AC-7 / AC-10） | 新增 |
| 既有累积回归 | 回归 | 节点 / 边累积、`begin_instance` 分配、ask 应答、终态收口语义不变（AC-6 / AC-7 零改动锚） | 回归 |
| 广播零载荷 | 边界 | `ConfirmWait` 携带 archive 载荷但 `RunNotice::ConfirmWait` 仍 kind-only 零载荷（停等载荷不出进程，客户端经 `get_change_detail` 重查；AC-7 / D2） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `#[tokio::test]` 进程内 tokio 原语（broadcast / oneshot / watch / Mutex）真实组合；多任务挂起/唤醒以 `tokio::spawn` + `yield_now` 轮询握手 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

- new_run_id(): 铸造 `run-<millis>`（不变）
- walk_run(): `RunRequest` +`main_root` +`stop_before`；`walk_run` +`vcs: Arc<dyn ArchiveVcsPort>`；`drive` 删除 `next.evaluator.is_none()` 检查、增循环顶部停止门、收口后确认块跳过 `phase == stop_before`；`StepContext` +`vcs`；`PhaseCursor` +`preflight` / `rebase_conflicts` / `sync_specs` / `seal_outcome`；`run_step` 增 8 归档步分支；`step_phase_log` 增空 checklist 第三分支；收口语义改写（archive 完成才 Completed）
- 组合链路（挂靠本节，链路发起方 = walk_run）: walk_run → 三执行缝（exec / agent / git）→ 真件 store 全链落库

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 归档相位 10 步执行序 | 正向 | 假引擎 + 假 vcs + 假写面驱动 archive 相位：PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Seal → Finalize → PhaseLog 按序执行，时间线逐步在案（AC-3） | 新增 |
| git 步走 vcs 缝 | 正向 | Commit / Rebase / Merge / Finalize 四步经 `vcs` 缝直调（假 vcs 捕获 commit_all / rebase_branch / ff_merge / commit_paths 调用；core 零 spawn 断言——无 `std::process` 触点）（AC-3 / R2） | 新增 |
| Rebase → ResolveConflicts → Merge 三段 | 正向 | Rebase 产出 `rebase_conflicts` 入 `PhaseCursor`；ResolveConflicts 无冲突 → `passed` + detail（跳过因）；有冲突 → agent 解算（`rebase_continue` + 后验）；Merge 独立 `ff_merge`（AC-3 / D4） | 新增 |
| ResolveConflicts 条件执行 | 边界 | `rebase_conflicts` 为空 → ResolveConflicts `passed` + detail 携「无冲突」，不唤 agent；非空 → 唤解冲突 agent + 后验（AC-3） | 新增 |
| SpecSync 条件执行 | 边界 | delta specs 在场且用户未跳过 → agent 会话（`WorkerRole::SpecSync`、provenance `<id>/archive/spec_sync/<attempt>`）；无 delta / 用户跳过 → `passed` + detail（AC-3 / D2 / D3） | 新增 |
| Commit 步内短路 | 边界 | 干净 worktree（`dirty=false`）→ Commit `passed` + detail 携「干净」跳过，不调 `commit_all`（AC-8 幂等重试） | 新增 |
| Finalize 步内短路 | 边界 | 已落盘（`branch_merged=true`）→ Finalize `passed` + detail 携「已落盘」跳过，不调 `commit_paths`（AC-8） | 新增 |
| Seal 双写收口 | 正向 | Seal 经 `workflow::write::archive` 双写（假写面捕获 ArchiveOutcome）；`seal_outcome` 入 cursor 供 Finalize 派生归档目录路径（AC-3） | 新增 |
| 空 checklist 落账 | 正向 | 归档末尾 PhaseLog 构造 `PhaseLogInput { phase:"archive", report:"", checklist:vec![], skipped:false, 三槽位 None }` 直调 `run_phase_log`，落 `verdict=pass` PhaseRecord；零 `if phase == "archive"` 特判（AC-4） | 新增 |
| stop-before-archive 停等 | 正向 | 前 8 路由相位 pass 后 `phase_next` 出 `next=archive`，`stop_before=Some("archive")` → 发 `ConfirmWait { phase:"archive", archive: Some(preflight) }` 并 `wait_confirm`；`proceed=false` → stopped、`proceed=true` → 进 archive（AC-7） | 新增 |
| stop-before-archive 载荷 | 正向 | 停止门在发 ConfirmWait 前经 `ToolCommand::ArchivePreflight` 算前置只读载荷随行；`sync_specs` 回流进 `PhaseCursor`（AC-7 / D2） | 新增 |
| auto 模式同样停等 | 边界 | `auto_next_phase=true` 时 archive 前仍停等一次（停止门不因 auto 跳过）；archive 收口后不再补确认（直接 done）（AC-7 / D1） | 新增 |
| evaluator 检查移除 | 正向 | drive 无 `next.evaluator.is_none()` 硬检查（archive 相位 `evaluator=None` 仍可进入）；声明 Evaluator/VerdictGate 步而 `evaluator=None` 的漂移仍被 `step_evaluator` 拦截（AC-5） | 新增 |
| 失败停链 | 异常 | archive 任一步失败（commit / rebase / seal 半完成 / spec_sync / 冲突无法裁决）→ `Terminal` → run 直接 `Failed` 收口，不唤决策、不触发 phase retry（AC-8） | 新增 |
| 收口语义改写 | 正向 | archive 完成（空 checklist 落账）后 `phase_next` 出 done → run `Completed`；archive 未完成 → 不 Completed（AC-7） | 新增 |
| 幂等重试短路 | 边界 | 半完成续半边：干净跳过 / 已合入跳过 / 已落盘跳过 / 双写续半边（Seal 幂等覆写）各短路形态（AC-8） | 新增 |
| new_run_id 回归 | 回归 | `new_run_id()` 返回 `run-<millis>` 前缀、同进程单调递增（既有语义） | 回归 |
| 组合链路落库 | 正向 | 假引擎 + 假 vcs + 真件 store 驱动 archive 相位收口后查询：8 归档步词节点 + PhaseStart / PhaseLog 全落 `RunNodeRecord`（身份键稳定唯一），forward 边落 `RunEdgeRecord`（AC-6） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| WorkerAgentPort（入参例外） | 既有假引擎（预录产出 + WorkerTurnRequest 全量捕获 + 时间线登记；角色感知含 SpecSync / ResolveConflicts） | 「10 步执行序」「ResolveConflicts / SpecSync 会话」 |
| ToolStepPort / StaticCheckRunner / TestExecutionRunner（入参例外） | 既有假 tool port / 假 runner（可编程产出，捕获 ArchivePreflight / ArchiveSeal 命令） | 「Seal 双写」「ArchivePreflight 载荷」 |
| ArchiveVcsPort（入参例外） | 假 vcs（commit_all / rebase_branch / ff_merge / commit_paths / current_branch / branch_merged / dirty 可编程返回值 + 调用捕获 + Err 注入） | 「git 步走 vcs 缝」「短路」「失败停链」 |
| RunHistoryPort（入参例外） | 既有假 history（两写计数 + 命令捕获）或真件 StoreRunHistory | 「两写时序」「组合链路落库」 |
| ChangeStateStore（真件） | tempfile 真件库（`Store::open_workspace`，`walker_test.rs` `TestDb` 先例） | 「组合链路落库」「空 checklist 落账」「phase_next 路由」 |
| ChangeFlowControl / RunGuard | 真件进程内注册表（tokio 原语真实组合；命令层以 Arc 托管） | 「stop-before-archive 停等」「confirm 回流」 |
| 磁盘目录树（进程边界） | tempdir 真实目录（change 目录 + specs/ + worktree 定位面） | 「Preflight 探测」「Seal 双写定位」 |

---

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

#### 待测功能

- change_flow_start(): 摘 `ArchiveControl` 反向互斥面；装配 `vcs: Arc<dyn ArchiveVcsPort>`（`ProcessArchiveVcs`）+ `main_root` + `stop_before=Some("archive")` 注入 `LocalToolSteps` 与 `walk_run`
- change_flow_confirm(): 增 `sync_specs: bool` 并透传 `control.confirm`
- change_flow_stop / change_flow_answer / change_flow_watch(): 请求-应答语义不变（回归锚）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| change_flow_start 装配 | 正向 | `change_flow_start_with` 注入 vcs + main_root + stop_before，构造 RunRequest 携 `stop_before=Some("archive")` 与 vcs 缝（假 vcs / 假 walk_run 捕获面断言）（AC-10） | 新增 |
| change_flow_start 前置校验 | 异常 | blank root / id、未建档 change、非 requirement workflow_type、worktree 目录不存在 → 显式 `Err`（AC-10 / 既有语义） | 回归 |
| ArchiveControl 互斥面摘除 | 正向 | `change_flow_start` 不再读取 `ArchiveControl` state（归档链并入 run 后无反互斥检查；AC-10） | 新增 |
| change_flow_confirm sync_specs | 正向 | `change_flow_confirm(app, root, id, proceed, sync_specs)` 透传 `control.confirm(proceed, sync_specs)`（假 control 捕获二元组；AC-7 / D2） | 新增 |
| change_flow_confirm 校验 | 异常 | blank root / id → `Err`；无运行 run / 无等待方 → `Err` 透传（AC-7 / 既有语义） | 回归 |
| stop / answer / watch 回归 | 回归 | `change_flow_stop`（停 run 自然停 archive 相位）/ `change_flow_answer` / `change_flow_watch` 请求-应答语义不变（AC-10 零改动锚） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri Runtime（进程边界） | 既有 `MockRuntime` 句柄（沿 `agent_start_with` / `change_flow_start_with` 先例）注入测试 | 本节全部用例 |
| ChangeFlowControl / ChangeStateStore / ArchiveVcsPort / ToolStepPort（入参/state 例外） | 真件 store + 假 vcs / 假 tool port + 真件 control（或假 control 捕获面） | 「装配」「confirm 透传」 |
| 磁盘目录树（进程边界） | tempdir 真实目录（worktree / exec root） | 「前置校验」 |

---

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

#### 待测功能

- get_change_detail(): `ActiveRunView` 增 `archive: Option<ArchivePreflight>`（自快照投影）
- archive_change(): 裸双写薄命令保留零改动（回归锚）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ActiveRunView archive 出线 | 正向 | `get_change_detail` 活面投影 `ActiveRunView.archive` 自 `ChangeRunSnapshot.archive` 直出（Some / None 两态；AC-7 / AC-10） | 新增 |
| ActiveRunView 既有字段回归 | 回归 | `ActiveRunView` 既有字段（run_id / status / phase / attempt / ask / started_at / nodes / edges）投影不变（AC-10 零改动锚） | 回归 |
| get_change_detail 边界 | 边界 | 未知 id / blank root / 开库失败 → `None`；终态 run 除名 → `active_run=None`（archive 槽随之缺席）（AC-10 / 既有语义） | 回归 |
| archive_change 回归 | 回归 | 裸双写直调面零改动（Seal 步之外的既有逃生口，D7；AC-10 零改动锚） | 回归 |
| archive_change 写面异常 | 异常 | archive_change 双写写面 Err（目录改名 / status 翻转失败）→ `Err(String)` 透传不 panic，其余详情面不受阻断（AC-10 / D7 逃生口守卫） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri State / Runtime（进程边界） | 既有 `State<WorkspaceStores>` + `State<Arc<ChangeFlowControl>>` 真件（或测试注入 MockRuntime） | 本节全部用例 |
| ChangeStateStore（入参例外） | 真件 tempfile 库 | 「ActiveRunView archive 出线」「get_change_detail」 |

---

### packages/desktop/src/views/changes/flow/layout.ts -> packages/desktop/src/views/changes/flow/layout.test.ts

#### 待测功能

- PIPELINE_PHASES: 9 → 10 列（追加 `"archive"`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 10 列布局 | 正向 | `PIPELINE_PHASES` 长度为 10 且含 `"archive"`（第 10 列、`code-analyze` 之后）；与后端 `queries/detail.rs` 的 `PIPELINE_PHASES` 同序（AC-1 / D5） | 新增 |
| 10 列布局 | 边界 | `nodePosition` 列坐标对第 10 列（colIndex=9）产出合法 x 坐标；archive 列空列时节点列表不报错（AC-1） | 新增 |
| 10 列布局 | 异常 | 非法相位名（非 10 列）`runStepNodes` 不入图（pipeline 外相位节点丢弃，既有坑位校验；AC-1 / AC-9） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `PIPELINE_PHASES` / `nodePosition` 纯函数直测（内存 fixture，零 react / 零进程边界依赖） | 本节全部用例 |

---

### packages/desktop/src/views/changes/flow/run-state.ts -> packages/desktop/src/views/changes/flow/run-state.test.ts

#### 待测功能

- normalizeKind(): 加 8 词映射（snake 词 → camel 词，`resolve_conflicts`→`resolveConflicts` / `spec_sync`→`specSync`；单词步词 passthrough）
- stepWord(): 加 8 词反向映射（camel → snake，`resolveConflicts`→`resolve_conflicts` / `specSync`→`spec_sync`）
- groupOf(): 加 git 分类（commit/rebase/merge/finalize→`git`、specSync/resolveConflicts→`workerAgent`、preflight/seal→`toolStep`）
- runStepNodes(): 归档步词落到对应分支、节点按后端 `(phase, attempt, step, instanceNo)` 直渲染

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 8 词归一 | 正向 | `normalizeKind` / `stepWord` 8 新词双向归一（`resolve_conflicts`↔`resolveConflicts`、`spec_sync`↔`specSync`；preflight / commit / rebase / merge / seal / finalize 单词 passthrough）（AC-2） | 新增 |
| git 分类 | 正向 | `groupOf`：commit / rebase / merge / finalize → `git`；specSync / resolveConflicts → `workerAgent`；preflight / seal → `toolStep`；既有 executor/evaluator/decision → workerAgent、三门 gate → gate（AC-9） | 新增 |
| 归档节点渲染 | 正向 | 归档步节点（含 git 分类）经 `runStepNodes` 按 `(phase, attempt, step, instanceNo)` 直渲染为 `RuntimeFlowNode`，`group` / `runStepKind` / `instanceNo` 逐字段在案（AC-9） | 新增 |
| 归档节点渲染 | 边界 | 零节点 → 空节点列表；archive 列零步 → 空列不报错（miss 非错误） | 新增 |
| 归档节点渲染 | 异常 | 未知步词 / 缺 instanceNo → 不 panic，降级处理（不渲染该节点或默认分组；AC-9 稳健面） | 新增 |
| 既有归一回归 | 回归 | 既有 10 词归一 / `unifiedRunNodes` / `unifiedRunEdges` 双源合成语义不变（AC-9 零改动锚） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `normalizeKind` / `stepWord` / `groupOf` / `runStepNodes` 纯函数直测（内存构造 DTO fixture，零 react / 零进程边界依赖） | 本节全部用例 |

---

### packages/desktop/src/views/changes/flow/run-step-node.tsx -> packages/desktop/src/views/changes/flow/run-step-node.test.tsx

#### 待测功能

- GROUP_LABEL: 三分类 → 四分类（+`git: 'Git'`）
- STEP_LABEL: 随 `ChangeStepKind` 十八值补 8 归档步词文案

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| git 徽章 | 正向 | git 步节点（commit / rebase / merge / finalize）渲染 `Git` 徽章（`data-testid="run-step-group"` 文案 = Git；`data-run-group="git"`）（AC-9） | 新增 |
| 归档步词文案 | 正向 | 8 归档步词渲染各自文案（preflight / commit / rebase / resolveConflicts / merge / specSync / seal / finalize），STEP_LABEL 十八值穷举可查（AC-2 / AC-9） | 新增 |
| 分组徽章三分类回归 | 边界 | 既有 workerAgent / toolStep / gate 徽章文案不变；agent 步（specSync / resolveConflicts）仍 workerAgent 徽章（非 git）（AC-9 / D4） | 回归 |
| 状态视觉 | 异常 | 归档步节点各状态（running / passed / failed / stopped）视觉 class 与 data-run-status 不变（AC-9 零改动锚） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `@testing-library/react` 直渲染 `RunStepNode`（内存构造 `RuntimeFlowNode` fixture；零进程边界依赖） | 本节全部用例 |

---

### packages/desktop/src/views/changes/flow/run-control-panel.tsx -> packages/desktop/src/views/changes/flow/run-control-panel.test.tsx

#### 待测功能

- ConfirmCard: 检测 `phase === 'archive'` 时渲染归档确认卡片（`activeRun.archive` 完成度警告 / delta specs / merge 目标 + 跳过同步勾选），确认走 `confirm(proceed, syncSpecs)`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 归档确认卡片 | 正向 | `activeRun.status === 'waitingConfirm'` 且 `phase === 'archive'` → 渲染归档确认卡片（呈现 `activeRun.archive` 的 completed 警告 / incomplete_phases / delta_specs / merge 目标 + 跳过同步勾选）（AC-7 / AC-9） | 新增 |
| 归档确认卡片 | 正向 | 勾选「跳过同步」→ 确认按钮调用 `confirm(true, true)`；不勾选 → `confirm(true, false)`；终止 → `confirm(false, …)`（AC-7 / D2） | 新增 |
| 普通相位确认 | 边界 | `phase !== 'archive'`（如 acceptance）→ 渲染既有普通 ConfirmCard（文案「已收口，是否继续推进下一相位」；`confirm(proceed)` 单参语义不变）（AC-7 零改动锚） | 回归 |
| archive 载荷缺省 | 边界 | `phase === 'archive'` 而 `activeRun.archive === null` → 归档卡片降级渲染（不崩溃，跳过同步勾选仍可用）（AC-7 稳健面） | 新增 |
| 普通确认回归 | 回归 | proceed=true / false 两按钮行为不变（AC-7 零改动锚） | 回归 |
| confirm 后端异常 | 异常 | invoke `confirm` 被后端拒绝（无运行 run / 无等待方返回 Err）→ 卡片不崩溃、错误态呈现或保持等待确认，不静默吞错（AC-7 稳健面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core`（进程边界） | 既有 `vi.mock('@tauri-apps/api/core')` 装置（invoke 分发 + 可编程 Channel） | 本节全部用例 |
| React hooks（内部模块） | 真实组合（`useChangeFlowRun` 真实调用，仅 invoke 面 mock） | 本节全部用例 |

---

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

#### 待测功能

- 摘 `ArchivePanel` 装配与 `useArchiveFlow` 调用；`archiveOpen` 状态与归档按钮路径移除（归档入口 = run 流程的 stop-before-archive 停等面）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ArchivePanel 移除 | 正向 | 详情页不再渲染 `ArchivePanel`（`data-testid` 缺席）；不再调用 `useArchiveFlow`（AC-9） | 新增 |
| 归档按钮路径移除 | 正向 | 归档触发按钮（`data-testid="archive-trigger"`）移除，归档入口由 run 流程停等面承载（AC-9） | 新增 |
| 归档列上图 | 正向 | archive 列节点经 `ChangeFlowGraph` 上图（10 列布局 + 归档步节点可辨；AC-9） | 新增 |
| 既有渲染回归 | 回归 | 详情头 / 状态徽章 / 产物 / 抽屉等既有渲染不受影响（AC-9 零改动锚） | 回归 |
| archive 列空态 | 边界 | archive 列零节点（无 run / 无 archive attempt）或 `activeRun.archive === null` → 详情图空列不报错、停等面降级渲染（miss 非错误；AC-9 稳健面） | 新增 |
| detail 拉取失败 | 异常 | `get_change_detail` 返回 Err / null（backend 拒绝 / 未建档 change）→ 详情页降级渲染（错误态 / 空态）不崩溃（AC-9 稳健面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core`（进程边界） | 既有 `vi.mock('@tauri-apps/api/core')` 装置（invoke 分发 + 可编程 Channel） | 本节全部用例 |
| React hooks（内部模块） | 真实组合（`useChangeDetail` / `useChangeFlowRun` 真实调用，仅 invoke 面 mock） | 本节全部用例 |

---

## 不可测试项

- **再导出出口面（`crates/core/orchestration/src/lib.rs`）** — 摘 `pub mod archive_flow` 与全部 archive 再导出、挂 `ArchivePreflight` 再导出、摘 `archive_flow_test` 模块声明。零逻辑再导出，正确性由消费侧测试以导出可导入性编译面承载，无独立测试路径；不新增导出供测试调用（knip 纪律）。
- **命令面登记（`src/commands/mod.rs` 摘 `pub mod archive_flow` 与 5 条命令注册、`src/main.rs` 摘 `app.manage(Arc::new(ArchiveControl::new()))`）** — 模块/命令登记为编译面，正确性由「退役模块不可导入 / 命令不可注册」的编译面断言与 AC-10 命令面收编行为面（`change_flow/mod_test.rs`）交叉承载，无独立测试路径。
- **前端类型声明面（`src/views/changes/flow/types.ts` 的 `RunStepGroup` +`'git'`）** — 无同名测试文件（`types.test.ts` 不在设计范围）；类型字段增改由消费侧测试（`run-state.test.ts` / `run-step-node.test.tsx`）以构造与投影交叉承载，编译面由 tsc 拦截。
- **bindings 再生成一致性（`src/types/generated/bindings.ts`）** — 生成物零手改（`ArchivePreflight` / `ConfirmWait.archive` / `ActiveRunView.archive` / `changeFlowConfirm.syncSpecs` / 退役 archive_flow 五命令 / 十八值词汇），由 bindings 一致性守卫（`bindings:check` 管线命令）拦截漂移，非单测面；前端类型消费正确性由各前端测试以新类型 fixture 承载。
- **`packages/desktop/package.json` version bump（0.4.36 → 0.4.37）** — 静态配置核对项，归实现阶段守线与验收阶段承载。
- **AC-3「core 零 spawn」grep 面 / AC-5「`evaluator.is_none()` 检查删除」diff 面 / AC-9「ArchivePanel / StageList / useArchiveFlow / archive-state.ts 删除」diff 面 / AC-10「archive_flow 五命令 + ArchiveControl + ArchiveSink 删除」diff 面** — 均为源码静态/diff 审查约束（删除与「零某逻辑」不可进程内正向断言），由实现阶段红线自查 grep 与 code-review 承载；行为面（git 步走 vcs 缝、无 evaluator 硬检查仍进 archive、归档入口停等面、命令面收编发起/停止）已由 `walker_test.rs` / `change_flow/mod_test.rs` / `change-detail-view.test.tsx` 覆盖。
- **AC-4「零 `if phase == archive` 特判」grep 面** — 「无特判」是 diff 面断言（三分流按产物在否天然分离）；行为面（空 checklist 落账 → `derive_verdict([])=Pass` → done）已由 `walker_test.rs` + `phase_next_test.rs` 覆盖。
- **AC-6「零新表」grep 面** — 复用既有三表（`RunNodeRecord` / `RunEdgeRecord` / `PhaseRecord`）是 model/存储面零新增断言；行为面（归档节点/边/PhaseRecord 落既有表 + 十八值投影 + refresh 常驻）已由 `walker_test.rs` + `detail_test.rs` 覆盖。
- **AC-6 / AC-10 golden「人工确认留痕」** — `DESKTOP_GOLDEN_REWRITE=1` 显式重写流程中的人工确认是流程性步骤，不可自动化断言；golden 等值回归本身由 `corpus_golden_test.rs` 覆盖（覆写模式仅在重写流程启用）。
- **AC-10 管线与静态核对（`vp test` / `client:check` / knip / `bindings:check` 全绿、`plugins/dev-team` 2.10.44 零改动、version 0.4.37）** — 管线守线与静态核对项，归实现阶段守线任务与验收阶段承载。
