# 设计: native-db-store-upgrade

> **变更**: native-db-store-upgrade
> **日期**: 2026-09-23

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| store 模型层 | 三个模型定义与 native_model 版本封装：`WorkspaceRecord` / `AgentRunRecord` 现状平移、`AgentEventRecord` 类型化新建（嵌装 `agent::AgentEvent`） | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | serde、native_db（derive）、native_model（derive）、`agent`（纯类型载荷） | `#[native_model(id, version)]` + `#[native_db]` 属性宏 |
| store 操作面 | `Store` 句柄与全部读写操作（workspace 三操作、agent run 四操作、事件追加/重放），`StoreError` 错误面 | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 模型层、native_db、canonical | native_db `Database`（`Send + Sync`，可挂 Tauri State） |
| legacy 迁移模块 | 旧 redb 手写表格式探测 → read_only 逐表读出 → 写 native_db 新文件 → `.bak` 留档；事件行解不出落 `Raw` 兜底 | `packages/desktop/src-tauri/crates/infra/store/src/migrate.rs`（新） | redb（读旧，仅此模块）、模型层 | redb `Database::open_read_only` + native_db 建库 + 两段改名 |
| 记录信封 API | 模型层通用只读读面：`list_models()` / `scan(model, offset, limit)`；内部静态模型注册表（name → count/scan/key 映射）是唯一扩展点 | `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs`（新） | 模型层、native_db、serde_json | fn-pointer 注册表 + 主键自然序流式扫描 |
| db 命令轨道 | `db_models` / `db_records` 两条只读 IPC 命令，薄包装（`State<Store>` → 信封 API → DTO） | `packages/desktop/src-tauri/src/commands/db/mod.rs`（新）+ `src/commands/mod.rs` + `src/main.rs` | store 信封 API | Tauri command，`Result<T, String>` |
| 侧栏系统工具组 | 「页面」组收敛为 [变更]；「系统工具」组收入 [Agent 调试]（平移）与 [DB 查看]（新增）；`TopPage` 增 `db` 变体 | `packages/desktop/src/components/AppSidebar.tsx` | shadcn sidebar、lucide-react | 本地 state 切换，无路由 |
| DB 查看页 | 只读查看面四件套：模型清单 + 计数、分页扫描、单条 JSON 查看、空态/inline 错误态 | `packages/desktop/src/views/db/DbInspectorView.tsx`（新） | useDbInspector、dto | React + `JSON.stringify(value, null, 2)` |
| 查看器取数 hook | 信封 API 取数收口：模型清单挂载取一次、记录分页由用户动作触发、错误 inline 持久（无 toast） | `packages/desktop/src/hooks/useDbInspector.ts`（新） | `@tauri-apps/api/core` invoke | 查询轨语义（error 态，非动作轨） |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/infra/store/src/migrate.rs` | legacy 一次性迁移模块：格式探测（native_db open **之前** redb read_only 探测 `user_meta` 表，次序裁定见决策表「legacy 格式探测次序」行）、三表读出与转换（事件行解不出包 `AgentEventKind::Raw`，`seq` 取自旧表复合键不取自 value）、临时文件建库 + 两段改名（旧 → `.bak`，新 → 原路径）+ 失败回退。crate 私有（`pub(crate)`），经 `Store::open` 调用 |
| `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | 记录信封 API：`ModelInfo` / `RecordEnvelope` 类型与静态模型注册表；`list_models` / `scan` 的实现体（Store 方法委托至此）。新模型接入点 = 注册表内登记一行 |
| `packages/desktop/src-tauri/src/commands/db/mod.rs` | db 查看命令轨道：`db_models` / `db_records` 两条只读薄包装，`State<'_, Store>` → 信封 API，错误约定沿 workspaces 轨道模板（`Result<T, String>`，不静默吞错）。轨道纪律：只读，MUST NOT 出现写命令 |
| `packages/desktop/src/views/db/DbInspectorView.tsx` | DB 查看页（只读）：模型清单（含计数，0 计数呈空态）、选中模型后分页扫描、单条记录 JSON 查看（key 与 value 信封完整呈现）、翻页控件、inline 持久错误区。无任何写操作入口、无轮询 |
| `packages/desktop/src/hooks/useDbInspector.ts` | 查看器取数 hook：`useDbInspector()` 收口 `db_models` / `db_records` invoke——模型清单挂载取一次（进入页面即用户显式动作）、选中模型/翻页触发记录取数、查询轨 error 态 inline 持久（不走 toast） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `WorkspaceRecord` / `AgentRunRecord` 加 `#[native_model(id = 1/2, version = 1)]` + `#[native_db]`（PK 不变：`root` / `id`）；删除手写 `encode` / `decode`（native_model 编解码接管）；新增 `AgentEventRecord` | 模型形状（字段面、serde camelCase）零变化，仅落库载体换 native_model 封装 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `Store` 私有字段 redb `Database` → native_db `Database`；`open` 改为「不存在则 native_db create → 存在则先 legacy 格式探测（先于 open，次序裁定见决策表）→ 命中走迁移后 open，未命中直通 open」；全部操作改经 native_db（workspace upsert/list/remove 含 canonical 词法回退、run begin `last()+1` / finish、事件追加与二级索引重放）；删除 `init_schema` 与 META 轮账；`list_agent_runs` 维持全表读 + 内存排序（行为不变）；`append_agent_run_events` / `list_agent_run_events` 改类型化签名 | `schema_version` 手工轮账整体退役；redb 类型不再出现在任何代码路径（迁移模块除外） |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | 模块声明增 `migrate` / `envelope`；导出增 `AgentEventRecord` / `ModelInfo` / `RecordEnvelope`；crate 级文档重写（redb 直驱 → native_db 模型层、单进程约束与维度语义改由 db 文件归属承载、schema_version 章节删除） | 纯接线与文档 |
| `packages/desktop/src-tauri/crates/infra/store/src/canonical.rs` | 无修改 | canonical 口径与引擎无关，原样复用 |
| `packages/desktop/src-tauri/crates/infra/store/Cargo.toml` | dependencies 增 `native_db` / `native_model` / `agent`（workspace）；保留 `redb`（注释标注：迁移期专用，读旧库，由后续变更收掉）；crate 注释「禁 core crate」→「仅依赖 `agent` 纯类型作嵌装载荷」 | 对应 desktop-crate-layout 依赖规则修订 |
| `packages/desktop/src-tauri/Cargo.toml` | `[workspace.dependencies]` 增 `native_db = "=0.8.2"`、`native_model = "=0.4.20"`；`redb = "=4.3.0"` 保留不动 | 精确 pin（pre-1.0 lockstep 升级策略）；native_model 必须与 native_db 内部 pin（0.4.20）一致，避免双版本 trait 失配 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | 增 `pub mod db;` | 轨道接线 |
| `packages/desktop/src-tauri/src/main.rs` | invoke_handler 注册 `commands::db::db_models` / `commands::db::db_records` | 其余（db 路径解析、单实例、updater）不动 |
| `packages/desktop/src-tauri/src/commands/exec/agent.rs` | tee 循环中 `serde_json::to_value(&event)` 分支删除，直接 `store.append_agent_run_events(record.id, std::slice::from_ref(&event))` | store 事件 API 类型化的连带简化（设计延伸文件，不在 proposal 实现清单，属必要涟漪） |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | `agent_run_events` 删除 `Value → AgentEvent` 反序列化步骤，直接返回 `store.list_agent_run_events(run_id)` | 同上 |
| `packages/desktop/src/components/AppSidebar.tsx` | `TopPage` 增 `'db'` 变体；`PageNavGroup` 收敛为仅 [变更]；新增 `SystemToolsGroup`（组标签「系统工具」：[Agent 调试] 自页面组平移 + [DB 查看]，lucide `Database` 图标，`data-testid="nav-db"`）；`AppSidebar` 渲染两组 | 侧栏重组，本地 state 切换语义不变 |
| `packages/desktop/src/App.tsx` | 顶层视图切换支持 `db`：三分支渲染 `ChangeView` / `AgentDebugView` / `DbInspectorView` | 切页不触发 change 取数（useChangeList 挂载于 App 层，本就不随 page 重取） |
| `packages/desktop/src/types/dto.ts` | 增 `ModelInfo` / `RecordEnvelope` 镜像类型 | 信封 DTO，value 为 JSON 值（`unknown`） |
| `packages/desktop/package.json` | `version` `0.2.1` → `0.3.0` | 新页面 + 存量库迁移，minor bump |

