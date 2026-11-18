# desktop-archi-contract-panel Specification（delta: module-entry-docs-semantics）

## Purpose

本 delta 只做**启用判据的目录串换段**（E-1 目录迁移）：`archi_api_check` 的 `enabled` 由「`<archi_api_dir>` 递归含 ≥1 份 `*.md`」改为「`<archi_models_dir>` 递归含 ≥1 份 `*.md`」，`api::enabled` 仍是同一单点。命令形态、出参字段、三类差异语义、错误面、面板两轴呈现与生成交互**逐字不变**——本变更零前端产品代码改动，变的只是后端派生出的路径串（前端仍只消费 DTO 字符串）。

## MODIFIED Requirements

### Requirement: 检查命令契约（archi_api_check）

后端 SHALL 提供一条**零写入**的检查命令 `archi_api_check(root: String) -> Result<ArchiApiCheck, String>`，出参 SHALL 为「生成物新鲜度」一条轴的完整事实：

| 字段 | 类型 | 语义 |
|---|---|---|
| `enabled` | `bool` | 契约面是否启用（`<archi_models_dir>` 递归含 ≥1 份 `*.md`），`api::enabled` 单点判定，MUST NOT 在命令层另判一套 |
| `meta` | `ArchiApiMeta` | 生成载元：`toolVersion`（生成器 crate 版本）+ `commonDigest`（映射规则资产摘要 `sha256:<hex>`，渲染侧截前 12 位）；取自 `api::meta()`，MUST NOT 在命令层重算摘要 |
| `mismatched` | `Vec<String>` | 内容不符：文件在盘但字节与渲染结果不同 |
| `orphaned` | `Vec<String>` | 应删未删：盘上存在、命名符合生成物判据、不在应生成集 |
| `missing` | `Vec<String>` | 应生成未生成 |

