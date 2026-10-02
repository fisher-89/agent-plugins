# 设计: desktop-change-create

> **变更**: desktop-change-create
> **日期**: 2026-10-02

---

## 提案与规格同步状态

`proposal.md` 与 `specs/desktop-change-create/spec.md`（路径相对域根）已由提案阶段写入并通过评估，属已完成产物，不在本变更清单与任务列表内。本设计基于其定稿文本展开，仅覆盖「变更范围 - 实现文件」九文件（测试文件归 test-design / test-gen / test-execution 阶段承接）。

## 关键设计决策（design 定稿点）

| # | 问题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | `create` 签名与产出 DTO | `pub fn create(layout: &Layout, name: &str, goal: &str) -> Result<CreateOutcome, String>`；`CreateOutcome { name: String, created: String }`（`Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type` + `#[serde(rename_all = "camelCase")]`，沿 `queries/list.rs` DTO derive 先例）。spec Module Contract「签名与 DTO 形状 design 定稿」在此定稿 | 与写面既有四操作同型（`&Layout` 入参 + `Result<_, String>`）；DTO 仅 `name` + `created`（磁盘路径知识不下沉前端），`created` 由写面铸出后随 DTO 直达命令返回，无需回读 |
| D2 | workflow.json 写出机制（键序契约落点） | 局部 `#[derive(Serialize)]` 结构体按字段声明序 `workflow_type → created → file_log` 序列化，`serde_json::to_string_pretty` + 尾换行写出；**不经** `serde_json::Value` 对象组装——本 workspace serde_json 未启用 `preserve_order`，Value 对象为字母序 Map，键序契约无法经 Value 保证 | 键序契约（AC-1/AC-2）需要声明序写出，结构体字段序是 serde 唯一保序通道；2 空格 pretty + 尾换行与写面 `persist::save` 出线同式。与插件 `createChange` 的差异仅在序列化形态（插件为紧凑单行 `JSON.stringify`），字段集 / 键序 / 值三一致，zod 解析与空白形态无关。文件名复用 `parse::WORKFLOW_FILE_NAME` 单源；`explore.md` 文件名字面量沿 `artifacts/markdown_doc.rs`、`write/phase_table.rs` 既有先例（非命名隔离禁令字面量） |
| D3 | kebab-case 校验实现与宽度 | 不引 regex 依赖，字符级判定实现 spec 正则 `^[a-z][a-z0-9]*(-[a-z0-9]+)*$` 语义 + ≤128 字符；前端 TS 侧用同口径正则字面量 | workflow crate 零 regex 依赖、全 desktop crates 亦无，单点校验不值得入树新依赖。宽度差异留痕：插件 `kebabCasePattern` 为 `^[a-z0-9][a-z0-9-]*$`（容前导数字 / 连号连字符 / 尾连字符），desktop 写面按 spec 更严——双创建者名称可接受集存在窄差，属 spec 权威裁决（desktop 端名称校验自权威），workflow.json 产出形状 parity 不受影响；前端本地校验与写面同口径保证「本地放行 ⇒ 后端必过」 |
| D4 | 校验顺序与失败原子性 | 顺序：名称校验 → goal 非空白（`trim().is_empty()`）→ 已存在拒绝（`changes_root/<name>` 存在即 `Err`，错误含目录路径）——全部前置在任何 IO 之前，拒绝面零目录零文件、既有目录零改动（AC-3）；通过后 `create_dir_all` 建树 → 写 workflow.json → 写 explore.md | IO 中途失败（workflow.json 已写而 explore.md 写失败）非事务：与插件 `createChange`（mkdir 后 writeFileSync）同类形态，V1 接受显式 `Err` 抵达调用方，不做残留补偿；列入待决问题备案 |
| D5 | 命令形态与落位 | 新组 `commands/changes/`：`create_change` 为 **sync 纯函数命令**（无 `State` / `AppHandle` / `Channel`，沿 explores 组写命令先例），直测无需 `_with` 泛型测试缝（change_flow 因 `AppHandle` 才需要缝）；blank root 命令层显式 `Err`（不进入写面链路），name/goal 校验为写面权威、命令层不过关 | 三件事纪律薄包装（参数转换 → 调写面 → 错误映射）；change 域三分职责就位——`queries`（纯读）/ `changes`（记录面写）/ `change_flow`（run 编排控制），沿 explores 组「读 + 记录面」同组先例 |
| D6 | 前端挂载与流转 | `ChangeListView` 仅在 `root` 非空时挂载对话框（无 workspace 无建档语义）；`ChangeCreateDialogProps { root: string; onCreated: (name: string) => void }`（非 null，沿 `ExploreCreateDialogProps` 先例）；提交前 trim 名称与 goal（沿 explore TopicEntry 先例）；`onCreated` 由父层 `state.refresh()` + `navigate('/changes/<name>')` 组成；创建后不自动发起 run（proposal 拍板） | 清单页 `root` 为 `string | null`（`useChangeList` 口径），对话框收敛为非 null props 使 invoke 面类型干净；折叠态 toggle 常驻不干扰空态展示 |
| D7 | explore.md 形态定稿 | goal 原文直写（UTF-8），零标题前缀、零章节包装（proposal 待决问题 #2 在此定稿收口） | spec 场景已锁定「无附加结构包装」；包装层属结构假设，与 free-form explore context 读取语义相悖 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 写面 create 操作 | 名称（kebab-case + ≤128，字符级判定）/ goal 非空白 / 已存在拒绝三道前置校验（零 IO 前完成）；`create_dir_all` 建树；workflow.json 键序定形写出（D2：`{"workflow_type":"requirement","created":"<UTC YYYY-MM-DD>","file_log":[]}`，2 空格 pretty + 尾换行、无 `eval` 键）；explore.md 写 goal 原文；sync 零 Tauri | `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`（新） | foundation（`Layout`）、serde / serde_json、time（UTC 日期）、crate 内 `parse::WORKFLOW_FILE_NAME`、specta（DTO derive） | Rust（sync） |
| 写面导出面 | `mod create` 挂载 + `create` / `CreateOutcome` 公共再导出；既有四操作导出与面注释不变 | `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | 同子模块 | Rust |
| 写面持久层（既有，仅注释） | `load_doc` doc 注释换血：「`change_create` 是唯一创建者，写面从不创建文件」→「插件 MCP `change_create` 与桌面写面 `create` 并存」双创建者声明；逻辑零改动 | `packages/desktop/src-tauri/crates/core/workflow/src/write/persist.rs` | — | Rust（仅注释） |
| create_change 命令 | 三件事薄包装：blank root 显式 `Err` → `layout::resolve` → 写面 `create` → `Result<CreateOutcome, String>` 错误映射；`#[tauri::command]` + `#[specta::specta]` 出线；组 doc 注释含能力 spec 指针（相对域根定式）与记录面定位 | `packages/desktop/src-tauri/src/commands/changes/mod.rs`（新组） | `workflow::write`（create / CreateOutcome）、foundation | Rust、tauri IPC + specta |
| 命令登记面 | `pub mod changes;` + `all_commands!` 追加 `create_change`（单一登记面） | `packages/desktop/src-tauri/src/commands/mod.rs` | 同 crate 命令组 | Rust 宏 |
| 新建对话框 | toggle 展开；名称输入（kebab-case 提示）+ goal 多行输入均必填；提交前 trim + 本地同口径校验，不合法禁提交且不发起 invoke；`commands.createChange(root, name, goal)`；后端错误 break-all 行内块；成功回调 `onCreated(name)`；data-testid 挂钩 | `packages/desktop/src/views/changes/components/change-create-dialog.tsx`（新，`components/` 为新目录） | bindings（`createChange` / `CreateOutcome`） | React、Tailwind |
| 清单页挂载 | `root` 非空时于头部行下挂载对话框；`onCreated` = `state.refresh()` + `navigate('/changes/<name>')` | `packages/desktop/src/views/changes/change-list-view.tsx` | ChangeCreateDialog、use-change-list、react-router | React |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。测试文件归 test-design / test-gen / test-execution 阶段，不在本清单。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | 写面 `create` 操作：三道前置校验、建树、workflow.json + explore.md 写出、`CreateOutcome` DTO |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | 新命令组（change 域记录面）：`create_change` IPC 命令 |
| `packages/desktop/src/views/changes/components/change-create-dialog.tsx` | 新建对话框（`components/` 新目录）：toggle / 名称 + goal 输入 / 本地校验 / 行内错误 / 成功回调 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | `mod create;` 声明 + `pub use create::{create, CreateOutcome};` + `#[cfg(test)] mod create_test;` 挂载位 | 写面导出面扩一操作；既有四操作导出零改动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/persist.rs` | `load_doc` doc 注释「唯一创建者」措辞换血为双创建者并存声明 | 零逻辑改动（注释与磁盘事实同步） |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `pub mod changes;` + `all_commands!` 宏清单追加 `create_change` | 单一登记面 |
| `packages/desktop/src/views/changes/change-list-view.tsx` | `root` 非空时挂载 `ChangeCreateDialog`（头部行下）；增 `onCreated` 回调（`state.refresh()` + `navigate`） | 新建入口与成功流转落点 |
| `packages/desktop/src/types/generated/bindings.ts` | 经 `pnpm -C packages/desktop run bindings:export` 再生成（非手改）：`createChange` typed 包装 + `CreateOutcome` 类型出线 | specta 管线既有守卫（`bindings:check` 零 diff） |
| `packages/desktop/package.json` | `version` 0.4.0 → 0.4.1 | 用户可见新功能；`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动 |

