# 设计: desktop-workspace-db-split

> **变更**: desktop-workspace-db-split
> **日期**: 2026-09-29

---

## 提案与规格同步状态

`proposal.md` 与四个能力规格（desktop-workspace-store / desktop-data-dimensions / desktop-agent-execution / desktop-db-inspector）已在提案阶段写入并通过评审（11/11），本设计不再将其列入变更清单与任务。本设计承接提案的 4 个 dev-design 定夺项（全局库文件名、workspace 库文件名组成、句柄缓存策略、db 页 scope 交互形态），落点见「关键设计定夺」。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 静态模型注册（拆两组） | 全局组仅 `WorkspaceRecord`；workspace 组仅 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord`，与两库一一对应，无交叉注册 | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | native_db | 两个 `OnceLock<&'static Models>` 静态 |
| `Store` 双维度构造器 | 注入式单文件句柄语义不变；打开入口按维度显式命名（`open_global` / `open_workspace`），实例私有携带维度供信封过滤 | 同上 | native_db | 命名构造器 + 私有 `open_with(path, models, dimension)` 收口 |
| workspace 库路径派生单点 | canonical root → `workspaces/` 子树 db 文件路径；纯函数、确定性、抗碰撞、文件名不含路径分隔符；store 内唯一组装点，消费侧零派生逻辑 | 同上 | sha2（哈希成分）、dunce（既有 canonical 口径前置归一） | SHA-256 截断 128-bit hex + 清洗可读段 |
| `WorkspaceStores` 注册表 | 两级解析：`global()` → 全局库；`for_root(root)` → workspace 库；per-root 实例缓存复用（进程内单开，进程生命周期常开）；挂 Tauri State | 同上 | — | `Mutex<HashMap<String, Arc<Store>>>` + Send/Sync 编译期断言 |
| 记录信封 API（维度标记） | 注册表行增维度标签，`list_models` / `scan` 按实例维度过滤；信封外形与「查看器零模型特定代码」不变（分维度是数据行，非分支代码） | `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | serde_json | fn-pointer 静态注册表（既有机制） |
| app setup 与数据根注入 | `home_dir()/.dev-team` 数据根 → `WorkspaceStores::open` → `app.manage`；全局库打开失败 fail fast（口径同现状）；store 内零环境解析不变 | `packages/desktop/src-tauri/src/main.rs` | store | tauri setup |
| workspace 注册命令轨道 | 注册三命令走全局库；`add_workspace` 注册成功后预开对应 workspace 库（fail fast）；`remove_workspace` 仅删注册记录 | `packages/desktop/src-tauri/src/commands/workspaces/mod.rs` | `WorkspaceStores` | `State<'_, WorkspaceStores>` 薄包装 |
| explore 命令轨道 | 记录面与 `scan_explores` 经 `for_root(root)` 路由至 workspace 库；命令签名不变；blank root 纪律保持 | `packages/desktop/src-tauri/src/commands/explores/mod.rs` | `WorkspaceStores` | root 寻址薄路由 |
| exec 命令轨道 | `agent_start` / `agent_stop` / `agent_runs` / `agent_run_events` / `agent_run_chain` root 寻址；`RunStopRegistry` 寻址键演进为 `(root, run_id)` | `packages/desktop/src-tauri/src/commands/exec/mod.rs` + `agent.rs` | `WorkspaceStores`、`RunStopRegistry` | 复合键注册表 + 预解析 `Arc<Store>` 后台任务持有 |
| db 查看命令轨道 | `db_models` / `db_records` 增 scope（全局库 / 当前 workspace 库）寻址；信封 API 调用面不变 | `packages/desktop/src-tauri/src/commands/db/mod.rs` | `WorkspaceStores` | `DbDimension` 参数化薄包装 |
| 前端会话基建 | 链还原与事件重放、停止 invoke 携 root；hook 对外签名不变 | `packages/desktop/src/hooks/use-agent-chat.ts` | 生成绑定 | invoke 入参面更新 |
| 前端运行历史 | `useAgentRunHistory` 增 root 入参；root 为 null 跳过取数 | `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts`、`agent-debug-view.tsx` | 生成绑定 | 入参透传 |
| 前端 db 查看页 | scope 双 tab 切换（全局库 / workspace 库）+ root 透传；切换即重置选中与分页 | `packages/desktop/src/views/db/hooks/use-db-inspector.ts`、`db-inspector-view.tsx`、`src/routes.tsx` | 生成绑定 | scope 态 + aria-pressed 按钮组（同 agent 页切换行模式） |
| agent-transport | `agent_start` IPC 入参面不变（root 本就是第 1 个 IPC 参数），`AgentStartArgs` 索引不漂移 | `packages/desktop/src/lib/agent-transport.ts` | 生成绑定 | 核对项，预期零修改 |

