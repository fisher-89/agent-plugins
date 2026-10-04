# 测试设计: move-queries-command-to-change

> **日期**: 2026-10-04

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | `list_changes` / `get_change_detail` / `read_artifact` 三命令实现位于 `commands/changes/mod.rs`；`commands/queries/` 目录不存在（无空目录 / 空模块残留）；`src/commands/mod.rs` 无 `pub mod queries;`；`cargo check` 通过 | `packages/desktop/src-tauri/src/commands/changes/mod_test.rs`（行为半面：三命令经 `commands::changes` 落位后直调语义保持；目录消解 / 声明移除的结构半面见 ## 不可测试项） |
| AC-2 | `all_commands!` 仍登记同名三命令（路径改经 `changes::`）；`pnpm -C packages/desktop run bindings:check` 通过（重导出后 `src/types/generated` 零 diff）；前端源码零改动 | `packages/desktop/src-tauri/src/commands/changes/mod_test.rs`（进程内行为互补面：与 core 直调 serde 等值 + DTO 形状断言；bindings 零 diff 与前端零改动半面见 ## 不可测试项） |
| AC-3 | 合并后 `changes/mod_test.rs` 覆盖原 queries 侧全部 7 个测试与原 create_change 侧 4 个测试，`cargo test` 全绿；blank root 双口径并存不互换——读命令返回空结果 / `None`，`create_change` 返回显式 `Err` | `packages/desktop/src-tauri/src/commands/changes/mod_test.rs` |
| AC-4 | `changes/mod.rs` 模块 doc 声明「读 + 记录面」二分组织（沿 explores 先例），不再引用 `queries` 独立轨道；specs/ delta 覆盖 desktop-app-shell 三处 requirement 与 desktop-change-create 落位表述 | `packages/desktop/src-tauri/src/commands/explores/mod_test.rs`（叙事收口的测试侧组成：`queries` 引用清除的断言消息 re-point，design D4 指派本轨道；模块 doc 与 specs/ delta 半面见 ## 不可测试项） |
| AC-5 | `packages/desktop/package.json` version 保持 0.4.2 不变（0.4.2 为 agent-turn-eof 归档 bump 后的现状值，本变更加其上零提升）；`plugins/dev-team` 无任何改动 | —（无可自动化验证面，见 ## 不可测试项） |

---

## 单元测试

框架识别（`test_detect_frameworks`）：`packages/desktop/src-tauri` → rust 套件（Rust 原生 `#[test]`，同目录 `mod_test.rs` 宿主；套件注册在 src-tauri 根，合并后文件自动纳入，无需逐 crate 登记）。路径解析（`test_resolve_paths`）对 design 变更清单（修改文件，删除文件不入被测范围）返回六个同目录映射对，其中四对不设章节，在此注明：

1. `commands/mod.rs -> commands/mod_test.rs` — 登记面宏清单 re-path（D3）无运行时行为用例；该测试文件不存在且本变更禁新建清单外文件，不启用；
2. `commands/config/mod.rs -> commands/mod_test.rs`、`commands/stats/mod.rs -> commands/mod_test.rs` — 源文件仅模块 doc 注释 re-point（D4），零逻辑改动，既有 `mod_test.rs` 属禁触碰面，不启用；
3. `src/types/generated/bindings.ts -> bindings.test.ts` — 生成产物预期零 diff（design 变更清单「非内容修改条目」），该测试文件不存在且禁新建，不启用。

core 层（`crates/core/workflow`）自带语义不逐项重验：`locate_change` 的路径穿越拒绝、core `read_artifact` 的 source 校验与 canonical 包含性兜底、queries 聚合语义均由 core 既有测试保障；本设计只覆盖命令层自研组装面（「参数 → resolve → core → DTO」的等值 / 早退 / 边界集成）。

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

修改既有测试文件（合并重写）。7 个用例随迁自 `commands/queries/mod_test.rs`（该文件随 queries 轨道消解由实现阶段结构性删除，design D5；「测试内容除模块路径外零改动」，原文基准经 git 历史 HEAD 可核对）；既有 4 个 `create_change` 用例全部保持。四命令均为 sync 纯函数（无 State / AppHandle / Channel），`#[tauri::command]` 保留原函数可直调，不启动 Tauri runtime，直调即测。

**TempWs 装置统一定稿**（proposal 待决预告项，design D5 归本相位）：

