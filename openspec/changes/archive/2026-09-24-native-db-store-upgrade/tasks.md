# 任务: native-db-store-upgrade

> 依赖顺序：依赖地基 → store 操作面 → legacy 迁移 → 信封 API → 命令轨道 → 前端 → 收尾。
> 每阶段内任务按序执行；跨阶段后置任务依赖前置产出。

## 阶段 1：依赖地基与 store 模型层

- [x] `packages/desktop/src-tauri/Cargo.toml`：`[workspace.dependencies]` 增 `native_db`（精确 pin crates.io 当前 0.8.x 最新补丁版，证据基线 0.8.2）与 `native_model = "=0.4.20"`（与 native_db 内部 pin 一致）；`redb = "=4.3.0"` 保留不动
- [x] `packages/desktop/src-tauri/crates/infra/store/Cargo.toml`：dependencies 增 `native_db` / `native_model` / `agent`（均 workspace）；`redb` 保留并注释「迁移期专用（读旧库），由后续变更收掉」；crate 级依赖注释改为「仅依赖 `agent` 纯类型作嵌装载荷」
- [x] `crates/infra/store/src/model.rs`：`WorkspaceRecord`（`#[native_model(id = 1, version = 1)]` + `#[native_db]`，PK `root`）与 `AgentRunRecord`（id = 2，PK `id`）平移挂载 derive，删除手写 `encode` / `decode`，字段面与 serde camelCase 零变化
- [x] `crates/infra/store/src/model.rs`：新建 `AgentEventRecord`（id = 3）：`event_key: u128`（`#[primary_key]`，`(run_id as u128) << 64 | seq` 打包）、`run_id: i64`（`#[secondary_key]` 非唯一）、`event: agent::AgentEvent`（嵌装载荷）；提供打包/解包助手（`new(run_id, event)` 与 `run_id`/`seq` 还原）
- [x] 编译验证 `u128` 主键可用（最小模型 + `cargo check`）；若 native_db 不支持 u128 键，退零填充 String 键（`format!("{run_id:016x}{seq:016x}")`，字典序即重放序），仅改键字段类型、不动二级索引与 API 形态
- [x] 确认 `crates/core/agent` git diff 为空（derive 全部落在 store 侧包装 struct 上）

## 阶段 2：Store 操作面平移 native_db

- [x] `crates/infra/store/src/store.rs`：`Store` 私有字段改为 native_db `Database`；`open` 按「父目录补齐 → 不存在则 `DatabaseBuilder` create → 存在则尝试 native_db open → 失败交迁移模块（阶段 3）」重组；模型定义处按序 `define` 三个模型
- [x] `store.rs`：workspace 三操作平移——`add_workspace`（canonical key upsert，`added_at` 保留首添值）、`list_workspaces`（PK 自然序）、`remove_workspace`（canonical 命中 + 目录消失时词法回退扫描、miss 幂等），复用 `canonical.rs` 不改动
- [x] `store.rs`：agent run 操作平移——`begin_agent_run`（`last()` 主键序取 max+1）、`finish_agent_run`（整行替换）、`list_agent_runs`（全表读 + `started_at` 降序、id 降序内存排序，行为与现状一致）
- [x] `store.rs`：事件操作类型化——`append_agent_run_events(&self, run_id: i64, events: &[AgentEvent])`（seq 取自事件本体，逐条打包 `event_key` 写入）与 `list_agent_run_events(&self, run_id: i64) -> Result<Vec<AgentEvent>, StoreError>`（经 `run_id` 二级索引扫描，seq 升序）
- [x] `store.rs`：删除 `init_schema`、`USER_META` / `SCHEMA_VERSION` 轮账与全部 redb 手写表定义；`StoreError` 两变体沿用，错误串保留 `db:` / `canonicalize:` 前缀口径
- [x] `crates/infra/store/src/lib.rs`：模块声明与导出更新（`migrate` / `envelope` 待阶段 3/4 接入时补声明）；crate 级文档重写——native_db 模型层架构、单进程约束平移、维度语义改由 db 文件归属承载、schema_version 章节移除

## 阶段 3：legacy 一次性迁移

- [x] 新建 `crates/infra/store/src/migrate.rs`：格式探测——native_db open 失败后以 redb `open_read_only` 探测 `user_meta` 表判定 legacy 格式；判定失败返回「无法识别的 db 格式」错误
- [x] `migrate.rs`：legacy 三表 read_only 读出——`user_workspaces`（key=root, value=`WorkspaceRecord` JSON）、`user_agent_runs`（key=id）、`user_agent_run_events`（复合键 `(run_id, seq)`，value=事件 JSON）
- [x] `migrate.rs`：事件行转换——解出 `AgentEvent` 正常嵌装；解不出的行包 `AgentEventKind::Raw { event_type: JSON "kind" 字段或 "unknown", raw_json: 原文字节串 }`，`seq` 取自旧表键、`timestamp_ms` 取 JSON `timestampMs` 或 0，不丢行不中断
- [x] `migrate.rs`：原子落盘——全部记录在临时文件（原路径同级 `<name>.native-tmp`）构建 native_db 新库，写完落盘并 drop 句柄后两段改名（旧 → `.bak`，已存在 `.bak` 先移除；新 → 原路径）；第二段改名失败尽力回退 `.bak` → 原路径并报错
- [x] `migrate.rs`：失败语义——读旧失败 / 写新失败 / 改名失败任一发生时旧文件保持原样（未改名未截断），`Store::open` 返回 `StoreError::Db`（`迁移:` 语境前缀）
- [x] `store.rs` + `lib.rs`：迁移模块接入 `Store::open`（native_db open 失败且 legacy 判定成立 → 迁移 → 重开），模块声明接线

