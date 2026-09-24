# 任务: desktop-explore-page

> 依赖顺序：store 域 → 查询/执行契约 → watch 通道 → 命令层 → 前端基础 → hooks → 组件与页面 → 路由导航 → 管线收尾。
> 测试编写属独立工作流阶段，本列表不含；文件路径基准同 design.md 变更清单（`src-tauri/*` 为 packages/desktop/src-tauri/ 下，`src/*` 为 packages/desktop/src/ 下）。

## 阶段一：store 域（模型与操作面）

- [x] `src-tauri/crates/infra/store/src/model.rs`：`AgentRunRecord` 演进 `version = 2`，新增 `source`（serde default `"debug"`）/ `source_ref: Option<String>` / `parent_run_id: Option<i64>`（均 serde default）；保留同 id `version = 1` 的 v1 版本化结构（13 字段，不注册 `#[native_db]`）并实现 v1→v2 升级转换（形式以 native_model 0.4.20 API 为准，回填 design D9）
- [x] `src-tauri/crates/infra/store/src/model.rs`：新增 `ExploreRecord`（`#[native_model(id = 4, version = 1)]` + `#[native_db]`，serde camelCase）：`id: i64` 主键 / `root: String` / `name: String` / `created_at: i64` / `updated_at: i64`；模型文档注释声明 user 维度与「文件是记录的可丢弃投影」语义（注释不含磁盘域根目录名）
- [x] `src-tauri/crates/infra/store/src/store.rs`：`models()` 登记 `ExploreRecord`；实现 `list_explore_records(root)`（id 升序、root 精确过滤）/ `find_explore_record(root, name)` / `create_explore_record(root, name)`（max+1 分配、单分量名校验、`(root, name)` 重复 → `Err`、只写 DB）/ `rename_explore_record(root, name, new_name)`（in-place 保主键、目标名冲突 → `Err`、刷新 `updated_at`）/ `delete_explore_record(root, name)`（miss 幂等，不动磁盘）
- [x] `src-tauri/crates/infra/store/src/store.rs`：实现 `restore_run_chain(source, source_ref)`——全表读过滤 → `(started_at, id)` 最新为链头 → 沿 `parent_run_id` 回溯（visited 防环）→ 反转为发起顺序；空链返回空 `Vec`
- [x] `src-tauri/crates/infra/store/src/envelope.rs`：`MODEL_ENTRIES` 登记 `explore` 行（count / scan / key_of），信封 API 函数本体零改动

## 阶段二：查询与执行契约（core / infra）

- [x] `src-tauri/crates/core/workflow/src/queries/mod.rs`：私有 `is_change_name` 提升为 `pub(crate) fn is_single_component_name(name: &str) -> bool`（`locate_change` 改用同口径）；`pub mod explore;` 并导出 explore 读面类型与函数
- [x] `src-tauri/crates/core/workflow/src/queries/explore.rs`：实现 `read_explore(layout, name) -> Option<ExploreDoc>`（stem 校验防穿越、`explores_root/<name>.md` UTF-8 读取、缺失/非 UTF-8 → `None`、纯读）与 `scan_explores(layout) -> Vec<ExploreScanEntry>`（顶层 `*.md`、stem 升序 + `modified_at`、缺失目录空、零绑定语义）；`ExploreDoc` / `ExploreScanEntry` serde camelCase；纯读不写任何文件
- [x] `src-tauri/crates/core/agent/src/runner.rs`：`AgentRunParams` 增 `resume_session_id: Option<String>`（文档注释说明「唯一进契约的续会话参数」）
- [x] `src-tauri/crates/infra/agent/src/flags.rs`：`build_args` 在 permission 档位后追加 `--resume <id>`（`None` 时无此 flag、其余 flag 序列不变）；模块文档 MVP 边界措辞同步（移除「无 resume」、保留「无 `--continue`」）

## 阶段三：watch 通道（infra）

