# 任务: desktop-change-db-identity

> **变更**: desktop-change-db-identity
> **依据**: proposal.md + design.md（D1–D14 决策编号见 design）

任务边界：本列表只含实现任务；**新增测试锚**（id 回环读写、旧形态库作废重建、表名版本段与旧表不可见断言、db 单源列表与磁盘零发现、未知 id 未找到、13 命令 id 寻址、注册表键与 provenance 逐字一致、前端单身份源 / `flow-empty` 退役）由 test-design / test-gen / test-execution 阶段承接——但类型 / 载荷 / 签名演进触及既有测试文件编译面，本列表阶段内含**机械随动**（改签名 / 改基设，不写新断言）。**语料 id 归键、文档形态样本退役改造与 golden 显式重写（`DESKTOP_GOLDEN_REWRITE=1` 流程 + diff 人工确认留痕）同归测试阶段承接，范围声明见 design D13**。任务内文件均在 design.md 变更清单内。

只读红线：`plugins/dev-team` 全部；磁盘 / git 的 name 化面（change 目录名、worktree 落位与 branch `change/<name>`、归档目录 `YYYY-MM-DD-<name>` 前缀与后缀扫描判定）；存量 CLI change 的磁盘目录与 workflow.json（零触碰）；会话转录落库机制 / StopRegistry / `agent_sessions` 查询语义；相位机语义（校验 / 白名单 / 重试预算）与 run 双写时序；通知信封 `RunNotice`；存档面 DTO 字段面；主基线 specs。

## 阶段一：store 模型 + 打开路径（infra）

- [x] `crates/infra/store/src/model.rs`：`ChangeRecord` 升 v3（`#[native_model(id = 9, version = 3)]`）——`id: String` 主键（uuid 形态身份锚，`#[primary_key]`）+ `name: String` 普通属性（无唯一约束，注释注明「恒裸名 / 可变 / MUST NOT 作身份键」）；`new` 构造携 id 入参；`ChangeRecordV1` 及双向 `From` 整体删除；`PhaseRecord` / `StepRecord` / `RunRecord` 升 v2（`change` → `change_id`，`#[secondary_key]`，注释「名称引用非外键约束」改「指向 ChangeRecord.id」）；`StoreMetaRecord` 新增（`#[native_model(id = 15, version = 1)]`，`key` 主键 + `format_version: u32`，`pub(crate)`）+ `WORKSPACE_STORE_FORMAT_VERSION: u32 = 2` 常量（D3 / D5 / D6，注释注明语义编号与「缺失即旧形态」判定）
- [x] `crates/infra/store/src/store.rs`（打开路径）：`open_workspace` 增格式版本探测与旧库作废重建（D3 / D4 定形逐字）——文件缺失 / 空文件分支：create 新库 + 写标记；存量文件分支：open → 读标记（`StoreMetaRecord` primary `"format"`）：== 2 → 就绪；缺失 / < 2 → drop 句柄 → `fs::remove_file` → create 新库 → 写标记；删除失败 → `StoreError::Db`（环境故障语境）；就绪后照常 `calibrate_interrupted_runs`；`workspace_models()` 注册 `StoreMetaRecord`（十模型 → 十一模型，模块注释随动）；`open_global` 与既有「旧文件惰性废弃」条款注释零触碰（D5 衔接措辞写入打开路径文档注释）
- [x] `crates/infra/store/src/store.rs`（change 域操作面）：全操作面 id 形参——`find_change_record(id)` / `delete_change_record(id)` / `set_change_archived(id, …)` / `start_change_phase(change_id, …)` / `amend_change_decision_session(change_id, …)` / `list_phase_records(change_id)` / `list_change_steps(change_id, …)` / `list_change_runs(change_id)`；`create_change_record` 改同 id 防御拒绝（`StoreError::Conflict`，MUST NOT 静默覆写）且**删除同名检查**（D11——同名 active 拒绝归写面前置）；`log_change_phase` / `finish_change_run` / `calibrate_interrupted_runs` 按 `change_id` 定位记录（事务内 join / active_phase 清位语义不变）；`start_change_phase` 的 attempt 推导扫描过滤改 `change_id`
- [x] `crates/infra/store/src/change_port.rs`：`impl ChangeStateStore for Store` 全方法 id 形参委托随动（映射语义零改动）
- [x] `crates/infra/store/src/envelope.rs`：`change_key` 投影 `value["name"]` → `value["id"]`（`StoreMetaRecord` 不入 `MODEL_ENTRIES`，D3 注释锚）
- [x] 编译面机械随动：`model_test.rs`（`ChangeRecordV1` 用例组删除 / 表名版本段基设改）/ `store_test.rs` / `change_port_test.rs` / `envelope_test.rs` 既有用例保编译（作废重建 / 表名断言 / id 回环断言留测试相位）