### 关键设计定夺（proposal 待决问题落点）

1. **全局库新文件名**：`desktop-global.redb`（常量 `GLOBAL_DB_FILE_NAME`，store crate 单点持有）。旧 `desktop-store.redb` 为四模型旧布局，换名使新旧布局文件面彻底无歧义，免格式探测。
2. **workspace 库文件名组成**：`{可读段}-{哈希}.redb`。哈希 = SHA-256(canonical root UTF-8 字节) 前 16 字节的 32 位小写 hex（128-bit，抗碰撞余量充足；sha2 已在依赖锁内为传递依赖，无新包入树）；可读段 = `dir_name(root)`（与 `WorkspaceRecord::from_root` 同源取末段）经清洗（OS 非法字符置换 `_`、按 char boundary 截断 ≤24 字符、去尾部 `.` 与空格），为空时回退纯哈希名。常量 `WORKSPACES_DIR_NAME = "workspaces"` 与派生函数同置 store crate 单点。
3. **workspace 库句柄缓存策略**：进程生命周期常开（不引入 LRU）。理由：workspace 数量本机个位数，内存占用可忽略；常开使重加同 root 即读即得（AC-7），并免去驱逐后二次打开的锁竞争面。`remove_workspace` 不驱逐缓存实例（文件保留语义的进程内对应面）。
4. **db 页 scope 交互形态**：双 tab 按钮组（「全局库」/「workspace 库」，`aria-pressed` 模式同 agent 页切换行）；默认 scope 为当前 workspace 库（调试主看 workspace 域模型），切全局库为显式动作；scope/root 任一变更即重置选中模型与分页并重取清单。
5. **`ExploreRecord` 链锚与路径关联字段**：维持现状——链锚经 `(source="explore", source_ref=记录 id)` 派生、磁盘路径经展示名 `name` 派生，不引入 `head_run_id` 双写与路径字段。这是 AC-8「模型 shape 零变化」的强制结论，非自由取舍。
6. **`Store::open` 拆分**：提案「`Store::open(path)` 注入式打开语义不变」指注入式路径 + 单文件句柄语义；模型组选择必须发生在打开点，故拆为 `open_global` / `open_workspace` 两个命名构造器（语义不变、入口按维度命名），原符号退役。
7. **blank root 纪律矩阵**（命令层，store 不感知）：查询命令（`agent_runs` / `agent_run_events` / `agent_run_chain` / `list_explore_records` / `scan_explores` / db 两命令 workspace scope）blank root → 空结果；`agent_start` blank root → `Err`（无 cwd 无从发起）；explore 写命令（create / rename / delete）blank root → `Err`（写无空结果语义）；`agent_stop` blank root → 幂等 `Ok`。
8. **`add_workspace` 预开顺序**：按 workspace-store 规格文本「注册成功后预开」执行——先注册后 `for_root` 预开；坏文件在注册时以 `Err` 暴露（注册记录保留，重加同 root 时 upsert 幂等并再次校验）。
9. **`for_root` 不校验注册归属**：派生为纯函数，不查全局注册表（免逐命令跨库 IO 与耦合）。root 恒来自前端清单（canonical root），与现状「cwd 恒来自前端」同一信任级别；blank root 已在命令层拦下。

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  proposal.md「变更范围 - 实现文件」12 条全部覆盖；级联文件以「清单外补入」标注。
-->

