# 设计: desktop-redb-store

> **变更**: desktop-redb-store
> **日期**: 2026-09-21

---

## 提案与规格同步状态

`proposal.md` 与 `specs/`（desktop-workspace-store / desktop-crate-layout / desktop-app-shell 三个 spec）已由提案阶段写入并通过评审，均不在本变更实现清单与任务范围内。本设计对提案「待决问题」中标注 dev-design 必须拍板的事项逐项给出决策，见下节。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| store crate（`store`） | redb 本地库，db 边界第一成员：`Store::open` + workspace 四操作 + `schema_version` | `packages/desktop/src-tauri/crates/infra/store/` | redb、dunce、serde、serde_json；禁 Tauri、禁 core crate | Rust + redb 4.3.0 |
| WorkspaceRecord 模型 | 清单记录模型：字段定义、name 提取、JSON 编解码、时间戳取值 | `crates/infra/store/src/model.rs` | serde、serde_json | Rust（纯层，无 IO） |
| canonical 口径 | 路径规范化的单一口径：canonical key 生成 + 消失目录的回退匹配 | `crates/infra/store/src/canonical.rs` | dunce | Rust |
| Store 持久化 | 表定义、事务读写、`schema_version` 写入/校验、`StoreError` | `crates/infra/store/src/store.rs` | redb、model、canonical | Rust |
| workspace 命令轨道 | 四命令薄包装：`State<Store>` 取 store + 参数转换 + `Err(String)` 错误约定 | `packages/desktop/src-tauri/src/commands/workspaces/` | store、tauri | Rust + Tauri 2 |
| 启动装配 | home_dir 解析、`Store::open` 注入、`.manage()`、命令注册、fail fast | `packages/desktop/src-tauri/src/main.rs` | store、commands、tauri | Rust + Tauri 2 |
| WorkspaceRecord DTO | Rust 模型的 TS 镜像 | `packages/desktop/src/types/dto.ts` | — | TypeScript |
| useWorkspaces hook | workspace 清单取数收口：启动自动 load 一次 + add/remove/touch 动作 | `packages/desktop/src/hooks/useWorkspaces.ts` | @tauri-apps/api | React + TS |
| 欢迎屏视图 | 最近 workspace 清单 + 添加新文件夹入口 + 移除动作 | `packages/desktop/src/views/WelcomeView.tsx` | dto、useWorkspaces 类型 | React |
| App 壳 | 自动恢复（收在 useWorkspaces）、Header 下拉切换、移除当前项切换剩余第一名、错误呈现 | `packages/desktop/src/App.tsx` | hooks、views | React |

crate 分层由 `desktop-crate-layout` spec 固化：`desktop-app → infra/store / core/*`；store 自含模型（不依赖 core）、禁 Tauri；core 依赖不到 store（crate 图机械保证）。

---

## 关键设计决策（dev-design 拍板）

提案「待决问题」中要求 dev-design 明确的事项，逐项决策如下：

### D1 Windows canonicalize 单一口径：`dunce::canonicalize`

