# 任务: desktop-workflow-db-state

> **变更**: desktop-workflow-db-state
> **依据**: proposal.md + design.md（D1–D12 决策编号见 design）

任务边界：本列表只含实现任务；测试编写与语料重构（db 种子夹具 / `corpus_golden_test.rs` 重写 / golden 显式再生成复核 / fake port 断言改写）由 test-design / test-gen / test-execution 阶段承接。任务内引用的文件均在 design.md 变更清单内。

## 阶段一：状态缝与 store 四模型（加性落地，独立可编译）

- [x] 新增 `crates/core/workflow/src/state.rs`：`ChangeStateStore` trait（读半边 `get_change` / `list_change_records` / `list_phase_records` / `list_steps`，写半边 `create_change_record` / `start_phase` / `log_phase` / `apply_backtrack` / `amend_decision_session` / `set_archived` / `append_step`）与中性类型（`ChangeStateRecord` / `ActivePhaseState` / `PhaseStateRecord` / `StepStateRecord` / `StepKind` / `ChangeStatus` / `PhaseStartState` / `PhaseLogCommand` / `BacktrackCommand` / `StepCommand` / `StoreFault`），时间戳统一 i64 unix millis（design D1）；`crates/core/workflow/src/lib.rs` 挂 `pub mod state`
- [x] `crates/infra/store/src/model.rs`：落 `ChangeRecord`（id=9）/ `PhaseRecord`（id=10）/ `ChecklistItemRecord`（id=11）/ `StepRecord`（id=12）四模型（native_model version=1，默认 bincode）；打包键 `(phase_id as u128) << 64 | item_index` 及十六进制字符串 serde 定制（沿 `event_key_serde` 先例）；`ChangeActivePhase` 嵌套结构与各构造器（design D3/D4）
- [x] `crates/infra/store/src/store.rs`：`workspace_models()` 注册组 4→8（四新模型入组）；change 域操作面（design API 表 `Store::*` 十方法）——建档同名 active 查重、`log_change_phase` 单写事务原子（PhaseRecord 行 + checklist 子行 + `active_phase` 清位 + attempt 事务内推导与 `(change, phase, attempt)` 查重）、`apply_change_backtrack` 单事务（回跳标记 + 目标最新 pass 置 stale + `stale_dependents` 全条目置 stale）、amend / archived 翻转 / 步骤追加（id max+1）与查询；记录 ↔ `workflow::state` 中性类型映射收本文件单点（design D2）
- [x] `crates/infra/store/src/change_port.rs`：`impl ChangeStateStore for Store` 委托适配；`crates/infra/store/src/lib.rs` 挂 `mod change_port`；`crates/infra/store/Cargo.toml` `[dependencies]` 增 `workflow`

## 阶段二：core/workflow 写面 db 化

- [x] `crates/core/workflow/src/write/phase_next.rs`：签名改 `phase_next(store: &dyn ChangeStateStore, change, run_id, anchors)`；读源改 `list_phase_records` + `get_change`；锚点基线平移为 PhaseRecord 行数（design D8）；`LastResult.timestamp` → `Option<i64>`；round 上限 / 重试上限 / backtrack 检测 / prompt 插值 / 白名单语义逐项对照保持
- [x] `crates/core/workflow/src/write/phase_start.rs`：签名改 `phase_start(store, change, phase)`；表位校验保留后调 `store.start_phase`；`PhaseStartOutcome.start_at` → `i64`
- [x] `crates/core/workflow/src/write/phase_log.rs`：签名改 `phase_log(store, change, input)`；verdict 推导 / skipped-pass 约束 / report ≤2000 / 表位 / active_phase 匹配校验前置保留；持久化改 `store.log_phase`（checklist 信封 + 三会话槽位随行）；`PhaseLogInput` / `PhaseLogOutcome` 形状不变
- [x] `crates/core/workflow/src/write/backtrack.rs`：签名改 `backtrack(store, change, input)`；白名单二次校验 / 双端表位 / reason ≤500 保留；stale 闭包计算（目标最新 pass + `phase_table::dependents` BFS）自 persist 迁入；持久化改 `store.apply_backtrack`
- [x] `crates/core/workflow/src/write/decision_log.rs`：签名改 `decision_log(store, change, phase, session_id)`；持久化改 `store.amend_change_decision_session`（amend 幂等语义不变，design D9）
- [x] 删除 `crates/core/workflow/src/write/persist.rs`；`crates/core/workflow/src/write/mod.rs` 导出面收敛（`persist` 移除、`archive` 加入）
- [x] `crates/core/workflow/src/write/create.rs`：签名改 `create(layout, store, name, goal)`；冲突双检查（目录已存在或 db 同名 active 记录，全 IO 前置）→ `store.create_change_record` 建档先行 → `create_dir_all` + explore.md（goal 原文）→ fs 失败补偿删本次建档（design D5）；workflow.json 初始文档写出段删除；`CreateOutcome.created` 改取 db `created_at` 日期
- [x] 新增 `crates/core/workflow/src/write/archive.rs`：`archive(layout, store, change)` 归档双写——db 建档校验（无建档显式拒绝）→ active 树定位 → 目标 `YYYY-MM-DD-<name>` 冲突预检 → 目录改名 → `store.set_archived`；改名成功而翻转失败走 Err 半完成态，重试经「archive 树前缀命中 + db 仍 active」分支仅补翻转（design D6）；`ArchiveOutcome { name, archived_date }`