### 删除文件

无整文件删除。store 内 redb 手写表定义、`init_schema` / `user_meta` 轮账代码、模型手写 `encode` / `decode` 均为文件内删除；redb 直依赖过渡期保留（迁移模块专用），由后续变更收掉。

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `Store::open` | `crates/infra/store/src/store.rs` | 修改 | `pub fn open(path: &Path) -> Result<Self, StoreError>` | 签名不变，行为升级：native_db 打开 + legacy 格式探测迁移；仍 fail fast、路径完全由入参注入 |
| `Store::add_workspace` | 同上 | 修改 | `pub fn add_workspace(&self, root: &Path) -> Result<WorkspaceRecord, StoreError>` | 签名与 upsert 语义不变（`added_at` 保留首添值），载体换 native_db |
| `Store::list_workspaces` | 同上 | 修改 | `pub fn list_workspaces(&self) -> Result<Vec<WorkspaceRecord>, StoreError>` | PK（canonical root）自然序不变 |
| `Store::remove_workspace` | 同上 | 修改 | `pub fn remove_workspace(&self, root: &Path) -> Result<bool, StoreError>` | 语义不变（含消失目录词法回退、miss 幂等） |
| `Store::begin_agent_run` | 同上 | 修改 | `pub fn begin_agent_run(&self, run: &AgentRunRecord) -> Result<AgentRunRecord, StoreError>` | `last()+1` 自增分配语义不变（native_db 主键序保持） |
| `Store::finish_agent_run` | 同上 | 修改 | `pub fn finish_agent_run(&self, run_id: i64, record: &AgentRunRecord) -> Result<(), StoreError>` | 语义不变 |
| `Store::list_agent_runs` | 同上 | 修改 | `pub fn list_agent_runs(&self) -> Result<Vec<AgentRunRecord>, StoreError>` | `started_at` 降序 + id 降序的内存排序维持（调试页数据量小；二级索引查询形态留给 workflow 租户） |
| `Store::append_agent_run_events` | 同上 | 修改 | `pub fn append_agent_run_events(&self, run_id: i64, events: &[AgentEvent]) -> Result<(), StoreError>` | 入参 `&[serde_json::Value]` → `&[AgentEvent]`（类型化）；`seq` 取自事件本体，不再有「缺 seq」错误路径 |
| `Store::list_agent_run_events` | 同上 | 修改 | `pub fn list_agent_run_events(&self, run_id: i64) -> Result<Vec<AgentEvent>, StoreError>` | 返回 `Vec<serde_json::Value>` → `Vec<AgentEvent>`；经 `run_id` 二级索引扫描（首个真实索引查询），seq 升序 |
| `Store::list_models` | 同上 | 新增 | `pub fn list_models(&self) -> Result<Vec<ModelInfo>, StoreError>` | 全部已注册模型清单与记录计数（计数 0 也列出）；注册表驱动，新模型零改动覆盖 |
| `Store::scan` | 同上 | 新增 | `pub fn scan(&self, model: &str, offset: u32, limit: u32) -> Result<Vec<RecordEnvelope>, StoreError>` | 按模型主键自然序流式扫描（`skip(offset).take(limit)`）；`limit` 上限 500（超出截断）；未知模型名 `Err`；key/value 均 `serde_json::Value`，native_db 类型不越信封 |
| `db_models` | `src/commands/db/mod.rs` | 新增 | `pub fn db_models(store: State<'_, Store>) -> Result<Vec<ModelInfo>, String>` | 只读薄包装 |
| `db_records` | `src/commands/db/mod.rs` | 新增 | `pub fn db_records(store: State<'_, Store>, model: String, offset: u32, limit: u32) -> Result<Vec<RecordEnvelope>, String>` | 只读薄包装；`Err` 由 Tauri 转前端 reject |
| `useDbInspector` | `packages/desktop/src/hooks/useDbInspector.ts` | 新增 | `function useDbInspector(): DbInspectorState` | 挂载取模型清单一次；`selectModel` 重置 offset 取第一页；`nextPage` / `prevPage` 翻页；`refresh` 重取当前页 |
| `DbInspectorView` | `packages/desktop/src/views/db/DbInspectorView.tsx` | 新增 | `export function DbInspectorView(): React.JSX.Element` | 只读查看页组件 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `AgentEventRecord` | `crates/infra/store/src/model.rs` | 新增 | 类型化事件记录：`event_key: u128`（PK，`(run_id as u128) << 64 \| seq` 打包）、`run_id: i64`（非唯一二级索引）、`event: agent::AgentEvent`（嵌装载荷，core 类型 derive-free 原样引用）；`#[native_model(id = 3, version = 1)]` |
| `ModelInfo` | `crates/infra/store/src/envelope.rs` | 新增 | `{ name: String, count: u64 }`，serde camelCase |
| `RecordEnvelope` | `crates/infra/store/src/envelope.rs` | 新增 | `{ key: serde_json::Value, value: serde_json::Value }`；value 由模型记录经 serde 转 Value（人可读，无二进制） |
| `StoreError` | `crates/infra/store/src/store.rs` | 不变 | 两变体沿用；迁移失败经 `Db(msg)`（`迁移:` 语境前缀）呈现，不扩变体 |
| `ModelInfo` | `packages/desktop/src/types/dto.ts` | 新增 | `{ name: string; count: number }` |
| `RecordEnvelope` | `packages/desktop/src/types/dto.ts` | 新增 | `{ key: unknown; value: unknown }`（JSON 值信封） |
| `TopPage` | `packages/desktop/src/components/AppSidebar.tsx` | 修改 | `'changes' \| 'agent'` → `'changes' \| 'agent' \| 'db'` |
| `DbInspectorState` | `packages/desktop/src/hooks/useDbInspector.ts` | 新增 | hook 返回形态：`models` / `loading` / `error` / `selected` / `records` / `recordsLoading` / `recordsError` / `offset` / `hasMore` + 动作 `refresh` / `selectModel` / `nextPage` / `prevPage` |

