# 测试设计: desktop-test-only-flow

> **变更**: desktop-test-only-flow
> **阶段**: test-design（只设计测试，不写测试命令细节）
> **基于**: proposal.md、design.md
> **测试框架识别**：
> - **Rust**：`cargo test`（workspace 根 `packages/desktop/src-tauri`），单元测试为与被测模块同目录的 `*_test.rs`（如 `phase_table_test.rs`、`detail_test.rs`、`mod_test.rs`），断言用 `assert_eq!` / `assert!` / `assert!(matches!(…))`，`#[test]` / `#[tokio::test]` 描述名用中文；命令层测试用 `tauri::test::mock_app()` + 真实 `WorkspaceStores`（tempdir 数据根）+ 真实 fs/git 组合（零 mock）。
> - **TypeScript / React**：`pnpm -C packages/desktop run client:test`（`vp test`，vitest 内核 + `@testing-library/react` + jsdom），测试文件与被测模块同目录 `*.test.ts` / `*.test.tsx`，断言风格 `import { describe, expect, it } from 'vite-plus/test'`，仅 mock 进程边界（`@tauri-apps/api/core` 的 `invoke` / `Channel`），内部 hook / 纯函数真实组合。
> - **命名惯例**：Rust `{module}_test.rs` 与 TS `{module}.test.{ts,tsx}` 均与被测源文件同目录；无既有测试时按语言惯例推定。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | `phase_table` 两值分派：`"requirement"` → 9 相位、`"test-only"` → 6 相位、未知类型 → `None`；`TEST_ONLY_TABLE` 复用矩阵成立 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` |
| AC-2 | `CODE_ANALYZE_STEPS` P→E 形态；`code-analyze-planner.md` / `code-analyze-evaluator.md` 经 `include_str!` 显式装配；code-analyze 产物清单空 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`、`packages/desktop/src-tauri/crates/core/workflow/src/prompts/code-analyze-planner.md`、`packages/desktop/src-tauri/crates/core/workflow/src/prompts/code-analyze-evaluator.md` |
| AC-3 | `PhaseDefinition.prerequisites` 表内自描述；无独立 `prerequisites()` match；`dependents` 遍历表内；test-only 依赖图与 backtrack stale 传播正确 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` |
| AC-4 | 写面 / 发起面对未知类型显式 `Err`（文案通吃 `{requirement, test-only}`）；读面不 fallback requirement；存量 requirement 记录零迁移照常可读 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` / `packages/desktop/src-tauri/crates/core/workflow/src/write/{phase_next,phase_start,phase_log,backtrack}.rs` / `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` / `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` |
| AC-5 | `queries/detail.rs` 无 `PIPELINE_PHASES`；`change_detail.pipeline` 随 `phase_table(workflow_type)` 出线（9 / 6 列）；前端零 `PIPELINE_PHASES.indexOf` | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` / `packages/desktop/src/views/changes/flow/{layout,graph,run-state,attachments}.ts` / `packages/desktop/src/views/changes/change-detail-view.tsx` |
| AC-6 | 前端 test-only 详情 6 列、requirement 9 列；code-analyze 非占位、archive 末列 | `packages/desktop/src/views/changes/flow/{graph,run-state,attachments}.ts` / `packages/desktop/src/views/changes/change-detail-view.tsx` |
| AC-7 | `create_change` 四参透传落库；新建对话框默认 requirement 可改 test-only；非法值 `Err` 零建档 | `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` / `packages/desktop/src-tauri/src/commands/changes/mod.rs` / `packages/desktop/src/views/changes/components/change-create-dialog.tsx` |
| AC-8 | stance 类型推算约定；`extractWorkflowType` 回填（read 路径零写入）；promote 三参透传，铸出 `workflow_type` = 用户最终选择 | `packages/desktop/src/lib/explore-stance.ts` / `packages/desktop/src/lib/explore-workflow-type.ts` / `packages/desktop/src/views/explores/explore-detail-view.tsx` / `packages/desktop/src/views/explores/hooks/use-explore-list.ts` / `packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs` / `packages/desktop/src-tauri/src/commands/explores/mod.rs` |
| AC-9 | `ExploreRecord` v3 含 `workflow_type`；存量 v2 打开升级读出 `"requirement"`；`set_explore_workflow_type` 合法值写入、空白 / 非法值 `Err` | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` / `packages/desktop/src-tauri/crates/infra/store/src/store.rs` / `packages/desktop/src-tauri/src/commands/explores/mod.rs` |
| AC-10 | `client:check` / `client:test` / `cargo test --workspace` 全绿；bindings 一致性守卫绿；golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写 | 全部上述被测文件 + `packages/desktop/src-tauri/src/commands/mod.rs`（`all_commands!` 登记）+ `packages/desktop/src/types/generated/bindings.ts`（codegen） |
| AC-11 | `packages/desktop/package.json` `version` 0.4.40 → 0.4.41（归档时执行）；`plugins/dev-team` 保持 2.10.44 | `packages/desktop/package.json` |

---