- [x] `src-tauri/Cargo.toml`：`[workspace] members` 增 `crates/infra/watch`；`[workspace.dependencies]` 增 `watch = { path = "crates/infra/watch" }` 与 `notify = "8"`；根包 `[dependencies]` 增 `watch`
- [x] `src-tauri/crates/infra/watch/Cargo.toml`：新 crate（裸名 `watch`）manifest——`notify` 依赖、`tempfile` dev-dependency、零 Tauri
- [x] `src-tauri/crates/infra/watch/src/lib.rs`：实现 `FileWatchSignal { path: String }`（仅修改信号、无内容字节、serde camelCase）、`subscribe(path: PathBuf, sender: mpsc::Sender<FileWatchSignal>) -> Result<Watcher, WatchError>`（notify 单文件订阅；目标不存在不报错；过滤出目标文件的修改事件）、`Watcher`（持 notify watcher 与 mpsc Sender，drop 即停流退订）；crate/模块注释留痕平台边界（Windows 父目录语义不丢 rename-replace 事件、inotify rename 陷阱跨平台再议，不预建兼容层）

## 阶段四：命令层与注册（壳）

- [x] `src-tauri/src/commands/exec/agent.rs`：新增 `RunProvenance`（`source` / `source_ref` / `parent_run_id` + `debug()` 构造）；`run_agent` / `run_agent_with` 增 provenance 入参并在 `running` 记录初值填充来源与链字段（tee / 状态机收敛逻辑不动）
- [x] `src-tauri/src/commands/exec/mod.rs`：`agent_start` 增可选参数 `resume_session_id: Option<String>` / `source: Option<String>` / `source_ref: Option<String>` / `parent_run_id: Option<i64>`（`source` 缺省 `debug` 组装 `RunProvenance::debug()`；`resume_session_id` 进 `AgentRunParams`）；轨道文档更新
- [x] `src-tauri/src/commands/exec/mod.rs`：新增查询薄包装 `agent_run_chain(store, source, source_ref) -> Result<Vec<AgentRunRecord>, String>`（委托 `restore_run_chain`）
- [x] `src-tauri/src/commands/explores/mod.rs`：新轨道七命令——`read_explore` / `scan_explores`（以 store 清单求差滤除已绑定）/ `explore_doc_path`（布局派生路径、stem 校验、无 IO）/ `list_explore_records` / `create_explore_record` / `rename_explore_record` / `delete_explore_record`；blank root 空结果纪律（本地 `is_blank_root` 同口径）；命令注释 spec 指针用「specs/<capability>/spec.md，路径相对域根」定式，不含磁盘域根目录名
- [x] `src-tauri/src/commands/watch/mod.rs`：新轨道——`WatchRegistry`（`Mutex<HashMap<u64, WatchSubscription>>`，canonical path 键幂等）+ `watch_subscribe(registry, on_event: Channel<FileWatchEvent>, path) -> Result<u64, String>`（建订阅 + std 线程桥接 mpsc → Channel）+ `watch_unsubscribe(registry, subscription_id) -> Result<bool, String>`（drop `Watcher` 停流，miss 幂等）
- [x] `src-tauri/src/commands/mod.rs`：注册 `pub mod explores;` / `pub mod watch;` 并更新轨道清单文档
- [x] `src-tauri/src/main.rs`：setup 中 `app.manage(WatchRegistry::default())`；`generate_handler!` 登记 10 条新命令（explores 七条、watch 两条、`agent_run_chain`）

## 阶段五：前端基础

- [x] `package.json` / `src/components/ui/`：执行 `pnpm dlx shadcn@latest add message-scroller message resizable`——生成 `message-scroller.tsx` / `message.tsx` / `resizable.tsx`，依赖写入 package.json；核验 `components.json` 零改动；验证 message-scroller 组件 API 形态（不符则自实现滚动容器并回填 design D8 与待决问题）
- [x] `src/types/dto.ts`：新增 `ExploreRecord` / `ExploreDoc` / `ExploreScanEntry` / `FileWatchEvent`（镜像 store / workflow 自有类型 serde camelCase）；`AgentRunRecord` 增 `source: string` / `sourceRef: string | null` / `parentRunId: number | null`
- [x] `src/lib/exploreStance.ts`：`buildExplorePrompt(userInput: string): string`——精简 explore stance 前导模板 + 用户输入；文件头注释与 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源互链（注明接受漂移、模板归属 desktop 包）

