# desktop-archi-canvas Specification（delta: module-doc-field-docs-retirement）

## Purpose

本 delta 只改元素抽屉的信息面（E-2）：原「② `docs` 关联列表（三源条目，按强度序）」与「③ ADR 条目列表」整段退役，替换为**入口文档单条**——`api = none` 元素且 `doc` 非空时呈一条「入口文档」（可经 `FileViewer` + `read_file` 打开）；`api = declared` 元素的入口文档早已由契约段 `ContractPathRow("契约", api.contractPath)` 呈现，MUST NOT 出现两条。画布本体、停摆态、漂移标记与能力差异零改动。

## MODIFIED Requirements

### Requirement: 抽屉内容与本期边界

元素抽屉 SHALL 呈现四类信息：① 元素概览（`kind` / `path` / `gate` / `modelFile:line`，位置可读且可直接用于 `edit`）；② **入口文档单条**（仅 `api = none` 元素且 `doc` 非空时呈现，值为 `ArchiModuleDetail.doc` 的路径串，经 `FileViewer` + 通用 `read_file` 打开；`api = declared` 元素不渲本段——其入口文档由契约段的 `ContractPathRow("契约", api.contractPath)` 承担，**MUST NOT 出现两条入口文档**）；③ 契约段（`api.status` + 三路径 + 本元素判据，见 `desktop-archi-contract-panel`「抽屉契约段」）；④ `incremental` 模式下 `gate` 的编辑入口。

以下内容 MUST NOT 在本能力出现（防"半成品字段"；漂移判定的消费面止于边 / 节点标记，抽屉不承载报告细节）：

- 「相邻报告」区块（最近一次检查的 errors / warnings）——本期抽屉零该区块、零占位框；
- 漂移统计 / 门禁结论 / 豁免 reason 编辑入口；
- **`docs` 关联列表与其中的 ADR 条目**——`docs` 三源关联（含 `kind = "adr"` 的 `text` 源）已随 `desktop-archi-module-docs` 整体退役；抽屉 MUST NOT 保留文档列表位、MUST NOT 保留「按强度序」排序语义、MUST NOT 保留 ADR 正文按需展开的交互（ADR 改经入口文档内链接 / 左栏文件树 / 通用文件搜索抵达）。

`doc` 的路径取数 SHALL 与契约路径同一通路（通用 `read_file`，绝对路径入参）：本能力 MUST NOT 新增专用文档读取命令、MUST NOT 使该通路成为任意文件读取面的**新**风险点——`read_file` 本来就是通用绝对路径读面，本变更只是把「点开一个模型声明的路径」这一个动作接到它上面。

#### Scenario: 概览含可定位声明位置

- **WHEN** 打开某元素抽屉
- **THEN** 呈现 `kind` / `path` / `gate` 与 `modelFile:line`（该文件该行为该元素表头），agent 与人可据此直接定位声明

#### Scenario: api = none 元素的入口文档单条

- **WHEN** 聚焦一个 `api = none` 且 `doc` 非空的元素打开抽屉
- **THEN** 抽屉内呈一条「入口文档」行，点「查看」经 `read_file` 打开该路径；同一抽屉内零文档列表、零 ADR 条目

#### Scenario: declared 元素只有契约段一条入口文档

- **WHEN** 聚焦契约在案的元素（如 `App.Backend.Infra.ArchiRuntime`）打开抽屉
- **THEN** 入口文档只出现在契约段的 `ContractPathRow("契约")` 一行；抽屉内 MUST NOT 出现第二条入口文档行（`doc` 为 `null`）；段序为「概览 → 契约 → 入口文档 → 豁免」，`declared` 与 `api = none` 未声明 `doc` 时「入口文档」段整段不渲染

#### Scenario: 空态可辨

- **WHEN** 聚焦 `api = none` 且未声明 `doc` 的元素
- **THEN** 「入口文档」段呈一条空态文案（「无入口文档」，testid 随字段面更名）；不报错、不显示失效路径、不产生额外 invoke

#### Scenario: 本期无相邻报告区块

- **WHEN** 审查抽屉渲染产物
- **THEN** 无「相邻报告」区块、无 `errors` / `warnings` 计数字段、无空占位框

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/architecture/components/element-drawer.tsx` | 右抽屉 | 概览（`kind` / `path` / `gate` / `modelFile:line`）+ 契约段 + **入口文档单条**（仅 `api = none` 且 `doc` 非空；空态文案随字段面更名）+ `incremental` 下 `gate` 编辑入口；`DocsSection` / `DocRow` 删除；本期零「相邻报告」区块 |
| `packages/desktop/src/views/architecture/hooks/use-archi-module.ts` | 抽屉取数 | 仍单次 `archi_module`；载荷的 `docs` / `warnings` 位次由 `doc` 取代（取数逻辑与触发条件零改动） |
| `packages/desktop/src/views/architecture/lib/drift-marks.ts` | 报告 → 边标记纯函数 | 零改动（只读 `observations` / `suppressed`；`apiFindings` 仍不参与节点 / 边标记） |
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | 共享画布 | 零改动（三段布局、左栏三视图、抽屉接线、`mode` 语义逐字不变） |
| `packages/desktop/src/views/architecture/components/element-drawer.test.tsx` | 抽屉语料 | `docsFixture` 删；`docs: []` fixture → `doc: null`；`archi-docs-empty` 用例改「入口文档空态」；新增两条否定断言（`declared` 元素零入口文档行 / 抽屉内零 `docs` 列表） |