### 配置

不涉及（无 plugin.json / settings / hooks 配置键变更；workspace `Cargo.toml` 的依赖 pin 属依赖声明，见「依赖」节）。

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `WorkspaceRecord`（native_model id=1, version=1） | `root`（PK，canonical 完整路径）、`name`、`added_at`（UTC unix 毫秒） | 无 | user db（`.dev-team/desktop-store.redb`，native_db 格式）；payload 经 native_model bincode 封装 |
| `AgentRunRecord`（id=2, version=1） | `id`（PK，i64 自增语义）、`prompt`、`cwd`、`env`、`permission_mode`、`status`、`started_at`、`finished_at?`、`num_turns?`、`cost_usd?`、`duration_ms?`、`session_id?`、`error?` | `id` ← `AgentEventRecord.run_id`（1:N） | 同上；字段面与 serde camelCase 线格式零变化（联动字段 `source` / `workflow_run_id` / `phase` 随 workflow 租户变更引入，本变更不加） |
| `AgentEventRecord`（id=3, version=1，新） | `event_key`（PK，u128 = `(run_id as u128) << 64 \| seq`）、`run_id`（二级索引，非唯一）、`event`（嵌装 `agent::AgentEvent` 五变体，含 `Raw` 逃生舱） | `run_id` → `AgentRunRecord.id` | 同上；PK 打包保证同 run 内 seq 有序（扫描自然序即重放序）；native_model id/version 存于 store 侧，core 类型零改动 |
| 未来租户（本变更不注册） | `WorkflowRunRecord`（PK id；`workspace_root` + `change_name` + 二级索引）、`PhaseExecutionRecord`、`AgentRunRecord` 可选联动字段 | 按 desktop-data-dimensions「workflow 过程数据归 user 维度」约束设计 | 随首个写入方变更「定义 struct + 注册表登记一行」即可上线，无生产者的模型不预建 |