<!-- 删除文件：无（proposal「删除文件：无」） -->

### 公共函数 / API

Rust 签名为 Rust 语法；TS 为 TypeScript 语法。`mod.rs` / `commands/mod.rs` 为声明与再导出面，不设独立条目。

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `create` | `crates/core/workflow/src/write/create.rs` | 新增 | `pub fn create(layout: &Layout, name: &str, goal: &str) -> Result<CreateOutcome, String>` | 校验（kebab-case `^[a-z][a-z0-9]*(-[a-z0-9]+)*$` 语义 + ≤128 → goal 非空白 → 已存在拒绝，全 IO 前置）→ `create_dir_all` 建树 → workflow.json（键序 `workflow_type → created → file_log`、无 `eval` 键、2 空格 pretty + 尾换行、`workflow_type` 恒 `"requirement"`、`created` 取 UTC 日期 `YYYY-MM-DD`）→ explore.md 写 goal 原文；sync 零 Tauri |
| `create_change` | `src-tauri/src/commands/changes/mod.rs` | 新增 | `pub fn create_change(root: String, name: String, goal: String) -> Result<CreateOutcome, String>` | 三件事薄包装；blank root 显式 `Err`（不进入写面链路）；`#[tauri::command]` + `#[specta::specta]`；sync 纯函数无缝直测 |
| `ChangeCreateDialog` | `src/views/changes/components/change-create-dialog.tsx` | 新增 | `export function ChangeCreateDialog(props: ChangeCreateDialogProps): React.JSX.Element` | toggle 展开；名称 / goal 必填 + 本地同口径校验禁提交不 invoke；成功 `onCreated(name)`；后端错误行内呈现 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `CreateOutcome` | `crates/core/workflow/src/write/create.rs` | 新增 | `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]` + `#[serde(rename_all = "camelCase")]`；字段 `pub name: String`、`pub created: String`（磁盘路径不下沉；经 `all_commands!` / specta 出线为 bindings 的 `CreateOutcome`） |
| `ChangeCreateDialogProps` | `src/views/changes/components/change-create-dialog.tsx` | 新增 | `export interface ChangeCreateDialogProps { root: string; onCreated: (name: string) => void }`（沿 `ExploreCreateDialogProps` 先例） |

