# 任务: desktop-change-create

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖 design.md 变更清单的实现文件。顺序按依赖排列：写面 → 命令 → 绑定 → 前端 → 交付 → 守线。

## 阶段一：core/workflow 写面 create 操作（sync 零 Tauri）

- [x] 新增 `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`：`CreateOutcome { pub name: String, pub created: String }`（`Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type` + `#[serde(rename_all = "camelCase")]`）与 `pub fn create(layout: &Layout, name: &str, goal: &str) -> Result<CreateOutcome, String>`——校验全 IO 前置：kebab-case 字符级判定（spec 正则 `^[a-z][a-z0-9]*(-[a-z0-9]+)*$` 语义，不引 regex）+ ≤128 字符 → goal 非空白（`trim().is_empty()`）→ `changes_root/<name>` 已存在拒绝（错误含目录路径）；通过后 `create_dir_all` 建树，workflow.json 经局部 `#[derive(Serialize)]` 结构体（字段声明序 `workflow_type → created → file_log`，`workflow_type` 恒 `"requirement"`、`created` 取 UTC 日期 `YYYY-MM-DD`、`file_log` 恒 `[]`）`to_string_pretty` + 尾换行写出（无 `eval` 键；文件名复用 `parse::WORKFLOW_FILE_NAME`；不经 `serde_json::Value` 组装——无 `preserve_order` 时 Value 对象字母序，键序契约不保）；explore.md 写 goal 原文（UTF-8 零结构包装）；模块 doc 注释与错误消息不含 layout 命名隔离扫描的双禁令字面量（磁盘域根目录名与配置文件名），spec 指针用相对域根定式
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs`：`mod create;` 声明 + `pub use create::{create, CreateOutcome};`；既有四操作导出与面注释零改动（`create_test` 测试模块挂载位归 test-gen 阶段）
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/write/persist.rs`：`load_doc` doc 注释「`change_create` 是唯一创建者，写面从不创建文件」换血为「插件 MCP `change_create` 与桌面写面 `create` 并存」双创建者声明；零逻辑改动

## 阶段二：create_change 命令组与登记

- [x] 新增 `packages/desktop/src-tauri/src/commands/changes/mod.rs`（新命令组，change 域记录面定位）：`#[tauri::command]` + `#[specta::specta]` 的 `pub fn create_change(root: String, name: String, goal: String) -> Result<CreateOutcome, String>`——三件事薄包装：blank root 显式 `Err`（不进入写面链路）→ `foundation::layout::resolve` → 写面 `create` → 错误映射；sync 纯函数命令（无 `State` / `AppHandle` / `Channel`，无需 `_with` 测试缝）；组 doc 注释含能力 spec 指针（相对域根定式，禁令字面量零出现）
- [x] 修改 `packages/desktop/src-tauri/src/commands/mod.rs`：`pub mod changes;` + `all_commands!` 宏清单追加 `$crate::commands::changes::create_change`

## 阶段三：绑定再生成

- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生成 `packages/desktop/src/types/generated/bindings.ts`（非手改）：`createChange(root, name, goal)` typed 包装与 `CreateOutcome` 类型出线

## 阶段四：前端新建对话框与清单页挂载

- [x] 新增 `packages/desktop/src/views/changes/components/change-create-dialog.tsx`（`components/` 新目录）：`export interface ChangeCreateDialogProps { root: string; onCreated: (name: string) => void }` + `export function ChangeCreateDialog(...)`——toggle 展开（沿 `explore-create-dialog` 先例的折叠卡片）；名称输入（kebab-case 提示）与 goal 多行输入均必填；提交前 trim 名称与 goal；本地同口径校验（与写面同一正则字面量 + ≤128）不合法或 goal 空白时禁用提交且不发起 invoke；`commands.createChange(root, name, goal)` 成功后 `onCreated(name)`；后端错误 break-all 行内错误块；data-testid 挂钩：`change-create-dialog` / `change-create-toggle` / `change-create-name` / `change-create-goal` / `change-create-submit` / `change-create-error`（命名沿 explore-* 风格，不以样式类名作挂钩）
- [x] 修改 `packages/desktop/src/views/changes/change-list-view.tsx`：`root` 非空时在头部行下挂载 `<ChangeCreateDialog root={root} onCreated={...}>`；`onCreated` 回调 `state.refresh()` + `navigate(\`/changes/${name}\`)`（创建后不自动发起 run，用户在详情 / flow 视图显式发起）

## 阶段五：版本交付

- [x] 修改 `packages/desktop/package.json` `version` 0.4.0 → 0.4.1（`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）；`plugins/dev-team` 零改动（`change-create.ts`、MCP 工具面、版本 2.10.44、三类交付产物均不动）

## 阶段六：守线（静态，不含测试执行）

- [x] 静态检查全绿：`pnpm -C packages/desktop run server:check`（cargo fmt + clippy）与 `pnpm -C packages/desktop run client:check`（`vp check --fix` + knip，无新增豁免条目）；自动化套件的执行验证由 test-execution 阶段承接
- [x] `pnpm -C packages/desktop run bindings:check` 零 diff（再生成链路确定性验证）
- [x] 静态自查：新增/修改的产品 `.rs` 源码（含注释与 doc comment）不含 layout 命名隔离扫描的双禁令字面量（磁盘域根目录名与配置文件名，豁免面以 `foundation/src/layout/mod.rs` 为准）
- [x] 静态自查：写触点收口——`create` 仅写 change 域目录内 workflow.json 与 explore.md 两文件、全部磁盘路径经 `Layout` 取得（零自拼域目录名）、零 CLI 子进程写通道；既有写面四操作（`phase_next` / `phase_start` / `phase_log` / `backtrack`）、`model/**` 与 `phase_table.rs` 零改动
- [x] 变更清单核对：design.md 变更清单与实际触达文件双向一致（清单外零改动、清单内零遗漏）