**legacy → 新格式映射**：`user_workspaces` → `WorkspaceRecord`（key=root 原样）；`user_agent_runs` → `AgentRunRecord`（key=id 原样）；`user_agent_run_events` → `AgentEventRecord`（复合键 `(run_id, seq)` 打包为 `event_key`，value 解出 `AgentEvent` 嵌装；解不出 → `AgentEventKind::Raw { event_type: JSON "kind" 字段或 "unknown", raw_json: 原文字节串 }`，`seq` 取自旧表键，`timestamp_ms` 取 JSON `timestampMs` 或 0）；`user_meta` → 删除（`schema_version` 轮账退役，shape 演进交 native_model 版本机制）。维度语义载体从「`user_` 表名前缀」平移为「db 文件归属（user db 落 app data dir）」。

---

## 路由/API 设计（Tauri IPC，非 HTTP）

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| invoke | `db_models` | 模型清单与计数（只读） | 无 | `ModelInfo[]`；失败 reject `Err(String)` | 本地应用（Tauri IPC，无额外认证） |
| invoke | `db_records` | 按模型分页扫描记录（只读） | `{ model: string, offset: number, limit: number }` | `RecordEnvelope[]`（`key` / `value` 为 JSON 值）；未知模型或失败 reject | 同上 |

只读边界：db 轨道仅此两命令，无任何写入口；取数由用户显式动作触发（进页、选中模型、翻页、刷新），无轮询、无事件订阅。