### 配置

| 配置键 | 所在文件 | 类型 | 值类型 | 默认值 | 说明 |
|--------|----------|------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string | `"0.4.1"` | 0.4.0 → 0.4.1；`tauri.conf.json` 经 `../package.json` 自动跟随；`plugins/dev-team` 保持 2.10.44 零改动 |

<!-- 其余无配置键变更：workflow.json schema 零新字段、无环境变量、无 hooks/settings 触点。 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `workflow.json`（初始文档，schema 零新字段） | `workflow_type: "requirement"`（恒值不入参）、`created: "YYYY-MM-DD"`（UTC 日历日期）、`file_log: []`（恒空数组起步）；无 `eval` 键 | 被 `detect_inventory` 判为 v2（`file_log` 键存在）、`list_changes` / `change_detail` 既有读面直读、`change_flow_start` 三项前置校验（存在 + 可解析 + requirement 相位表在位）通过；serde 写出形状可被插件 zod `workflowFileSchema` 解析（字段集 / 键序 / 值三一致） | 磁盘 `changes_root/<name>/workflow.json`（2 空格 pretty + 尾换行；文件名经 `parse::WORKFLOW_FILE_NAME`） |
| `explore.md`（goal 载体） | goal 原文（UTF-8 free-form，零结构包装） | proposal 相位 executor prompt 既有交接行（`phase_table.rs` `proposal_explore_handoff`，文本零改动）指引 proposal-planner Read 为 free-form explore context——goal 无需模板 / schema 变更即达提案阶段；经 `artifacts/markdown_doc.rs` 既有收录规则（`("explore.md", "探索")`）在详情产物清单呈「探索」条目 | 磁盘 `changes_root/<name>/explore.md` |
| `CreateOutcome`（IPC DTO） | `name`、`created` | 写面产出 → 命令返回 → bindings typed 包装；前端仅凭 `name` 导航 | 不持久化（IPC 瞬时面） |