## 阶段 4：记录信封 API

- [x] 新建 `crates/infra/store/src/envelope.rs`：`ModelInfo { name, count }` 与 `RecordEnvelope { key, value }`（serde camelCase，serde_json::Value）
- [x] `envelope.rs`：静态模型注册表——`name → { count, scan, key_of }` fn-pointer 三元组登记 `WorkspaceRecord` / `AgentRunRecord` / `AgentEventRecord` 三行；count 走 `Database::len`，scan 走主键自然序迭代 `skip(offset).take(limit)`，`key_of` 由记录主键构造 JSON 值（AgentEventRecord 还原 `{runId, seq}` 形态）
- [x] `store.rs`：新增 `pub fn list_models(&self) -> Result<Vec<ModelInfo>, StoreError>`（注册表全量，计数 0 也列出）与 `pub fn scan(&self, model: &str, offset: u32, limit: u32) -> Result<Vec<RecordEnvelope>, StoreError>`（limit 上限 500 截断；value 经 serde 转 `serde_json::Value`，native_db 类型不越信封；未知模型名 Err）
- [x] `lib.rs`：导出 `AgentEventRecord` / `ModelInfo` / `RecordEnvelope`，声明 `envelope` 模块

## 阶段 5：db 命令轨道与装配

- [x] 新建 `packages/desktop/src-tauri/src/commands/db/mod.rs`：`db_models` / `db_records` 两条只读薄包装（`State<'_, Store>` → 信封 API，`Result<T, String>`，模块文档声明只读轨道纪律）
- [x] `src/commands/mod.rs`：增 `pub mod db;` 并更新轨道文档注释（queries / exec / db 三轨）
- [x] `src/main.rs`：invoke_handler 注册 `commands::db::db_models` / `commands::db::db_records`
- [x] `src/commands/exec/agent.rs`：tee 循环删除 `serde_json::to_value` 分支，改为 `store.append_agent_run_events(record.id, std::slice::from_ref(&event))`（store 写失败收敛 failed 的路径不变）
- [x] `src/commands/exec/mod.rs`：`agent_run_events` 删除 Value→AgentEvent 反序列化步骤，直接返回 `store.list_agent_run_events(run_id)`

## 阶段 6：前端 DB 查看页与侧栏重组

- [x] `packages/desktop/src/types/dto.ts`：增 `ModelInfo { name: string; count: number }` 与 `RecordEnvelope { key: unknown; value: unknown }` 镜像类型
- [x] 新建 `packages/desktop/src/hooks/useDbInspector.ts`：`useDbInspector()` ——挂载 invoke `db_models` 一次；`selectModel(name)` 置选中并取第 0 页；`nextPage` / `prevPage` 按 `PAGE_SIZE`（50）翻页，`hasMore` 以「本页记录数 === PAGE_SIZE」判定；查询轨错误置 error 态 inline 持久（不用 toast）；无轮询
- [x] 新建 `packages/desktop/src/views/db/DbInspectorView.tsx`：模型清单（名称 + 计数，`data-testid="db-model-list"` / `db-model-item`，计数 0 呈空态）、选中模型分页记录列表（key 预览 + value 摘要）、单条记录 JSON 查看（`JSON.stringify(value, null, 2)`，`db-record-json`）、翻页控件（`db-page-prev` / `db-page-next`）、inline 持久错误区（`db-inspector-error`）；无任何写操作入口
- [x] `packages/desktop/src/components/AppSidebar.tsx`：`TopPage` 增 `'db'` 变体；`PageNavGroup` 收敛为仅 [变更]（`data-testid="nav-changes"`）；新增 `SystemToolsGroup`（组标签「系统工具」）收入 [Agent 调试]（`nav-agent` 平移）与 [DB 查看]（`nav-db`，lucide `Database` 图标）；点击切换本地 state
- [x] `packages/desktop/src/App.tsx`：顶层切换三分支渲染 `ChangeView` / `AgentDebugView` / `DbInspectorView`；确认切至 db 页不触发 change 取数（useChangeList 取数时机不在 page 依赖上）

## 阶段 7：收尾

- [x] `packages/desktop/package.json`：`version` `0.2.1` → `0.3.0`
- [x] 全量静态检查收口：`src-tauri` 下 `cargo fmt` + `cargo clippy`；前端 `vp check`（含 knip 无新增未用导出）
- [x] 自查验收映射：AC-1（存量行为不变）/ AC-2（迁移）/ AC-3（无 META 轮账残留，grep `schema_version`）/ AC-4（`crates/core/agent` diff 为空、store 公共 API 与 DTO 无 native_db 类型）/ AC-5（信封 API 经命令可达）/ AC-6（侧栏两组与欢迎态隔离）/ AC-7（查看器只读四件套）/ AC-8（store workspace 内依赖仅增 `agent`）