## 阶段三：core/workflow 读面与 parse 退役

- [x] `crates/core/workflow/src/artifacts/registry.rs`：`discover_artifacts` / `read_artifact` 入参 `(Inventory, Option<&Workflow>)` → `&[PhaseStateRecord]`；eval-checklist 候选锚定自 `Workflow.eval` 下标平移到 PhaseRecord 序列；matcher 注册表与信封机制零改动
- [x] `crates/core/workflow/src/queries/mod.rs`：`locate_change` 增 archive 树日期前缀后缀匹配（db 名 `foo` ↔ `YYYY-MM-DD-foo`）；单分量名校验保留
- [x] `crates/core/workflow/src/queries/list.rs`：签名改 `list_changes(layout, store)`；条目集合 = db 记录 ∪ 磁盘目录去重并集（同名以 db 为准）；`ChangeSummary` 删 `inventory` / `unparsable`、增 `status: Option<ChangeStatus>` / `active_phase`；归档按月分组（db 取 `archived_at`、磁盘回退目录前缀、无前缀入未知时间组置尾）；缺目录空结果不报错
- [x] `crates/core/workflow/src/queries/detail.rs`：签名改 `change_detail(layout, store, name)`；流水线自 PhaseRecord / ChecklistItemRecord（打包键序）重组；`ChangeDetail` 删 `inventory` / `unparsable` / `file_log`；`AttemptRecord` 槽位三列直读透出（None → null）；时间出线 ISO 串 + null 收本层单点（i64 millis → RFC3339）；db 缺记录返回空流水线 + 产物清单（零 workflow.json 读取）
- [x] `crates/core/workflow/src/model/domain.rs`（新）：`Verdict` / `ChecklistItem` 自磁盘模型迁入；`crates/core/workflow/src/model/mod.rs` 导出收敛；删除 `crates/core/workflow/src/model/workflow.rs` 与 `crates/core/workflow/src/model/inventory.rs`
- [x] 删除 `crates/core/workflow/src/parse/` 目录（`mod.rs` / `detect.rs` / `detect_test.rs` / `workflow_file.rs` / `workflow_file_test.rs`）；删除 `crates/core/workflow/tests/generation_parse_test.rs`
- [x] `crates/core/workflow/src/lib.rs`：模块声明收敛（`parse` 移除、`state` 在位、`model` 导出面跟随）

## 阶段四：orchestration 接线

- [x] `crates/core/orchestration/src/steps.rs`：`LocalToolSteps` 增 `store: Arc<dyn ChangeStateStore>` 与 run 级 `run_id` 字段；七个命令臂（PhaseNext / PhaseStart / PhaseLog / Backtrack / DecisionLog / StaticCheck / TestExecution）包络 StepRecord 审计落库（`StepCommand`，summary ≤500 截断留痕、reference 携 checks 报告目录或会话 id，成功与失败皆落行）；相位机四步写面调用签名跟随；`ToolStepPort` 直调面不变
- [x] `crates/core/orchestration/src/snapshot.rs`：`FsSnapshot` → `StoreSnapshot { root, store }`，`detail` 经 `workflow::queries::change_detail` db 读源；「unparsable 显式 Err」分支删除；`WorkflowSnapshotPort` 契约不变

## 阶段五：壳层命令接线