<!-- 不涉及 HTTP API（IPC 命令面已由架构组件 + 公共函数表 + AC-4 覆盖），路由/API 设计节省略。 -->

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | `create.rs`（D2）：局部 `Serialize` 结构体声明序保证键序 `workflow_type → created → file_log`；`to_string_pretty` + 尾换行；无 `eval` 键；`created` 取 UTC 日期；`create_dir_all` 使空白树建树成立；explore.md 写 goal 原文（UTF-8 零包装） |
| AC-2 | 初始文档 `file_log: []` 在位 → `detect_inventory` 判 v2；`list_changes` / `get_change_detail` 既有读面零改动直读；`change_flow_start` 前置校验三项（存在 + 可解析 + `phase_table("requirement")` 在位）对新文档天然通过；字段集 / 键序 / 值与插件 `createChange` 一致，zod `workflowFileSchema` 可解析（序列化形态差异仅空白布局，D2） |
| AC-3 | 校验全 IO 前置（D4 顺序：名称 → goal → 已存在），拒绝面目标目录与文件零产生、既有 change 零改动；错误信息含失败原因显式 `Err` 抵达调用方 |
| AC-4 | `commands/changes/mod.rs` 新组 + `commands/mod.rs` `all_commands!` 登记 + `bindings:export` 再生成（`bindings:check` 零 diff 守卫）；blank root 命令层显式 `Err`；返回 DTO 仅 `name` + `created`，零磁盘路径字段 |
| AC-5 | goal 走 explore.md 既有通道：`phase_table.rs` proposal 交接行零改动即指引读取；`explore.md` 经 `markdown_doc.rs` 既有收录在详情页呈「探索」条目；相位表与 workflow.json schema 零改动（修改文件清单零 `model/**`、零 `phase_table.rs`） |
| AC-6 | `change-create-dialog.tsx`：toggle 展开、名称 / goal 均必填、本地同口径 kebab 校验（D3）不合法禁提交且不发起 invoke、成功后 `onCreated` → 清单刷新 + 导航 `/changes/<name>`、后端错误 break-all 行内块、挂钩均 data-testid（`change-create-dialog` / `change-create-toggle` / `change-create-name` / `change-create-goal` / `change-create-submit` / `change-create-error`，命名沿 explore-* 风格）；`change-list-view.tsx` 挂载与流转（D6，不自动发起 run） |
| AC-7 | `package.json` version 0.4.0 → 0.4.1；变更清单零 `plugins/dev-team/**` 条目（插件面冻结：`change-create.ts`、MCP 工具面、2.10.44、三类交付产物均不动）；守线为静态检查（fmt / clippy / client:check 含 knip / bindings:check 零 diff），自动化套件的执行验证由 test-execution 阶段承接 |

---

## 依赖

### 运行时依赖

- 无新增 npm / cargo 外部依赖 — `workflow` crate 既有 `foundation` / `serde` / `serde_json` / `specta` / `time` 全覆盖 create 所需（UTC 日期用既有 `time`；kebab-case 字符级判定，**不引 regex**）；前端仅消费既有 bindings 管线产物
- `tauri` / `specta` — `create_change` IPC 出线（既有依赖，版本不动）

### 构建/测试依赖

- 无新增 — 沿 `vp`（check）、`knip`、cargo workspace（src-tauri 根，套件自动覆盖）、`bindings:export` / `bindings:check` 既有工具链

---

## 待决问题

- goal 编辑能力（详情页对 explore.md 的编辑器）是否立项独立变更——V1 边界留痕（spec「V1 范围与边界留痕」第 5 条），不阻塞本期；
- IO 中途失败的残留补偿（workflow.json 已写而 explore.md 写失败时目录半成品）：V1 与插件 `createChange` 同形态（无补偿、显式 `Err`），实测出现后评估是否补残留目录清理；
- 归档树同名 change 的清单歧义：V1 仅拒 active 冲突（插件 parity），清单以 v2 徽标 + 创建时间区分；如实操混淆再评估桌面侧收紧口径（届时插件侧同步裁决）。