## 阶段二：workflow 状态缝 + 写面 + 查询（core）

- [x] 依赖面（增补）：`packages/desktop/src-tauri/Cargo.toml` `[workspace.dependencies]` 增 `uuid = { version = "1", features = ["v7"] }`；`crates/core/workflow/Cargo.toml` `[dependencies]` 增 `uuid = { workspace = true }`（D1——铸出点单点消费，注释注明非 infra 面）
- [x] `crates/core/workflow/src/state.rs`：`ChangeStateRecord` 增 `id: String`（首字段，身份锚注释）；`PhaseStateRecord` / `StepStateRecord` / `RunStateRecord` 与 `PhaseLogCommand` / `BacktrackCommand` / `StepCommand` / `RunStartCommand` / `RunFinishCommand` 载荷 `change → change_id`；`ChangeStateStore` trait 全方法 id 形参（`get_change(id)` / `list_phase_records(change_id)` / `list_steps(change_id, …)` / `delete_change_record(id)` / `start_phase(change_id, …)` / `amend_decision_session(change_id, …)` / `set_archived(id, …)` / `list_runs(change_id)` 等）；`get_change` 文档注释「None = 文档形态」语义删除（未知 id 语义）
- [x] `crates/core/workflow/src/write/create.rs`：前置④ 改 name 扫描查重（`list_change_records` 过滤 `name == 入参 && status == Active` → 拒绝；归档同名不拒，D11）；前置七道全过后铸 `uuid::Uuid::now_v7().to_string()` 入 `ChangeStateRecord.id`（D1，铸出点注释锚）；`CreateOutcome` 增 `id`（首字段）；补偿链 `compensate_record_delete` 改按 id
- [x] `crates/core/workflow/src/write/archive.rs`：`archive(layout, store, id)`——`get_change(id)` 读记录（None → 既有无建档拒绝面）、`record.name` 供给 active 目录改名 / archive 树定位（后缀扫描与单分量校验施于解析出 name）；id / name 不随改名变；worktree merge-first 引导分支语义不变；错误文案随 id 语境修订（呈现记录名）
- [x] `crates/core/workflow/src/write/phase_start.rs` + `phase_log.rs` + `backtrack.rs` + `decision_log.rs`：形参 `change → change_id`；内部 `get_change(change_id)` 读记录（workflow_type / 表位校验语义零改动）
- [x] `crates/core/workflow/src/write/phase_next.rs`：`phase_next(store, change_id, run_id, anchors)`；prompt 插值改 `record.name`（`interpolate(&spec.prompt, &record.name, …)`）；`SessionAnchors` 键 `(change_id, run_id)`；`run.rs` 两函数读记录面随动
- [x] `crates/core/workflow/src/queries/list.rs`：db 单源——`scan_dir_names` / `scan_archive_dirs` / `locate_prefixed_archive_name` / 磁盘半边 `archive_month` 回退段删除；`list_changes(store: &dyn ChangeStateStore) -> ChangeList`（去 `Layout` 形参）；`ChangeSummary` 增 `id`（首字段）、`name` 取 `record.name`（恒裸名）；月分组 `archived_at` 唯一权威（缺失归「未知时间」置尾）；`source` 自 status 派生；模块注释重写（零磁盘触点）
- [x] `crates/core/workflow/src/queries/detail.rs`：`change_detail(layout, store, id)`——`get_change(id)`；record `None` → 恒返回 `None`（文档形态分支 / 空流水线面删除，D7 / AC-4）；`locate_change(layout, record.worktree, record.name)`；`ChangeDetail` 增 `id`、`name` 自记录直读；`created` 磁盘前缀回退删除；`queries/mod.rs` 的 `locate_change` 消费注释与单分量校验措辞随动
- [x] 编译面机械随动：workflow crate 内实现 `ChangeStateStore` 的测试假件（写面 / 查询既有 fake）补 id 形参并将载荷字段改 `change_id`，`cargo check -p workflow` 过（断言更新留测试相位）