---

## 依赖

### 运行时依赖

- `native_db = "=0.8.2"` — 模型层（构建于 redb 之上）：模型宏注册、二级索引、内建迁移、（未接线的）实时订阅；精确 pin，升级按 lockstep 处理
- `native_model = "=0.4.20"` — 模型身份/版本封装（`#[native_model]` derive 与 encode/decode）；pin 必须与 native_db 内部依赖严格一致
- `redb = "=4.3.0"`（保留）— legacy 迁移模块读旧库专用；与 native_db 传递的 redb 2.x 双版本共存（cargo 支持，读旧/写新为不同文件无锁冲突）；迁移稳定一个版本后由后续变更收掉
- `agent`（workspace path）— `AgentEvent` 纯类型作嵌装载荷（依赖规则修订的唯一新增 workspace 内依赖；禁用 agent 域行为）
- `dunce` / `serde` / `serde_json` — 既有，沿用（canonical 口径 / 模型 derive / 信封 Value 编解码）
- 前端：无新增依赖（lucide-react `Database` 图标、`@tauri-apps/api` 既有）

### 构建/测试依赖

- `tempfile` — 既有 dev 依赖，沿用（store / 命令层真实开库测试；测试设计属独立阶段，此处仅声明依赖面不变）
- `vp`（vite-plus）/ `cargo fmt` / `cargo clippy` — 既有工具链，不变

---

## 关键设计决策