## 单元测试

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能
- `phase_table(workflow_type: &str) -> Option<&'static [PhaseDefinition]>`：两值分派——`requirement` → 9 相位、`test-only` → 6 相位、其余（含空白 / `bug-fix`）→ `None`（不退化）。
- `PhaseDefinition`：增 `prerequisites: &'static [&'static str]` 表内自描述字段；删除独立 `prerequisites()` match；`dependents(table, phase)` 改遍历 `candidate.prerequisites.contains(&phase)`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `phase_table` | 正向 | `phase_table("requirement")` 返回 9 相位，序恒 `proposal/dev-design/test-design/implement/test-gen/test-execution/code-review/acceptance/archive` | 改写 |
| `phase_table` | 正向 | `phase_table("test-only")` 返回 6 相位，序恒 `proposal/code-analyze/test-design/test-gen/test-execution/archive` | 新增 |
| `phase_table` | 正向 | `TEST_ONLY_TABLE` 的 proposal / test-design / test-gen / test-execution 复用 requirement 同名 steps 与 prompt；archive 复用 `ARCHIVE_STEPS`；code-analyze 用 `CODE_ANALYZE_STEPS` | 新增 |
| `phase_table` | 正向 | `CODE_ANALYZE_STEPS` 步序 = `[PhaseStart, Executor, Evaluator, VerdictGate, PhaseLog]`；`VerdictGate.on_fail = Terminal`；executor / evaluator 均 `ModelLevel::High`；artifact_manifest 为空数组 | 新增 |
| `phase_table` | 正向 | `CODE_ANALYZE_EXECUTOR_PROMPT` / `CODE_ANALYZE_EVALUATOR_PROMPT` 两常量静态非空且零占位（`include_str!` 装配，与步 `prompt_ref` 同源） | 新增 |
| `phase_table` | 异常 | `phase_table("bug-fix")` / `""` / `"refactor"` / `" "` 均返回 `None`（不退化） | 改写 |
| `phase_table` | 边界 | trim + `eq_ignore_ascii_case` 命中：`"Test-Only"` / `" requirement "` 分派到对应表，`" test-only "` 不因空白误判 | 新增 |
| `PhaseDefinition` | 边界 | 穷尽解构含 `prerequisites` 字段（无 `..` 兜底——字段增删即编译失败）；requirement 表 prerequisites 照搬现 match | 新增 |
| `PhaseDefinition` | 边界 | test-only 表 prerequisites：proposal=[]、code-analyze=[proposal]、test-design=[proposal, code-analyze]、test-gen=[test-design]、test-execution=[test-gen]、archive=[] | 新增 |
| `dependents` / `allowed_backtrack_phases` | 正向 | `dependents(table, phase)` 遍历 `candidate.prerequisites.contains(&phase)`，在 requirement 与 test-only 两表下回溯白名单正确（test-only：code-analyze 回溯含 proposal、test-design 含 proposal+code-analyze 等） | 新增 |
| `dependents` | 异常 | 未知 phase / 空 phase 返回空 Vec / 空集，不臆测不 panic | 保留 |
| `phase_table` | 边界 | code-analyze artifact_manifest 空、proposal 复用 `PROPOSAL_MANIFEST`、test-execution 复用 `TEST_EXECUTION_MANIFEST` | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 编译期 const 表纯函数，`#[test]` 直测 `phase_table` / `dependents` / `allowed_backtrack_phases` | 全部用例零 mock；prompt 常量经 `include_str!` 编译期装配，直接断言非空与零占位 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs

#### 待测功能
- `create(…, workflow_type: &str) -> Result<CreateOutcome, String>`：新增 `workflow_type` 入参（位于 `title` 之后）；前置校验 `phase_table(trim 小写).is_none()` → `Err`；`ChangeStateRecord.workflow_type` 由硬编码改为 `workflow_type.clone()`（canonical 小写）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `create` | 正向 | 传 `workflow_type="test-only"` → 落库 `ChangeStateRecord.workflow_type == "test-only"`，建域照常 | 新增 |
| `create` | 正向 | 传 `workflow_type="requirement"` → 落库 `"requirement"`（既有默认行为持衡） | 改写 |
| `create` | 正向 | 大小写 / 首尾空白 canonical：`"Test-Only "` → 落库 `"test-only"` | 新增 |
| `create` | 异常 | 传非法 `workflow_type`（`"bug-fix"` / `""` / `" "`）→ 显式 `Err`，零建档（无 change 记录、无磁盘建域） | 新增 |
| `create` | 异常 | name / goal 非法拒绝路径持衡（零 regression） | 保留 |
| `create` | 边界 | `workflow_type` trim 后空串 → `Err`；`" requirement "`（带空白）→ 命中并落小写 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `store: &dyn ChangeStateStore` / `vcs: &dyn WorktreePort`（显式入参注入依赖） | 沿用既有 `create_test.rs` 装置（真实 tempdir store；WorktreePort 按既有方式真实 / 假件注入） | 建档落库与 worktree 建域路径；零新增 mock，只新增 `workflow_type` 入参与断言 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/promote_test.rs