## 阶段三：orchestration——注册表 / provenance / 载荷

- [x] `crates/core/orchestration/src/control.rs`：注册表键 `(root, change)` → `(root, change_id)`——`begin_run` / `subscribe` / `request_stop` / `current_session` / `publish` / `set_session` / `answer` / `confirm` / `snapshot` / `take_pending` 全方法签名与键构造点随动（proposal 口径 12 处）；头注释「复合键 = (workspace root, change)」措辞改 change id 语义
- [x] `crates/core/orchestration/src/archive_flow.rs`：`ArchiveControl` / `ArchiveGuard` 键 `(root, change_id)`（全方法随动）；`ArchiveRequest.change → change_id`；`drive` 前置段读记录后一次解析 `record.name`（branch `change/<name>`、`spec_sync_prompt` / `merge_conflict_prompt` 内文、`missing_artifacts` / `detect_delta_specs`、pathspec 圈定与 `archived_dir` 组装、`ArchiveSummary.name` 全用 name；prompt 模板措辞零改动）；两会话 provenance `source_ref = <id>/archive/spec-sync` 与 `<id>/archive/merge-conflict`；末段 `workflow::write::archive` 调用改传 id 且经 id → 记录 → name 供给双写语义；`preflight(main_root, store, id, vcs, run_active)`（worktree 自记录读）
- [x] `crates/core/orchestration/src/walker.rs`：`RunRequest.change → change_id`；provenance `source_ref = <id>/<phase>/<role>/<attempt>`（`run_worker` 组装点；模块 / 函数注释定式同步）；`ToolCommand` 构造点 / `snapshot.detail` 调用点随动（每 run 两写时序零改动）
- [x] `crates/core/orchestration/src/port.rs`：`ToolCommand` 六变体载荷 `change → change_id`；`StaticCheck` 增 `change_id: String`（D12）；`WorkflowSnapshotPort::detail(root, id)`；`TestExecutionRunner::run` 形参改名 `name` + 文档注释「磁盘面 port 收 name，解析在 steps 消费点」（D7）
- [x] `crates/core/orchestration/src/steps.rs`：各臂载荷 `change_id` 透传写面；`audit_outcome` 审计行 `change_id` 归键（`StaticCheck` 空串占位退役，D12）；`TestExecution` 臂调用前经 `store.get_change(change_id)` 解析 `record.name` 后调 runner（未建档 → `Err`）
- [x] `crates/core/orchestration/src/run_history.rs` + `snapshot.rs`：`finish_command` 载荷 `change_id`（过滤单点语义零改动）；`snapshot.rs` `detail(root, id)` 形参与 `change_detail` 调用随动
- [x] 编译面机械随动：`control_test.rs` / `archive_flow_test.rs` / `walker_test.rs` / `port_test.rs` / `steps_test.rs` / `run_history_test.rs` / `snapshot_test.rs` 既有用例保编译（签名 / 载荷 / fake 构造随动；注册表键与 provenance 逐字断言留测试相位）

## 阶段四：命令面 + bindings

- [x] `src/commands/changes/mod.rs`：`get_change_detail` / `read_artifact` / `archive_change` 定位参数 `change → id`（参数名 `id`；blank 检查文案随 id 语境）；`read_artifact` 经 `find_change_record(id)` 供给 worktree / name；`list_changes` core 调用改 `queries::list_changes(store)`（去 layout 解析）；`create_change` 零签名改动（`CreateOutcome.id` 随 DTO 自动出线）；模块头注释（含「文档形态」措辞）随动重写
- [x] `src/commands/change_flow/mod.rs`：五命令定位参数 `id`；装配（建档 / workflow_type / exec root / 归档互斥 / blank 文案）经 id 读记录；`ChangeFlowSink` / `begin_run` / `subscribe` 键 id；`RunRequest` 构造随动
- [x] `src/commands/archive_flow/mod.rs`：五命令定位参数 `id`；`ArchiveSink` / `ArchiveControl` 调用键 id；前置校验（建档 / status=active / run 互斥快照）经 id 读记录
- [x] bindings 再生成：`pnpm -C packages/desktop run bindings:export` 落 `src/types/generated/bindings.ts`（13 命令 id 参数 + `ChangeSummary.id` / `ChangeDetail.id` / `CreateOutcome.id`）；一致性守卫（`bindings:check`）拦截漂移
- [x] 编译面机械随动：`src/commands/changes/mod_test.rs`（含归档条目名断言段改写）/ `change_flow/mod_test.rs` / `archive_flow/mod_test.rs` 既有用例保编译（id 寻址断言重写留测试相位）

