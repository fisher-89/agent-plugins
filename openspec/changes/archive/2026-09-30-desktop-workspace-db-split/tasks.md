# 任务: desktop-workspace-db-split

> **变更**: desktop-workspace-db-split
> **日期**: 2026-09-29

任务按依赖排序：阶段一（store 双库基建）是阶段二（命令面）的前置，阶段二先于阶段三（前端接线，消费再生成绑定），阶段四收尾依赖前三阶段全部落地。测试编写与执行归 test-design / test-gen / test-execution 阶段，不在本列表。

## 阶段一：store crate 双库基建

- [x] `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：维度归属文档注释修正——`ExploreRecord` 由「user 维度 / 落 app data dir」措辞改为 workspace 维度、落所属 workspace 库，`AgentRunRecord` / `AgentEventRecord` 同步补落位说明；模型 struct 字段面与 native_model id / version 零变化
- [x] `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：新增 `DbDimension` 枚举（`User` / `Workspace`，derive `Serialize` / `Deserialize`（`rename_all = "lowercase"`）+ `specta::Type` + `Copy`）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：`models()` 拆为 `global_models()`（仅 `WorkspaceRecord`）与 `workspace_models()`（`AgentRunRecord` / `AgentEventRecord` / `ExploreRecord`）两组 `OnceLock` 静态，无交叉注册
- [x] `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：`Store::open` 拆为 `open_global(path)` / `open_workspace(path)` 双命名构造器（私有 `open_with(path, models, dimension)` 收口；注入式单文件句柄与「空文件视同不存在」逻辑不变；实例私有携带维度）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：workspace 库路径派生单点——`workspace_db_file_name(canonical_root: &str) -> String`（可读段取 `dir_name` 清洗截断 ≤24 字符 + SHA-256 前 16 字节 32 位小写 hex，可读段为空回退纯哈希；文件名不含路径分隔符）与 `workspace_db_path(workspaces_dir: &Path, canonical_root: &str) -> PathBuf`；常量 `GLOBAL_DB_FILE_NAME = "desktop-global.redb"`、`WORKSPACES_DIR_NAME = "workspaces"` 同置此单点
- [x] `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：新增 `WorkspaceStores`——`open(data_root: &Path)`（打开全局库 + 记录 workspaces 子树根）、`global() -> &Store`、`for_root(root: &str) -> Result<Arc<Store>, StoreError>`（per-root `Mutex<HashMap<String, Arc<Store>>>` 缓存复用，进程生命周期常开；`Send + Sync` 编译期断言随 `Database<'static>` 断言同置）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs`：`ModelEntry` 注册表行增 `dimension: DbDimension` 标签；`list_models` / `scan` 内部分发按实例维度过滤，跨维度模型名按「未知模型」`Err`；`ModelInfo` / `RecordEnvelope` 外形不变
- [x] `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`：导出 `WorkspaceStores` / `DbDimension`；crate 文档由「单库四模型」改写为双库布局（含单进程约束与「不做旧库兼容 / 零迁移」立场留痕）
- [x] `packages/desktop/src-tauri/crates/infra/store/Cargo.toml` + `packages/desktop/src-tauri/Cargo.toml`：store 增 `sha2 = { workspace = true }` 直依赖，workspace 级 pin `sha2 = "0.10"`（锁内既有传递依赖，无新包入树）

## 阶段二：desktop-app 壳与命令面

- [x] `packages/desktop/src-tauri/src/main.rs`：`DB_FILE_NAME` 常量演进为 `DATA_DIR = ".dev-team"` 数据根；setup 改 `WorkspaceStores::open(&data_root)` + `app.manage(stores)`；全局库打开失败 fail fast 口径不变；`WatchRegistry` / `RunStopRegistry` 挂载不变
- [x] `packages/desktop/src-tauri/src/commands/workspaces/mod.rs`：`State<'_, Store>` 切 `State<'_, WorkspaceStores>`；`list_workspaces` / `add_workspace` / `remove_workspace` 经 `global()` 操作注册表；`add_workspace_inner` 注册成功后 `for_root(&record.root)` 预开校验（坏文件注册时 `Err` 暴露）；`remove_workspace_inner` 仅删注册记录（db 文件与缓存实例保留）；IPC 入参面不变
- [x] `packages/desktop/src-tauri/src/commands/explores/mod.rs`：`State` 切 `WorkspaceStores`；`scan_explores` 与 `list_explore_records` / `create_explore_record` / `rename_explore_record` / `delete_explore_record` 经 `for_root(&root)` 路由至 workspace 库；blank root 纪律落地（查询空结果保持现状，写命令 blank root `Err`）；命令签名不变
- [x] `packages/desktop/src-tauri/src/commands/exec/agent.rs`：`RunStopRegistry` 句柄表键 `HashMap<i64, RunHandle>` → `HashMap<(String, i64), RunHandle>`，`register` / `request_stop` / `remove` 增 root 参；`start_agent_run` / `start_agent_run_with` 改收 `&WorkspaceStores`，同步段 `for_root` 预解析 `Arc<Store>` 供后台任务持有收尾；`drive_agent_run` 签名不变（仍收 `&Store`）；能力 spec 指针注释沿相对域根定式（`specs/desktop-agent-execution/spec.md`，路径相对域根）
- [x] `packages/desktop/src-tauri/src/commands/exec/mod.rs`：`agent_start` blank root `Err`、落库经 `for_root` 至当前 workspace 库（提前 resolve 契约与 tee 双 sink 不变）；`agent_stop` 增 `root: String` 入参并按复合键寻址（miss 幂等 `Ok`）；`agent_runs` / `agent_run_events` / `agent_run_chain` 增 `root: String` 入参（blank root 空结果）；`Result<T, String>` 错误模板不变
- [x] `packages/desktop/src-tauri/src/commands/db/mod.rs`：`db_models` / `db_records` 增 `scope: DbDimension` 与 `root: String` 入参；`User` 走 `global()`、`Workspace` 走 `for_root(&root)`（blank root 空结果）；轨道纪律与信封调用面不变

## 阶段三：前端接线

- [x] `packages/desktop/src/hooks/use-agent-chat.ts`：`loadChain` 增 root 参（`commands.agentRunChain(root, source, sourceRef)` / `commands.agentRunEvents(root, run.id)`）；`stop` 携 root（`commands.agentStop(root, runId)`）；hook 对外签名与镜像 / 重放语义不变
- [x] `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts`：`useAgentRunHistory(root: string | null)` 增 root 入参，两条取数轨道透传；root 为 null 跳过 invoke 保持空态
- [x] `packages/desktop/src/views/agent/agent-debug-view.tsx`：root 透传 `useAgentRunHistory(root)`
- [x] `packages/desktop/src/views/db/hooks/use-db-inspector.ts`：`useDbInspector(root: string)` 增 root 入参与 scope 态（默认 workspace 库，`DbInspectorState` 暴露 `scope` / `setScope`）；invoke 更新为 `commands.dbModels(scope, root)` / `commands.dbRecords(scope, root, model, offset, PAGE_SIZE)`；scope / root 变更重置选中模型与分页并重取清单
- [x] `packages/desktop/src/views/db/db-inspector-view.tsx`：接收 `root: string` prop；模型区头部增 scope 双 tab 按钮组（「全局库」/「workspace 库」，`aria-pressed` 同 agent 页切换行模式）；空态与 inline 持久错误态不变、零写入口、无轮询
- [x] `packages/desktop/src/routes.tsx`：`/db` 路由向 `DbInspectorView` 传 `root={root}`（级联接线）
- [x] `packages/desktop/src/views/explores/hooks/use-explore-session.ts`：透传面核对——root 已为入参且新入参全部由 `useAgentChat` 承载，预期零修改；若绑定参数序波及则同步

## 阶段四：bindings 与版本收尾（静态，不含测试执行）

- [x] `pnpm -C packages/desktop run bindings:export` 再生成 `packages/desktop/src/types/generated/bindings.ts`，核对新增 root / scope 参数面与 `DbDimension`（`"user" | "workspace"`）类型出线
- [x] `packages/desktop/package.json`：`version` 0.3.8 → 0.3.9（用户可见行为变更：数据隔离 + 运行清单范围收窄 + 存量数据废弃）
- [x] 守线（静态）：`pnpm -C packages/desktop run server:check`（cargo fmt + cargo clippy 零告警）
- [x] 守线（静态）：`pnpm -C packages/desktop run client:check`（vp check --fix + knip 零未用导出）
- [x] 守线（静态）：`pnpm -C packages/desktop run bindings:check`（生成物零 diff 边界检查）

以上守线均为静态检查；测试执行由 test-execution 阶段承接。