- **决策**：全链路唯一规范函数 `canonical_key(input: &Path) -> Result<PathBuf, String>`，实现为 `dunce::canonicalize`。存库 key、去重比较、touch/remove 命中、前端展示同源（AC-3）。
- **理由**：`std::fs::canonicalize` 返回 `\\?\D:\...` UNC 前缀，存库与展示均难看；dunce 在安全时去前缀，且底层 canonicalize 返回盘上真实大小写——Windows 大小写不敏感去重免费获得。被否方案「存原样 + 归一化比较」需要维护展示/比较双份表示，且逐组件大小写折叠有 locale 陷阱（如 Turkish I），canonicalize 一次全解。
- **失败语义**：
  - `add_workspace`：目录必须存在，canonicalize 失败 → `Err(StoreError::Canonicalize)`（目录刚由选择器选定，正常不触发）。
  - `touch_workspace` / `remove_workspace`：先 canonicalize；目录已消失（canonicalize 失败）时回退**词法归一化匹配**——入参统一分隔符为 `\`、去 `\\?\` 前缀，与存量 key 逐个 ASCII 大小写不敏感比较。理由：目录消失后用户仍须能移除残留清单项（proposal 风险「目录消失」的用户救济路径），不能因 canonicalize 失败而堵死；前端 touch/remove 回传的是 list 结果中的 canonical root，回退匹配必然命中。
  - 回退仍未命中 → `Ok(false)`（幂等 miss），不报错。
- **已知退化**：超长路径（>260 字符）dunce 保留 `\\?\` 前缀，此时 key 与展示均带前缀——仍为唯一 canonical 形态，可接受。

### D2 db 文件名与落位：`home_dir()/.dev-team/desktop-store.redb`，不建子目录

- **决策**：db 文件名 `desktop-store.redb`，直接位于 `home_dir()/.dev-team` 根，`Store::open` 内部 `create_dir_all` 父目录（redb 建文件不建父目录，首启必须补齐）。
- **理由**：MVP 单文件单租户，子目录是无第二消费者的组织结构；数据维度语义已由表名承载（`user_workspaces` / `user_meta`），文件名带维度会在将来 workspace 维度多文件化时语义混乱。workspace 维度落盘若选定 hash 子目录方案，那时再立子目录结构。

### D3 自动恢复同时 touch：确认「选中即 touch」口径

- **决策**：维持提案口径——选中 workspace（自动恢复、下拉切换、添加后打开）一律触发 `touch_workspace`。与 desktop-app-shell spec「选中 workspace SHALL 触发 touch」一致，不改。
- **理由**：`last_opened_at` 是恢复依据，选中而不 touch 会让恢复顺序漂移；touch 为 fire-and-forget，失败仅呈现错误态、不阻断打开。

### D4 时间戳表示：UTC unix 毫秒 `i64`

- **决策**：`added_at` / `last_opened_at` 均为 UTC unix 毫秒 `i64`，随 record JSON 编码入 value；前端 DTO 为 `number`。
- **理由**：排序零解析零格式歧义（RFC3339 字符串排序在亚秒精度位数可变时不可靠）；MVP 无时刻展示需求，无需 RFC3339；store crate 不引入 `time` 依赖，依赖面保持最小（redb + dunce + serde 系）。与探索笔记中 `OffsetDateTime` 草图的偏差即此：exploration 为形状草图，本设计按依赖最小化收敛。

### D5 store 错误面：`StoreError` 小枚举，命令面仍为 `Result<T, String>`

- **决策**：store 公共 API 返回 `Result<T, StoreError>`，`StoreError` 为 `Db(String)` / `Canonicalize(String)` 两变体，impl `Display` + `std::error::Error`；命令层 `.map_err(|e| e.to_string())` 转为 `Err(String)`。
- **边界澄清**：提案决策「拒绝自定义错误 enum（MVP 过重）」针对的是**命令签名面**——命令面 `Result<T, String>` 约定不变；`StoreError` 是 store crate 内部错误面，不进入命令签名。两个变体即刻使用（canonicalize 失败与 db 读写失败是两类故障模式），错误字符串携带 `db:` / `canonicalize:` 前缀，直接服务 proposal 风险「清单丢失成玄学」的可排查性；同时 redb 错误类型不越过 store 公共面（AC-6 的自有类型）。

### D6 store 公共面收敛：私有模块 + 顶层 `pub use`

- **决策**：store crate 的 `lib.rs` 以私有模块（`mod model; mod canonical; mod store;`）组织，公共面仅 `pub use store::{Store, StoreError};` + `pub use model::WorkspaceRecord;` 三个名字。与 workflow crate 的 `pub mod` 风格有意不同。
- **理由**：AC-6「公共 API 只暴露自有类型、无 redb 类型」用可见性机械保证而非注释纪律；`canonical_key` 等内部机制不升为公共 API。

### D7 欢迎屏提取为视图组件，Header 保留在 App.tsx

- **决策**：新增 `views/WelcomeView.tsx`（欢迎屏是带清单渲染 + 移除动作 + 错误态的完整屏，与既有 `views/ChangeListView.tsx` 同型）；Header 下拉保留在 `App.tsx` 内改造（shell chrome，与 App 状态紧耦合）。欢迎屏清单与 Header 下拉不抽共享渲染组件（各自渲染，避免单用途抽象）。
- **理由**：proposal 实现文件「App.tsx 及欢迎屏 / Header 视图」在此落位；App.tsx 若同时长出欢迎屏清单与下拉会超重。

### D8 记录语义细节

- `add_workspace` = canonicalize + upsert + touch：已存在 → 保留原 `added_at`、刷新 `last_opened_at`；新建 → 两值同为 now。返回落库后的 `WorkspaceRecord`（canonical root 即前端当前 root）。
- `touch_workspace` / `remove_workspace` 返回 `Result<bool, StoreError>`：bool 为是否命中；miss 幂等返回 `Ok(false)`，不算错误。
- `list_workspaces` 排序：`last_opened_at` 降序，并列时按 `root` 字典序升序（顺序确定，测试可复现）；第一名即「上次打开」，不单设 API（AC-4）。
- 表命名携带 user 维度（AC-5）：`user_workspaces`、`user_meta`。

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  覆盖 proposal「变更范围 - 实现文件」全部 8 项（目录级条目展开到具体文件）。
  测试文件不在本清单（由 test-design / test-gen 阶段承接）。
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/infra/store/Cargo.toml` | store crate 清单：crate 名 `store`；`[dependencies]` 仅 redb / dunce / serde / serde_json（均 workspace 引用），`[dev-dependencies]` tempfile；零 Tauri、零 core crate 依赖（AC-1） |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | crate 入口：文档注释（单进程约束显式声明、db 路径注入约定、schema_version 演进说明）+ 私有模块声明 + `pub use` 收敛公共面为 `Store` / `StoreError` / `WorkspaceRecord`（D6） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `WorkspaceRecord` 模型：serde camelCase、`from_root`（name = 目录名最后一段）、JSON 编解码、`now_millis`；纯层无 IO |
| `packages/desktop/src-tauri/crates/infra/store/src/canonical.rs` | canonical 口径（D1）：`canonical_key`（dunce 主口径）+ 消失目录的词法归一化回退匹配 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `Store` + `StoreError`：表定义（`user_workspaces` / `user_meta`）、`open`（create_dir_all + `Database::create` + schema_version 写入/校验）、四操作 |
| `packages/desktop/src-tauri/src/commands/workspaces/mod.rs` | workspace 命令轨道：四命令（`State<'_, Store>` + 私有 `*_inner` 委托 + `map_err(String)`），命令体 = 参数转换 + store 调用 + DTO 返回 |
| `packages/desktop/src/hooks/useWorkspaces.ts` | workspace 清单取数收口 hook：挂载自动 load 一次（唯一例外，无轮询无 watch）；add/remove/touch 成功后内部刷新清单；error 态 |
| `packages/desktop/src/views/WelcomeView.tsx` | 欢迎屏：最近 workspace 清单（悬停 title 完整 path）+「添加新文件夹」入口 + 清单项移除按钮 + error-note 呈现 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/Cargo.toml` | `[workspace].members` 增 `crates/infra/store`；`[workspace.dependencies]` 收敛 `redb = "=4.3.0"`、`dunce = "1"`、`tempfile = "3"`、`store = { path = "crates/infra/store" }`；desktop-app `[dependencies]` 增 `store`、新增 `[dev-dependencies]`（tempfile）；依赖注释更新为「core 与 infra 均禁 Tauri」 | crate 分层扩展为 core/infra/desktop-app 三组（AC-1）；redb 锁定具体版本（proposal 风险「4.3.0 较新」）；不改 `openspec/config.json`（AC-10） |
| `packages/desktop/src-tauri/src/main.rs` | 新增 `.setup()`：`app.path().home_dir()` 解析 → 拼接 `desktop-store.redb` → `Store::open` → `app.manage(store)`；`invoke_handler` 注册四个 workspace 命令 | db 路径注入（store 内无 app data 解析）；setup 返回 Err → `run` Err → `expect` 报错启动失败（AC-8 fail fast，无静默空清单） |
| `packages/desktop/src-tauri/src/commands/mod.rs` | 增 `pub mod workspaces;` | 命令双轨（queries/exec）扩展为三轨；queries/exec 语义不变 |
| `packages/desktop/src/types/dto.ts` | 增 `WorkspaceRecord` interface | Rust 模型 TS 镜像（camelCase），无任何 redb 概念（AC-6） |
| `packages/desktop/src/App.tsx` | workspace 选择升级为「选择与恢复」：自动恢复收在 `useWorkspaces`（一次守卫 ref）、Header 由「切换按钮」改为 workspace 下拉（清单项 title 完整 path + 移除按钮）、移除当前打开项切换到剩余第一名、对话框添加流改经 `useWorkspaces().add`、workspace 错误态呈现 | AC-9 落点；列表/详情取数仍由既有 `useChangeList` / `useChangeDetail` 按 root 变化驱动，不新增取数路径 |

### 删除文件

<!-- 无删除文件（proposal「删除文件：无」），省略此子节 -->

### 公共函数 / API

<!-- identifier：store 公共方法 / Tauri 命令 / 前端导出函数；私有 `*_inner` 不列入 -->

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `Store::open` | `crates/infra/store/src/store.rs` | 新增 | `pub fn open(path: &Path) -> Result<Store, StoreError>` | 打开（不存在则创建）db：`create_dir_all` 父目录 + `Database::create` + META 写入/校验 `schema_version`；打不开即 Err（AC-2/AC-5/AC-8 上游） |
| `Store::add_workspace` | `crates/infra/store/src/store.rs` | 新增 | `pub fn add_workspace(&self, root: &Path) -> Result<WorkspaceRecord, StoreError>` | canonicalize + upsert + touch；等价路径仅一条记录且 `last_opened_at` 刷新（AC-3） |
| `Store::list_workspaces` | `crates/infra/store/src/store.rs` | 新增 | `pub fn list_workspaces(&self) -> Result<Vec<WorkspaceRecord>, StoreError>` | `last_opened_at` 降序（并列按 root 字典序），第一名即最近打开（AC-4） |
| `Store::remove_workspace` | `crates/infra/store/src/store.rs` | 新增 | `pub fn remove_workspace(&self, root: &Path) -> Result<bool, StoreError>` | 按 canonical key 删除（含消失目录回退匹配）；miss 幂等 `Ok(false)`（AC-3） |
| `Store::touch_workspace` | `crates/infra/store/src/store.rs` | 新增 | `pub fn touch_workspace(&self, root: &Path) -> Result<bool, StoreError>` | 刷新 `last_opened_at`；miss 幂等 `Ok(false)`（AC-3） |
| `list_workspaces` | `src/commands/workspaces/mod.rs` | 新增 | `#[tauri::command] pub fn list_workspaces(store: State<'_, Store>) -> Result<Vec<WorkspaceRecord>, String>` | 薄包装：State 取 store → `list_workspaces` → `map_err` 为 `String`（AC-7） |
| `add_workspace` | `src/commands/workspaces/mod.rs` | 新增 | `#[tauri::command] pub fn add_workspace(store: State<'_, Store>, root: String) -> Result<WorkspaceRecord, String>` | 入参 String → `Path` 转 store 调用；返回 canonical 记录（AC-7） |
| `remove_workspace` | `src/commands/workspaces/mod.rs` | 新增 | `#[tauri::command] pub fn remove_workspace(store: State<'_, Store>, root: String) -> Result<bool, String>` | 失败路径 `Err(String)`，前端 error 态接住（AC-7） |
| `touch_workspace` | `src/commands/workspaces/mod.rs` | 新增 | `#[tauri::command] pub fn touch_workspace(store: State<'_, Store>, root: String) -> Result<bool, String>` | 选中即 touch 的命令面（AC-7） |
| `useWorkspaces` | `packages/desktop/src/hooks/useWorkspaces.ts` | 新增 | `function useWorkspaces(): WorkspaceState` | 清单取数收口：挂载自动 load 一次；add/remove/touch 由用户动作触发，成功后内部刷新清单（AC-9） |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `WorkspaceRecord` | `crates/infra/store/src/model.rs` | 新增 | `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "camelCase")]`；字段 `root: String` / `name: String` / `added_at: i64` / `last_opened_at: i64`；value 以 JSON 编码入表（AC-4/AC-6） |
| `StoreError` | `crates/infra/store/src/store.rs` | 新增 | `Db(String)` / `Canonicalize(String)` 两变体；impl `Display` + `std::error::Error`；错误字符串带 `db:` / `canonicalize:` 前缀（D5） |
| `Store` | `crates/infra/store/src/store.rs` | 新增 | 公共 API struct：私有持有 `redb::Database`（`Send + Sync`），公共方法见上表；redb 类型不出现在任何公共签名（AC-6） |
| `WorkspaceState` | `packages/desktop/src/hooks/useWorkspaces.ts` | 新增 | interface：`workspaces: WorkspaceRecord[]` / `loading: boolean` / `error: string | null` / `add(root: string): Promise<WorkspaceRecord | null>` / `remove(root: string): Promise<boolean>` / `touch(root: string): Promise<boolean>`；add 失败返回 `null`，remove/touch 失败返回 `false`，均置 error 态 |
| `WorkspaceRecord` | `packages/desktop/src/types/dto.ts` | 新增 | TS 镜像：`root: string` / `name: string` / `addedAt: number` / `lastOpenedAt: number` |