- 合并为单一 `TempWs`（元组结构 + `Drop` RAII 自动清理 + `new` 时预清重建），统一临时目录前缀 `desktop-changes-test-{pid}-{tag}`（取代 queries 侧 `desktop-app-queries-test-` 与 changes 侧 `desktop-changes-create-test-` 两前缀）；
- helper 集合并：`root_string()` / `layout()` / `change_dir(name)`（原 changes 侧）+ `with_sample_tree()`（原 queries 侧：一个 active v2 change 含 workflow.json（requirement 型 + proposal pass eval 条目）与 proposal.md + 一个 archive v0 change `2026-02-02-old-docs`）；
- 7 个随迁用例体内 `ws.0` 直接字段访问与断言原文零改动（「除模块路径外零改动」红线）；4 个既有 `create_change` 用例仅装置定义与前缀随统一变化，断言零改动；
- tag 命名空间合并后无冲突（`cmd` / `core` / `dto-shape` / `blank-root` / `passthrough-err` 与 `passthrough` / `detail-known` / `detail-unknown` / `read-artifact` / `read-artifact-ok` / `hostile-source` 两两互异，pid + tag 双重隔离）；
- 装置 doc 注释「沿 commands/queries mod_test TempWs 先例」引用随统一改写为自述（D4 测试侧残留自然消除点）；测试文件头部模块 doc 由「create_change 单命令」扩展为本组四命令（三读 + `create_change`）合并叙事，双口径并存声明保持；
- import 机械合并：`use super::{create_change, get_change_detail, list_changes, read_artifact};`，`std::path::Path` 与 `serde_json::json` 并入去重。

#### 待测功能

- list_changes(root: String): ChangeList — blank root 早退空 `ChangeList`（读面空结果口径）；否则 resolve → core `queries::list_changes` 纯透传
- get_change_detail(root: String, change: String): Option<ChangeDetail> — blank root / 未知 change 名返回 `None`（非错误）
- read_artifact(root: String, change: String, kind: String, source: String): Option<ArtifactEnvelope> — kind 未注册 / source 非法或解析失败返回 `None`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `list_changes` — 纯透传等值 | 正向 | TempWs 真盘 sample tree（active v2 `sample-v2` + archive v0 `2026-02-02-old-docs`）下，命令结果与同根直调 core `queries::list_changes`（同一 `resolve` layout）serde 序列化等值（薄包装不加工）；内容抽查 `active` 恰 1 条、`archiveGroups` 恰 1 组且 `month` = `2026-02` | 新增 |
| `get_change_detail` — 详情 DTO | 正向 | 已知 v2 change（`sample-v2`）→ `Some`；DTO `name` / `inventory` = `v2`、`pipeline` 恰 9 段、`fileLog` 为数组、`artifacts` 非空 | 新增 |
| `get_change_detail` — 详情 DTO | 异常 | 未知 change 名（`不存在`）→ `None`（非错误） | 新增 |
| `read_artifact` — 信封读取 | 异常 | 未注册 kind（`file-log`）与合法 kind + 不存在 source（`markdown-doc` + `没有这个.md`）→ 各自 `None` | 新增 |
| `read_artifact` — 信封读取 | 正向 | `tasks-progress` + `tasks.md`（2 项 1 完成）→ `Some`；信封 `kind` / `version` = 1 / `payload.total` = 2 / `payload.done` = 1 / `fallback_text` 为 `Some` | 新增 |
| 三读命令 — blank root 空结果语义 | 边界 | root 为空串：`list_changes` 返回空 `active` + 空 `archive_groups` 不 panic、`get_change_detail` 返回 `None`（读面空结果口径；与既有 `blank_root显式err不进写面链路` 的写面显式 `Err` 双口径并存不互换，AC-3） | 新增 |
| `read_artifact` — 敌意 source 命令边界拒绝 | 边界 | 七种敌意 source（`../secret.md` / `sample-v2/../../secret.md` / `..\..\secret.md` / `C:\evil\secret.md` / `/secret.md` / `..` / 空串）对 workspace 根 `secret.md` 均返回 `None`（不逃逸 change 目录树）；正向对照 `proposal.md` 合法相对 source 返回 `Some` 不误伤 | 新增 |