## 阶段五：前端——路由 / 行键 / hooks / sourceRef

- [x] `packages/desktop/src/routes.tsx`：`/changes/:name` → `/changes/:id`
- [x] `packages/desktop/src/views/changes/change-list-view.tsx`：`ChangeRow` 行 `key` 与 `onSelect`/`navigate` 取 `summary.id`；行展示取裸名；`onCreated` 改携 id 导航；空态文案随 db 单源修订；`change-create-dialog.tsx` 成功回调改 `onCreated(result.id)`（`onCreated: (id: string) => void`）
- [x] `packages/desktop/src/views/changes/change-detail-view.tsx`：`useParams<'id'>`；页面数据链标识符 id 语义化（`selected → id`）；`FlowSection` 单态呈现（`flow-empty` 分支与「（文档形态：未建档，仅产物清单）」文案删除，D10 / AC-4）；`DetailLoaded` 面板传参收敛——`changeId`（URL id）+ `name`（取自 `detail.name`）
- [x] `packages/desktop/src/views/changes/hooks/` 四文件：`use-change-detail.ts`（`useChangeDetail(root, id)`，`loadUnifiedView` / `readArtifactSafe` 命令参数随动）；`use-change-flow-run.ts`（`{ root, id, … }`，五命令调用随动）；`use-archive-flow.ts`（`{ root, id, … }`，五命令 / 快照 / 补订随动）；`use-change-list.ts`（DTO id 消费随 bindings 自动跟随，注释随动）
- [x] `packages/desktop/src/views/changes/flow/detail-drawer.tsx`：prop `change → changeId`；`selectionRoleRefs` 三处 sourceRef 定式组装身份段改 changeId（`<id>/<phase>/<role>/<attempt>` 与 active 三 role 反查同式）；`hooks/use-session-transcript.ts` 与 `flow/types.ts` 注释措辞随动（查询逻辑 / 类型面零改动）
- [x] `packages/desktop/src/views/changes/flow/run-control-panel.tsx` + `archive-panel.tsx`：props `change` 拆 `changeId`（命令面）+ `name`（展示面：aria-label / 标题 / 确认对话文案取 detail.name）；命令透传 id；data-testid 零改号
- [x] 前端测试机械随动：`views/changes/**.test.ts(x)` + `src/app.test.tsx` 基设改新契约保编译（路由 id / 行键 / sourceRef / `flow-empty` 退役断言留测试相位）；knip 零未用导出残留

## 阶段六：版本与守线收口（静态，不含测试执行）

- [x] `packages/desktop/package.json` `version` 0.4.30 → 0.4.31（D14：`src-tauri/tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动）
- [x] `pnpm -C packages/desktop run server:check`（cargo fmt + clippy）零错误零新告警；`pnpm -C packages/desktop run client:check`（vp check + knip）零错误且 knip 零新增豁免；`bindings:check` 绿
- [x] 红线自查 grep：`plugins/dev-team` 零 diff（版本 2.10.44、三类交付产物）；磁盘 / git name 化面零触碰（change 目录名、worktree 落位、branch `change/<name>`、归档前缀与后缀扫描）；`RunNotice` 与存档面 DTO 定义零 diff；`workflow.json` 零读写触点复核；主基线 specs 零改动；相位机语义与 run 双写时序零 diff
- [x] 变更清单对账：实现文件与 design.md 变更清单逐项对账（无清单外改动、无清单内遗漏；design「增补」三项含入：`Cargo.toml` ×2、`change-create-dialog.tsx`、`flow/types.ts`）；AC-11 版本交付 0.4.31 已执行