- [x] `src-tauri/src/commands/changes/mod.rs`：list / detail / read_artifact / create 经 `WorkspaceStores::for_root(root)` 取 store 接线新签名；新增 `archive_change` 命令（`AppHandle` + 三件事纪律 + `archive_change_with<R: Runtime>` 泛型测试缝，design D11 无 UI 入口）；组文档注释更新
- [x] `src-tauri/src/commands/change_flow/mod.rs`：发起前置校验改 db 建档校验（`find_change_record` 在案 + `phase_table(workflow_type)` 在位；「workflow.json 可解析」校验删除）；组合根向 `LocalToolSteps::new` / `StoreSnapshot::new` 注入 `for_root` store 与 run_id；停止 / 应答 / 确认 / watch 命令不动
- [x] `src-tauri/src/commands/mod.rs`：`archive_change` 进命令注册宏清单

## 阶段六：前端退役面

- [x] `packages/desktop/src/views/changes/change-list-view.tsx`：`InventoryBadge` 与「无法解析」标注删除；条目状态面改 `status` / `active_phase` 消费；文档形态条目照常入列
- [x] `packages/desktop/src/views/changes/change-detail-view.tsx`：代际徽章映射、`UnparsableNote`、`WorkflowPanel` 删除；两态分流改 `status` 在场判别（建档 = 完整状态面 / 缺席 = 文档形态空图占位）；error / loading / 未找到降级页沿用
- [x] `packages/desktop/src/views/changes/flow/detail-drawer.tsx`：右列三分节改二分节（本站文档 | eval report+checklist），文件表节与 `FileLogTable` 引用、`hasFileLog` 链路删除；左列会话区、双列滚动、单一交互入口不变
- [x] `packages/desktop/src/views/changes/flow/attachments.ts`：file_log 挂节点分支与 `scope='workflow'` 图外素材分支删除；文档挂列映射表与 eval-checklist 挂节点保持
- [x] `packages/desktop/src/views/changes/flow/types.ts`：`FileLogEntry` 派生类型与 `nodeFiles` / `outsideFiles` 挂载状态字段删除
- [x] 删除 `packages/desktop/src/views/changes/flow/file-log-table.tsx`（随退役，其测试文件同轮删除）
- [x] `packages/desktop/src/types/generated/bindings.ts`：经 `pnpm -C packages/desktop run bindings:export` 重导出（DTO 三字段删除 + 状态面 / 槽位类型跟随）

## 阶段七：语料与 golden 处置

- [x] 删除 `crates/core/workflow/tests/fixtures/` 下 13 个 workflow.json 语料目录（v0-a / v0-b / v1-a / v1-b / v1-c / v2-a / v2-b / v3-a / corrupt-* 五族）与 `crates/core/workflow/tests/golden/` 对应 change golden 快照（design D12 处置定稿 = 删除）；`fixtures/README.md` 改写为 db 种子语料矩阵说明（覆盖面清单：多 attempt / backtrack stale / 槽位全缺 / 文档形态 / 坏行——新语料构造与 golden 再生成由 test-design / test-gen 阶段承接）

## 阶段八：守线收口（静态，不含测试执行）

- [x] `pnpm -C packages/desktop run server:check`（cargo fmt + clippy）零错误；`crates/core/workflow/Cargo.toml` 审查确认零 infra/store 依赖（AC-2 红线，写面 sync 签名无 tokio / Tauri）
- [x] 双向墙机械自查：全 desktop 源码（core / infra / commands / views）grep `workflow.json` 读 / 写触点零命中（测试夹具字面量除外）；`crates/core/workflow/src/parse/` 目录不存在；desktop 全链 `file_log` 触点零命中
- [x] `pnpm -C packages/desktop run client:check`（vp check --fix + knip）零错误且 knip 零新增豁免；退役词汇（`inventory` / `unparsable` / 「无法解析」/ `fileLog`）在清单 / 详情视图源码零残留
- [x] `pnpm -C packages/desktop run bindings:check`（导出 + `git diff --exit-code -- src/types/generated`）绿
- [x] 变更清单核对：实现文件与 design.md 变更清单逐项对账（无清单外改动、无清单内遗漏）；AC-10 版本交付（`packages/desktop/package.json` 0.4.12 → 0.4.13，`src-tauri/Cargo.toml` 不随动）留待归档时执行，不在本阶段
