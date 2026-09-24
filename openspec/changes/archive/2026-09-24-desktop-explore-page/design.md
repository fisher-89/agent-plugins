# 设计: desktop-explore-page

> **变更**: desktop-explore-page
> **日期**: 2026-09-24

---

## 全局约束

- **命名隔离**：desktop 包全部产品 `.rs`（`src-tauri/src`、`src-tauri/crates/*/src`；`*_test.rs` 豁免）MUST NOT 含磁盘域根目录名字面量（`foundation/src/layout.rs` 唯一例外，`layout_test.rs` 机械扫描拦截）。本变更全部新增 Rust 文件的注释/字串一律经 `foundation::layout::Layout::explores_root` 派生路径；注释中的 spec 指针一律用「`specs/<capability>/spec.md`，路径相对域根」定式。前端 TS 虽不在扫描面内，目录名知识同样不下沉前端（文件路径经 `explore_doc_path` 布局派生，见 D6）。
- **core/infra 禁 Tauri**：`crates/infra/watch` 零 Tauri，Tauri `Channel` 包装只出现在壳层命令（`commands/watch`）。
- **store 无 legacy 迁移层**：`AgentRunRecord` shape 演进由 native_model 版本机制承担（v1→v2 原地升级），不引入迁移代码；`ExploreRecord` 为新模型新 id，与迁移无关。
- **提案与规格同步状态**：proposal.md 与 6 个 capability spec delta（`openspec/changes/desktop-explore-page/specs/`）已由提案阶段写入并通过评审，不在本 design 变更清单与任务列表内。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| explore 查询读面 | `read_explore` 单文件读取（stem 校验防穿越、缺失 → `None`）+ `scan_explores` 导入扫描（顶层 `*.md`，缺失目录空）；纯读零写入 | `src-tauri/crates/core/workflow/src/queries/explore.rs`（新） | `foundation::layout` | Rust 纯函数 + `std::fs` |
| resume 运行契约 | `AgentRunParams` 增可选续会话参数，CLI flag 组装 `--resume` | `src-tauri/crates/core/agent/src/runner.rs`、`src-tauri/crates/infra/agent/src/flags.rs` | — | Rust |
| store 模型演进 | `ExploreRecord` 新模型（清单/绑定）；`AgentRunRecord` v1→v2 加来源与链字段 | `src-tauri/crates/infra/store/src/model.rs` | native_model 版本机制 | Rust derive |
| store 操作面 | explore 清单 CRUD、`(source, source_ref)` 单链还原查询单点 | `src-tauri/crates/infra/store/src/store.rs` | 模型层 | native_db 事务 |
| 信封注册 | `ExploreRecord` 登记注册表一行，DB 查看器零代码覆盖 | `src-tauri/crates/infra/store/src/envelope.rs` | 模型层 | fn-pointer 注册表 |
| 文件 watch 通道 | notify 单文件订阅 →「文件被修改」信号流（std mpsc）；零内容、零去抖、零 Tauri；`Watcher` drop 即退订 | `src-tauri/crates/infra/watch/`（新 crate） | notify | Rust std mpsc |
| explores 命令轨道 | `read_explore` / `scan_explores`（含已绑定求差）/ `explore_doc_path` / explore 记录 CRUD 薄命令 | `src-tauri/src/commands/explores/mod.rs`（新轨道） | workflow + store | Tauri command |
| watch 命令轨道 | `WatchRegistry` 托管订阅 + mpsc→Tauri `Channel` 桥接线程 | `src-tauri/src/commands/watch/mod.rs`（新轨道） | watch crate | Tauri State + Channel |
| exec 命令扩展 | `agent_start` 增可选 resume/来源参数透传；`agent_run_chain` 链查询；`run_agent()` 编排填充记录来源与链字段 | `src-tauri/src/commands/exec/mod.rs`、`src-tauri/src/commands/exec/agent.rs` | agent + agent-cli + store | Tauri command |
| explore 页面（清单） | store 清单按 root 过滤 + 新建流程（导入扫描绑定 / 新话题建档） | `src/views/explores/ExploreView.tsx`（新）+ `components/ExploreCreateDialog.tsx` + `hooks/useExploreList.ts` | 命令轨道 | React + react-router |
| explore 页面（详情） | resizable 双栏（左对话区右预览）+ 双恢复（磁盘读 + 链重放）+ watch 防抖刷新 | `src/views/explores/ExploreDetailView.tsx`（新）+ `components/`（对话区 / composer / 预览）+ `hooks/`（useExploreDoc / useExploreSession） | 命令轨道 + ui 件 | React + `message-scroller`/`message`/`resizable` |
| stance 前导模板 | 精简 explore stance 拼进 prompt 头部；与 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源注释互链 | `src/lib/exploreStance.ts`（新） | — | TS 纯函数 |
| 路由与导航 | `/explores`、`/explores/:name` 路由 + 侧栏 [探索] 入口 | `src/routes.tsx`、`src/components/AppSidebar.tsx` | — | react-router NavLink |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。路径基准：src-tauri/* 为 packages/desktop/src-tauri/ 下，src/* 为 packages/desktop/src/ 下。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `src-tauri/crates/core/workflow/src/queries/explore.rs` | explore 读面：`read_explore` / `scan_explores` / `ExploreDoc` / `ExploreScanEntry` / stem 校验；纯读、零 Tauri |
| `src-tauri/crates/infra/watch/Cargo.toml` | 新 crate（裸名 `watch`）manifest：notify 依赖 + tempfile（dev-dep） |
| `src-tauri/crates/infra/watch/src/lib.rs` | 通道契约：`FileWatchSignal`（仅修改信号，无内容）、`subscribe(path, sender) -> Watcher`、`Watcher` 持 notify watcher 与 mpsc Sender（drop 即停流退订）；目标缺失不报错；平台边界注释（Windows 父目录语义 / inotify rename 陷阱留痕） |
| `src-tauri/src/commands/explores/mod.rs` | 新轨道：`read_explore` / `scan_explores` / `explore_doc_path` / `list_explore_records` / `create_explore_record` / `rename_explore_record` / `delete_explore_record`；blank root 空结果纪律照 `commands/queries/mod.rs` 的 `is_blank_root` 口径 |
| `src-tauri/src/commands/watch/mod.rs` | 新轨道：`WatchRegistry`（`State` 托管，canonical path 键幂等）+ `watch_subscribe` / `watch_unsubscribe`；mpsc → Tauri `Channel<FileWatchEvent>` 桥接线程 |
| `src/views/explores/ExploreView.tsx` | 路由双态容器 + 清单页 + 新建入口（照 `ChangeView` 的 params 派生 / 切换 replace 过渡抑制模式） |
| `src/views/explores/ExploreDetailView.tsx` | resizable 双栏详情：左对话区右预览；watch 订阅生命周期 = 页面生命周期 |
| `src/views/explores/components/ExploreConversation.tsx` | 对话区：`message-scroller` 滚动承载 + 事件→气泡映射（Text→markdown、Thinking→折叠、ToolUse→卡片、ToolResult 成对）+ `AskUserQuestion` 静态卡片 |
| `src/views/explores/components/ExploreComposer.tsx` | composer：prompt 输入 + env / permission-mode 档位（沿用调试页语义与默认值 default + bypassPermissions） |
| `src/views/explores/components/ExplorePreview.tsx` | 预览：复用 `MarkdownDocRenderer` 渲染 `read_explore` 文本；未落盘/已删除空态 |
| `src/views/explores/components/ExploreCreateDialog.tsx` | 新建对话框：两入口——导入扫描（未绑定 `*.md` 选中即建档）/ 新话题（仅建记录，不落盘文件） |
| `src/views/explores/hooks/useExploreList.ts` | 清单取数（store 按 root）+ create / rename / delete 动作封装 |
| `src/views/explores/hooks/useExploreDoc.ts` | 文档拉取（`read_explore`）+ watch 订阅（`explore_doc_path` 取路径 → `watch_subscribe`）+ 信号防抖（500ms trailing）显式重拉 + 卸载退订 |
| `src/views/explores/hooks/useExploreSession.ts` | 会话链：`agent_run_chain` 还原 + 逐 run `agent_run_events` 重放；发送拼 stance、以链尾 `session_id` 作 `resume_session_id`、携带 source 三元组；run 终态定点重读文档 |
| `src/lib/exploreStance.ts` | `buildExplorePrompt`：精简 explore stance 前导模板；注释与 SKILL.md 双源互链 |
| `src/components/ui/message-scroller.tsx` | shadcn registry 件（`pnpm dlx shadcn@latest add message-scroller message` 生成；官方 chat primitives，aria 家族） |
| `src/components/ui/message.tsx` | 同上配套件 |
| `src/components/ui/resizable.tsx` | shadcn registry 件（`pnpm dlx shadcn@latest add resizable` 生成，底层 react-resizable-panels） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `src-tauri/crates/core/workflow/src/queries/mod.rs` | `pub mod explore;` + 导出 explore 读面；既有私有 `is_change_name` 提升为 `pub(crate) fn is_single_component_name(name: &str) -> bool`，`locate_change` 与 `read_explore` 共用 | 单分量名校验口径单点化，防两处漂移 |
| `src-tauri/crates/core/agent/src/runner.rs` | `AgentRunParams` 增 `resume_session_id: Option<String>` | 唯一进 runner 契约的新参数（它改变 CLI 行为）；来源三元组不进契约（见 D4） |
| `src-tauri/crates/infra/agent/src/flags.rs` | `build_args` 尾部追加 `--resume <id>`（`None` 时无此 flag）；模块文档 MVP 边界措辞同步（移除「无 resume」） | 其余 flag 组装不变 |
| `src-tauri/crates/infra/store/src/model.rs` | 新增 `ExploreRecord`（`#[native_model(id = 4, version = 1)]` + `#[native_db]`，字段见数据模型）；`AgentRunRecord` 演进 `version = 2` 加三字段（serde default），保留 v1 版本化结构（非 `#[native_db]` 注册）并实现 v1→v2 升级转换（bincode 非自描述，必须多版本结构，见 D9） | 首个字段演进的既有模型 |
| `src-tauri/crates/infra/store/src/store.rs` | `models()` 登记 `ExploreRecord`；新增 explore 清单操作面五方法 + `restore_run_chain` 单链还原 | 见公共函数表 |
| `src-tauri/crates/infra/store/src/envelope.rs` | `MODEL_ENTRIES` 登记 `explore` 行（count / scan / key_of 三元组） | 信封 API 函数本体零改动，登记即覆盖（AC-3） |
| `src-tauri/src/commands/exec/mod.rs` | `agent_start` 增四个可选参数并组装 `AgentRunParams` + `RunProvenance`；新增 `agent_run_chain` 查询薄包装；轨道文档由「首批三命令」更新 | 调试页不传新参数行为不变（Tauri Option 参数缺省 `None`） |
| `src-tauri/src/commands/exec/agent.rs` | 新增 `RunProvenance`（`source` / `source_ref` / `parent_run_id`，含 `debug()` 构造）；`run_agent` / `run_agent_with` 增 provenance 入参，`running` 记录初值填充来源与链字段 | 编排填充点单点；假 runner 测试缝不变 |
| `src-tauri/src/commands/mod.rs` | `pub mod explores;` `pub mod watch;` + 轨道清单文档更新 | — |
| `src-tauri/src/main.rs` | `generate_handler!` 登记 10 条新命令 | `watch_subscribe` 的 `WatchRegistry` 在 setup `app.manage` |
| `src-tauri/Cargo.toml`（根） | `[workspace] members` 增 `crates/infra/watch`；`[workspace.dependencies]` 增 `watch = { path = "crates/infra/watch" }` 与 `notify = "8"`；根包 `[dependencies]` 增 `watch` | notify 版本收敛于 workspace 级 |
| `src/routes.tsx` | 路由表增 `/explores` 与 `/explores/:name`（`ExploreView`，prop `root`） | 未知路径回 `/changes` 不变 |
| `src/components/AppSidebar.tsx` | 「页面」组增 [探索] `NavLink`（`data-testid="nav-explores"`，icon 取 lucide `Compass`），active 由 URL 派生（`/explores` 与 `/explores/:name` 均 active） | 既有 testid 语义不变 |
| `src/types/dto.ts` | 新增 `ExploreRecord` / `ExploreDoc` / `ExploreScanEntry` / `FileWatchEvent`；`AgentRunRecord` 增 `source` / `sourceRef` / `parentRunId` | 与 store 自有类型 serde camelCase 一一对应 |
| `package.json` | `pnpm dlx shadcn@latest add message-scroller message resizable` 写入依赖（`react-resizable-panels`、`@shadcn/react` 及件的其他传递依赖，以 add 实际写入为准） | 依赖变更唯一入口 |
| `components.json` | 核验项：预期零改动（既有 new-york / Tailwind v4 / alias 配置直接可用） | 若 add 调整了配置以落地输出为准 |

<!-- 删除文件：无（全新增 + 既有文件原地演进），省略此子节 -->

### 公共函数 / API

<!-- 仅列模块级导出函数、Tauri 命令与导出 hook；pub(crate) 内部 helper 不列。签名省略 State/生命周期省略号。 -->

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `read_explore` | `src-tauri/crates/core/workflow/src/queries/explore.rs` | 新增 | `pub fn read_explore(layout: &Layout, name: &str) -> Option<ExploreDoc>` | stem 单分量校验（防穿越）→ 读 `explores_root/<name>.md` UTF-8 文本；缺失/非 UTF-8 → `None`；纯读 |
| `scan_explores` | `src-tauri/crates/core/workflow/src/queries/explore.rs` | 新增 | `pub fn scan_explores(layout: &Layout) -> Vec<ExploreScanEntry>` | 列 explores_root 顶层 `*.md`（stem 升序 + 修改时间）；目录缺失返回空；不做绑定过滤（store 域，workflow 不认识 store） |
| `build_args` | `src-tauri/crates/infra/agent/src/flags.rs` | 修改 | `pub fn build_args(params: &AgentRunParams) -> Vec<String>` | 签名不变；`resume_session_id = Some(id)` 时追加 `--resume <id>`，`None` 时与既有序列完全一致 |
| `Store::list_explore_records` | `src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn list_explore_records(&self, root: &str) -> Result<Vec<ExploreRecord>, StoreError>` | 按 root 过滤，主键 id 升序（稳定序）；root 为记录归属键（前端持有的 canonical root，store 不二次 canonicalize） |
| `Store::find_explore_record` | `src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn find_explore_record(&self, root: &str, name: &str) -> Result<Option<ExploreRecord>, StoreError>` | 详情页按 URL `name` 寻址记录 |
| `Store::create_explore_record` | `src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn create_explore_record(&self, root: &str, name: &str) -> Result<ExploreRecord, StoreError>` | 写事务内 `max(id)+1` 分配；name 校验单分量；同 `(root, name)` 重复建档 → `Err`；`created_at = updated_at = now`；只写 DB，不触磁盘 |
| `Store::rename_explore_record` | `src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn rename_explore_record(&self, root: &str, name: &str, new_name: &str) -> Result<ExploreRecord, StoreError>` | in-place 改 `name`（保主键 → 保 `source_ref` 链绑定），`updated_at` 刷新；目标名已存在 → `Err`。删+重建会产生新主键破链，故改名必须 in-place（见 D2） |
| `Store::delete_explore_record` | `src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn delete_explore_record(&self, root: &str, name: &str) -> Result<bool, StoreError>` | 删记录不动磁盘文件；miss 幂等 `Ok(false)` |
| `Store::restore_run_chain` | `src-tauri/crates/infra/store/src/store.rs` | 新增 | `pub fn restore_run_chain(&self, source: &str, source_ref: &str) -> Result<Vec<AgentRunRecord>, StoreError>` | 链还原单点收口：全表读过滤 `(source, source_ref)` → 取 `(started_at, id)` 最新为链头 → 沿 `parent_run_id` 回溯（visited 集防环）→ 反转为发起顺序；无链返回空 `Vec` |
| `watch::subscribe` | `src-tauri/crates/infra/watch/src/lib.rs` | 新增 | `pub fn subscribe(path: PathBuf, sender: mpsc::Sender<FileWatchSignal>) -> Result<Watcher, WatchError>` | notify 订阅单文件；目标不存在不报错（信号流就绪，文件出现后事件生效）；`Watcher` 持 notify watcher 与 `sender`，drop 即停流（退订语义） |
| `read_explore`（命令） | `src-tauri/src/commands/explores/mod.rs` | 新增 | `pub fn read_explore(root: String, name: String) -> Option<ExploreDoc>` | blank root → `None`；三件事薄包装 |
| `scan_explores`（命令） | `src-tauri/src/commands/explores/mod.rs` | 新增 | `pub fn scan_explores(root: String, store: State<'_, Store>) -> Result<Vec<ExploreScanEntry>, String>` | workflow 扫描后以 store 清单求差滤除已绑定 stem（绑定过滤在命令层）；blank root → `Ok(vec![])` |
| `explore_doc_path` | `src-tauri/src/commands/explores/mod.rs` | 新增 | `pub fn explore_doc_path(root: String, name: String) -> Option<String>` | 布局派生 `explores_root/<name>.md` 完整路径（无 IO，stem 校验同口径）；供前端 watch 订阅取路径，目录名知识不下沉前端（D6） |
| `list_explore_records`（命令） | `src-tauri/src/commands/explores/mod.rs` | 新增 | `pub fn list_explore_records(store: State<'_, Store>, root: String) -> Result<Vec<ExploreRecord>, String>` | blank root → `Ok(vec![])` |
| `create_explore_record`（命令） | `src-tauri/src/commands/explores/mod.rs` | 新增 | `pub fn create_explore_record(store: State<'_, Store>, root: String, name: String) -> Result<ExploreRecord, String>` | 新话题建档 / 导入绑定共用 |
| `rename_explore_record`（命令） | `src-tauri/src/commands/explores/mod.rs` | 新增 | `pub fn rename_explore_record(store: State<'_, Store>, root: String, name: String, new_name: String) -> Result<ExploreRecord, String>` | 文件改名后的记录重关联入口 |
| `delete_explore_record`（命令） | `src-tauri/src/commands/explores/mod.rs` | 新增 | `pub fn delete_explore_record(store: State<'_, Store>, root: String, name: String) -> Result<bool, String>` | — |
| `watch_subscribe` | `src-tauri/src/commands/watch/mod.rs` | 新增 | `pub fn watch_subscribe(registry: State<'_, WatchRegistry>, on_event: Channel<FileWatchEvent>, path: String) -> Result<u64, String>` | 建订阅并绑定 Channel；同一路径重复订阅幂等（canonical path 键命中即返回既有 subscription_id，不产生重复信号）；目标缺失不报错 |
| `watch_unsubscribe` | `src-tauri/src/commands/watch/mod.rs` | 新增 | `pub fn watch_unsubscribe(registry: State<'_, WatchRegistry>, subscription_id: u64) -> Result<bool, String>` | 移除并 drop `Watcher`（停流 + 桥接线程随通道关闭退出）；miss 幂等 `Ok(false)` |
| `agent_start` | `src-tauri/src/commands/exec/mod.rs` | 修改 | `pub async fn agent_start(store: State<'_, Store>, on_event: Channel<AgentEvent>, root: String, prompt: String, env: AgentEnvMode, permission_mode: AgentPermissionMode, resume_session_id: Option<String>, source: Option<String>, source_ref: Option<String>, parent_run_id: Option<i64>) -> Result<AgentRunRecord, String>` | 四个新参均可选；`source` 缺省 `debug`；调试页 invoke（不传新参）行为与现状一致 |
| `agent_run_chain` | `src-tauri/src/commands/exec/mod.rs` | 新增 | `pub fn agent_run_chain(store: State<'_, Store>, source: String, source_ref: String) -> Result<Vec<AgentRunRecord>, String>` | `restore_run_chain` 的命令薄包装（通用面，非 explore 专属） |
| `buildExplorePrompt` | `src/lib/exploreStance.ts` | 新增 | `function buildExplorePrompt(userInput: string): string` | stance 前导 + 用户输入；每条 explore run 的 prompt 均拼前导 |
| `useExploreList` | `src/views/explores/hooks/useExploreList.ts` | 新增 | `function useExploreList(root: string | null): ExploreListState` | `records` / `loading` / `error` / `refresh` / `create` / `rename` / `remove`；root 变更与显式动作触发取数，无轮询 |
| `useExploreDoc` | `src/views/explores/hooks/useExploreDoc.ts` | 新增 | `function useExploreDoc(root: string | null, name: string | null): ExploreDocState` | `doc` / `loading` / `error` / `refresh`；内聚 watch 订阅与防抖刷新（订阅一次随页面生命周期） |
| `useExploreSession` | `src/views/explores/hooks/useExploreSession.ts` | 新增 | `function useExploreSession(root: string | null, record: ExploreRecord | null): ExploreSessionState` | `chain`（发起序 run 记录）/ `events`（重放 + 实时累积）/ `running` / `error` / `send`；`send` 内拼 stance、取链尾 `sessionId` 续话、携带来源三元组 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `ExploreRecord` | `src-tauri/crates/infra/store/src/model.rs` | 新增 | `#[native_model(id = 4, version = 1)]` + `#[native_db]`，serde camelCase：`id: i64`（`#[primary_key]`，max+1）、`root: String`（workspace 归属）、`name: String`（展示名 = 文件 stem，寻址键）、`created_at: i64` / `updated_at: i64`（UTC unix 毫秒）；user 维度声明写入模型文档注释 |
| `AgentRunRecord` | `src-tauri/crates/infra/store/src/model.rs` | 修改 | `version = 2`；新增 `source: String`（`#[serde(default = "default_run_source")]` → `"debug"`）、`source_ref: Option<String>`（`#[serde(default)]`，explore 指向 `ExploreRecord.id` 的十进制字符串）、`parent_run_id: Option<i64>`（`#[serde(default)]`）；同时保留同 id `version = 1` 的 v1 版本化结构（13 字段，不注册 `#[native_db]`）供升级转换与 v1 fixture 测试 |
| `RunProvenance` | `src-tauri/src/commands/exec/agent.rs` | 新增 | `pub(crate) struct RunProvenance { source: String, source_ref: Option<String>, parent_run_id: Option<i64> }` + `pub(crate) fn debug() -> Self`；app 层微形态，不进 runner 契约 |
| `AgentRunParams` | `src-tauri/crates/core/agent/src/runner.rs` | 修改 | 增 `resume_session_id: Option<String>`（`Eq` derive 保持成立） |
| `ExploreDoc` | `src-tauri/crates/core/workflow/src/queries/explore.rs` | 新增 | `{ name: String, content: String }`，serde camelCase；纯文本 DTO，MUST NOT 套 `ArtifactEnvelope` |
| `ExploreScanEntry` | `src-tauri/crates/core/workflow/src/queries/explore.rs` | 新增 | `{ name: String, modified_at: Option<i64> }`（unix 毫秒，metadata 不可得为 `None`） |
| `FileWatchSignal` | `src-tauri/crates/infra/watch/src/lib.rs` | 新增 | `{ path: String }`——失效信号，无内容字节；serde camelCase 与桥接后 `FileWatchEvent` 同构 |
| `Watcher` | `src-tauri/crates/infra/watch/src/lib.rs` | 新增 | 持 notify watcher 与 mpsc Sender 的句柄；drop 即停流退订 |
| `WatchRegistry` | `src-tauri/src/commands/watch/mod.rs` | 新增 | `Mutex<HashMap<u64, WatchSubscription>>`（键 `subscription_id`，值含 `Watcher` 与订阅路径）；`State` 托管，main setup `manage` |
| `FileWatchEvent` | `src/types/dto.ts` | 新增 | `{ path: string }`（Channel 载荷镜像） |
| `ExploreRecord` / `ExploreDoc` / `ExploreScanEntry`（TS） | `src/types/dto.ts` | 新增 | `{ id: number; root: string; name: string; createdAt: number; updatedAt: number }` / `{ name: string; content: string }` / `{ name: string; modifiedAt: number \| null }` |
| `AgentRunRecord`（TS） | `src/types/dto.ts` | 修改 | 增 `source: string; sourceRef: string \| null; parentRunId: number \| null` |

<!-- 配置：本变更无配置键变更（components.json 预期零改动；依赖变更见「依赖」节与 package.json 行），省略此子节 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `ExploreRecord`（新，user db） | `id`(PK, max+1) / `root` / `name` / `created_at` / `updated_at` | `root` → `WorkspaceRecord.root`（逻辑归属，无外键）；被 `AgentRunRecord.source_ref`（`source="explore"` 时）指向（十进制 id 串） | app data dir user db（`models()` 注册 + 信封注册表一行） |
| `AgentRunRecord`（v2） | 既有 13 字段 + `source` / `source_ref` / `parent_run_id` | `parent_run_id` → `AgentRunRecord.id`（resume 链显式指针）；`source_ref` → `ExploreRecord.id`（explore 来源） | 同上；v1 记录经 native_model 读路径自动升级（`source` 缺省 `debug`），事件表零改动 |
| `AgentEventRecord`（不动） | `event_key`(PK) / `run_id`(二级索引) / `event` | `run_id` → `AgentRunRecord.id`（全局锚定，链重放按链序逐 run 取事件） | 同上，serde_json 编解码不变 |
| `WorkspaceRecord`（不动） | `root`(PK) / `name` / `added_at` | explore 清单的过滤键 | 同上 |
| explore.md（磁盘，非 DB 模型） | 自由 markdown | `ExploreRecord.name` 派生路径 `<root>/<explores_root>/<name>.md`；记录是身份、文件是可丢弃投影（懒创建 / 孤儿保留） | workspace repo，内容唯一真源 |

数据三分：**记录**（`ExploreRecord`，DB）/ **内容**（`explore.md`，磁盘）/ **对话**（`AgentRunRecord` + 事件，DB）。三者互不依赖对方存在（双恢复语义的基础）。

---

## 路由/API 设计

<!-- 非 HTTP API；本节承载前端路由表与 Tauri IPC 命令面（推送语义归类）。命令完整签名见「公共函数 / API」。 -->

前端路由表（react-router HashRouter，路由表声明独立组件，`max-lines-per-function: 50` 约束不变）：

| 路径 | 组件 | 说明 |
|------|------|------|
| `/explores` | `ExploreView` | 清单页（store 清单按当前 root 过滤 + 新建入口） |
| `/explores/:name` | `ExploreView`（内渲染 `ExploreDetailView`） | 详情页；`name` 为路由参数（记录寻址键）；workspace 切换 replace 回 `/explores`（照 `ChangeView` 过渡抑制模式） |
| `/`、未知路径 | — | 重定向 `/changes`，不变 |

Tauri IPC 命令面（错误模板 `Result<T, String>`；`Err` 抵达前端 reject）：

| 命令 | 轨道 | 类别 | 推送语义 |
|------|------|------|----------|
| `read_explore` / `scan_explores` / `explore_doc_path` / `list_explore_records` / `create_explore_record` / `rename_explore_record` / `delete_explore_record` | `commands/explores` | 查询 + 显式写（仅 DB 记录） | 显式取数 |
| `watch_subscribe` / `watch_unsubscribe` | `commands/watch` | 订阅管理 | **失效信号通道**（第二个被认可的推送语义；通知无内容，前端防抖后显式 `read_explore` 拉取） |
| `agent_start`（扩展） | `commands/exec` | 执行 | 执行流通道（命令作用域 Channel 实时流，既有语义不变） |
| `agent_run_chain` | `commands/exec` | 查询 | 显式取数 |

---

## 依赖

### 运行时依赖

- `notify = "8"` — 文件系统事件订阅（watch crate 唯一新增 Rust 依赖；workspace 级收敛版本）
- `react-resizable-panels` — `resizable` 件底层（shadcn add 写入 package.json）
- `@shadcn/react` — `message-scroller`/`message` registry 件的依赖（官方 registry 已核实二者存在、属 aria 家族 chat primitives；传递依赖以 add 实际写入为准）

### 构建/测试依赖

- `tempfile`（既有 workspace 依赖）— watch crate dev-dependency，信号流测试临时目录复用
- 无其他新增；TS 工具链维持 vite-plus（`vp check` / `vp test`），Rust 套件注册于 src-tauri 根（`cargo test --workspace` 自动覆盖新 crate，无需逐 crate 注册）

---

## 关键设计决策

提案「待决问题」五项在本节落定：

- **D1 会话链锚形态 → `(source, source_ref)` 派生，不存 `head_run_id`**：`ExploreRecord` 不加链锚字段。O(1) 链头需 run 终态与记录双写（`finish_agent_run` 处反向更新 explore 记录），引入跨模型写事务一致性负担；派生方案靠 `restore_run_chain` 全表过滤 + 回溯，本机桌面量级下与既有 `list_agent_runs`（全表读 + 内存排序）同哲学。链还原收口 store 单点，前端 hook 不拼链。
- **D2 记录 ↔ 磁盘路径关联 → 以 `name` 派生，不设独立路径字段**：`<name>.md` 由 `explores_root` 派生，导入（名 = stem）与新话题（名 = 主题名，agent 按 skill 指引落盘同名文件）两种来源自然统一，零双写。文件改名场景由 `rename_explore_record` in-place 更新记录 `name` 兑现「主键不变、链绑定不破、更新文件关联后重新可读」——删 + 重建会分配新主键导致 `source_ref` 断链，故改名必须 in-place（这也是提供 rename 操作面的唯一理由；MVP 无 rename UI 不妨碍命令/ store 面完备）。
- **D3 run 来源检索形态 → 无 native_db 二级索引，全表读 + 内存过滤**：v2 若给 `AgentRunRecord` 加 `#[secondary_key]` 属 native_db 模型定义变更（非 native_model 值版本），存量库存在打开期索引失配风险，与「无 legacy 迁移层」纪律冲突；量级论证同 D1。**调试页不加 source 徽标**（`views/agent/` 零改动；全局跨源视图留未来变更）。
- **D4 来源字段流向 → `resume_session_id` 进 runner 契约，来源三元组走旁路**：只有 resume 改变 CLI 行为（flag 组装），故进 `AgentRunParams`；`source` / `source_ref` / `parent_run_id` 是 store 记录面元数据，经 `RunProvenance` 由 `run_agent()` 编排填充，`AgentRunner` trait 面保持「逻辑运行参数」纯净。`parent_run_id` 由前端显式传链尾 run id（发送前已知），编排无回查。resume 的 fork/延续歧义由链模型免疫：链还原只认 `parent_run_id` 显式指针，`session_id` 仅作 CLI 续话入参，两者口径解耦。
- **D5 watch 生命周期与防抖 → 订阅一次随页面生命周期，防抖 500ms trailing 前端持有**：详情页挂载时经 `explore_doc_path` 取路径并 `watch_subscribe`；未落盘目标照常订阅（后端不报错，文件出现后信号生效）；Windows 父目录语义下文件删除/重建不丢事件，页面生命周期内**无中途重挂**（inotify 平台的 rename 失效属跨平台议题，留痕不预建）。信号到达防抖 500ms 合并为一次 `read_explore` 拉取；后端零去抖。重复订阅以 canonical path 为键幂等。
- **D6 目录名知识不下沉前端 → `explore_doc_path` 布局派生命令**：watch 通道保持通用 `path` 入参（不绑 explore 语义）；explore 消费侧所需的文件路径由命令层经 `Layout` 派生返回，前端零目录名常量，与「全局约束」的命名隔离纪律同构。
- **D7 stance 注入 → 内置前导模板逐条拼接**：每条 explore run 的 prompt 都以 stance 前导开头（含 resume 续话轮），用户输入随后；模板注释与 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源互链，归属 desktop 包、接受漂移（提案已拍板）。调试页 run 的 prompt 不含模板。
- **D8 registry 件落地 → 已核实存在，实现首任务验证形态**：官方 registry index 确认 `message-scroller` / `message`（aria 家族 chat primitives，`message-scroller` 依赖 `@shadcn/react`）与 `resizable`（依赖 `react-resizable-panels`）存在。add 成功后以实际组件 API 装配；若 `message-scroller` 形态与需求不符，退路为自实现滚动容器（turn 锚定 + 流式跟随 + 历史前置不跳）并在本节回填记录。注意：`message-scroller` / `message` 为 aria 家族件，若其滚动/聚焦行为引入 React Aria 运行时约定，气泡内容渲染仍留在应用层（组件只管滚动，消息状态不进组件）。**实现期回填**：`shadcn add` 成功落地三件；`@shadcn/react@0.3.1` 为零依赖 unstyled primitives（无 React Aria 运行时面），message-scroller 形态（Provider/Root/Viewport/Content/Item/Button + `preserveScrollOnPrepend`）与需求相符，未走自实现退路；registry 件按本包先例 vendored 内部化收敛（`cn` 导入改 `@/lib/utils`、Button 变体对齐本地件、未消费导出按 knip 纪律删减），add 顺带写入的 `cn` / `radix-ui` 依赖因零消费移除。
- **D9 `AgentRunRecord` v1→v2 演进机制 → 多版本结构 + 升级转换**：落库编码为 bincode（非自描述），v1 载荷（13 字段）无法直接反序列化为 v2 结构（16 字段），必须在 `model.rs` 保留 `version = 1` 的 v1 结构并实现到 v2 的转换，由 native_model 版本机制在读路径自动升级、写路径落 v2（升级 API 的具体形式——`From` 转换或 `native_model::upgrade` trait——以 native_model 0.4.20 文档为准，见待决问题）。新字段全部可缺省：`source` serde default `debug`，其余 `Option` + default。`AgentEventRecord`（version 1）与信封 API 零改动。**实现期回填**：0.4.20 的升级形式为宏属性 `#[native_model(id = 2, version = 2, from = AgentRunRecordV1)]` + `From<AgentRunRecordV1> for AgentRunRecord` 转换（读路径 `from::native_model_decode_upgrade_body(...).map(Into::into)` 自动升级）；宏生成的 downgrade 编码路径同时约束反向 `From<AgentRunRecord> for AgentRunRecordV1`（应用只升级不降级，反向转换仅满足该 trait 约束）。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | `read_explore` 查询（stem 校验复用 `is_single_component_name`、缺失 → `None`、纯读）+ `commands/explores::read_explore`（blank root → `None`） |
| AC-2 | `scan_explores`（顶层 `*.md` stem、缺失目录空）+ 命令层以 store 清单求差滤除已绑定 |
| AC-3 | `ExploreRecord` 独立主键 + `list/find/create/rename/delete` 操作面（孤儿保留、in-place 改名保链）+ `envelope.rs` 登记行零代码覆盖 |
| AC-4 | `AgentRunRecord` v2 演进（多版本结构 + serde default，`source` 缺省 `debug`）+ 新写入经 `RunProvenance` 携带三字段 |
| AC-5 | `Store::restore_run_chain`（链头定位 → `parent_run_id` 回溯 → 发起序）+ `agent_run_events` 逐 run 重放 |
| AC-6 | `build_args` 尾部追加 `--resume <id>`（None 无 flag、其余不变）；调试页不传新参 → `AgentRunParams`/`RunProvenance` 取既有缺省值 |
| AC-7 | `watch_subscribe`/`watch_unsubscribe` + `FileWatchSignal`/`FileWatchEvent` 无内容 + 前端 500ms 防抖 → 显式 `read_explore` + 卸载退订 |
| AC-8 | `/explores` 清单（store 按 root 过滤）+ `ExploreCreateDialog` 导入绑定 / 新话题建档（只写 DB，应用零落盘） |
| AC-9 | `ExploreDetailView` resizable 双栏 + `ExploreConversation` 四变体映射 + `AskUserQuestion` 静态卡片 + `useExploreSession` 开链重放 / stance 拼接 / resume 续话 |
| AC-10 | `routes.tsx` 两路由 + `nav-explores`（URL 派生 active）+ workspace 切换 replace 回清单（`ChangeView` 模式平移）+ 未知路径回 `/changes` 不变 |
| AC-11 | 收尾任务：`cargo fmt` / `clippy`、`vp check` / knip（无豁免新增）；测试全绿由测试阶段兑现 |

---

## 待决问题

- **native_model 0.4.20 多版本升级 API 的具体形式**（`From` 转换 vs `native_model::upgrade` trait 实现）——已落定：`from` 属性 + `From` 转换（见 D9 回填）。
- **CLI `--resume` 的 fork/延续语义 E2E 结论**——设计已按 `parent_run_id` 免疫该歧义（D4），实现阶段首条链实测一次并在本节回填实测结果（链重放与实际对话是否一致）。（留待测试阶段实测回填）
- **`message-scroller` / `message` 的组件 API 形态**——已落定：add 成功、形态相符，未走退路（见 D8 回填）。
- **`@shadcn/react` 传递依赖的实际集合**——已落定：零依赖 unstyled primitives，无更重 aria 运行时面（见 D8 回填）。