### 配置

<!-- 无配置键变更：openspec/config.json 在 proposal「不要修改」清单（rust 套件已收敛为 src-tauri 根单条 + cargo test --workspace，新 crate 仅需进 members）；无 plugin.json / hooks.json / settings.json 变更。依赖收敛见「修改文件」与「依赖」节 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `WorkspaceRecord` | `root`（canonical 完整路径，String）/ `name`（目录名最后一段，String）/ `added_at`（UTC unix 毫秒 i64）/ `last_opened_at`（UTC unix 毫秒 i64） | user 维度注册表一行；key 即 `root`，无外键 | redb 表 `user_workspaces`（`TableDefinition<&str, &[u8]>`），value = `serde_json::to_vec(&record)` |
| META `schema_version` | key = `"schema_version"`，value = `1`（`SCHEMA_VERSION: u64` 常量） | 独立于业务表 | redb 表 `user_meta`（`TableDefinition<&str, u64>`）；`Store::open` 时 absent → 写入当前版本，已存在且大于支持版本 → `Err`（为将来迁移留的第一颗牙） |

- **db 文件**：`<home_dir>/.dev-team/desktop-store.redb`（D2）；路径完全来自 `Store::open` 入参，store 内无 app data 解析（desktop-workspace-store spec「db 路径注入」场景）。
- **key 语义**：canonical path 作 key（D1），insert 即 upsert；目录移动 = 删旧加新的两次用户操作，不追踪路径身份连续性。
- **维度语义**：表命名 `user_*` 前缀承载 user 维度，为 workspace 维度表留格（AC-5）。
- **单进程约束**：redb 面向单进程嵌入（进程内 MVCC、单写多读），双开（dev + 正式版指向同一 db）不保证安全；约束以 `lib.rs` crate 文档显式声明，store 不自研跨进程锁，tauri-plugin-single-instance 为后手不进本变更。