#### 待测功能
- `promote_explore(…, workflow_type: &str) -> Result<PromoteOutcome, String>`：新增 `workflow_type` 入参并透传 `super::create`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `promote_explore` | 正向 | 传 `workflow_type="test-only"` → 铸出 `ChangeRecord.workflow_type == "test-only"`（透传 create 生效） | 新增 |
| `promote_explore` | 正向 | 传 `workflow_type="requirement"` → `"requirement"`（既有 promote 语义持衡） | 改写 |
| `promote_explore` | 异常 | 传非法 `workflow_type` → `create` 校验 `Err` 透传（零铸出） | 新增 |
| `promote_explore` | 边界 | 大小写 / 空白 canonical（`"Test-Only"` → 落库 `"test-only"`） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `store: &dyn ChangeStateStore` / `vcs: &dyn WorktreePort`（显式入参注入依赖） | 沿用既有 `promote_test.rs` 装置（create 组合真实；worktree/vcs 按既有方式注入） | promote → create 透传链路；零新增 mock |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs

#### 待测功能
- `phase_next()`：`phase_table` 命中 `None` 时显式 `Err`，文案通吃 `{requirement, test-only}`（不再「V1 仅 requirement」）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 未知类型严格拒绝 | 异常 | `phase_table` 返回 `None` 的 change 推进 → 显式 `Err`，文案含「支持 requirement / test-only 工作流」 | 改写 |
| test-only 类型分派可走通 | 正向 | `workflow_type="test-only"` 的 change 推进 `phase_next` → 不因类型被拒（按 `TEST_ONLY_TABLE` 序推进） | 新增 |
| 文案边界 | 边界 | 未知类型 `Err` 文案不再出现「V1 仅 requirement」字样，且覆盖 `{requirement, test-only}` 两值 | 改写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `store` / 运行环境（写面注入依赖） | 沿用既有 `phase_next_test.rs` 装置（真实 / 假件 store，按既有方式） | 严格分派拒绝面 + test-only 可走通面 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start_test.rs

#### 待测功能
- `phase_start()`：`phase_table` 命中 `None` 时显式 `Err`，文案通吃 `{requirement, test-only}`（不再「V1 仅 requirement」）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 未知类型严格拒绝 | 异常 | `phase_table` 返回 `None` 的 change 开相 → 显式 `Err`，文案含「支持 requirement / test-only 工作流」 | 改写 |
| test-only 类型分派可走通 | 正向 | `workflow_type="test-only"` 的 change 开相 → 不因类型被拒 | 新增 |
| 文案边界 | 边界 | `Err` 文案不再出现「V1 仅 requirement」字样 | 改写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `store`（写面注入依赖） | 沿用既有 `phase_start_test.rs` 装置 | 严格分派拒绝面 + test-only 可走通面 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log_test.rs

#### 待测功能
- `phase_log()`：`phase_table` 命中 `None` 时显式 `Err`，文案通吃 `{requirement, test-only}`（不再「V1 仅 requirement」）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 未知类型严格拒绝 | 异常 | `phase_table` 返回 `None` 的 change 落账 → 显式 `Err`，文案含「支持 requirement / test-only 工作流」 | 改写 |
| test-only 类型分派可走通 | 正向 | `workflow_type="test-only"` 的 change 落账 → 不因类型被拒 | 新增 |
| 文案边界 | 边界 | `Err` 文案不再出现「V1 仅 requirement」字样 | 改写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `store`（写面注入依赖） | 沿用既有 `phase_log_test.rs` 装置 | 严格分派拒绝面 + test-only 可走通面 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack_test.rs

#### 待测功能
- `backtrack()`：`phase_table` 命中 `None` 时显式 `Err`，文案通吃 `{requirement, test-only}`；stale 传播随表内 `prerequisites` 类型感知。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 未知类型严格拒绝 | 异常 | `phase_table` 返回 `None` 的 change 回溯 → 显式 `Err`，文案含「支持 requirement / test-only 工作流」 | 改写 |
| test-only 回溯 stale 传播 | 正向 | `workflow_type="test-only"` 下 backtrack 按表内 `prerequisites` 推导 stale 传播（如 test-gen 回改使 test-execution stale、code-analyze 回改使 test-design stale） | 新增 |
| 文案边界 | 边界 | `Err` 文案不再出现「V1 仅 requirement」字样 | 改写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `store`（写面注入依赖） | 沿用既有 `backtrack_test.rs` 装置 | 严格分派拒绝面 + test-only 表内 prerequisites 的 stale 传播 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