- **薄包装形态**：命令 SHALL 为薄包装（`check_root` → 调 `core/archi` 领域函数 → 映射错误），领域装配在 `archi_api_check_inner(root: &Path)` 内以便离 Tauri 测试（沿 `commands/archi/` 既有 `*_inner` 形态）；命令层 MUST NOT 承载渲染、比对或路径归一。
- **零写盘零 spawn**：命令 MUST NOT 创建 / 覆盖 / 删除任何文件，MUST NOT 派生进程，MUST NOT 引入 watch 或轮询。
- **三类差异即生成预览**：三列合起来 SHALL 恰好是「点生成会写什么」的完整清单（内容不符的会被覆盖、缺失的会被创建、孤儿不会被删）——面板据此先看清单再点生成。
- **`enabled = false` 仍照跑**：契约面未启用时命令 SHALL 返回 `Ok`（而非 `Err` 或空壳），三类差异照实计算——此时应生成集只含 `INDEX.toml`，故 `missing` 通常就是那一条（其落点为 `<archi_graph_dir>/INDEX.toml`）；MUST NOT 因 `enabled = false` 返回空的结构化结果让面板无从呈现冷启动态。
- **`enabled` 的两处换根后果（本变更）**：① 被监测目录由 `<archi_api_dir>` 改为 `<archi_models_dir>`，而模型目录在模型落地后**恒存在**——故 `enabled` 不再可能因「目录缺席」而为假，冷启动态的新形态是「目录在、递归零份 `*.md`」（六个 `NN-<域>.toml` 不是启用证据）；② 原「架构域存在而契约目录缺席」的判据面因此只剩「零份 `*.md`」一条，`enabled` 的语义（契约面是否启用）与读数在本仓 **仍为 `true`**，MUST NOT 被改写成「模型在即启用」。
- **路径串口径**：三列元素 SHALL 为 repo 相对 POSIX 串（剥 `root` 前缀、`\` → `/`、去前导 `./`），经 `archi::display` 单点产出；MUST NOT 在命令层或前端另写归一份归一。
- **错误面**：空白 / 不可读 / 非目录 root、模型结构诊断非空、契约资产非法（任一契约不可解析 / 双向 gate 失败 / `$defs` 规则失败）SHALL 以 `Result<T, String>` 的 `Err` 返回且零部分结果（沿既有命令面口径；`ApiError::message()` 透传原文，MUST NOT 包装、MUST NOT 补默认值）。
- **出线形状**：`ArchiApiCheck` / `ArchiApiMeta` SHALL 带 `specta::Type` 与 `serde(rename_all = "camelCase")`，字段序即声明序，经 `bindings:export` 生成到 `bindings.ts` / `bindings.zod.ts`；MUST NOT 手写前端类型镜像。**本变更零形状改动，但绑定文件不是零 diff**：`dto.rs` 的两处 doc 方位串经 specta 抄进 `bindings.ts`（`ApiStatus` / `ArchiApiRef.contractPath`）⇒ 该文件恰 2 行 doc 注释随动；`ArchiApiCheck` / `ArchiApiMeta` 与其余出线类型的字段名 / 字段序 / 联合成员零变化、`bindings.zod.ts` 零 diff、`bindings:export` 幂等（出现形状面或 zod 面 diff 才是误动）。
- **`meta` 只带两项**：`ArchiApiMeta` SHALL 只含 `toolVersion` / `commonDigest`——`ApiMeta` 的 `schemaUri` / `generatedNotice` / `common` 规则表 MUST NOT 出线（前端不消费，出线等于把内部载元变成协议面）。

#### Scenario: 零写盘且三类差异齐备

- **WHEN** 在本仓根分别于调用前后对工作树取字节快照并调用 `archi_api_check`
- **THEN** 两次快照完全相同；出参三列按「内容不符 / 应删未删 / 应生成未生成」分别列出条目（零差异时为三个空数组），`enabled = true`，`meta.toolVersion` 与生成器 crate 版本一致

#### Scenario: 冷启动态照实报告

- **WHEN** 观察根的架构域存在、`models/` 含六个 `NN-<域>.toml` 但递归零份 `*.md`（或在 `models/` 缺席的极端形态下）
- **THEN** `enabled = false` 且命令返回 `Ok`；`missing` 含 `<archi_graph_dir>/INDEX.toml` 一条（应生成集仅含索引），`mismatched` / `orphaned` 依盘上实况列出；MUST NOT 返回 `Err`、MUST NOT 返回全空的伪造结果、MUST NOT 因「模型文件在场」把 `enabled` 报成 `true`

#### Scenario: 结构错与非法契约一律 Err

- **WHEN** 模型含任一结构诊断后调用该命令，或把某份契约改成不可解析
- **THEN** 命令返回 `Err`（文案点名成因），三列不出线；磁盘字节零变化

#### Scenario: 三条路径串同源

- **WHEN** 审查出参三列的串形态与 `archi::display` 的输出
- **THEN** 逐条相等（反斜杠归一为 `/`、无前导 `./`、无绝对路径）；`orphaned` / `mismatched` 里出现的契约生成物串形如 `.dev-team/architecture/models/<Domain>/<FQN>.schema.json`、索引串形如 `.dev-team/architecture/graph/INDEX.toml`；源码内零第二份「strip 前缀」实现

## Module Contract

### API: 检查 / 生成两命令（零形态变化，仅串换段）

| 模块 | 变化 | 关键契约 |
|---|---|---|
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | 零改动 | `archi_api_check(root) -> Result<ArchiApiCheck, String>` 薄包装、`*_inner` 测试缝、`Err` 面与 `display` 单点逐字不变；`enabled` / 三列全部来自 `core/archi` |
| `crates/core/archi/src/api/mod.rs::enabled` | 改（换根，见 `desktop-archi-api-check`） | 契约面启用判据 = `<archi_models_dir>` 递归含 ≥1 份 `*.md`；命令层 MUST NOT 另判一套 |
| `packages/desktop/src/views/architecture/components/contract-panel.tsx` + `hooks/use-archi-api-check.ts` | 零改动 | 两轴呈现、状态矩阵、「契约面未启用」文案分支与三类差异渲染逐字不变（`enabled` 语义与字段名均未变，变的只是后端填值口径中的目录） |
| `packages/desktop/src/views/architecture/**` 的测试 | 零改动 | 面板用例的 `enabled` / 三列均为 DTO 载荷样例，不参与路径派生；`architecture-canvas.test.tsx` / `element-drawer.test.tsx` 内的旧契约路径串保持原样（fixture 契约，见提案「测试文件」） |
| `packages/desktop/src/types/generated/` | 仅 `bindings.ts` 的 2 行 doc 注释 | 出参形状零改动；`dto.rs` 的 doc 换段经 specta 进 `bindings.ts`（`ApiStatus` / `ArchiApiRef.contractPath`）⇒ 恰 2 行注释随动，`bindings.zod.ts` 零 diff、`bindings:export` 幂等 |
