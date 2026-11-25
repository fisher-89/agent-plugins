# desktop-archi-canvas Specification（delta: archi-agent-context-review）

## Purpose

本 delta 只同步画布对判据词汇的引用：`apiFindings` 收为十值（三值离场），旧条文里「入口文档判据三值不触发停摆」的列举随之作废。

- 停摆态的唯一成因仍是模型结构错；接口文件装载失败走**命令面错误**（`Err` + 面板行内错误），MUST NOT 被读成停摆。
- 文档面零射程：`.md` 在不在盘、写了什么都不改变图渲染态。
- 右栏「文档」段按三条路径的非空项出页签——三路径各自独立后自然正确，组件零功能改动。

## MODIFIED Requirements

### Requirement: 结构错的停摆态（全或无）

模型有**任何**结构错时，画布 SHALL 停摆：不渲染残缺图、不渲染左栏树（文件树与元素树均不挂载）、不降级样式、不留幽灵位置；图形区域 SHALL 改为显示错误面板，逐条列出结构错（`kind` + `message` + `file:line`）并提示「修好后再看」。停摆态下画布数据 SHALL 为零图数据（`elements` / `relations` 皆空），MUST NOT 以「残缺图 + 高亮」替代。

三条策略各管一段且 MUST NOT 互相替代：**读取**照常加载并返回结构化诊断（绝不 panic）；**渲染**停摆（绝不画畸形图）；**门禁**判结构错（归 `desktop-archi-gate`）。

无结构错时 SHALL 渲染完整图。**接口面的判据**（`apiFindings` 内的 `docMissing` 等）MUST NOT 触发停摆，也 MUST NOT 触发任何降级样式。

- **判据词汇的呈现面 SHALL 与报告同源**：画布只消费 `apiFindings` 的**计数与标记**，不解释 `kind` 语义；本变更后该词汇收为**十值**（`apiReexportDrift` / `apiDocMissing` / `docNotFound` 三值离场）。画布 MUST NOT 残留按这三值分支的渲染逻辑或常量——旧条文把它们列为「不触发停摆的入口文档判据」，该列举随之作废（三者已不存在）。
- **接口文件非法不是结构错**：某元素 `<FQN>.schema.json` 装载校验失败会让检查面以 `ApiError` 停摆（见 `desktop-archi-api` / `desktop-archi-api-check`），该错误走**命令面错误**（报告 `Err` + 面板行内错误），MUST NOT 被画布读成「模型结构错」而进入停摆态——停摆态的唯一成因仍是模型结构错。
- **文档面零射程**：文档（`.md`）在不在盘、写了什么都不改变图渲染态。

#### Scenario: 有结构错即停摆

- **WHEN** 模型含一个环（或未定义引用 / 孤儿层级 / 缺必填字段等任一结构错）时请求画布数据
- **THEN** 返回停摆态：`elements` 与 `relations` 均为空，错误面板逐条呈现诊断（含 `file:line`）；画布零节点零边，左栏树零节点（文件树与元素树均不挂载）

#### Scenario: 无结构错渲染完整图

- **WHEN** 模型零结构错，且报告含 38 条 `docMissing`
- **THEN** 返回完整图数据，左栏树与画布正常渲染；`docMissing` 零标记、零降级样式

#### Scenario: 接口文件坏走命令面错误而非停摆

- **WHEN** 某元素的 `<FQN>.schema.json` 被写坏（绕过写闸），随后请求画布数据与漂移报告
- **THEN** 画布数据照常返回（模型无结构错 ⇒ 不停摆）；漂移 / 检查命令以 `ApiError` 返回 `Err`，面板呈行内错误——两者可分辨，MUST NOT 混为一个停摆态

#### Scenario: 停摆态不给部分形状

- **WHEN** 审查停摆态的渲染产物
- **THEN** 不存在「部分节点 + 高亮」「悬空边丢弃」「缺字段节点灰化」等任意降级形态（零降级样式类、零占位节点）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/architecture/components/dashboard/architecture-node.tsx` | 画布右栏与分段视图（**零功能改动**） | `CardContent` 三段 Tabs（文档 / 接口 / 关系图）逐字不变；「文档」段按本元素三条路径的非空项出页签——三路径现在各自独立出线（`contractPath` = 文档、`schemaPath` = 接口、`generatedPath` = 生成物），故「缺一份即三条全空」的旧行为消失，组件侧无需改动 |
| `packages/desktop/src/views/architecture/components/model-info.tsx` | 元素信息面板（**零功能改动**） | 概览 / 契约段 / 入口文档 / 改档入口逐字不变（`doc` 字段零判据但 UI 语义不变：`api = none` 元素仍呈单条「入口文档」） |
| `packages/desktop/src/views/architecture/lib/**`（标记派生面） | 零改动 | 标记只认 `apiFindings` 的计数与 `kind` 值域外的结构事实；`kind` 词汇收缩不改标记派生（画布不解释 `kind`） |
| `packages/desktop/src/views/architecture/hooks/use-archi-module.ts` / `use-archi-graph.ts` / `use-archi-drift.ts` | 零改动 | 触发条件、载荷与取数纪律逐字不变；`ArchiModuleDetail` 的新字段 `assets` / `reading` 由 agent 工具消费，前端不读 |
