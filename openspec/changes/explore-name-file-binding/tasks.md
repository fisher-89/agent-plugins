# 任务: explore-name-file-binding

> **变更**: explore-name-file-binding
> **依据**: proposal.md + design.md（D1–D9 决策编号见 design）

任务边界：本列表只含实现任务；**新增测试锚**（ExploreRecord v1→v2 / ChangeRecord v3→v4 decode-only 升级、kebab 校验、`set_explore_title` / `mark_explore_promoted`、create title 入参、promote move 语义与拒绝面、list / detail title 投影、`promote_explore` / `update_explore_title` 命令、`scan_explores` kebab 过滤、前端 title 显示 / promoted 态 / promote 入口 / kebab 校验 / title 回填、`extractMarkdownTitle` / stance 注入、change title 显示断言）由 test-design / test-gen / test-execution 阶段承接——但类型 / 载荷 / 签名演进触及既有测试文件编译面，本列表阶段内含**机械随动**（改签名 / 改基设，不写新断言）。**语料 title 两态样本与 list / detail golden 显式重写（`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕）同归测试阶段承接**。任务内文件均在 design.md 变更清单内。

只读红线：`plugins/dev-team` 全部（MCP 工具、hooks、CLI 工作流、版本 2.10.44、三类交付产物）；`crates/core/workflow/src/queries/explore.rs`（`read_explore` / `scan_explores` 宽松单分量口径）；`crates/core/foundation/src/layout/*`；`crates/infra/vcs/*`；`crates/core/orchestration/**`；相位表与 prompt 模板（`write/phase_table.rs`）；workflow.json 领域模型与双向墙；`ChangeRecord` 身份锚（id 主键）；change 归档 / 编排 / worktree 建域既有语义；主基线 specs。

## 阶段一：store 模型 + 操作面（infra）

- [x] `crates/infra/store/src/model.rs`：`ExploreRecord` 升 v2（`#[native_model(id = 4, version = 2, from = ExploreRecordV1)]`）——新增 `title: String`（注释「人类可读标题，恒非空；创建与升级默认 = name」）与 `promoted_to: Option<String>`（注释「指向已 promote 的 change id 身份锚；None = 草稿态」）；`name` 注释改「笔记文件 stem 寻址键（kebab-case）」；新增 `pub(crate) struct ExploreRecordV1`（`#[native_model(id = 4, version = 1)]`，`#[serde(rename_all = "camelCase")]`，字段面 = v1 既有五字段，不 `#[native_db]` 不 `#[primary_key]`）与双向 `From`（升级补 `title = name`、`promoted_to = None`，降级丢弃新字段）；`ExploreRecord::new` 构造补 `title: name.to_owned()` / `promoted_to: None`
- [x] `crates/infra/store/src/model.rs`：`ChangeRecord` 升 v4（`#[native_model(id = 9, version = 4, from = ChangeRecordV3)]`）——新增 `title: String`（注释「人类可读标题，恒非空；创建与升级默认 = name，promote 继承 explore.title」）；新增 `pub(crate) struct ChangeRecordV3`（`#[native_model(id = 9, version = 3)]`，字段面 = v3 既有字段含 `#[serde(default)]` 字段，不 `#[native_db]` 不 `#[primary_key]`）与双向 `From`（升级补 `title = name`，降级丢弃 title）；`ChangeRecord::new` 构造补 `title: name.to_owned()`
- [x] `crates/infra/store/src/store.rs`（change 映射面）：`change_state()` 增 `title: record.title.clone()`；`create_change_record` 的 `ChangeRecord` 字面量增 `title: record.title.clone()`
- [x] `crates/infra/store/src/store.rs`（explore 校验 + 操作面）：新增本地 `is_kebab_case(name: &str) -> bool`（等价 `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`，不引 regex）与名称长度常量（≤128）；`create_explore_record` / `rename_explore_record` 的非法名 / 目标名校验改为 kebab + 长度（错误文案「须为 kebab-case（小写字母/数字，可用 `-` 连接）」）
- [x] `crates/infra/store/src/store.rs`（新操作面）：新增 `set_explore_title(root, name, title)`（`title.trim().is_empty()` → `Err`；按 `(root, name)` 找到记录后 in-place 写 `title` + 刷新 `updated_at`，miss → `Err`）与 `mark_explore_promoted(root, name, change_id)`（记录 `promoted_to.is_some()` → `Err`；in-place 写 `promoted_to = Some(change_id)` + 刷新 `updated_at`，miss → `Err`）——两方法保主键保链
- [x] 编译面机械随动：`crates/infra/store/src/model_test.rs` / `store_test.rs` 既有用例保编译（`ChangeRecord` / `ExploreRecord` 构造与回环、kebab 校验基设；decode-only 升级与两新操作面断言留测试相位）

## 阶段二：workflow 状态缝 + 写面 + 查询（core）

- [x] `crates/core/workflow/src/state.rs`：`ChangeStateRecord` 增 `title: String`（`name` 之后；注释「人类可读标题，恒非空」）
- [x] `crates/core/workflow/src/write/create.rs`：`create` 增第 7 参 `title: &str`；名称 / goal 前置校验后补标题回退单点 `let title = if title.trim().is_empty() { name } else { title };`；`ChangeStateRecord` 建档载荷增 `title: title.to_owned()`；`CreateOutcome` 字段面零改动
- [x] `crates/core/workflow/src/write/promote.rs`（新）：`PromoteOutcome { change_id: String, change_name: String }`（serde camelCase + specta）；`promote_explore(main_root, worktree_root, store, vcs, name, note, title)`——直调 `super::create::create(..., name, note, title)` 成功后 `let layout = foundation::layout::resolve(main_root);` 删 `layout.explores_root.join(format!("{name}.md"))`；删失败 → `Err` 携 `outcome.id` 与「笔记全文已在 change explore.md 留底，可手动删除或回写」指引；成功映射 `PromoteOutcome { change_id: outcome.id, change_name: outcome.name }`
- [x] `crates/core/workflow/src/write/mod.rs`：追加 `mod promote;` 与 `pub use promote::{promote_explore, PromoteOutcome};`
- [x] `crates/core/workflow/src/queries/list.rs`：`ChangeSummary` 增 `title: String`（`name` 之后；注释「人类可读标题，恒非空」）；`db_entry` 映射 `title: record.title.clone()`
- [x] `crates/core/workflow/src/queries/detail.rs`：`ChangeDetail` 增 `title: String`（`name` 之后）；`change_detail` 聚合 `title: record.title.clone()`
- [x] 编译面机械随动：workflow crate 内实现 `ChangeStateStore` 的测试假件与 `create` 调用点（`create_test` 基设）补 title 参数 / `ChangeStateRecord` 载荷字段，`cargo check -p workflow` 过（新断言留测试相位）

## 阶段三：命令面 + bindings

- [x] `src/commands/changes/mod.rs`：`create_change_with` 增第 5 参 `title: String` 并透传 `write::create(..., &name, &goal, &title)`；`create_change` IPC 保持三参，内部 `create_change_with(app, root, name, goal, name.clone()).await`（title=name）
- [x] `src/commands/explores/mod.rs`（过滤）：新增本地 `is_kebab_case(name: &str) -> bool`（与 store 同口径）；`scan_explores` 命令层在未绑定过滤之后追加非 kebab stem 过滤（`filter(|entry| is_kebab_case(&entry.name))`）
- [x] `src/commands/explores/mod.rs`（update_explore_title）：新增 `pub fn update_explore_title(stores, root, name, title) -> Result<ExploreRecord, String>`——blank root / 空白 title 显式 `Err`；经 `for_root` 调 `set_explore_title`；`#[tauri::command]` + `#[specta::specta]` 并登记入 `all_commands!`
- [x] `src/commands/explores/mod.rs`（promote_explore）：新增 `pub async fn promote_explore(app, root, name) -> Result<PromoteOutcome, String>` 与 `pub(crate) async fn promote_explore_with<R: Runtime>(app, root, name)`——blank root / 非 kebab name 显式 `Err`；`let layout = resolve(Path::new(&root));` ① `queries::read_explore(&layout, &name)` 命中且 `content.trim()` 非空白否则显式 `Err` ② `store.find_explore_record(&root, &name)` 存在且 `promoted_to` 为空否则 `Err` ③ 经 `worktree_dir(data_root.inner(), &root, &name)` 派生父锚后 `spawn_blocking` 调 `write::promote_explore(main_root, worktree_root, store_for_promote, &vcs, &name, &note, &record.title)` ④ 成功后 `store.mark_explore_promoted(&root, &name, &outcome.change_id)`（失败 `Err` 携 change_id + 手动恢复指引）；命令层 MUST NOT 直接 `remove_file`
- [x] bindings 再生成：`pnpm -C packages/desktop run bindings:export` 落 `src/types/generated/bindings.ts`（`ExploreRecord.title` / `promotedTo`、`ChangeSummary.title` / `ChangeDetail.title`、`promote_explore` / `update_explore_title`）；一致性守卫（`bindings:check`）拦截漂移
- [x] 编译面机械随动：`src/commands/changes/mod_test.rs` / `src/commands/explores/mod_test.rs` 既有用例保编译（title 默认、kebab 过滤、两新命令签名；新断言留测试相位）

## 阶段四：前端 explore 面

- [x] `packages/desktop/src/lib/explore-title.ts`（新）：`extractMarkdownTitle(content: string): string | null`——取首行（`split(/\r?\n/, 1)[0]`）匹配 `/^#\s+(.+?)\s*$/`，标题 `trim()` 后非空才返回，否则 `null`
- [x] `packages/desktop/src/lib/explore-stance.ts`：`buildExplorePrompt(userInput: string, topic: string): string` 改两参；stance 前导改为按 `topic` 插值的函数——笔记落盘约定明示「本次探索笔记 MUST 写入 `openspec/explores/<topic>.md`（文件名 MUST 等于 `<topic>`，MUST NOT 自拟其它文件名）」与「笔记首行写 `# <标题>`（人类可读标题，用于清单展示）」
- [x] `packages/desktop/src/views/explores/hooks/use-explore-session.ts`：`send` 组装 `buildExplorePrompt(input.prompt, record.name)`
- [x] `packages/desktop/src/views/explores/hooks/use-explore-list.ts`：`ExploreListState` / `ExploreActions` 增 `updateTitle(name, title): Promise<void>`（`commands.updateExploreTitle(root, name, title)` 后 `refresh`）与 `promote(name): Promise<void>`（`commands.promoteExplore(root, name)` 后 `refresh`）；两动作失败 `onError` 且向外 rethrow（详情页行内错误捕获）
- [x] `packages/desktop/src/views/explores/components/explore-create-dialog.tsx`：新增本地 `isKebabCase`（与后端同口径）；`TopicEntry` 提交 `disabled` 增加「非法 kebab 禁提交」（`disabled={!isKebabCase(topic.trim())}` 或等价），占位 / 校验文案随动
- [x] `packages/desktop/src/views/explores/explore-view.tsx`：`ExploreListItem` 渲染 `record.title` 为标题（挂 `data-testid="explore-item-title"`，`record.name` 不再作标题展示；`data-name` 保留）；`ExploreDetailView` 透传 `list={list}`
- [x] `packages/desktop/src/views/explores/explore-detail-view.tsx`：props 增 `list: ExploreListState`；新增详情头（渲染 `record.title`，挂 `data-testid="explore-detail-title"`）；草稿态（`record.promotedTo === null`）渲染「启动变更」入口（`data-testid="explore-promote-trigger"`，点击调 `list.promote(record.name)`，行内 `break-all` 错误块）；promoted 态渲染「已转变更」徽标 + 「查看变更」按钮（`useNavigate` → `/changes/${record.promotedTo}`，挂 `data-testid`）；title 回填 effect（`doc.doc?.content` 变化时 `extractMarkdownTitle` → 与 `record.title` 不同则 `list.updateTitle(record.name, title)`，失败静默）；promoted 时 `useExploreDoc` 的 `name` 传 `null`（停 read/watch）并在右栏渲染 promoted 提示面板（`data-testid="explore-promoted-preview"`）替代 `ExplorePreview`
- [x] 前端测试机械随动：`src/views/explores/**/*.test.ts(x)` 与 `src/lib/explore-stance.test.ts` / 既有 explore-title 引用保编译（title 显示、promoted 态、kebab 校验、stance 两参基设；新断言留测试相位）

## 阶段五：前端 change 面

- [x] `packages/desktop/src/views/changes/change-list-view.tsx`：`ChangeRow` 标题改渲染 `summary.title`（`summary.name` 不再作标题展示；行键 / 导航恒 `summary.id`；data-testid 不变）
- [x] `packages/desktop/src/views/changes/change-detail-view.tsx`：`DetailHeader` 标题改渲染 `detail.title`（`detail.name` 保留供归档 / run 控制面板 `name` prop；data-testid 不变）
- [x] 前端测试机械随动：`src/views/changes/*.test.ts(x)` 既有 title 相关基设保编译（title 显示断言改写留测试相位）；knip 零未用导出残留

## 阶段六：版本与守线收口（静态收口；静态脚本由桌面端执行，agent 不重复执行）

- [x] `packages/desktop/package.json` `version` 0.4.32 → 0.4.33（D9：`src-tauri/tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动）
- [x] 红线自查 grep：`plugins/dev-team` 零 diff（版本 2.10.44、三类交付产物）；`queries/explore.rs` 宽松口径零 diff；`write/phase_table.rs` 零 diff；`workflow.json` 零读写触点复核；`ChangeRecord` id 主键零 diff；主基线 specs 零改动；命令层零 `remove_file` 直接调用
- [x] 变更清单对账：实现文件与 design.md 变更清单逐项对账（无清单外改动、无清单内遗漏；两新增文件 `write/promote.rs` / `explore-title.ts` 已含入）；bindings 已再生成；AC-10 版本交付 0.4.33 已执行（`vp test` / `client:check` / knip / golden 复核由桌面端守线执行）
