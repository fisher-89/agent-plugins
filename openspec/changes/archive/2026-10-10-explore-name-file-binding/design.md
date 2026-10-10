# 设计: explore-name-file-binding

> **变更**: explore-name-file-binding
> **日期**: 2026-10-10

---

## 提案与规格同步状态

`proposal.md` 与 `specs/**`（8 能力 delta：desktop-workspace-store / desktop-explore-page / desktop-explore-queries / desktop-change-create / desktop-change-state-store / desktop-change-queries / desktop-change-flow-view / desktop-corpus-regression）已由提案阶段定稿；本设计不重复其内容、不将其列为待办，只在其「待决问题」与「变更范围 - 实现文件」之上逐项定稿（D1–D9），并补设计面决策。

改动基线核实在场（逐点可查）：

- `ExploreRecord`（`crates/infra/store/src/model.rs`，native_model 4:v1，`name` 双职：展示名 = 笔记 stem）；
- `ChangeRecord`（同文件，native_model 9:v3，无 `title`）；`change_state` / `create_change_record`（`crates/infra/store/src/store.rs`）；
- `ChangeStateRecord`（`crates/core/workflow/src/state.rs`）无 `title`；
- 写面 `create`（`crates/core/workflow/src/write/create.rs`）六参无 `title`，`write_fs_half` 直写 goal 原文进 worktree 内 `explore.md`；
- `read_explore` / `scan_explores`（`crates/core/workflow/src/queries/explore.rs`）宽松单分量名口径、纯读；
- `ChangeSummary` / `ChangeDetail`（`queries/list.rs` / `queries/detail.rs`）无 `title`，前端渲染 `summary.name` / `detail.name`；
- `create_change` IPC 三参 + `create_change_with` 泛型缝四参（`src/commands/changes/mod.rs`）；
- explore 命令轨道（`src/commands/explores/mod.rs`）七命令，`scan_explores` 仅过滤已绑定；
- 前端 explore 清单 / 详情 / 会话组装 / stance 模板 / `use-explore-doc`（500ms watch 防抖）。

**关键既有约束（设计必须绕开的）**：