---

## 命令面 / API 设计

<!-- 本变更不涉及 HTTP API；下表为 Tauri IPC 命令面（前端 invoke 契约）。本地单用户应用，无认证维度 -->

| invoke 命令 | 输入 | 输出 | 描述 |
|-------------|------|------|------|
| `list_workspaces` | `{}` | `WorkspaceRecord[]`（`last_opened_at` 降序）或 reject（`Err(String)`） | 最近 workspace 清单；第一名即「上次打开」，供欢迎屏与 Header 下拉共用 |
| `add_workspace` | `{ root: string }` | `WorkspaceRecord`（canonical）或 reject | 文件夹选择器选定后入库：canonicalize + upsert + touch |
| `remove_workspace` | `{ root: string }` | `boolean`（是否命中）或 reject | 移除清单项；前端刷新后切换到剩余清单第一名（清单为空时回欢迎屏） |
| `touch_workspace` | `{ root: string }` | `boolean`（是否命中）或 reject | 选中 workspace（自动恢复 / 下拉切换 / 添加后打开）时刷新 `last_opened_at` |

- 既有三查询命令（`list_changes` / `get_change_detail` / `read_artifact`）签名与语义不变；`commands/exec` 保持空轨道。
- 错误约定模板（后续可失败命令照此办理）：命令返回 `Result<T, String>` → Tauri 将 `Err` 转前端 reject → hook error 态 `setError(String(err))` 接住呈现；不静默吞错。