### 新增文件

<!-- 无新增文件：拆分是既有文件的归属平移，测试文件由 test-design / test-gen 阶段承接，不在本清单 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `models()` 拆 `global_models()` / `workspace_models()` 两组静态注册；`Store::open` 拆 `open_global` / `open_workspace`（实例携带 `DbDimension`）；新增 `DbDimension` 枚举；workspace 库路径派生单点（`workspace_db_file_name` / `workspace_db_path`，SHA-256 截断 + 可读段清洗）；新增 `WorkspaceStores`（`open` / `global` / `for_root`，per-root `Arc<Store>` 缓存、进程常开、Send/Sync 断言）；文件名 / 子树常量单点 | 双库布局核心；消费侧零派生逻辑 |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 仅维度归属文档注释修正（`ExploreRecord`「user 维度 / 落 app data dir」措辞改为 workspace 维度、落所属 workspace 库；run / 事件两模型同步补落位说明） | 模型 struct 字段面与 native_model id / version 零变化（AC-8） |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | 导出 `WorkspaceStores` / `DbDimension`；crate 文档由「单库四模型」改写为双库布局 | 公共导出面 |
| `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | `ModelEntry` 注册表行增 `dimension` 标签；`list_models` / `scan` 内部分发按实例维度过滤；`ModelInfo` / `RecordEnvelope` 外形不变 | 「新模型 = 定义 struct + 登记一行」机制不变；分维度是数据行非分支代码（AC-6） |
| `packages/desktop/src-tauri/src/main.rs` | `DB_FILE_NAME` 常量演进为数据根 `DATA_DIR = ".dev-team"`；setup 改 `WorkspaceStores::open(&data_root)` + `app.manage`；全局库打开失败 fail fast 同现状；`RunStopRegistry` / `WatchRegistry` 挂载不变 | 全局库新文件名与 `workspaces/` 子树语义由 store 常量单点承载，main 只注入数据根 |
| `packages/desktop/src-tauri/src/commands/workspaces/mod.rs` | `State<'_, Store>` 切 `State<'_, WorkspaceStores>`；三命令经 `global()` 操作注册表；`add_workspace_inner` 注册成功后 `for_root` 预开校验；`remove_workspace_inner` 仅删注册记录（db 文件与缓存实例保留） | 全局轨命令不误路由 workspace 库；IPC 入参面不变 |
| `packages/desktop/src-tauri/src/commands/explores/mod.rs` | `State` 切 `WorkspaceStores`；`scan_explores` 与 `list/create/rename/delete_explore_record` 经 `for_root(&root)` 路由；blank root 纪律保持（查询空结果、写命令 `Err`） | 命令签名不变，仅内部路由切换 |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | `agent_start`：`State` 切 `WorkspaceStores`，blank root `Err`，落库经 `for_root`；`agent_stop` 增 `root: String` 入参；`agent_runs` / `agent_run_events` / `agent_run_chain` 增 `root: String` 入参（查询 blank root 空结果） | 运行清单收窄为当前 workspace 历史；`Result<T, String>` 错误模板不变 |
| `packages/desktop/src-tauri/src/commands/exec/agent.rs` | `RunStopRegistry` 句柄表键 `HashMap<i64, RunHandle>` → `HashMap<(String, i64), RunHandle>`（`register` / `request_stop` / `remove` 增 root 参）；`start_agent_run` / `start_agent_run_with` 改收 `&WorkspaceStores`，同步段预解析 `for_root` 得 `Arc<Store>` 供后台任务持有收尾；`drive_agent_run` 签名不变（仍收 `&Store`） | run id 域内化配套：同 id 并行时停止命中发起方所在库 |
| `packages/desktop/src-tauri/src/commands/db/mod.rs` | `db_models` / `db_records` 增 `scope: DbDimension` 与 `root: String` 入参；Global 走 `global()`，Workspace 走 `for_root`（blank root 空结果） | 信封 API 与零模型特定代码不变（AC-6） |
| `packages/desktop/src/hooks/use-agent-chat.ts` | `loadChain` 增 root 参（`agentRunChain(root, source, sourceRef)`、`agentRunEvents(root, run.id)`）；`stop` 携 root（`agentStop(root, runId)`）；hook 对外签名与镜像语义不变 | 链状态归本 hook 的边界不变 |
| `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | `useAgentRunHistory` 增 `root: string | null` 入参并透传两条取数轨道；root 为 null 跳过 invoke 保持空态 | 运行清单收窄为当前 workspace |
| `packages/desktop/src/views/agent/agent-debug-view.tsx` | root 透传 `useAgentRunHistory(root)` | 页面已有 root prop，接线即可 |
| `packages/desktop/src/views/explores/hooks/use-explore-session.ts` | 透传面核对：root 已为入参且经 `useAgentChat` 承载全部新入参，本文件预期零修改（波及时同步注释） | proposal 实现文件点名项，落实为核对任务 |
| `packages/desktop/src/views/db/hooks/use-db-inspector.ts` | `useDbInspector` 增 `root: string` 入参与 scope 态（默认 workspace 库，暴露 `scope` / `setScope`）；invoke 更新为 `dbModels(scope, root)` / `dbRecords(scope, root, model, offset, PAGE_SIZE)`；scope / root 变更重置选中模型与分页并重取 | 取数仍全部用户显式动作触发，无轮询 |
| `packages/desktop/src/views/db/db-inspector-view.tsx` | 接收 `root: string` prop；模型区头部增 scope 双 tab 按钮组（`aria-pressed` 同 agent 页切换行模式）；空态 / inline 持久错误态不变 | 交互形态定夺项 4 的落点 |
| `packages/desktop/src/types/generated/bindings.ts` | 随 `bindings:export` 再生成：命令入参增 root / scope，`DbDimension` 类型出线 | 生成物，零手写 |
| `packages/desktop/package.json` | `version` 0.3.8 → 0.3.9 | 用户可见行为变更（数据隔离 + 运行清单收窄 + 存量数据废弃） |

