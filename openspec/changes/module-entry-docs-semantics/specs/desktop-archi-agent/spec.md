# desktop-archi-agent Specification（delta: module-entry-docs-semantics）

## Purpose

本 delta 只做**契约路径串换段**（E-1 目录迁移）：`archi_module` 的 `api` 并列字段按 `<archi_models_dir>/<Domain>/<FQN>.md` 定位（原 `<archi_api_dir>` 已退役），且架构会话知识文本注入的目录方位由「契约目录」改为「契约资产在模型目录的 `<Domain>/` 子目录 + 索引在 `graph/`」。工具面形状、注入矩阵、只读属性、正文不经响应的边界**逐字不变**——本变更零 agent 工具新增、零矩阵改动。

## MODIFIED Requirements

### Requirement: archi_module 只读工具的定位契约

SDK 引擎 SHALL 提供专用只读工具 `archi_module`（不在七通用工具 `TOOL_NAMES` 封闭集内，经 `SessionToolset` 受控注入），语义为**定位**（已知在看哪个模块）：

```jsonc
{ "name": "archi_module",
  "parameters": { "element": "可选。元素 FQN；省略时返回全部元素的薄概览" } }
```

- 指定 `element` 时响应 SHALL 含：`element`（`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `modelFile` / `modelLine`）、`dependsOn`（`target` + `description`）、`dependedBy`（`source` + `description`）、`children`（直接子元素 FQN）、`docs`（四字段关联条目）、**`api`（并列契约状态字段）**。
- 省略 `element` 时 SHALL 返回全部元素的**薄概览**（`fqn` / `kind` / `gate` / 一行 `summary` / **`api.status`**，无邻接、无文档、无契约路径），并受既有单结果字节上限约束（超限截断留痕）。
- `modelFile` + `modelLine` SHALL 来自解析器已产出的声明位置（零额外实现），使 agent 能直接 `edit` 到该行而无需 grep 猜测。
- 元素不存在 SHALL 返回可诊断 `Err`（附近似 FQN 或总量提示），MUST NOT 返回空对象。
- 模型结构错时 SHALL 返回结构化诊断清单作为 `Err`（绝不 panic），使 agent 能看出"模型本身坏了"而不是"没有这个模块"。
- 响应 SHALL 经 `archi::dto` 单点投影产出（MUST NOT 在工具执行体内另拼一份 JSON）。
- **`api` 并列字段**：单元素响应 SHALL 并出 `api` 对象（`status`（`declared` / `none`）/ `contractPath` / `schemaPath` / `generatedPath`），定位 SHALL 按 `<archi_models_dir>/<Domain>/<FQN>.md` 与相应的 `.schema.json` / 生成物路径**派生**（全经 `foundation::layout` 的派生族与 `api::paths` 单点，MUST NOT 行内拼路径；`api` 段已退役，契约资产落在模型目录的 `<Domain>/` 子目录内）；文件不存在即 `status = none` 且三个路径字段为 `null`（零配置，「有契约 / 无契约」成为可枚举事实）。该字段 MUST NOT 承载协议正文（正文由 agent 用 `read` 按路径自取），MUST NOT 成为独立工具、MUST NOT 改变本工具的只读属性与注入矩阵。

#### Scenario: 定位单元素

- **WHEN** 模型会话中调用 `archi_module(element="App.Auth")`
- **THEN** 返回该元素的 `kind` / `gate` / `path` / `metadata` / `modelFile` / `modelLine`，其出边与入边（各含对端 FQN 与 `description`）、直接子元素清单、`docs` 条目（按强度序）

#### Scenario: 省略元素给薄概览并截断

- **WHEN** 在不传 `element` 的调用下模型含数百元素
- **THEN** 返回全元素薄概览（无邻接无文档），超出单结果字节上限时截断并留痕（提示可指定元素收窄）

#### Scenario: 坏模型可辨

- **WHEN** 模型文件被写坏（语法错 / 环）后调用 `archi_module`
- **THEN** 返回结构化诊断（含 `file:line`）且 `is_error` 为真；执行体不 panic，run 不中断（agent 可转去修模型）

#### Scenario: api 并列字段两态可辨

- **WHEN** 分别对契约在案的元素（`App.Backend.Infra.ArchiRuntime`）与无契约元素调用 `archi_module(element = …)`
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向 `.dev-team/architecture/models/Infra/` 下的实际文件、`generatedPath` 指向入口同级生成物；后者 `api.status = "none"` 且三者 `null`；其余字段（元素 / 邻接 / 子元素 / `docs`）与既有契约逐字一致

#### Scenario: 薄概览带 api.status 但不带路径

- **WHEN** 省略 `element` 调用 `archi_module`（模型含数百元素）
- **THEN** 每条薄概览含 `api.status`（`declared` / `none`），不含 `contractPath` / `schemaPath` / `generatedPath`；超单结果字节上限时仍按既有纪律截断留痕

#### Scenario: 契约正文不经工具响应

- **WHEN** 审查 `archi_module` 响应体与体积
- **THEN** 只含 `api` 四字段（状态 + 三路径），零签名、零 `$defs`、零散文；正文由 agent 用 `read` 按 `contractPath` 自取

#### Scenario: 契约路径零 `api` 段残留

- **WHEN** 对本仓契约在案元素（`App.Backend.Infra.ArchiRuntime`）取 `archi_module` 响应，并审查响应内的三个路径串
- **THEN** `contractPath` 为 `.dev-team/architecture/models/Infra/App.Backend.Infra.ArchiRuntime.md`、`schemaPath` 为同目录的 `.schema.json`；三串内 MUST NOT 出现 `architecture/api` 形态（旧段已退役）；派生点消失后仍由 `api::paths` 单点产出（工具执行体零路径拼接）

## Module Contract

### Function: 受影响入口

| 模块 | 变化 | 关键契约 |
|---|---|---|
| `crates/infra/agent/src/archi_knowledge.rs` | 改（目录方位串） | 注入段 SHALL 由「契约目录：`<archi_api_rel_dir>`」改为「契约资产：`<archi_models_rel_dir>/<Domain>/<FQN>.md`（索引 `<archi_graph_rel_dir>/INDEX.toml`）」；`api` 段串零残留；四面词表注入、纯函数零 IO、降级为 `None` 的语义逐字不变；体量仍受 4KB 上限约束 |
| `crates/infra/agent/src/archi_knowledge_test.rs` | 改（断言串） | 「契约目录：」引导语断言改为「契约资产：」+「契约索引：」两条并各断言 `<archi_models_rel_dir>` / `<archi_graph_rel_dir>` 逐字注入；示例片段可解析、四面词表在场、体量上限、纯函数性四条语义不变 |
| `crates/core/archi/src/dto.rs`（契约投影面，见 `desktop-archi-model`） | 改（定位串换段） | `ArchiApiRef` 三路径由 `api::paths` 派生后转 repo 相对形态；契约 md / schema 在 `<archi_models_dir>/<Domain>/`；DTO 形状（字段名 / 顺序 / 派生）零变化。**绑定面**：两处 doc 方位串经 specta 抄进 `bindings.ts` ⇒ 恰 2 行 doc 注释随动、`bindings.zod.ts` 零 diff（「形状零变化」不等于「绑定零 diff」） |
