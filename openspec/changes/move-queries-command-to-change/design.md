# 设计: move-queries-command-to-change

> **变更**: move-queries-command-to-change
> **日期**: 2026-10-03

---

## 提案与规格同步状态

`proposal.md` 与 `specs/desktop-app-shell/spec.md`、`specs/desktop-change-create/spec.md`（路径相对域根）已由提案阶段写入并通过评估，属已完成产物，不在本变更清单与任务列表内。本设计基于其定稿文本展开，覆盖「变更范围 - 实现文件」四文件 + 删除两文件（queries 轨道消解），并吸收 proposal 评估留痕的三处源码 doc 注释残留（config / stats / explores 模块 doc，见 D4）；测试文件面（`queries/mod_test.rs` 的 7 个用例并入 `changes/mod_test.rs`、TempWs 装置统一）由 test-design / test-gen / test-execution 阶段承接，不在本清单（D5）。

## 关键设计决策（design 定稿点）

| # | 问题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 合并后组内组织与模块 doc | `is_blank_root` 助手 + 三读命令置于 `create_change` **之前**（读先写后，沿 explores 组「三读 + 四记录面」组内次序先例）；模块 doc 由「三分职责」重写为「二分」——本组（change 域读 + 记录面，沿 explores 组同组先例）/ `change_flow`（run 编排控制），组内 blank root 双口径并存且不互换入 doc；`queries/mod.rs` 既有模块 doc 的「IPC 字符串入参的显式格式/包含性检查口径」段（root / change / source / kind 四条）随读命令并入组 doc；能力 spec 指针增补 desktop-app-shell（与既有 desktop-change-create 指针并列）；`#[cfg(test)] mod mod_test;` 声明保持 `changes/mod.rs` 既有位置（imports 后） | 读命令的入参检查口径 doc 是三命令的活语义文档，须随命令走；spec 指针补 app-shell 因轨道组织 requirement（含 queries 消解条款）落位该能力；二分叙事即 proposal 第 5 步收口 |
| D2 | 平移纪律（IPC 零变化红线） | 三命令函数名、签名、`#[tauri::command]` + `#[specta::specta]` 属性、命令体逻辑与逐函数 doc 注释**逐字零改动**；import 块机械合并去重（`std::path::Path`、`foundation::layout::resolve` 单一见）；`create_change` 命令体零改动——内联 `root.trim().is_empty()` 判定**不顺手**改用 `is_blank_root`（写面零改动红线，见「不要修改」） | 命令名是前端 invoke 与生成 bindings 的键；bindings 零 diff（AC-2）要求 specta 出线零变化，任何签名 / 属性 / DTO 触动即破坏。顺手统一属无收益纯风险（沿 proposal 决策表「顺手重命名」同款否决逻辑） |
| D3 | `all_commands!` 登记处置 | 三条目（`list_changes` / `get_change_detail` / `read_artifact`）**in-place re-path**——路径段 `queries` → `changes`，宏清单内位置零位移（不聚拢到 `changes::create_change` 相邻处）；`pub mod queries;` 移除后模块声明清单仍保持字母序 | 宏清单被 `main.rs` 的 `generate_handler` 与 `src/bindings` 的 specta builder 双消费，登记序与 bindings.ts 出线序的耦合性未经验证——位置零位移使「重导出零 diff」不依赖该耦合性假设（AC-2 确定性优先）；功能上宏清单是集合语义，条目位置无语义 |
| D4 | `commands/queries` 注释残留清扫范围 | 实现面吸收**源码 doc** 四处：`explores/mod.rs` 模块 doc「三件事纪律沿 `commands/queries` 模板」与 `is_blank_root` doc「（同 `commands::queries` 口径）」（后两处即 proposal 实现文件条目）、`config/mod.rs` 与 `stats/mod.rs` 模块 doc 各一处「三件事纪律沿 `commands/queries` 模板」（proposal 清单外残留，提案评估留痕「design 相位可吸收」）——统一 re-point 为 `commands/changes`（读命令所在组，模板实体随迁）。**测试侧**两处残留不入实现清单：`changes/mod_test.rs` doc「沿 commands/queries mod_test TempWs 先例」随测试合并重写自然消除、`explores/mod_test.rs` 断言消息「同 commands/queries 敌意 source 口径」由 test-design 阶段纳入随迁清扫（测试文件归测试轨道纪律） | 源码 doc 是活导航，悬挂引用使「queries 轨道已消解」的叙事失真延续；三命令与模板实体迁入 `commands/changes` 后 re-point 即恢复指向真身。测试文件触碰归测试轨道，实现面保持零测试文件写入 |
| D5 | queries 轨道消解形态与测试承接 | 目录**整体删除**（`mod.rs` + `mod_test.rs` 两文件，无空模块、无 re-export shim）；`mod_test.rs` 随模块删除属轨道消解的结构性组成（非测试编写）；其 7 个用例的随迁落盘归 test-design / test-gen 阶段——用例名与断言面以 proposal AC-3 清单为准（纯透传等值 / 详情 DTO / 未知 change → None / 敌意 source 拒绝 / blank root 空结果语义等），「测试内容除模块路径外零改动」的原文基准可经 git 历史核对（本工作流变更全程未提交，HEAD 保有 `queries/mod_test.rs` 原文）；TempWs 装置统一细节（单一装置、统一临时目录前缀）由 test-design 定稿（proposal 待决问题预告项） | AC-1 要求 `commands/queries/` 目录不存在；沿 exec 轨道「MUST NOT 空壳」纪律与 workflow-file-inventory / agent-component-ai-sdk 先例（随模块移除连带删除测试文件、用例随迁归测试阶段）；git 历史基准使「零改动」可核对而非凭描述重写 |
| D6 | golden / fixture 内 `commands/queries` 字符串 | 零触碰。`crates/core/workflow/tests/golden/**` 与 `tests/fixtures/**` 内的 `commands/queries` 路径串是 v2-a 历史 change 的落盘文档内容（fixture 即历史事实记录），非活引用 | golden 线契约冻结（既有裁决）；改写 fixture 等于篡改历史落盘文档 |
| D7 | 版本 | `packages/desktop/package.json` version **保持 0.4.2 不 bump**（0.4.2 系 agent-turn-eof 归档 bump 后现状）；实现清单零 package.json 触点 | 纯内部重构零用户可见变化，沿「仅用户可见变更才提升」既有裁决（AC-5） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| change 域命令组（读 + 记录面） | 承接自 queries 轨道并入的三读命令（`list_changes` / `get_change_detail` / `read_artifact`）与 `is_blank_root` 助手 + 既有 `create_change`；组 doc 二分叙事（本组读 + 记录面 / `change_flow` 编排控制）、读命令 IPC 入参检查口径段与 blank root 双口径并存声明（D1）；四命令均无状态薄包装、sync 纯函数（无 State / AppHandle / Channel） | `packages/desktop/src-tauri/src/commands/changes/mod.rs` | `foundation`（`layout::resolve`）、`workflow`（`queries` / `artifacts` / `model` / `parse` / `write`）、`tauri` IPC、`specta` 出线 | Rust（sync 纯函数命令 + tauri IPC + specta） |
| 命令登记面与轨道消解 | `pub mod` 清单移除 `queries`（保持字母序）；`all_commands!` 三条目 in-place re-path（D3）；`main.rs` 的 `generate_handler` 与 `src/bindings` 的 specta builder 经宏同源自动跟随，零额外触点；`commands/queries/` 目录整体删除（两文件，D5） | `packages/desktop/src-tauri/src/commands/mod.rs`；`packages/desktop/src-tauri/src/commands/queries/`（删除） | 同 crate 命令组 | Rust 宏（单一登记面） |
| 注释口径清扫面 | `config` / `stats` / `explores` 三组模块 doc 中「沿 `commands/queries` 模板 / 同 `commands::queries` 口径」悬挂引用 re-point 为 `commands/changes`（D4，共四处源码 doc） | `packages/desktop/src-tauri/src/commands/config/mod.rs`、`stats/mod.rs`、`explores/mod.rs` | — | Rust（仅 doc 注释，无逻辑） |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。测试文件归 test-design / test-gen / test-execution 阶段，不在本清单：queries/mod_test.rs 的删除是轨道消解的结构性组成（列入删除文件表），其 7 用例的随迁落盘归测试轨道（D5）；changes/mod_test.rs 的合并重写归测试轨道。 -->