#### 待测功能
- `change_detail()`：删 `PIPELINE_PHASES` 常量；先取 `phase_table(record.workflow_type)`（`None` → 返回 `None`），建站与产物清单同用该表相位序。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `change_detail` 列序出线 | 正向 | requirement 记录 → `pipeline` 9 列（`proposal…archive`，archive 第 9 列，无 code-analyze 占位列） | 改写 |
| `change_detail` 列序出线 | 正向 | test-only 记录 → `pipeline` 6 列（`proposal/code-analyze/test-design/test-gen/test-execution/archive`，archive 第 6 列，code-analyze 第 2 列） | 新增 |
| `change_detail` 产物清单 | 正向 | 产物清单 = `phase_table(record.workflow_type)` 的 artifact_manifest（test-only 下 code-analyze 清单空、proposal / test-execution 复用既有清单） | 改写 |
| `change_detail` 未知类型 | 异常 | `workflow_type="bug-fix"` 记录 → `change_detail` 返回 `None`（不 fallback requirement，MUST NOT 静默当 requirement 跑） | 新增 |
| `change_detail` 未知 id | 异常 | db 无该 id → `None`（既有语义持衡） | 保留 |
| `change_detail` 边界 | 边界 | requirement（9）与 test-only（6）两表 archive 列位置正确（`pipeline` 末列）；`detail.rs` 无 `PIPELINE_PHASES` 常量（编译期锚） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `ChangeStateStore`（进程内假件 `DetailStore`） | 沿用既有 `detail_test.rs` 假件（id 键记录表 + 归属键过滤条目序列；workflow 自环 dev-dep 导致真实 `Store` 不可达） | 注入 `workflow_type` 为 `requirement` / `test-only` / `bug-fix` 的记录，断言 pipeline 列序与 None 呈现 |
| 磁盘树 | 真实 tempdir（active 裸名目录 / archive `YYYY-MM-DD-` 前缀目录 + 产物文件） | 产物清单发现与归档前缀语义 |

---

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

#### 待测功能
- `create_change(app, root, name, goal, workflow_type)`：三参 → 四参，`title = name.clone()` 后透传 `create_change_with`。
- `create_change_with(app, root, name, goal, title, workflow_type)`：泛型测试缝新增 `workflow_type` 并透传 `write::create`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `create_change_with` | 正向 | 传 `workflow_type="test-only"` → 落库 `record.workflow_type == "test-only"`，返回 DTO 五键（created/id/name/warnings/worktree）不变 | 新增 |
| `create_change_with` | 正向 | 传 `workflow_type="requirement"` → 落库 `"requirement"`（既有 `record.workflow_type` 断言随入参改写） | 改写 |
| `create_change` | 正向 | 四参 IPC 命令：`title=name` 后透传 `workflow_type`（不丢参数） | 新增 |
| 跨模块组合（详情出线） | 正向 | `get_change_detail` 对 test-only 记录 → `pipeline` 6 列；对 requirement 记录 → 9 列（链路入口 = `get_change_detail` IPC） | 新增 |
| 跨模块组合（详情出线） | 异常 | `get_change_detail` 对 `workflow_type="bug-fix"` 记录 → `None`（不 fallback requirement） | 新增 |
| `create_change_with` | 异常 | 传非法 `workflow_type` → `Err`，零建档零建域 | 新增 |
| `create_change_with` | 边界 | 大小写 / 空白 canonical 落库（`"Test-Only"` → `"test-only"`） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（进程边界真实组合） | 沿用既有装置：`tauri::test::mock_app()` + 真实 `WorkspaceStores`（tempdir 数据根）+ 真实 git（`TEST_PATH_LOCK` 串行） | create 建域 / 详情出线链路零 mock；仅新增 `workflow_type` 入参与断言 |

---

### packages/desktop/src-tauri/src/commands/explores/mod.rs -> packages/desktop/src-tauri/src/commands/explores/mod_test.rs

#### 待测功能
- `promote_explore(app, root, name, workflow_type)`：二参 → 三参，透传 `promote_explore_with`。
- `promote_explore_with(app, root, name, workflow_type)`：新增 `workflow_type`，`phase_table` 命中校验 + 透传写面 `promote_explore`。
- `update_explore_workflow_type(stores, root, name, workflow_type)`：新增 IPC 命令；blank root / 非法值 → `Err`；调 store `set_explore_workflow_type` 回填。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `promote_explore_with` | 正向 | 传 `workflow_type="test-only"` → 铸出 change `workflow_type == "test-only"` | 新增 |
| `promote_explore_with` | 正向 | 传 `workflow_type="requirement"` → `"requirement"` | 改写 |
| `promote_explore_with` | 异常 | 传非法 `workflow_type` → `phase_table` 校验 `Err`（零铸出） | 新增 |
| `update_explore_workflow_type` | 正向 | 合法值 `"test-only"` → 回填 `ExploreRecord.workflowType`，刷新 `updated_at`，返回更新记录 | 新增 |
| `update_explore_workflow_type` | 正向 | 大小写 / 空白 canonical（`"Test-Only"` → 落 `"test-only"`） | 新增 |
| `update_explore_workflow_type` | 异常 | blank root / 空白 root → `Err` | 新增 |
| `update_explore_workflow_type` | 异常 | 非法值（`"bug-fix"` / `""`）→ `Err`（`phase_table` 不命中） | 新增 |
| `update_explore_workflow_type` | 异常 | 记录 miss → `Err`（store 透传） | 新增 |
| `update_explore_workflow_type` | 边界 | 读路径零写入：`read_explore` 不调用回填（回填唯一写入口 = 本命令） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（进程边界真实组合） | 沿用既有装置：`mock_app()` + 真实 `WorkspaceStores` + 真实 git（promote 复用 create） | promote 三参 + 回填命令链路零 mock；仅新增 `workflow_type` 入参与回填断言 |

---

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