### 前端交互时序

1. **启动**：`useWorkspaces` 挂载自动 `list_workspaces` 一次（唯一自动取数例外）。
2. **自动恢复（一次守卫）**：load 完成 && 尚未恢复过（`restoredRef`）&& 清单非空 → fire-and-forget `touch(first.root)`（先于 `setRoot` 派发，保证「取数 → touch → 以恢复根取列表」时序）→ `setRoot(first.root)`；恢复失败仅 error 态，不阻断列表取数与手动添加路径。守卫的必要性：touch 触发的内部刷新会再次取得非空清单，若无守卫将取数-恢复循环。
3. **root 恒等于清单第一名**：`list_workspaces` 按 `last_opened_at` 降序，hook 每次取数后 `setRoot(result[0]?.root ?? null)`；清单为空（含被移除一空）时 `root` 为 `null` 停欢迎屏。
4. **下拉切换**：`touch(新根)` 刷新 `last_opened_at` → hook 内部刷新 → 新根成为第一名 → `root` 随之切换并清空 change 选中；`root` 变化驱动既有 `useChangeList` / `useChangeDetail` effect 重取，不新增取数路径。
5. **移除**：`remove_workspace` → hook 内部刷新 → `root` 切换到剩余第一名并清空选中；清单为空时 `root` 为 `null` 回欢迎屏。
6. **添加**：文件夹选择器 → `add_workspace`（入库即 touch）→ hook 内部刷新 → 新记录 `last_opened_at` 最新即第一名 → 以返回记录的 canonical `root` 打开（Header 展示与库内 key 同源）。