## 阶段六：hooks

- [x] `src/views/explores/hooks/useExploreList.ts`：`useExploreList(root)`——invoke `list_explore_records`（root 变更与显式动作触发，无轮询）+ `create` / `rename` / `remove` 动作封装（invoke 后 refresh）
- [x] `src/views/explores/hooks/useExploreDoc.ts`：`useExploreDoc(root, name)`——mount/参数变更 invoke `read_explore`；经 `explore_doc_path` 取路径后 `watch_subscribe`（`new Channel<FileWatchEvent>()`），信号 500ms trailing 防抖合并为一次 `read_explore` 重拉；卸载 `watch_unsubscribe`；暴露 `doc` / `refresh`
- [x] `src/views/explores/hooks/useExploreSession.ts`：`useExploreSession(root, record)`——mount invoke `agent_run_chain`（`source: 'explore'`、`sourceRef: String(record.id)`）还原链并逐 run `agent_run_events` 重放拼接；`send(input)` 组装 prompt（`buildExplorePrompt` 拼接）、`resumeSessionId` 取链尾 `sessionId`、`parentRunId` 取链尾 id、携带来源三元组 invoke `agent_start`（Channel 事件实时累积）；run 终态定点 `refresh` 文档并把返回记录追加进链；运行中防重复发送

## 阶段七：组件与页面

- [x] `src/views/explores/components/ExploreConversation.tsx`：对话区——`message-scroller` 承载滚动；事件 → 气泡映射（user / assistant 各成气泡；Text→markdown、Thinking→折叠、ToolUse→卡片、ToolResult 与同 id ToolUse 成对）；`AskUserQuestion` ToolUse 渲染静态可读卡片（问题/选项原样）；result 汇总（num_turns / cost / duration / session_id）可读
- [x] `src/views/explores/components/ExploreComposer.tsx`：prompt 输入（沿用原生 styled textarea 模式）+ env / permission-mode 档位（默认 default + bypassPermissions，bare 档认证前提提示）；发送经 `send` 上抛
- [x] `src/views/explores/components/ExplorePreview.tsx`：复用 `MarkdownDocRenderer` 渲染 `doc.content`；未落盘 / 已删除空态呈现（不报错）
- [x] `src/views/explores/components/ExploreCreateDialog.tsx`：新建对话框两入口——「从已有文档创建」（invoke `scan_explores` 列未绑定文件，选中 `create_explore_record`）/「新话题」（输入主题名 `create_explore_record`，不落盘文件）；成功后 refresh 清单并导航进详情
- [x] `src/views/explores/ExploreDetailView.tsx`：`resizable` 双栏（左 `ExploreConversation` + `ExploreComposer`，右 `ExplorePreview`）；装配 `useExploreDoc` / `useExploreSession`；卸载即退订 watch（hook 内聚）
- [x] `src/views/explores/ExploreView.tsx`：路由双态容器（`useParams` 派生选中，无本地 state 双轨）+ 清单呈现（`useExploreList`，条目含删除入口）+ 新建入口（`ExploreCreateDialog`）；workspace 切换带旧选中时照 `ChangeView` 模式：过渡轮抑制误发查询 → `navigate('/explores', { replace: true })` → 清抑制位

## 阶段八：路由与导航

- [x] `src/routes.tsx`：路由表增 `/explores` 与 `/explores/:name`（`ExploreView`，prop `root`）；未知路径回 `/changes` 与 `max-lines-per-function: 50` 约束保持
- [x] `src/components/AppSidebar.tsx`：「页面」组增 [探索] `NavLink`（`data-testid="nav-explores"`、lucide `Compass` icon）；active 派生覆盖 `/explores` 与 `/explores/:name`；既有 testid 与导航语义不变

## 阶段九：管线收尾

- [x] `src-tauri/`：`cargo fmt` + `cargo clippy` 清零（新 crate 与全部改动文件覆盖；命名隔离机械测试通过——新产品 `.rs` 注释/字串不含磁盘域根目录名）
- [x] `packages/desktop`：`pnpm run client:check`（`vp check --fix` + knip）通过——无 knip 豁免新增，新导出均被消费