#### 待测功能
- `change_flow_start()`：保留「`phase_table` `None` → 显式拒绝」门，文案改通吃 `{requirement, test-only}`；walker 零 workflow_type 分支。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 发起前置校验 | 正向 | `workflow_type="requirement"` 记录 `change_flow_start` → 正常通过（既有语义持衡） | 保留 |
| 发起前置校验 | 正向 | `workflow_type="test-only"` 记录 `change_flow_start` → 不再被前置校验拒绝 | 新增 |
| 发起前置校验 | 异常 | 未知 `workflow_type`（`"bug-fix"` / 空）→ 显式拒绝，文案含「仅支持 requirement / test-only 工作流」 | 改写 |
| 发起前置校验 | 边界 | 拒绝文案不再出现「仅支持 requirement 工作流」字样；walker 零 workflow_type 分支（相位分派由写面承担） | 改写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（进程边界真实组合） | 沿用既有装置：`mock_app()` + 真实 `WorkspaceStores` + `Arc<ChangeFlowControl>` + `Arc<StopRegistry>` | 种子注入 `workflow_type`（requirement / test-only / bug-fix）驱动前置校验分支 |

---

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能
- `ExploreRecord::new(root, name, now)`：签名不变，新增 `workflow_type = "requirement"` 默认。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `ExploreRecord::new` | 正向 | `new` 产出的 `workflow_type == "requirement"`（默认值） | 新增 |
| `ExploreRecord`（v3 升级） | 正向 | v2 → v3 decode 读出 `workflow_type == "requirement"`，其余字段（title / promoted_to / id / 时间戳）直透 | 新增 |
| `ExploreRecord`（v3 升级） | 边界 | `native_model` 元数据 `version = 3, from = ExploreRecordV2`；升级链 V1 → V2 → V3 可解码 | 新增 |
| `From<ExploreRecord> for ExploreRecordV2` | 边界 | 降级半边形丢弃 `workflow_type`（只保升级语义真实性） | 新增 |
| `ExploreRecordV2` | 边界 | v2 历史形态七字段无 `workflow_type`，`From<ExploreRecordV1>` 平移 title=name、promoted_to=None | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 数据文件（native_model 序列化样本） | 用既有 `model_test.rs` 装置（`native_model` decode/encode 样本夹具） | v2→v3 升级语义与默认值断言；零进程 mock |

---

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能
- `set_explore_workflow_type(&self, root, name, workflow_type) -> Result<ExploreRecord, StoreError>`：in-place 写 + 刷新 `updated_at`；非法值 / 空白 → `Err`；记录 miss → `Err`（私有 `is_valid_workflow_type` 自持合法值集）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `set_explore_workflow_type` | 正向 | 合法值 `"test-only"` → in-place 写（保主键）、刷新 `updated_at`、返回更新记录 | 新增 |
| `set_explore_workflow_type` | 正向 | 大小写 / 空白 canonical（`"Test-Only"` → 落 `"test-only"`；trim 后命中） | 新增 |
| `set_explore_workflow_type` | 异常 | 空白值（`""` / `" "`）→ `Err` | 新增 |
| `set_explore_workflow_type` | 异常 | 非法值（`"bug-fix"` / `"refactor"`）→ `Err`（自持合法值集 `{requirement, test-only}`） | 新增 |
| `set_explore_workflow_type` | 异常 | 记录 miss → `Err` | 新增 |
| `set_explore_workflow_type` | 边界 | 幂等覆写同值不报错；`ExploreRecord::new` 默认 requirement 落库可再回填覆盖 | 新增 |
| `is_valid_workflow_type` | 边界 | store 自持校验不依赖 workflow crate（`{requirement, test-only}` 单源，canonical 小写） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| DB（进程边界依赖） | 沿用既有 `store_test.rs` 装置：真实 tempdir `Store`（native db 临时库），沿 `set_explore_title` 事务形态 | in-place 写 / 刷新 `updated_at` / 拒绝面零 mock（真实组合） |

---

### packages/desktop/src/lib/explore-stance.ts -> packages/desktop/src/lib/explore-stance.test.ts

#### 待测功能
- `buildExplorePrompt(userInput, topic)`：签名不变；`stancePreamble` 增「第二行 `workflow_type:` 标记行」约定与判断标准（补测试 / 生成测试用例 → test-only，否则 requirement）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `buildExplorePrompt` | 正向 | 输出含 `workflow_type: <requirement|test-only>` 标记行约定（与首行 `# <标题>` 同构） | 新增 |
| `buildExplorePrompt` | 正向 | 输出含判断标准：目标是补测试 / 生成测试用例 → test-only，否则 requirement | 新增 |
| `buildExplorePrompt` | 边界 | 首行标题约定与第二行 `workflow_type:` 约定并存且顺序正确 | 新增 |
| `buildExplorePrompt` | 异常 | 空 `userInput` / `topic` 不崩（既有模板拼接持衡） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无依赖） | 纯文本模板拼接零依赖直测 | 全部用例零 mock |

---

### packages/desktop/src/lib/explore-workflow-type.ts -> packages/desktop/src/lib/explore-workflow-type.test.ts