---

## 依赖

### 运行时依赖

- `redb = "=4.3.0"`（新增，store） — 嵌入式 KV 本地库；进程内 MVCC 单写多读，同步调用无需 async；锁定具体版本保证可复现构建（proposal 风险缓解）
- `dunce = "1"`（新增，store） — Windows canonicalize 去 `\\?\` 前缀的单一口径（D1）；纯 std 零传递依赖
- `serde` / `serde_json`（既有 workspace 收敛，store 引用） — record JSON 编解码
- `store`（新增 path 依赖，desktop-app） — 启动装配与命令轨道的持久化层
- `tauri` / `tauri-plugin-dialog` / `foundation` / `workflow`（既有，不变） — 壳与查询域

### 构建/测试依赖

- `tempfile = "3"`（新增 dev-dependencies：store + desktop-app） — 测试临时 db 文件与临时目录
- `cargo test --workspace`（既有，不变） — 新 crate 进 `[workspace].members` 即被收编，不新增套件注册（AC-10）
- 前端无新增依赖 — `vp test` / `vp build` / `knip` 照常

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | 新增 `crates/infra/store`（store crate 骨架）；store `Cargo.toml` 依赖仅 redb/dunce/serde/serde_json（零 Tauri、零 core）；根 `[workspace].members` 收敛 |
| AC-2 | `Store::open` + add/list/remove/touch 四操作签名与语义（store.rs）；upsert/touch 语义见 D8，重开持久化由 `Database::create` 打开既有文件保证 |
| AC-3 | D1 单一 canonical 口径（dunce）：等价路径 add 去重仅一条且 `last_opened_at` 刷新；等价路径 touch/remove 经 canonical 或回退匹配命中同一条 |
| AC-4 | `list_workspaces` 降序 + root 字典序 tie-break（D8）；`name` = `WorkspaceRecord::from_root` 取目录名最后一段 |
| AC-5 | `user_meta` 表 `schema_version` 打开即写入/校验；表命名 `user_workspaces` / `user_meta` 携带 user 维度 |
| AC-6 | D6 私有模块 + `pub use` 收敛公共面；`Store` 私有持有 `Database`；命令签名与 `types/dto.ts` 均无 redb 类型 |
| AC-7 | `commands/workspaces` 四命令：`State<'_, Store>` 取 store、命令体薄包装、`Err(String)` 经 reject 呈现于 hook error 态；`invoke_handler` 注册见 main.rs |
| AC-8 | main.rs `.setup()` 中 `Store::open` Err → setup Err → `run` Err → `expect` 报错启动失败；无空清单静默降级 |
| AC-9 | App.tsx 自动恢复（一次守卫 ref，收在 useWorkspaces）+ WelcomeView 空态与添加入口 + Header 下拉切换（清空 change 选中、title 完整 path）+ 移除当前项切换剩余第一名（清单空时回欢迎屏） |
| AC-10 | 新 crate 仅进 `[workspace].members` 即被 `cargo test --workspace` 收编；`openspec/config.json` 零改动（实现任务中显式自检） |

---

## 待决问题

- workspace 维度落盘候选（app_data_dir 按 hash 分目录 vs repo 内落盘）—— 长期架构探索（见 `openspec/explores/desktop-backend-architecture.md`），不阻塞本变更；user 维度表名已为其留格
- 图数据库选型与 api 边界用途 —— 等真实痛点出现再进，不在本变更范围
- 进程单实例治理（tauri-plugin-single-instance）—— 已知双开约束的后手，MVP 仅以 `lib.rs` 文档显式声明