1. **双向墙**：desktop 全链零 workflow.json 读写（测试夹具字面量除外）——本变更不触。
2. **promote 是 move 而非 rename**：explore 笔记在主仓 `explores/<name>.md`，change 的 `explore.md` 在 worktree 内；promote = 读主仓全文 → 写进 worktree 内 change 目录 → 删主仓原文件（跨 worktree 边界的内容搬运，MUST NOT `fs::rename`）。
3. **native_model decode-only 先例**：`AgentRunRecord` v4（`from = AgentRunRecordV3`）与 `ChangeRecord` 加字段先例——旧形态类型 `pub(crate)` 不注册进模型组、零 `migrate` 调用、零手工迁移。
4. **探索记录不得删**：`delete_explore_record` 会级联删名下会话链；promote 后记录保留、仅打标 `promoted_to`。
5. **explore 读面纯读纪律**：`read_explore` 是纯读，title 回填必须走显式写命令 `update_explore_title`，MUST NOT 在读取路径写入。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Explore 持久化模型 v2（新字段） | `ExploreRecord` v2：`name` 退化为 stem 寻址键、新增 `title` / `promoted_to`；`ExploreRecordV1` 双向 `From`（decode-only） | `crates/infra/store/src/model.rs`（修改） | native_db / native_model | native_model 4:v1→v2 decode-only，零迁移 |
| Change 持久化模型 v4（新字段） | `ChangeRecord` v4：新增 `title`；`ChangeRecordV3` 双向 `From`（decode-only） | `crates/infra/store/src/model.rs`（修改） | native_db / native_model | native_model 9:v3→v4 decode-only |
| store explore 操作面 | `create_explore_record` / `rename_explore_record` kebab 口径；新增 `set_explore_title` / `mark_explore_promoted` | `crates/infra/store/src/store.rs`（修改） | model.rs | in-place 写保主键保链；store 自持 kebab 校验 |
| store change 映射面 | `change_state()` / `create_change_record` 映射 title | `crates/infra/store/src/store.rs`（修改） | workflow::state | 记录 ↔ 中性类型映射单点 |
| 状态缝（中性快照） | `ChangeStateRecord` 增 `title` | `crates/core/workflow/src/state.rs`（修改） | serde / specta | 中性类型与持久化记录分离纪律不变 |
| 写面 create | `create` 增 `title` 入参（空/空白回退 name）；`ChangeStateRecord` 建档载荷含 title | `crates/core/workflow/src/write/create.rs`（修改） | uuid（既有）/ foundation::layout | sync 零 tokio；双写补偿链语义零改动 |
| 写面 promote（新） | `promote_explore`：复用 `create`（name / goal=笔记全文 / title）成功后删主仓 `explores/<name>.md`（move 半边）；`PromoteOutcome` | `crates/core/workflow/src/write/promote.rs`（新） | create / foundation::layout | sync 零 tokio；经 `Layout` 取路径 |
| 写面导出 | 追加导出 `promote_explore` / `PromoteOutcome` | `crates/core/workflow/src/write/mod.rs`（修改） | promote.rs | `create` 签名演进由 re-export 自动跟随 |
| change 列表查询 | `ChangeSummary` 增 `title`（纯投影），`db_entry` 映射 | `crates/core/workflow/src/queries/list.rs`（修改） | state.rs | db 单源零磁盘触点不变 |
| change 详情查询 | `ChangeDetail` 增 `title`（自记录直读），聚合映射 | `crates/core/workflow/src/queries/detail.rs`（修改） | queries/mod.rs | id 寻址与纯 derive 口径不变 |
| change 命令组 | `create_change_with` 增 title 参数；`create_change` IPC 保持三参、内部 title=name | `src/commands/changes/mod.rs`（修改） | workflow / store | 薄包装三件事纪律不变 |
| explore 命令轨道 | 新增 `update_explore_title` / `promote_explore`；`scan_explores` 命令层过滤非 kebab stem | `src/commands/explores/mod.rs`（修改） | workflow / store / vcs_runtime | 三件事纪律；blank root 双口径 |
| explore 清单页 | 条目渲染 `record.title`；向详情透传 `list`（refresh / updateTitle / promote） | `packages/desktop/src/views/explores/explore-view.tsx`（修改） | bindings / hooks | 路由 `/explores/:name` 不变 |
| explore 详情页 | 详情头渲染 `title`；promoted 态（「已转变更」徽标 + 跳转）；「启动变更」入口；title 回填 effect；promoted 时停读 `explores/<name>.md` | `packages/desktop/src/views/explores/explore-detail-view.tsx`（修改） | bindings / hooks / lib | watch 生命周期 = 页面生命周期不变 |
| explore 新建对话框 | 新话题本地 kebab-case 校验（非法禁提交） | `packages/desktop/src/views/explores/components/explore-create-dialog.tsx`（修改） | bindings | 导入列表消费命令层已过滤结果 |
| explore 清单 hook | 新增 `updateTitle` / `promote` 动作（invoke 后 refresh） | `packages/desktop/src/views/explores/hooks/use-explore-list.ts`（修改） | bindings | `create` / `rename` / `remove` 语义不变 |
| explore 会话组装 | `send` 组装 stance 时注入 `record.name` | `packages/desktop/src/views/explores/hooks/use-explore-session.ts`（修改） | lib/explore-stance | `buildExplorePrompt(userInput, record.name)` |
| explore stance 模板 | `buildExplorePrompt(userInput, topic)` 注入 name + 首行标题约定 | `packages/desktop/src/lib/explore-stance.ts`（修改） | — | 与 SKILL.md 双源互链不变 |
| explore 标题解析（新） | `extractMarkdownTitle` 纯函数（首行 `# 标题`） | `packages/desktop/src/lib/explore-title.ts`（新） | — | 纯函数零依赖 |
| change 清单页 | `ChangeRow` 渲染 `summary.title` | `packages/desktop/src/views/changes/change-list-view.tsx`（修改） | bindings | 行键 / 导航恒 id 不变 |
| change 详情页 | `DetailHeader` 渲染 `detail.title` | `packages/desktop/src/views/changes/change-detail-view.tsx`（修改） | bindings | `detail.name` 保留供归档 / run 控制面板 |
| 类型跟随 | 命令面与 DTO 再生成（`ExploreRecord.title` / `promotedTo`、`ChangeSummary.title` / `ChangeDetail.title`、`promote_explore` / `update_explore_title`） | `packages/desktop/src/types/generated/bindings.ts`（修改） | export-bindings 管线 | specta 生成物零手改 |
| 版本交付 | `version` 0.4.32 → 0.4.33 | `packages/desktop/package.json`（修改） | — | `tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动 |

**不变组件（零触点核对结论）**：`crates/core/workflow/src/queries/explore.rs`（`read_explore` / `scan_explores` 宽松口径零改动）；`crates/core/foundation/src/layout/*`（磁盘域根与 `explores_root` / `changes_root` 派生）；`crates/infra/vcs/*`（worktree 落位派生与 git 执行 name 化）；`crates/core/orchestration/**`（promote 不触 run 编排）；`desktop-change-worktree` / `desktop-file-watch` / `desktop-ipc-type-bindings` / `desktop-artifact-plugins`（promote 复用既有建域 / watch 失效信号 / specta 管线 / markdown-doc 插件）；`types/dto.ts`（纯 `export type *` shim，零改动）；`plugins/dev-team` 全部；主基线 specs。

---

## 关键设计决策

| # | 问题（proposal 待决 / 设计面） | 定稿 | 理由（含被拒备选） |
|---|------|------|------|
| D1 | `title` 非空单点与回退 | **三处默认 = name + 一处显式继承**：`ExploreRecord::new` / `ChangeRecord::new` 构造默认 `title = name`；写面 `create` 是 change 手动路径空白 title 回退单点（`title.trim().is_empty()` → `name`）；promote 路径经 `write::promote_explore` 显式传 `explore.title`。store 侧 `set_explore_title` / `create_change_record` 不做回退（入参恒已非空） | 消除前端空态回退分支（spec「title 恒非空」）；升级路径 `from` 默认 = name 与新建默认 = name 同源 |
| D2 | ExploreRecord v2 / ChangeRecord v4 升级形态 | **native_model decode-only**：`ExploreRecord` 4:v2 `from = ExploreRecordV1`、`ChangeRecord` 9:v4 `from = ChangeRecordV3`；旧形态类型 `pub(crate)` 且 **不注册进 `workspace_models()`**，新字段零 `#[serde(default)]`；`From<V1/V3>` 补 `title = name` / `promoted_to = None`，降级半边丢弃新字段 | 沿用 `AgentRunRecordV3` / `ChangeRecord` 加字段先例；零 `migrate`、零手工迁移（proposal AC-1 / AC-2）。**被拒**：给新字段加 `#[serde(default)]` 做行内兼容——与 `from` 升级链职责重叠，且降级半边语义不清 |
| D3 | promote 边界与失败补偿 | **写面半边 + 命令层四步**：`write::promote_explore` = 调 `create`（name=explore.name、goal=笔记全文、title=explore.title）成功后删主仓 `explores/<name>.md`，返回 `PromoteOutcome{change_id, change_name}`；命令层 `promote_explore` = ① 读笔记全文（命中且非空白）② 读记录（存在且 `promoted_to` 空）③ spawn_blocking 调写面 ④ `mark_explore_promoted`。失败面：create 失败 → 既有 create 补偿链零改动；删笔记失败 → `Err` 携 change id + 「笔记全文已在 change explore.md 留底，可手动删除或回写」指引；打标失败 → `Err` 携 change id + 手动恢复指引（R1 残留呈现） | 写面「复用 create + move 半边」单点、命令层零直接磁盘删除（spec「命令层零 `remove_file`」）；四步边界与 AC-7 拒绝面一一对应 |
| D4 | title 回填时机与竞态 | **复用既有 watch 防抖，不新增防抖窗口**：`useExploreDoc` 既有 500ms trailing watch 防抖 + run 终态定点重读即「内容稳定」信号；详情页 effect 消费 `doc.doc`（已稳定）解析首行，与 `record.title` 不同则 `update_explore_title`，成功后 `list.refresh()`；`set_explore_title` 幂等覆写吸收中间态（R2） | spec 既有链路已含 500ms 防抖与 run 收口重读；再叠加第二处防抖属冗余（watch 信号本身已失效合并）。**被拒**：run 收口定点再解析独立于 watch——既有 `wasRunning` effect 已定点 `doc.refresh()`，同源复用 |
| D5 | `set_explore_title` 空白拒绝与兜底 | **空白 title → `Err`（title 恒非空）；无标题行 → 不更新**：`extractMarkdownTitle` 返回 `null` 时保持 `record.title` 不变、零 invoke、零报错（agent 未写标题行时 title 保持 name 兜底展示） | spec「title 恒非空」+「解析失败不报错」；name 兜底满足「清单标题」目的的最小可用面 |
| D6 | promoted 详情呈现形态 | **仅 id 链接跳转，不反查 change title**：详情头渲染「已转变更」徽标 + 「查看变更」按钮（`navigate(/changes/<promotedTo>)`）；预览区渲染 promoted 提示面板而非继续读 `explores/<name>.md`（`promotedTo` 非空时 `name` 传 null 停 read/watch） | proposal 建议「仅以 id 链接跳转」；反查 title 需新命令面与取数耦合，V1 非必要 |
| D7 | explore 名称 kebab 口径 | **store 自持校验 + 命令层过滤**：store 新增本地 `is_kebab_case`（等价 `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`）+ 长度 ≤128，`create_explore_record` / `rename_explore_record` 双端校验升级；`scan_explores` 命令层在未绑定过滤之外再过滤非 kebab stem；workflow `scan_explores` / `read_explore` 宽松单分量口径**零改动**；存量非 kebab 记录保留可读，promote 前置由命令层 + `create` 双层拒绝 | 与 change 名称对齐是 promote 免转换冲突的隐藏前提（proposal 决策 3）；store 自持校验避免依赖 workflow 私有 `is_kebab_case`。**被拒**：promote 时转换 name——可能冲突且回到命名分叉 |
| D8 | promote 的 goal / title 语义与返回 | **goal = 笔记全文（保留首行 `# 标题`）、title = explore.title**；`write::promote_explore` 复用 `create` 并映射 `CreateOutcome` → `PromoteOutcome{change_id: id, change_name: name}`（`PromoteOutcome` 定义于 `write/promote.rs` 并 re-export） | proposal 决策 2/3（move 全文 + title 继承）；`PromoteOutcome` 最小两字段贴合 spec「命令返回 change_id / change_name」 |
| D9 | 版本交付与守线 | `packages/desktop` 0.4.32 → **0.4.33**；bindings 经 `bindings:export` 再生成；golden 显式重写（`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认）归测试相位承接；`plugins/dev-team` 零改动 | proposal AC-9 / AC-10；既有惯例 |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。子表按需填写。测试文件（既有用例的机械保编译随动除外）由 test-design / test-gen / test-execution 阶段承接，不入本清单。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs` | `promote_explore` 写操作（move 半边）+ `PromoteOutcome` DTO；见「公共函数 / API」「类型定义」 |
| `packages/desktop/src/lib/explore-title.ts` | `extractMarkdownTitle` 纯函数（首行 `# 标题` 解析）；见「公共函数 / API」 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `ExploreRecord` 升 v2（`#[native_model(id = 4, version = 2, from = ExploreRecordV1)]`）：新增 `title: String` / `promoted_to: Option<String>`；新增 `ExploreRecordV1`（`pub(crate)`，4:v1）与双向 `From`；`ChangeRecord` 升 v4（`#[native_model(id = 9, version = 4, from = ChangeRecordV3)]`）：新增 `title: String`；新增 `ChangeRecordV3`（`pub(crate)`，9:v3）与双向 `From`；`ExploreRecord::new` / `ChangeRecord::new` 构造默认 `title = name`（`promoted_to = None`） | D1 / D2 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `change_state()` / `create_change_record` 映射 `title`；新增本地 `is_kebab_case` + 名称长度 ≤128，`create_explore_record` / `rename_explore_record` 校验升级；新增 `set_explore_title` / `mark_explore_promoted` 两个 in-place 操作面 | D1 / D7 |
| `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | `ChangeStateRecord` 新增 `title: String`（`name` 之后；注释注明「人类可读标题，恒非空」） | D1 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | `create` 增 `title: &str` 入参；标题回退单点（`title.trim().is_empty()` → `name`）；`ChangeStateRecord` 建档载荷增 `title` | D1 / D8 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs` | 见「新增文件」——`promote_explore` 与 `PromoteOutcome` | D3 / D8 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | 追加 `mod promote;` 与 `pub use promote::{promote_explore, PromoteOutcome};` | D3 / D8 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs` | `ChangeSummary` 新增 `title: String`（`name` 之后）；`db_entry` 映射 `record.title` | D1 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | `ChangeDetail` 新增 `title: String`（`name` 之后）；`change_detail` 聚合 `record.title` | D1 |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | `create_change_with` 增 `title: String` 参数并透传 `write::create`；`create_change` IPC 保持三参、内部 `title = name` | D1 / D8 |
| `packages/desktop/src-tauri/src/commands/explores/mod.rs` | 新增 `update_explore_title(root, name, title)` / `promote_explore(root, name)` 命令（及 `promote_explore_with` 泛型缝）；`scan_explores` 命令层在未绑定过滤之外过滤非 kebab stem；新增本地 `is_kebab_case` | D3 / D5 / D7 |
| `packages/desktop/src/views/explores/explore-view.tsx` | `ExploreListItem` 渲染 `record.title`（挂 `data-testid`）；`ExploreDetailView` 透传 `list` | spec explore-page |
| `packages/desktop/src/views/explores/explore-detail-view.tsx` | 新增详情头渲染 `record.title`；promoted 态（「已转变更」徽标 + `navigate(/changes/<promotedTo>)`）；草稿态「启动变更」入口（调 `list.promote`，行内错误块）；title 回填 effect（`extractMarkdownTitle` → `list.updateTitle`）；promoted 时 `name` 传 null 停 read/watch 并渲染 promoted 提示面板 | spec explore-page |
| `packages/desktop/src/views/explores/components/explore-create-dialog.tsx` | 新话题本地 kebab-case 校验（复用本地 `isKebabCase`，非法禁提交） | D7 |
| `packages/desktop/src/views/explores/hooks/use-explore-list.ts` | `ExploreListState` / `ExploreActions` 新增 `updateTitle(name, title)` / `promote(name)`（invoke 后 refresh；返回 `Promise<void>` 供详情页行内错误捕获） | spec explore-page |
| `packages/desktop/src/views/explores/hooks/use-explore-session.ts` | `send` 组装 `buildExplorePrompt(input.prompt, record.name)` | spec explore-page |
| `packages/desktop/src/lib/explore-stance.ts` | `buildExplorePrompt(userInput, topic)` 改为两参；模板注入 `record.name`（落盘文件名 MUST 等于 name）与「笔记首行写 `# <标题>`」约定 | spec explore-page |
| `packages/desktop/src/lib/explore-title.ts` | 见「新增文件」——`extractMarkdownTitle` | D5 |
| `packages/desktop/src/views/changes/change-list-view.tsx` | `ChangeRow` 渲染 `summary.title` 为标题（`summary.name` 不再作标题展示；行键 / 导航恒 id） | spec change-flow-view |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | `DetailHeader` 渲染 `detail.title` 为标题（`detail.name` 保留供归档 / run 控制面板） | spec change-flow-view |
| `packages/desktop/src/types/generated/bindings.ts` | 经 `bindings:export` 再生成（`ExploreRecord.title` / `promotedTo`、`ChangeSummary.title` / `ChangeDetail.title`、`promote_explore` / `update_explore_title` 命令） | 一致性守卫 |
| `packages/desktop/package.json` | `version` 0.4.32 → 0.4.33 | D9 |

<!-- 删除文件：无整文件删除；按模板省略本子节 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `create` | `crates/core/workflow/src/write/create.rs` | 修改 | `pub fn create(main_root: &Path, worktree_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, name: &str, goal: &str, title: &str) -> Result<CreateOutcome, String>` | 新增 `title` 入参（空/空白回退 `name`，title 恒非空单点）；`CreateOutcome` 字段面不变 |
| `promote_explore` | `crates/core/workflow/src/write/promote.rs` | 新增 | `pub fn promote_explore(main_root: &Path, worktree_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, name: &str, note: &str, title: &str) -> Result<PromoteOutcome, String>` | 复用 `create`（goal=笔记全文、title 显式继承）成功后删主仓 `explores/<name>.md`（move 半边）；sync 零 Tauri |
| `Store.set_explore_title` | `crates/infra/store/src/store.rs` | 新增 | `pub fn set_explore_title(&self, root: &str, name: &str, title: &str) -> Result<ExploreRecord, StoreError>` | in-place 写 `title` + 刷新 `updated_at`；`title` 空白 → `Err`；miss → `Err` |
| `Store.mark_explore_promoted` | `crates/infra/store/src/store.rs` | 新增 | `pub fn mark_explore_promoted(&self, root: &str, name: &str, change_id: &str) -> Result<ExploreRecord, StoreError>` | in-place 写 `promoted_to = Some(change_id)` + 刷新 `updated_at`；已 promoted / miss → `Err` |
| `Store.create_explore_record` | `crates/infra/store/src/store.rs` | 修改 | `pub fn create_explore_record(&self, root: &str, name: &str) -> Result<ExploreRecord, StoreError>` | 签名不变；校验由单分量名升级为 kebab-case + 长度 ≤128 |
| `Store.rename_explore_record` | `crates/infra/store/src/store.rs` | 修改 | `pub fn rename_explore_record(&self, root: &str, name: &str, new_name: &str) -> Result<ExploreRecord, StoreError>` | 签名不变；目标名校验升级为 kebab-case + 长度 ≤128 |
| `Store.create_change_record` | `crates/infra/store/src/store.rs` | 修改 | `pub fn create_change_record(&self, record: ChangeStateRecord) -> Result<ChangeStateRecord, StoreError>` | 签名不变；映射 `title` |
| `ExploreRecord.new` | `crates/infra/store/src/model.rs` | 修改 | `pub fn new(root: &str, name: &str, now: i64) -> Self` | 签名不变；新增默认 `title = name`、`promoted_to = None` |
| `ChangeRecord.new` | `crates/infra/store/src/model.rs` | 修改 | `pub fn new(id: &str, name: &str, workflow_type: &str, created_at: i64, worktree: Option<String>, base_commit: Option<String>) -> Self` | 签名不变；新增默认 `title = name` |
| `update_explore_title` | `src/commands/explores/mod.rs` | 新增 | `pub fn update_explore_title(stores: State<'_, WorkspaceStores>, root: String, name: String, title: String) -> Result<ExploreRecord, String>` | Tauri 命令；blank root / 空白 title → 显式 `Err`；经 `set_explore_title` |
| `promote_explore` | `src/commands/explores/mod.rs` | 新增 | `pub async fn promote_explore(app: AppHandle, root: String, name: String) -> Result<PromoteOutcome, String>` | Tauri 命令；blank root / 非 kebab name → 显式 `Err`；读笔记 → 读记录 → 写面 → 打标四步 |
| `scan_explores` | `src/commands/explores/mod.rs` | 修改 | `pub fn scan_explores(root: String, stores: State<'_, WorkspaceStores>) -> Result<Vec<ExploreScanEntry>, String>` | 签名不变；命令层增非 kebab stem 过滤 |
| `create_change` | `src/commands/changes/mod.rs` | 修改 | `pub async fn create_change(app: AppHandle, root: String, name: String, goal: String) -> Result<CreateOutcome, String>` | IPC 签名不变；内部以 `title = name` 调 `create_change_with` |
| `buildExplorePrompt` | `packages/desktop/src/lib/explore-stance.ts` | 修改 | `export function buildExplorePrompt(userInput: string, topic: string): string` | 新增 `topic` 入参；注入当前记录 name 与「笔记首行写 `# <标题>`」约定 |
| `extractMarkdownTitle` | `packages/desktop/src/lib/explore-title.ts` | 新增 | `export function extractMarkdownTitle(content: string): string \| null` | 首行 `# 标题` 解析；空白 / 无标题行 → `null` |
| `useExploreList` | `packages/desktop/src/views/explores/hooks/use-explore-list.ts` | 修改 | `export function useExploreList(root: string \| null): ExploreListState` | 返回面新增 `updateTitle: (name: string, title: string) => Promise<void>` 与 `promote: (name: string) => Promise<void>` |
| `ExploreDetailView` | `packages/desktop/src/views/explores/explore-detail-view.tsx` | 修改 | `function ExploreDetailView({ root, record, list }: ExploreDetailViewProps): React.JSX.Element` | props 新增 `list: ExploreListState`（供 promote / updateTitle / refresh） |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `ExploreRecord` | `crates/infra/store/src/model.rs` | 修改 | native_model 4:v2；`name` 退化为笔记文件 stem 寻址键（kebab-case）；新增 `title: String`（恒非空）与 `promoted_to: Option<String>`（指向 change id 身份锚） |
| `ExploreRecordV1` | 同上 | 新增 | `pub(crate)`，native_model 4:v1；v1 历史形态（仅升级链解码用，不注册） |
| `ChangeRecord` | 同上 | 修改 | native_model 9:v4；新增 `title: String`（恒非空）；id 主键 / name 裸名属性不变 |
| `ChangeRecordV3` | 同上 | 新增 | `pub(crate)`，native_model 9:v3；v3 历史形态（仅升级链解码用，不注册） |
| `ChangeStateRecord` | `crates/core/workflow/src/state.rs` | 修改 | 新增 `title: String`（`name` 之后；port 流量像） |
| `ChangeSummary` | `crates/core/workflow/src/queries/list.rs` | 修改 | 新增 `title: String`（`name` 之后；纯投影零派生改写） |
| `ChangeDetail` | `crates/core/workflow/src/queries/detail.rs` | 修改 | 新增 `title: String`（`name` 之后；自记录直读） |
| `PromoteOutcome` | `crates/core/workflow/src/write/promote.rs` | 新增 | `change_id: String` / `change_name: String`（serde camelCase；specta 出线） |
| `ExploreListState` | `packages/desktop/src/views/explores/hooks/use-explore-list.ts` | 修改 | 新增 `updateTitle` / `promote` 动作字段 |
| `ExploreDetailViewProps` | `packages/desktop/src/views/explores/explore-detail-view.tsx` | 修改 | 新增 `list: ExploreListState` |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `"0.4.32"` → `"0.4.33"` | D9（`tauri.conf.json` 自动跟随） |

---

## 数据模型

### ExploreRecord v2（native_model 4:v2，`crates/infra/store/src/model.rs`）

| 字段 | 类型 | 语义 |
|------|------|------|
| `id` | `i64`（主键） | 记录 id（写事务内 max+1；身份与文件名解耦） |
| `root` | `String` | workspace 归属（canonical root） |
| `name` | `String` | 笔记文件 stem 寻址键（kebab-case 口径） |
| `title` | `String` | 人类可读标题（恒非空；创建与升级默认 = name，agent 产出后经回填覆盖） |
| `promoted_to` | `Option<String>` | 指向已 promote 的 **change id** 身份锚；`None` = 草稿态 |
| `created_at` / `updated_at` | `i64` | UTC unix 毫秒 |

`ExploreRecordV1`（4:v1，`pub(crate)`）承载 v1 历史形态（`id` / `root` / `name` / `created_at` / `updated_at`）；`From<ExploreRecordV1> for ExploreRecord` 补 `title = name`、`promoted_to = None`，降级半边 `From<ExploreRecord> for ExploreRecordV1` 丢弃新字段。

### ChangeRecord v4（native_model 9:v4，`crates/infra/store/src/model.rs`）

| 字段 | 类型 | 语义 |
|------|------|------|
| `id` | `String`（主键） | 建档铸出的身份锚（UUID 形态；终身恒定不复用） |
| `name` | `String` | change 名（恒裸名；无唯一约束） |
| `title` | `String` | 人类可读标题（恒非空；创建与升级默认 = name，promote 继承 explore.title） |
| `workflow_type` / `created_at` / `status` / `archived_at` / `active_phase` / `worktree` / `base_commit` | 既有 | 逐字不变（v3 字段面平移） |

`ChangeRecordV3`（9:v3，`pub(crate)`）承载 v3 历史形态；`From<ChangeRecordV3> for ChangeRecord` 补 `title = name`，降级半边丢弃 `title`。

### 物理表名映射（旧表对新读面不可见，D2）

| 旧表名 | 新表名 | 变化 |
|--------|--------|------|
| `4_1_id` | `4_2_id` | ExploreRecord 版本段 |
| `9_3_id` | `9_4_id` | ChangeRecord 版本段 |

`ExploreRecordV1` / `ChangeRecordV3` 不注册进 `workspace_models()`（旧行经 native_model `from` 链在读取时透明升级，零 `migrate` 调用）。

### promote（move）时序

```
命令层 promote_explore(root, name)：
  ① read_explore(root, name) → note（未落盘 / 空白 → Err）
  ② find_explore_record(root, name) → record（不存在 / 已 promoted → Err）
  ③ spawn_blocking → write::promote_explore(main_root, worktree_root, store, vcs, name, note, record.title)
        ├─ create(name, goal=note, title=record.title)  → worktree 内 changes/<name>/explore.md = note 全文
        └─ 成功后 fs::remove_file(主仓 explores/<name>.md)  → move 半边（失败 → Err 携 change id + 回写指引）
  ④ mark_explore_promoted(root, name, outcome.change_id)（失败 → Err 携 change id + 手动恢复指引）
```

探索生命周期（状态机）：

```
[草稿] promoted_to = None                 [已转变更] promoted_to = Some(<change_id>)
  笔记: explores/<name>.md（首行 # 标题）    笔记: changes/<name>/explore.md（已 move）
  ├─ 清单 / 详情头显示 title                ├─ 清单 / 详情头显示 title + 「已转变更」徽标
  ├─ 预览读 explores/<name>.md             ├─ 预览显示已 promote 提示（不再读原文件）
  └─ 可「启动变更」──promote──▶            └─ 会话链保留（记录不删，仅打标）
```

### 线面增量（golden 契约；逐字节以 golden 为准）

```json
// ExploreRecord（IPC 出线）
{ "id": 1, "root": "…", "name": "api-retry", "title": "API 重试调研",
  "promotedTo": "0192…" | null, "createdAt": 0, "updatedAt": 0 }
// ChangeSummary
{ "id": "0192…", "name": "api-retry", "title": "API 重试调研", "source": "active", … }
// ChangeDetail
{ "id": "0192…", "name": "api-retry", "title": "API 重试调研", "status": "active", … }
// PromoteOutcome
{ "changeId": "0192…", "changeName": "api-retry" }
```

---

## 路由 / API 设计

<!-- 本变更无 HTTP 路由面；新增 / 修改的 Tauri IPC 命令面已在「变更清单 - 公共函数 / API」逐条列出（update_explore_title / promote_explore / scan_explores 过滤 / create_change 内部 title），故本节省略。 -->

---

## 依赖

### 运行时依赖

- 无新增：`uuid`（v7）已在 workspace 依赖（desktop-change-db-identity 引入），`promote_explore` 复用 `create` 的 id 铸造点零新依赖；store / workflow / 壳层零新 crate。

### 构建 / 测试依赖

- 无新增：`redb` dev-dep、`tempfile`、`native_db` 等既有面不变。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | D2（ExploreRecord v1→v2 decode-only + `ExploreRecordV1` 双向 `From`）；数据模型节（title=name / promoted_to=None 升级语义）；`list_models` / `scan` 信封 API 零改动（不变组件节） |
| AC-2 | D1 / D2（ChangeRecord v3→v4 decode-only）；`change_state` / `create_change_record` 映射（store change 映射面组件行）；list / detail 增 title（组件表 + 类型定义） |
| AC-3 | D7（store 自持 kebab 校验 + ≤128；`create_explore_record` / `rename_explore_record`；`scan_explores` 命令层过滤；存量非 kebab 可读、promote 前置拒绝） |
| AC-4 | stance 模板组件行（`buildExplorePrompt(userInput, topic)` 注入 name + 首行标题约定）；调试页 run 不经 `buildExplorePrompt`（零触点） |
| AC-5 | D4 / D5（title 回填 effect 消费 watch 防抖后的 `doc.doc`，`extractMarkdownTitle` + `update_explore_title`；`read_explore` 纯读零写入） |
| AC-6 | D3 / D8（promote move 语义：`write::promote_explore` = create（goal=全文、title=explore.title）+ 删主仓笔记；命令层打标 `promoted_to = change.id`；会话链保留不删记录） |
| AC-7 | D3（promote 拒绝面：未落盘 / 空白 / 非 kebab / 已 promoted / 同名 active change 冲突均显式 `Err`；create 失败补偿链由既有 create 语义承担；命令层零直接磁盘删除） |
| AC-8 | D6（promoted 详情态：徽标 + 跳转 `/changes/<change_id>`、预览不再读原文件）；change 清单 / 详情头渲染 title（组件表） |
| AC-9 | D9（`ChangeSummary` / `ChangeDetail` 增 title 走 golden 显式重写流程，执行归测试相位；bindings 一致性守卫绿） |
| AC-10 | D9（0.4.33；`plugins/dev-team` 零改动；`vp test` / `client:check` / knip 全绿由桌面端守线执行） |

## 风险对齐（proposal R1–R6 → 设计落点）

R1 promote 后段失败 → D3（create → 删笔记 → 打标顺序 + 失败显式呈现残留对象与回写指引）；R2 title 回填竞态 → D4（复用既有 500ms 防抖 + run 终态重读，幂等覆写）；R3 kebab 口径升级影响存量 → D7（存量可读、promote 显式引导改名）；R4 手动新建 title 默认 = name 可读性弱 → D1（V1 接受，后续可加 title 输入）；R5 native_model 双升级破坏存量库 → D2（decode-only 先例 + AC-1 / AC-2 升级回归）；R6 title 四层贯穿传播面 → D9（纯投影零行为 + golden 显式重写 + bindings 守卫）。

---

## 待决问题

无遗留阻塞项——proposal 四项待决已全部定稿：promote 边界与失败补偿 → D3；title 回填防抖与内容稳定判定 → D4；`set_explore_title` 空白拒绝与 name 兜底 → D5；promoted 详情呈现形态 → D6。

边界留痕（非本变更待决）：

1. 手动新建 change 的 title 默认 = name 可读性弱（R4）——后续变更可加 title 输入框或 agent 派生，本变更提供 title 字段底座。
2. promoted 详情不反查 change title（D6）——如后续需要展示 change 标题，需新增命令面或复用 change 详情查询，本变更仅 id 链接跳转。
3. 存量非 kebab explore 的导流改名 UI（R3）——本变更仅后端拒绝 + 错误引导，UI 层面的批量改名入口留后续。