#### 待测功能
- `extractWorkflowType(content: string): "requirement" | "test-only" | null`：逐行扫描首个 `^workflow_type:\s*(\S+)\s*$` 标记，值 case-insensitive 命中 `{requirement, test-only}` 返回小写规范值，否则 `null`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `extractWorkflowType` | 正向 | `"workflow_type: test-only"` → `"test-only"`；`"workflow_type: requirement"` → `"requirement"` | 新增 |
| `extractWorkflowType` | 正向 | 非首行命中（第二行 / 后续行）→ 逐行扫描首个匹配返回 | 新增 |
| `extractWorkflowType` | 正向 | 大小写不敏感 + 规范小写（`"Workflow_Type: TEST-ONLY"` → `"test-only"`） | 新增 |
| `extractWorkflowType` | 异常 | 无标记 / 非法值（`"workflow_type: bug-fix"`）→ `null` | 新增 |
| `extractWorkflowType` | 异常 | 值缺失（`"workflow_type:"` 后空）→ `null`；解析失败不报错 | 新增 |
| `extractWorkflowType` | 边界 | 空串 / 纯空白 / 仅换行 → `null` | 新增 |
| `extractWorkflowType` | 边界 | 超长内容（>1000 字符）不截断、特殊字符（`\n` `\0` emoji）不崩 | 新增 |
| `extractWorkflowType` | 边界 | 非约定行（`# 标题`、`## 标题`）→ `null` | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无依赖） | 纯函数直测（零 mock，沿 `explore-title.test.ts` 风格） | 全部用例零 mock |

---

### packages/desktop/src/views/explores/hooks/use-explore-list.ts -> packages/desktop/src/views/explores/hooks/use-explore-list.test.ts

#### 待测功能
- `ExploreListState.promote(name, workflowType)`：调 `promoteExplore(root, name, workflowType)` 后 refresh（失败 onError + rethrow）。
- `ExploreListState.updateWorkflowType(name, workflowType)`：新增，调 `updateExploreWorkflowType(root, name, workflowType)` 后 refresh（失败 onError + rethrow）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `promote` 动作面 | 正向 | `promote(name, workflowType)` → `invoke("promote_explore", { root, name, workflowType })` → 成功后 refresh | 改写 |
| `updateWorkflowType` 动作面 | 正向 | 新增 `updateWorkflowType(name, workflowType)` → `invoke("update_explore_workflow_type", { root, name, workflowType })` → 成功后 refresh | 新增 |
| `promote` / `updateWorkflowType` | 异常 | reject → error 置位 + Promise 拒绝供详情页行内捕获（清单保持原值） | 改写 |
| 无 root 守卫 | 边界 | root 为 null → `promote` / `updateWorkflowType` 均 no-op（零 IPC） | 改写 |
| `ExploreRecord` fixture | 边界 | 记录 fixture 增 `workflowType` 字段（对齐 store v3 serde camelCase） | 改写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))` | 命令名 / 参数断言 + 失败语义 |
| 内部 hook | 真实组合（`renderHook` + `waitFor`） | 动作后 refresh / 清单回流语义 |

---

### packages/desktop/src/views/explores/explore-detail-view.tsx -> packages/desktop/src/views/explores/explore-detail-view.test.tsx

#### 待测功能
- `useWorkflowTypeBackfill()`：`extractWorkflowType` 命中且与当前不同则回填，失败静默。
- `usePromoteAction()`：增 `workflowType` 本地态（默认 `record.workflowType`）+ `onWorkflowTypeChange`（改即写 `updateWorkflowType` 落库）。
- `ExploreDetailHeader()`：草稿态「启动变更」旁内联两选项 select，`onPromote` 调 `promote(record.name, workflowType)`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| promote 类型选择器 | 正向 | promote 入口内联 select 渲染，默认值 = `record.workflowType`（`data-testid="explore-workflow-type-select"`） | 新增 |
| promote 类型选择器 | 正向 | 改选类型 → `updateWorkflowType` 回填落库（改即写） | 新增 |
| promote 传参 | 正向 | 点「启动变更」→ `promote(record.name, workflowType)` 携带最终选择类型 | 改写 |
| `useWorkflowTypeBackfill` | 正向 | `extractWorkflowType` 命中且与 `record.workflowType` 不同 → 调 `updateWorkflowType` 回填 | 新增 |
| `useWorkflowTypeBackfill` | 异常 | 回填失败 / 解析失败静默（`extractWorkflowType` 返回 null → 不回填、零报错） | 新增 |
| `updateWorkflowType` reject | 异常 | 回填 reject → 行内错误块呈现、页面不崩（沿 `useTitleBackfill` 先例） | 新增 |
| 记录 fixture | 边界 | `ExploreRecord` fixture 增 `workflowType` 字段（requirement / test-only 两态） | 改写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| IPC + Channel（进程边界） | `vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }))` | 笔记读取 / 订阅 / 回填命令应答 |
| `list`（`ExploreListState`，props 注入依赖） | 可编程假件（`makeList`，`promote` / `updateWorkflowType` 可 resolve / reject） | 动作调用与失败语义 |
| 内部 hook（`useExploreDoc` / `useExploreSession`） | 真实组合 | 详情页渲染链路 |

---

### packages/desktop/src/views/changes/components/change-create-dialog.tsx -> packages/desktop/src/views/changes/components/change-create-dialog.test.tsx

#### 待测功能
- `CreateForm()`：`WorkflowTypeField` 内联两选项 select（默认 requirement / test-only），提交四参 `createChange(root, name, goal, workflowType)`，成功 reset 归 `requirement`。
- `WorkflowTypeField()`：`data-testid="change-create-workflow-type"` 两字面选项类型选择器。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 类型选择器 | 正向 | 新建对话框渲染内联两选项 select（`data-testid="change-create-workflow-type"`），默认 `requirement` | 新增 |
| 四参 invoke | 正向 | 提交 → `invoke("create_change", { root, name, goal, workflowType })`（既有 `{root,name,goal}` 断言扩为四参） | 改写 |
| 类型改选 | 正向 | 改选 `test-only` → 提交载荷 `workflowType == "test-only"` | 新增 |
| 成功重置 | 正向 | 成功后 reset 时 `workflowType` 归 `'requirement'` | 新增 |
| 本地校验 | 异常 | name / goal 必填与 kebab-case 校验失败 → 零 invoke（类型选择器不影响校验语义） | 保留 |
| invoke reject | 异常 | reject → 错误块呈现、留窗、onCreated 零调用 | 保留 |
| 边界 | 边界 | `CreateFormData.workflowType` 初值 `'requirement'`；两选项仅 requirement / test-only 字面值 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))` | 四参 invoke 载荷断言 + 失败语义 |