<!-- 新增文件：无 —— proposal「新增能力：无」，纯搬迁 + 消解，省略此子节。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | 逐字平移 `is_blank_root` 助手与三读命令（置于 `create_change` 前，D1/D2）；import 块机械合并去重；模块 doc 二分重写 + 读命令入参检查口径段并入 + spec 指针增补 desktop-app-shell（D1） | IPC 面零变化（函数名 / 签名 / 属性 / 命令体逐字不动）；`create_change` 与 `#[cfg(test)] mod mod_test;` 声明位零改动 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | 移除 `pub mod queries;`；`all_commands!` 三条目 in-place re-path（`$crate::commands::queries::*` → `$crate::commands::changes::*`，位置零位移，D3） | 单一登记面；`main.rs` 与 `src/bindings` 经宏同源自动跟随 |
| `packages/desktop/src-tauri/src/commands/explores/mod.rs` | 仅 doc 注释两处：模块 doc「三件事纪律沿 `commands/queries` 模板」与 `is_blank_root` doc「（同 `commands::queries` 口径）」re-point 为 `commands/changes` | 注释级改动，无逻辑（proposal 实现文件条目 + D4 清单外残留吸收） |
| `packages/desktop/src-tauri/src/commands/config/mod.rs` | 仅 doc 注释一处：模块 doc「三件事纪律沿 `commands/queries` 模板」re-point 为 `commands/changes` | 注释级残留吸收（proposal 评估留痕「design 相位可吸收」，D4） |
| `packages/desktop/src-tauri/src/commands/stats/mod.rs` | 同 `config/mod.rs`：模块 doc 一处 re-point | 同上 |
| `packages/desktop/src/types/generated/bindings.ts` | **预期零 diff**（非内容修改条目）：命令名与类型零改动 → specta 出线零变化，`bindings:check` 的 `git diff --exit-code -- src/types/generated` 守卫即验收面（AC-2） | 实现面不落盘任何该文件 diff；零 diff 本身是搬迁纪律（D2/D3）的证明物 |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/src/commands/queries/mod.rs` | 三读命令 + `is_blank_root` 平移至 `changes/mod.rs` 后删除；queries 轨道消解（AC-1），无空壳无 re-export shim（D5） |
| `packages/desktop/src-tauri/src/commands/queries/mod_test.rs` | 随模块结构性删除；7 个用例随迁归 test-design / test-gen 阶段（原文基准经 git 历史可核对，D5） |

### 公共函数 / API

Rust 签名为 Rust 语法。三命令为**落位迁移**（类型列「修改」指所在模块变化），签名 / 命令体 / specta 出线零变化。

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `list_changes` | `src-tauri/src/commands/changes/mod.rs`（自 `commands/queries/mod.rs` 迁入） | 修改（迁移落位） | `pub fn list_changes(root: String) -> ChangeList` | blank root 早退空 `ChangeList` 语义原样（读面空结果口径） |
| `get_change_detail` | 同上 | 修改（迁移落位） | `pub fn get_change_detail(root: String, change: String) -> Option<ChangeDetail>` | 未知 change 名返回 `None`（非错误）语义原样 |
| `read_artifact` | 同上 | 修改（迁移落位） | `pub fn read_artifact(root: String, change: String, kind: String, source: String) -> Option<ArtifactEnvelope>` | kind 未注册 / source 非法或解析失败返回 `None` 语义原样 |

<!-- `is_blank_root` 为模块私有助手（`fn is_blank_root(root: &str) -> bool`），不在公共 API 面，随读命令平移（D2）。`create_change` 零改动不列（proposal「不要修改」红线）。 -->

<!-- 类型定义：无 —— ChangeList / ChangeDetail / ArtifactEnvelope / CreateOutcome 均为 `crates/core/workflow` 既有类型零变更，省略此子节。 -->

<!-- 配置：无 —— version 保持 0.4.2 不 bump（D7 / AC-5），零配置键触点。 -->

---

## 数据模型

本变更零 schema 变更、零新模型、零新持久化——搬迁不触任何落盘形态（workflow.json / explore.md / store 均零触点，`crates/core/workflow/**` 禁改）；下表仅留痕四命令 IPC DTO 既有形状（bindings 零 diff 的类型面，「字段面以 `crates/core/workflow` 既有定义为准」）。

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `ChangeList`（既有 IPC DTO） | `active: Vec<ChangeSummary>`、`archive_groups: Vec<ArchiveGroup>`（按月分组） | core 查询层产出 → `list_changes` 返回 → bindings typed 包装；源头为磁盘 change 目录树 + workflow.json 既有形态 | 不持久化（IPC 瞬时面；源头落盘形态零变更） |
| `ChangeDetail`（既有 IPC DTO） | `name`、`inventory`、`pipeline`、`active_phase`、`artifacts` 等（v2 详情聚合） | core 查询层产出 → `get_change_detail` 返回 → bindings | 不持久化（同上） |
| `ArtifactEnvelope`（既有 IPC DTO） | `kind`、`version`、`payload`、`fallback_text` | `workflow::artifacts` 信封读取产出 → `read_artifact` 返回 → bindings | 不持久化（IPC 瞬时面；tasks.md / proposal.md 等源文件零变更） |
| `CreateOutcome`（既有 IPC DTO） | `name`、`created` | 写面 `create` 产出 → `create_change` 返回 → bindings（零改动，列此对照双口径写面） | 不持久化（IPC 瞬时面） |

---

## 路由/API 设计

本变更不涉及 HTTP API；IPC 命令面（Tauri invoke 契约）**零变化**——搬迁仅改 Rust 模块落位，命令名（前端 invoke 键）与类型面不动：

| IPC 命令 | 输入 | 输出 | 变化 |
|------|------|------|------|
| `list_changes` | `root: String` | `ChangeList` | 落位 `commands::queries` → `commands::changes`（D3 登记路径），名 / 签名 / 出线零变化 |
| `get_change_detail` | `root: String, change: String` | `Option<ChangeDetail>` | 同上 |
| `read_artifact` | `root: String, change: String, kind: String, source: String` | `Option<ArtifactEnvelope>` | 同上 |
| `create_change` | `root: String, name: String, goal: String` | `Result<CreateOutcome, String>` | 零变化（既有，同组并列对照） |

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | 阶段一 / 阶段二：三命令实现落位 `commands/changes/mod.rs`（D1/D2 平移）；`commands/queries/` 两文件整体删除、`pub mod queries;` 移除（D5）；`cargo check` 编译面由守线 `server:check`（cargo fmt + clippy）覆盖 |
| AC-2 | D2 逐字平移 + D3 in-place re-path（位置零位移使出线序确定性不依赖登记序耦合假设）；守线 `bindings:check` 零 diff（`bindings.ts` 变更清单行即零 diff 证明物）与 `client:check` 前端零触点边界确认 |
| AC-3 | 双口径语义零变化的结构保障：三读命令与 `create_change` 命令体均逐字零改动（D2）+ 组 doc 双口径并存声明（D1）；测试合并（7 + 4 用例、TempWs 装置统一）由 test-design / test-gen 阶段承接、执行验证归 test-execution 阶段（D5，不入本设计与任务列表） |
| AC-4 | 阶段一模块 doc 二分重写（读 + 记录面，沿 explores 先例，不再引用 queries 独立轨道）+ 阶段三源码 doc 残留清扫（D4）；specs/ delta（desktop-app-shell 三 requirement + desktop-change-create 落位表述）已由 proposal 阶段落盘，Module Contract 行随归档同步（proposal 留痕） |
| AC-5 | D7：实现清单与任务列表零 `packages/desktop/package.json` 触点（version 保持 0.4.2）；守线静态自查 `plugins/dev-team/**` 零 diff |

---

## 依赖

### 运行时依赖

- 无新增 — `tauri`（IPC）、`specta`（出线）、`foundation`（`layout::resolve`）、`workflow`（queries / artifacts / model / parse / write）均为既有依赖，纯搬迁零新引入

### 构建/测试依赖

- 无新增 — 沿 cargo workspace（src-tauri 根注册，套件自动覆盖）、`pnpm -C packages/desktop run server:check` / `client:check` / `bindings:check` 既有工具链

---

## 待决问题

- 无 — proposal 待决问题为空（范围闭合），design 相位定稿了组内组织与 doc 形态（D1）、平移与登记纪律（D2 / D3）、残留清扫范围（D4）与消解形态（D5）；TempWs 装置统一细节按 proposal 预告归 test-design 定稿。
- 留痕（非待决）：`all_commands!` 宏清单的条目次序与 bindings.ts 出线序的耦合性未经验证，本变更以 in-place re-path 保守绕开（D3）；未来若需重排登记清单，应先以 `bindings:check` 验证出线零 diff 再落位。