上表 7 行即 queries 侧全部用例的随迁落位（用例名与断言面零改动）。既有 4 个 `create_change` 用例（`create_change命令到磁盘产物_与直调写面serde等值` / `返回dto恰name_created两键零磁盘路径字段` / `blank_root显式err不进写面链路` / `写面拒绝透传_非法名与空白goal经命令层err且零产生`）全部保持、断言语义不变（仅 TempWs 装置统一触点），不在表中逐行重复——合并后 7 + 4 = 11 个用例即 AC-3 的计数与全绿面。

边界用例不按参数类型映射表系统化扩张：本变更为零改动随迁（proposal「测试内容除模块路径外零改动」+ AC-3 计数 7 + 4 恒定），既有用例已覆盖字符串入参的空串 / 穿越分量 / 反斜杠 / 盘符 / 绝对路径 / 未知名 / 未注册 kind 边界；新增用例违反变更纪律。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统（workspace 磁盘树） | 不 mock：统一 TempWs RAII 临时目录内真实建树（v2 change 的 workflow.json + proposal.md、archive v0 change、tasks.md、根 secret.md），真实读写；薄包装等值以同根直调 core / 写面比对 | 全部 describe |

### packages/desktop/src-tauri/src/commands/explores/mod.rs -> packages/desktop/src-tauri/src/commands/explores/mod_test.rs

修改既有测试文件的单处断言消息（design D4 测试侧随迁清扫；源文件 `explores/mod.rs` 的 doc re-point 归实现阶段，不入本章节）。除该消息外，本文件装置与其余全部用例零改动。

#### 待测功能

<!-- design.md 未声明该文件的公共函数 / API 变更（仅 doc 注释 re-point，无逻辑改动）；本章节仅承接 D4 指派的测试侧断言消息清扫，不虚构 API 条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `read_explore` 穿越分量名拒绝 — 断言消息随迁清扫 (D4) | 边界 | 既有用例断言消息中的悬挂引用「（同 commands/queries 敌意 source 口径）」移除（queries 轨道消解后指向失真，design D4 测试侧残留） | 废弃 |
| `read_explore` 穿越分量名拒绝 — 断言消息随迁清扫 (D4) | 边界 | 同一用例断言消息 re-point 为「（同 commands/changes 敌意 source 口径）」——仅消息文案，断言对象（`a/b` / `..` / `../x` 三敌意名 → `None`）与断言逻辑零改动 | 新增 |

#### Mock策略

<!-- 无 Mock 变更：既有 Env（tempfile 数据根 + workspace 根）与 MockRuntime 装置零改动，仅断言消息文案 re-point。 -->

---

## 不可测试项

- AC-1 结构半面（`commands/queries/` 目录不存在、`pub mod queries;` 移除、编译通过） — **原因**: 文件系统结构与编译面属静态守线（实现阶段任务列表阶段四 + cargo 编译承载），非进程内用例可断言；为结构断言新建测试文件违反本变更「清单外零新增文件」边界。
- AC-2 bindings 零 diff 半面（`bindings:check` 的重导出幂等守卫）与前端零改动 — **原因**: specta 出线比对与前端源码 diff 属进程外构建产物核验；进程内以透传等值与 DTO 形状用例作行为互补面。
- AC-4 doc / specs 半面（`changes/mod.rs` 模块 doc 二分重写、specs/ delta、Module Contract 归档同步） — **原因**: 纯 prose 叙事与 spec 文本，无行为断言面；由归档 sync 步骤与静态检视承载。
- AC-5（`package.json` version 保持 0.4.2、`plugins/dev-team` 零改动） — **原因**: 静态 git diff 核验，无行为面。
- `commands/mod.rs` 登记面 in-place re-path（design D3） — **原因**: 宏清单为集合语义登记，无运行时行为；其消费正确性（`generate_handler` 与 specta builder 同源跟随）由编译与 bindings 零 diff 守卫承载，且禁新建 `commands/mod_test.rs`。
- `config/mod.rs` / `stats/mod.rs` 模块 doc re-point（design D4） — **原因**: 注释级改动无逻辑；两组既有 `mod_test.rs` 属零触碰面（沿「其他命令组逻辑」禁改红线）。
- `src/types/generated/bindings.ts` — **原因**: 生成产物，本变更预期零 diff（非内容修改条目）；同目录映射的 `bindings.test.ts` 不存在且禁新建。