---

### packages/desktop/src/views/changes/flow/layout.ts -> packages/desktop/src/views/changes/flow/layout.test.ts

#### 待测功能
- `nodePosition()`：坐标公式保留；`COL_W` / `ROW_H` / `COLUMN_HEADER_H` / `COLUMN_PAD_X` 常量持衡；`PIPELINE_PHASES` 常量彻底删除（测试改自备 phaseOrder fixture）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `nodePosition` / 常量 | 正向 | `nodePosition` 坐标公式（colIndex 0 / 4 / 8 抽样）、`COL_W=260` / `COLUMN_PAD_X=18` / `COLUMN_HEADER_H=48` / `ROW_H=128` 持衡 | 保留 |
| `nodePosition` | 边界 | 超大 order（1000）线性外推无钳制 / NaN / 不抛错 | 保留 |
| `PIPELINE_PHASES` 退役 | 异常 | 删除「十列布局」断言（`PIPELINE_PHASES` 长度 10 / 与后端同序 / archive 末列等）——测试改为自备 phaseOrder fixture | 删除 |
| `PIPELINE_PHASES` 退役 | 边界 | `layout.ts` 不再导出 `PIPELINE_PHASES`（编译期锚，删除 import 即编译失败） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无依赖） | 纯函数直测；列序 fixture 由测试自备（内联 phaseOrder），不再 import `PIPELINE_PHASES` | 坐标公式与常量持衡 |

---

### packages/desktop/src/views/changes/flow/graph.ts -> packages/desktop/src/views/changes/flow/graph.test.ts

#### 待测功能
- `buildFlowGraph(detail, runNodes?, runEdges?)`：签名不变；`columns` 改 `detail.pipeline.map((entry, colIndex) => ({ id: \`col:${entry.phase}\`, phase, colIndex }))`（空 pipeline 仍返回空图）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `buildFlowGraph` | 正向 | requirement detail（9 相位 pipeline）→ 9 列容器，`colIndex` 0..8，列序 = `detail.pipeline.map(p => p.phase)` | 改写 |
| `buildFlowGraph` | 正向 | test-only detail（6 相位 pipeline）→ 6 列，code-analyze 第 2 列、archive 第 6 列（`colIndex=5`） | 新增 |
| `buildFlowGraph` | 异常 | pipeline 空数组 → 空图（columns/nodes/edges 全空） | 保留 |
| `buildFlowGraph` | 异常 | pipeline 外相位不在 `detail.pipeline` → 零列（不臆造） | 新增 |
| `buildFlowGraph` | 边界 | 多 attempt + activePhase + runNodes 缺省 → 零节点零边（列数随 pipeline 长度） | 改写 |
| `buildFlowGraph` | 边界 | 超长 runNodes 列表保持顺序、零去重丢失（既有语义持衡） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无依赖） | 纯函数直测；fixture 自备 requirement / test-only 两套 `detail.pipeline`，不再 import `PIPELINE_PHASES` | 列骨架 / 空图 / 边表直读 |

---

### packages/desktop/src/views/changes/flow/run-state.ts -> packages/desktop/src/views/changes/flow/run-state.test.ts