| 问题 | 决策 | 理由与证据 |
|------|------|-----------|
| Spike① 事件主键 | **合成 u128 主键** `event_key = (run_id as u128) << 64 \| seq` + `run_id` 非唯一二级索引；不信复合键 | docs.rs（native_db 0.8.2 / db_type）未见任何 tuple `ToKey` 实现，文档示例均为原始类型/String；退路键（提案已列）转正为主案。残余验证收进任务 1（编译期即暴露），u128 键不受支持时退零填充 String 键（`{run_id:016x}{seq:016x}`，字典序即数值序），二级索引查询形态不受影响 |
| Spike② redb 版本收敛 | **不收敛，双版本共存**：workspace `redb = "=4.3.0"` 保留为迁移模块直依赖；native_db 自带 redb `^2.1.4` 作传递依赖 | docs.rs 证据：native_db 0.8.2 依赖 `redb ^2.1.4`（另有可选 `=1.5.1` 特性变体），与 `=4.3.0` 无交集；cargo 允许依赖树双版本，迁移读旧 / 写新为不同文件无锁冲突 |
| Spike③ 编码后端 | **默认 bincode（native_model 0.4.20 默认 `bincode_1_3`）落盘；人可读由信封 API 内部解码到 `serde_json::Value` 兑现** | docs.rs 证据：native_model 0.4.20 仅 `bincode_1_3` 模块 + `wrapper`，无 serde_json codec；自研 JSON codec 增维护面且存储体积/写放大劣化。查看层经信封拿到的恒为 JSON 值，二进制不越信封（满足「人可读呈现」scenario）。**实现偏差补注（implement 阶段裁定）**：`AgentEventRecord` 实际改用自研 `SerdeJsonCodec`（`model.rs`，native_model `with` 自定义 codec）——core `AgentEvent` 为 internally-tagged enum + `#[serde(flatten)]`，bincode 1.3 运行期无法编码（需自描述格式，实测报 "maps of unknown size"）；JSON 亦与 core「camelCase 线格式 = 落库形态」及 legacy 存储形态一致，`event_key` 超出 u64 的 u128 以十六进制字符串序列化。平面模型（`WorkspaceRecord` / `AgentRunRecord`）维持默认 bincode。db-inspector spec 明列两选一，不违 spec |
| legacy 格式探测次序 | **探测先于 native_db open**（`is_legacy_format` 用 redb 4.3.0 read_only 判定 `user_meta` 表存在与否，再决定迁移或直通 open） | **实现偏差补注（implement 阶段裁定）**：本设计原定「native_db 打开失败后再探测」，但实测 native_db 内部 redb 2.x 会接受 redb 4.3.0 写出的文件头、解析布局时直接 panic（`AllocatorStateKey` unreachable）——「失败后探测」不是可捕获错误，首次启动即崩；反向 redb 4.3.0 read_only 可安全读 2.x 字节，故探测先行作双格式安全读取器。workspace-store spec 明令先探测；`migrate.rs` / `store.rs` 留有理由注释 |
| 迁移原子性 | 临时文件建新库（完整写入并落盘后 drop 句柄）→ 旧文件改名 `.bak`（已存在则先移除）→ 新文件改名到原路径；第二步失败尽力回退改名并报错 | 任何时点旧库不丢：失败场景（读旧失败 / 写新失败 / 改名失败）旧文件保持原样或可回退，`.bak` 永不覆盖删除前未成功的新库写入 |
| 迁移竞态 | 迁移仅启动时执行一次；成功后原路径即 native 格式，后续启动走 native_db open 直通；单进程约束在 lib.rs 文档显式强化（双开不保证安全，既有声明平移） | 无「迁移完成标记」需求——文件格式本身就是标记 |
| 信封 API 扩展机制 | `envelope.rs` 内静态注册表（`name → { count, scan, key_of }` fn-pointer 三元组），`list_models` / `scan` 按表分发 | native_db 的 `len` / 扫描是按类型静态分派的，运行时反射不存在；注册表是「新模型 = 定义 struct + 登记一行」的最小机制，也是查看器零模型特定代码的落点 |
| `list_agent_runs` 查询形态 | 维持全表读 + 内存排序（`started_at` 降序、id 降序），不新增二级索引 | 调试页数据量小、行为零变化优先（AC-1）；二级索引查询形态已由 `AgentEventRecord.run_id` 兑现一处，workflow 租户的索引链随其变更落地 |
| store 事件 API 类型化涟漪 | `append_agent_run_events` / `list_agent_run_events` 改类型化签名，`commands/exec/agent.rs` 与 `exec/mod.rs` 随之简化（删除 `to_value` 分支与反序列化步骤） | opaque JSON 进出正是本变更要消灭的形态；两文件不在 proposal 实现清单但是类型化的必然涟漪，均不在「不要修改」清单内 |

---

## 待决问题

- `u128` 作为 native_db 主键类型：docs 未显式枚举键类型全集，按任务 1 编译验证收口；不受支持时退零填充 String 键（形态已定，不返工二级索引）
- `native_db` 实际 pin 值：以任务 1 时点 crates.io 上 0.8.x 最新补丁版为准精确 pin（当前证据 0.8.2）；native_model 随动（须等于 native_db 内部 pin）
- 收掉 redb 直依赖的时机（迁移稳定一个版本后）：后续 change 承担，本变更保留过渡态
- native_db 实时订阅能力本变更不接线（未来 UI 刷新通道），查看器取数维持用户显式触发