#### 清单外补入（级联文件）

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src/routes.tsx` | `/db` 路由向 `DbInspectorView` 传 `root={root}` | 级联：`DbInspectorView` 增 root prop 的唯一接线点 |
| `packages/desktop/src-tauri/Cargo.toml` | `[workspace.dependencies]` 增 `sha2 = "0.10"` | 级联：store 直依赖的 workspace 级版本收敛（依赖纪律） |
| `packages/desktop/src-tauri/crates/infra/store/Cargo.toml` | `[dependencies]` 增 `sha2 = { workspace = true }` | 级联：文件名哈希成分；sha2 已在依赖锁内为传递依赖，无新包入树 |
| `packages/desktop/src-tauri/Cargo.lock` | sha2 由传递依赖边升级为 store 直依赖边 | 级联：随构建再生，无版本树新增 |

### 删除文件

<!-- 无删除文件：旧单库文件 desktop-store.redb 为用户磁盘上的惰性残留，运行时不读、不改名、不删除；代码文件零删除 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `WorkspaceStores::open` | `.../infra/store/src/store.rs` | 新增 | `pub fn open(data_root: &Path) -> Result<WorkspaceStores, StoreError>` | 打开全局库（`data_root/desktop-global.redb`）并记录 `workspaces/` 子树根；任一失败 `Err`（setup fail fast） |
| `WorkspaceStores::global` | `.../infra/store/src/store.rs` | 新增 | `pub fn global(&self) -> &Store` | 全局库实例（user 维度注册表操作面） |
| `WorkspaceStores::for_root` | `.../infra/store/src/store.rs` | 新增 | `pub fn for_root(&self, root: &str) -> Result<Arc<Store>, StoreError>` | 按 canonical root 派生路径并解析 workspace 库实例；per-root 缓存复用（进程内单开）；坏文件 `Err` |
| `Store::open_global` | `.../infra/store/src/store.rs` | 新增 | `pub fn open_global(path: &Path) -> Result<Self, StoreError>` | user 维度组打开（仅 `WorkspaceRecord`） |
| `Store::open_workspace` | `.../infra/store/src/store.rs` | 新增 | `pub fn open_workspace(path: &Path) -> Result<Self, StoreError>` | workspace 维度组打开（run / 事件 / explore 三模型） |
| `Store::open` | `.../infra/store/src/store.rs` | 修改 | `pub fn open(path: &Path) -> Result<Self, StoreError>` | 退役：拆分至 `open_global` / `open_workspace`（注入式单文件句柄语义由两者继承） |
| `Store::list_models` | `.../infra/store/src/store.rs` | 修改 | `pub fn list_models(&self) -> Result<Vec<ModelInfo>, StoreError>` | 签名不变；按实例维度只列本库模型 |
| `Store::scan` | `.../infra/store/src/store.rs` | 修改 | `pub fn scan(&self, model: &str, offset: u32, limit: u32) -> Result<Vec<RecordEnvelope>, StoreError>` | 签名不变；跨维度模型名按「未知模型」`Err` |
| `list_workspaces` | `.../src/commands/workspaces/mod.rs` | 修改 | `pub fn list_workspaces(store: State<'_, WorkspaceStores>) -> Result<Vec<WorkspaceRecord>, String>` | State 类型切换；IPC 面不变 |
| `add_workspace` | `.../src/commands/workspaces/mod.rs` | 修改 | `pub fn add_workspace(store: State<'_, WorkspaceStores>, root: String) -> Result<WorkspaceRecord, String>` | 注册成功后预开对应 workspace 库（fail fast）；IPC 面不变 |
| `remove_workspace` | `.../src/commands/workspaces/mod.rs` | 修改 | `pub fn remove_workspace(store: State<'_, WorkspaceStores>, root: String) -> Result<bool, String>` | 仅删注册记录，db 文件保留；IPC 面不变 |
| `agent_start` | `.../src/commands/exec/mod.rs` | 修改 | `pub async fn agent_start(app: AppHandle, stores: State<'_, WorkspaceStores>, on_event: Channel<AgentRunMessage>, root: String, prompt: String, permission_mode: AgentPermissionMode, resume_session_id: Option<String>, source: Option<String>, source_ref: Option<String>, parent_run_id: Option<i64>) -> Result<AgentRunRecord, String>` | IPC 面不变；blank root `Err`；落库经 `for_root` 至当前 workspace 库 |
| `agent_stop` | `.../src/commands/exec/mod.rs` | 修改 | `pub fn agent_stop(registry: State<'_, RunStopRegistry>, root: String, run_id: i64) -> Result<(), String>` | 新增 root 入参；复合键寻址，不跨库误停；miss 幂等 `Ok` |
| `agent_runs` | `.../src/commands/exec/mod.rs` | 修改 | `pub fn agent_runs(stores: State<'_, WorkspaceStores>, root: String) -> Result<Vec<AgentRunRecord>, String>` | 新增 root 入参；清单收窄为当前 workspace 历史 |
| `agent_run_events` | `.../src/commands/exec/mod.rs` | 修改 | `pub fn agent_run_events(stores: State<'_, WorkspaceStores>, root: String, run_id: i64) -> Result<Vec<AgentEvent>, String>` | 新增 root 入参 |
| `agent_run_chain` | `.../src/commands/exec/mod.rs` | 修改 | `pub fn agent_run_chain(stores: State<'_, WorkspaceStores>, root: String, source: String, source_ref: String) -> Result<Vec<AgentRunRecord>, String>` | 新增 root 入参；`source_ref` 为 workspace 库域内 explore 记录 id |
| `db_models` | `.../src/commands/db/mod.rs` | 修改 | `pub fn db_models(stores: State<'_, WorkspaceStores>, scope: DbDimension, root: String) -> Result<Vec<ModelInfo>, String>` | scope 寻址两库；Global 忽略 root |
| `db_records` | `.../src/commands/db/mod.rs` | 修改 | `pub fn db_records(stores: State<'_, WorkspaceStores>, scope: DbDimension, root: String, model: String, offset: u32, limit: u32) -> Result<Vec<RecordEnvelope>, String>` | scope 寻址两库；信封分页语义不变 |
| `scan_explores` | `.../src/commands/explores/mod.rs` | 修改 | `pub fn scan_explores(root: String, store: State<'_, WorkspaceStores>) -> Result<Vec<ExploreScanEntry>, String>` | State 类型切换 + `for_root` 路由；IPC 面不变 |
| `list_explore_records` | `.../src/commands/explores/mod.rs` | 修改 | `pub fn list_explore_records(store: State<'_, WorkspaceStores>, root: String) -> Result<Vec<ExploreRecord>, String>` | 同上 |
| `create_explore_record` | `.../src/commands/explores/mod.rs` | 修改 | `pub fn create_explore_record(store: State<'_, WorkspaceStores>, root: String, name: String) -> Result<ExploreRecord, String>` | 同上；blank root `Err` |
| `rename_explore_record` | `.../src/commands/explores/mod.rs` | 修改 | `pub fn rename_explore_record(store: State<'_, WorkspaceStores>, root: String, name: String, new_name: String) -> Result<ExploreRecord, String>` | 同上 |
| `delete_explore_record` | `.../src/commands/explores/mod.rs` | 修改 | `pub fn delete_explore_record(store: State<'_, WorkspaceStores>, root: String, name: String) -> Result<bool, String>` | 同上；级联删除天然收敛同一 workspace 库内 |
| `useAgentChat` | `packages/desktop/src/hooks/use-agent-chat.ts` | 修改 | `function useAgentChat(params: UseAgentChatParams): UseAgentChatState` | 对外签名不变；链还原与停止 invoke 携 `params.root` |
| `useAgentRunHistory` | `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | 修改 | `function useAgentRunHistory(root: string | null): AgentRunHistoryState` | root 为 null 跳过取数保持空态 |
| `useDbInspector` | `packages/desktop/src/views/db/hooks/use-db-inspector.ts` | 修改 | `function useDbInspector(root: string): DbInspectorState` | scope 态 + root/scope 入参化取数 |
| `DbInspectorView` | `packages/desktop/src/views/db/db-inspector-view.tsx` | 修改 | `function DbInspectorView(props: { root: string }): React.JSX.Element` | scope 双 tab 切换 UI |

<!-- 私有函数不列：workspace_db_file_name / workspace_db_path / open_with 等派生与收口点为 pub(crate)，见 store.rs 修改内容行 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `DbDimension` | `.../infra/store/src/store.rs` | 新增 | Rust enum：`User` / `Workspace`；derive `Serialize` / `Deserialize`（`rename_all = "lowercase"`，线值 `"user"` / `"workspace"`）+ `specta::Type`；同时承载信封维度标签与 db 命令 scope 入参 |
| `WorkspaceStores` | `.../infra/store/src/store.rs` | 新增 | 两库注册表公共 struct：全局库实例 + per-root 缓存（`Mutex<HashMap<String, Arc<Store>>>`）+ workspaces 目录；挂 Tauri State |
| `RunStopRegistry` | `.../src/commands/exec/agent.rs` | 修改 | 句柄表键由 `i64` 演进为 `(String, i64)`（root + run id）；`pub(crate)` 方法面同步增 root 参；类型仍经 `commands::exec` 导出挂 State |
| `DbInspectorState` | `packages/desktop/src/views/db/hooks/use-db-inspector.ts` | 修改 | 增 `scope: DbDimension` 与 `setScope` 字段，余字段面不变 |

<!-- 模型 struct（WorkspaceRecord / AgentRunRecord / AgentEventRecord / ExploreRecord）零变化，不列入；UseAgentChatParams / AgentRunHistoryState 面不变 -->

### 配置

| 配置键 | 所在文件 | 类型 | 值 | 说明 |
|--------|----------|------|-----|------|
| `version` | `packages/desktop/package.json` | 修改 | `"0.3.9"`（自 `"0.3.8"`） | 用户可见行为变更：数据隔离 + 运行清单范围收窄 + 存量数据废弃（沿用 desktop 版本口径：仅用户可见变更 bump） |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `WorkspaceRecord` | `root`（PK，canonical 完整路径）/ `name` / `added_at` | — | **全局库**（`home_dir()/.dev-team/desktop-global.redb`，user 维度） |
| `AgentRunRecord` | `id`（PK，写事务内 max+1）/ `prompt` / `cwd` / `env` / `permission_mode` / `status` / `started_at` / `finished_at` / `num_turns` / `cost_usd` / `duration_ms` / `session_id` / `error` / `source` / `source_ref` / `parent_run_id` | `cwd` 恒为当前 workspace root（归属键）；被 `AgentEventRecord` 以 `run_id` N:1 引用；`(source, source_ref)` 反向锚定 explore 记录 | **workspace 库**（`home_dir()/.dev-team/workspaces/{可读段}-{hash32}.redb`，workspace 维度） |
| `AgentEventRecord` | `event_key`（PK，`(run_id as u128) << 64 \| seq` 打包键，serde 十六进制串）/ `run_id`（非唯一二级索引）/ `event`（嵌装 core `AgentEvent`） | N:1 → 同库 `AgentRunRecord`，同 run 内扫描自然序即重放序 | workspace 库（与所属 run 同库，无跨库引用） |
| `ExploreRecord` | `id`（PK，写事务内 max+1）/ `root`（canonical root，归属键）/ `name` / `created_at` / `updated_at` | `root` 归属当前 workspace；被同库 runs 经 `(source="explore", source_ref=id)` 锚定；删除记录同事务级联清名下 runs 与事件 | workspace 库（与名下会话链同库，链还原与级联同实例收敛） |

四模型 struct 字段面与 native_model id / version 全部不动（id 1 / 2 / 3 / 4，version 见 model.rs 现状）；维度语义由 **db 文件归属 + 模型注册分组** 承载，run id 与 explore id 语义随之演进为 workspace 库域内自增（store 实现本就按实例 max+1，拆分后语义自然成立，零额外代码）。

---

## 路由/API 设计

本变更不涉及 HTTP API；以下为 Tauri IPC 命令面（前端经生成绑定 invoke，错误经 `Result<T, String>` reject）。仅列入参面变化的命令。

| 命令 | 类型 | 入参 | 返回 | 说明 |
|------|------|------|------|------|
| `agent_start` | 修改 | `root` `prompt` `permissionMode` `resumeSessionId?` `source?` `sourceRef?` `parentRunId?` + `Channel<AgentRunMessage>` | `Promise<AgentRunRecord>`（提前 resolve running 记录） | 入参面不变；落库路由至当前 workspace 库；blank root reject |
| `agent_stop` | 修改 | `root` `runId` | `Promise<void>` | 新增 `root`；复合键寻址不跨库误停，miss 幂等 |
| `agent_runs` | 修改 | `root` | `Promise<AgentRunRecord[]>` | 新增 `root`；清单为当前 workspace 历史（started_at 降序） |
| `agent_run_events` | 修改 | `root` `runId` | `Promise<AgentEvent[]>` | 新增 `root`；seq 升序重放 |
| `agent_run_chain` | 修改 | `root` `source` `sourceRef` | `Promise<AgentRunRecord[]>` | 新增 `root`；发起顺序链还原 |
| `db_models` | 修改 | `scope`（`"user"` \| `"workspace"`） `root` | `Promise<ModelInfo[]>` | 新增 `scope` / `root`；两库清单互不混列 |
| `db_records` | 修改 | `scope` `root` `model` `offset` `limit` | `Promise<RecordEnvelope[]>` | 新增 `scope` / `root`；分页扫描对应库 |
| `list_workspaces` / `add_workspace` / `remove_workspace` | 修改 | 不变 | 不变 | State 切 `WorkspaceStores`；add 注册后预开校验，remove 文件保留 |
| `scan_explores` / `list_explore_records` / `create_explore_record` / `rename_explore_record` / `delete_explore_record` | 修改 | 不变 | 不变 | 内部路由切 `for_root`；签名与线格式零变化 |

`agent_start` 生成绑定参数序不变（`root` 仍为第 1 个 IPC 参数），`agent-transport.ts` 的 `AgentStartArgs` 索引不漂移。

---

## 依赖

### 运行时依赖

- `sha2 = "0.10"`（**新增**，store crate 直依赖）— workspace 库文件名哈希成分（SHA-256 截断 128-bit）；已在依赖锁内为传递依赖，无新包入树
- `native_db =0.8.2` / `native_model =0.4.20`（既有）— 双库引擎；两组 `Models` 静态注册与 per-实例 `Database<'static>`
- `dunce`（既有）— canonical 口径单点（派生入参前置归一，UNC 前缀 / 大小写策略同源）
- `tauri-plugin-single-instance`（既有）— 单进程约束留痕承接（同库文件双开敞口的既有缓解，非新增敞口）

### 构建/测试依赖

- `tauri-specta` / `specta-typescript`（既有）— 命令入参面变更后 TS bindings 再生成
- `tempfile`（既有）— store / commands 测试临时目录（测试编写与执行归 test-design / test-gen / test-execution 阶段，不在本变更任务）
- `vp` / `knip` / `cargo fmt` / `cargo clippy`（既有）— 守线静态检查

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | store.rs 两组静态注册（全局组仅 `WorkspaceRecord`，workspace 组仅 run / 事件 / explore 三模型）；全局库新文件名 `desktop-global.redb` + `workspaces/` 子树常量；派生单点纯函数（同 root 跨重开同路径为可断言性质） |
| AC-2 | `add_workspace` 走 `global()` 注册；explore 记录与 run 记录命令经 `for_root(root)` 落所属 workspace 库；两组 `Models` 无交叉注册使混入在打开点即不可能 |
| AC-3 | `WorkspaceStores` per-root `Arc<Store>` 缓存复用（`Mutex<HashMap>`），同 root 单开、异根独立；派生收口 `workspace_db_file_name` / `workspace_db_path` 单点，消费侧零派生逻辑 |
| AC-4 | 全新文件组冷启动：全局库换名 + workspace 库新文件，旧 `desktop-store.redb` 无任何代码引用；零迁移 / 零格式探测代码（`Store::open` 既有「空文件视同不存在」逻辑保留，语义不变） |
| AC-5 | 四条命令增 root 入参路由 `for_root`；`RunStopRegistry` 键 `(root, run_id)`；调试页清单由 `agent_runs(root)` 收窄为当前 workspace 历史 |
| AC-6 | `db_models` / `db_records` 增 `DbDimension` scope；信封注册表维度标签过滤；查看器零模型特定代码（分维度是注册表数据行，git diff 无 per-model 分支） |
| AC-7 | `remove_workspace` 仅删注册记录（文件与缓存实例保留）；重加同 root 时 `for_root` 复用实例、`list_explore_records` / `agent_run_chain` 历史完整可读 |
| AC-8 | model.rs 仅文档注释修正；四模型 id / version / 字段面零变化；前端 DTO 仅增 root / scope 入参；`tests/golden` 契约不触碰 |
| AC-9 | 维度声明由 spec 承载（提案阶段已同步）：workspace 维度落全局目录 per-workspace db、不进 repo；workflow 过程数据维持 user 维度裁定，落全局库不受本变更影响 |

---

## 待决问题

- 孤儿 workspace 库文件治理（含 `remove_workspace` 彻底清理入口与旧 `desktop-store.redb` 残留检测提示的交互形态）——proposal 已裁定二期再议，本变更不预留代码挂点
- workspace 库文件级治理 UX（按 workspace 备份 / 搬迁）——分库带来能力的自然延伸，二期另行提案
- 无阻塞性待决：proposal 路由至 dev-design 的 4 项（全局库文件名、workspace 库文件名组成、句柄缓存策略、scope 交互形态）已全部在「关键设计定夺」落定