#### 待测功能
- `runStepNodes(steps: ChangeStepState[], phaseOrder: string[])`：新增 `phaseOrder` 入参；`colIndex = phaseOrder.indexOf(step.phase)`，`< 0` 不入图；列内序仍按 phase 计数。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `runStepNodes` | 正向 | 传 requirement `phaseOrder` → `colIndex` 正确（`implement` 下标 3 等，改为 `phaseOrder.indexOf` 推导） | 改写 |
| `runStepNodes` | 正向 | 传 test-only `phaseOrder` → `code-analyze` 下标 1、`archive` 下标 5（列序随出线序） | 新增 |
| `runStepNodes` | 正向 | `phaseOrder` 显式传序：同键多实例、列内 order 归并序持衡 | 改写 |
| `runStepNodes` | 异常 | `step.phase` 不在 `phaseOrder` 内 → 不入图（`indexOf < 0` 跳过） | 改写 |
| `runStepNodes` | 异常 | `phaseOrder` 为空数组 → 全部步不入图 | 新增 |
| `runStepNodes` | 边界 | 十八步词 group / role 分类持衡；`nodeIdentityKey` 反归一持衡 | 保留 |
| `runStepNodes` | 边界 | archive 8 步词 `colIndex` 随传入 `phaseOrder`（requirement=9 列 / test-only=5 列） | 改写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无依赖） | 纯函数直测；测试自备 requirement / test-only 两套 `phaseOrder` | 显式传序 / 外相位不入图 / 列内归并 |

---

### packages/desktop/src/views/changes/flow/attachments.ts -> packages/desktop/src/views/changes/flow/attachments.test.ts

#### 待测功能
- `mountMaterials(detail, envelopes)`：签名不变；内部先 `new Set(detail.pipeline.map(p => p.phase))`，`artifactColumn` 改收 pipeline 相位集，去 `PIPELINE_PHASES.indexOf`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `mountMaterials` | 正向 | 清单归属驱动：proposal / design / test-execution 文档挂对应列（归属校验改用 `detail.pipeline` 相位集） | 改写 |
| `mountMaterials` | 正向 | test-only `detail.pipeline` → code-analyze 列无 artifact 归属（清单空），proposal / test-execution 照常挂 | 新增 |
| `mountMaterials` | 正向 | eval-checklist 按 `payload.phase` 挂对应 phase 列头 | 保留 |
| `mountMaterials` | 异常 | 空归属 / 未知 phase / pipeline 外相位 → 跳过该 artifact（不误挂错站） | 保留 |
| `mountMaterials` | 异常 | `payload.phase` 为 pipeline 外相位 → 不误挂任何列 | 保留 |
| `mountMaterials` | 边界 | 全空入参（envelopes=[]）→ `{ columnDocs: {} }`；file_log 分支退役键零驻留 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无依赖） | 纯函数直测；fixture 自备 requirement / test-only 两套 `detail.pipeline`（不再 import `PIPELINE_PHASES`） | 归属校验 / 挂列 / 空态 |

---

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

#### 待测功能
- `useFlowGraphData()`：先 `phaseOrder = detail.pipeline.map(p => p.phase)`，`runStepNodes(unifiedRunNodes(...), phaseOrder)` 显式传序。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 详情视图列序 | 正向 | requirement detail → 流程图 9 列（无 code-analyze 占位列，archive 第 9 列） | 改写 |
| 详情视图列序 | 正向 | test-only detail → 流程图 6 列，code-analyze 节点上图、archive 第 6 列 | 新增 |
| 传序接线 | 正向 | `phaseOrder = detail.pipeline.map(p => p.phase)` 派生后传 `runStepNodes(unifiedRunNodes(...), phaseOrder)` | 新增 |
| 详情视图 | 异常 | 空 pipeline → 空图渲染不崩 | 保留 |
| 详情视图 | 边界 | memo 依赖（detail / phaseOrder 派生并入 detail 依赖）不导致重复重算 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| IPC + Channel（进程边界） | `vi.mock('@tauri-apps/api/core', () => ({ invoke, Channel }))` | `get_change_detail` 等详情数据流入（fixture 区分 requirement 9 列 / test-only 6 列 pipeline） |
| 内部 hook / 子组件 | 真实组合 | 详情渲染与流程图出线 |

---

## 不可测试项

| 文件或模块 | 原因 |
|------------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/code-analyze-planner.md` | 全静态角色知识 md，无逻辑可单测；其装配正确性经 `phase_table.rs` 的 `CODE_ANALYZE_EXECUTOR_PROMPT` / `CODE_ANALYZE_STEPS` 在 `phase_table_test.rs` 覆盖（断言静态文本非空、零占位、与步 `prompt_ref` 同源） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/code-analyze-evaluator.md` | 同上（经 `CODE_ANALYZE_EVALUATOR_PROMPT` / Evaluator 步装配覆盖） |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `all_commands!` 宏登记（`update_explore_workflow_type`）无独立单测面；其正确性由 `bindings:export` / `bindings:check` 一致性守卫（desktop 侧 static-check）验证 |
| `packages/desktop/src/types/generated/bindings.ts` | 经 tauri-specta 再生的 codegen 产物（`create_change` 四参 / `promote_explore` 三参 / `update_explore_workflow_type` / `ExploreRecord.workflowType`）；由 `bindings:check` 一致性守卫拦截漂移，非手改不可单测 |
| `packages/desktop/package.json` | 配置版本 `0.4.40 → 0.4.41`（归档时执行）；无运行时逻辑可单测，由归档时静态校验承接 |
