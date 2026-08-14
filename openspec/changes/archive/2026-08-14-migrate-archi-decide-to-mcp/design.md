# 设计: migrate-archi-decide-to-mcp

> **变更**: migrate-archi-decide-to-mcp
> **日期**: 2026-08-12

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `archi-decide` lib | ADR create / list / update；slug 文件名；状态校验；从 `templates/adr.md` 渲染 create | `plugins/dev-team/bin/src/lib/archi-decide.ts` | `node:fs`、`node:path`、本模块 schema 类型 | TypeScript |
| `archi-decide` Zod schema | `archi_decide` 入参/出参；按 `action` 判别联合 | `plugins/dev-team/bin/src/schemas/archi-decide.schema.ts` | `zod/v4`、`projectRootSchema` | TypeScript |
| schemas barrel | 导出新 schema | `plugins/dev-team/bin/src/schemas/index.ts` | `archi-decide.schema.ts` | TypeScript |
| MCP 注册 | 注册工具 `archi_decide`；`withResolvedProjectRoot` 后按 action 调 lib | `plugins/dev-team/bin/src/mcp.ts` | schemas、`lib/archi-decide`、`lib/project-root` | MCP SDK |
| architecture agent | Decide mode 与工具清单改为 MCP `archi_decide` | `plugins/dev-team/agents/architecture.md` | MCP（`__MCP:archi_decide__`） | Markdown |
| ADR 模板 | create 渲染源（章节结构权威） | `plugins/dev-team/templates/adr.md` | 无 | Markdown |
| 构建产物 | 删除产物中的 `utils/archi-decide.py`，刷新 agent / manifest | `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/` | `pnpm -C plugins/dev-team run build` | 构建输出 |

### 组件图

```
宿主 Agent (architecture Decide)
        |
        v
MCP archi_decide  (mcp.ts)
        |
        +-- withResolvedProjectRoot(project_root)
        |
        v
lib/archi-decide.ts
  createAdr  --> 读 templates/adr.md --> 写 openspec/architecture/decisions/
  listAdrs   --> 读 decisions/*.md   --> 按日期降序 / 可选 status 过滤
  updateAdr  --> 改 **状态** / 可选 **取代者**
```

### 设计决策（相对 proposal 待决项）

| 问题 | 决定 | 理由 |
|------|------|------|
| Zod 校验形态 | `z.discriminatedUnion('action', …)`；`project_root` 写入每个 action 分支 | explore 已定；MCP JSON Schema 每个分支自洽，避免松散 optional 串扰 |
| 模板运行时根 | 相对**插件根**解析 `templates/adr.md`：从当前模块目录向上探测含该文件的根（打包后通常为 `bin/` 的父目录；源码/vitest 为 `plugins/dev-team/`） | 与 assemble 复制 `templates/` 到插件根一致；不依赖 `__DEV_TEAM_ROOT__`（仅 agent 文案 token）；找不到则结构化失败且不写半文件 |
| scope 入参 | MCP 使用 `scope?: string[]`（元素 id / FQN 列表） | 结构化优于 shell 逗号串；写入「影响范围」仍为 `- item` 列表 |
| alternatives 入参 | `alternatives?: { name; description?; pros?; cons? }[]`；`pros`/`cons` 为 `string \| string[]` | 对齐旧脚本 JSON 形状，去掉 shell 转义 |
| create 与旧 py 字节级一致 | **不保证** | 以模板章节与正式字段契约为准（AC-07） |
| CLI | 不新增 | AC-11；`cli.ts` 不改 |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/lib/archi-decide.ts` | ADR create/list/update 实现；模板解析与渲染 |
| `plugins/dev-team/bin/src/schemas/archi-decide.schema.ts` | `archiDecideInputSchema` / `archiDecideOutputSchema` 及关联类型 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/schemas/index.ts` | 导出 `archiDecideInputSchema`、`archiDecideOutputSchema` 及必要类型 | 供 `mcp.ts` 与其它模块引用 |
| `plugins/dev-team/bin/src/mcp.ts` | 在 `MCP_TOOLS` 注册 `archi_decide`；handler 包 `withResolvedProjectRoot`，按 `action` 调用 lib | AC-01、AC-06 |
| `plugins/dev-team/agents/architecture.md` | description、工具清单、DECIDE 步骤改为 MCP `archi_decide`；删除 `archi-decide.py` / `python …` 引用；可去掉过时「Python utilities」段或改为仅 MCP | AC-08 |
| `plugins/dev-team/utils/archi-decide.py` | **删除**整文件 | AC-09 |
| `plugins/dev-team/templates/adr.md` | 仅在渲染需要时做小幅占位符/空行对齐；章节标题保持不变 | AC-07；非必须改则保持现状 |
| `plugins/dev-team/package.json` | `version` bump（当前 `2.10.28` → 下一 patch） | 项目规则：改源码后升版 |
| `CLAUDE.md` | 「Python utilities … ADRs」改为 MCP / 无 Python ADR 表述 | 文档对齐，非行为变更 |
| `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/` | 经 build 刷新：无 `utils/archi-decide.py`；agent 与 `manifest.json` 同步 | AC-09 |

<!-- 配置：无独立 hooks/settings 键变更；版本见 package.json（上表）。测试文件（archi-decide.test.ts、mcp.test.ts、hooks.test.ts）属测试阶段，本清单不列。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `createAdr` | `plugins/dev-team/bin/src/lib/archi-decide.ts` | 新增 | `function createAdr(projectRoot: string, input: ArchiDecideCreateInput): ArchiDecideCreateResult` | 渲染模板并写入 `openspec/architecture/decisions/YYYY-MM-DD-<kebab>.md`；默认 `status: 'proposed'`；模板缺失 / 非法 status / 文件已存在 → `{ success: false, error }` |
| `listAdrs` | `plugins/dev-team/bin/src/lib/archi-decide.ts` | 新增 | `function listAdrs(projectRoot: string, status?: AdrStatus): ArchiDecideListResult` | 目录不存在返回空列表；按日期降序；可选 status 过滤 |
| `updateAdr` | `plugins/dev-team/bin/src/lib/archi-decide.ts` | 新增 | `function updateAdr(projectRoot: string, input: ArchiDecideUpdateInput): ArchiDecideUpdateResult` | 更新 `**状态**`；`superseded` 无 `superseded_by` 失败且不写盘；成功时可写 `**取代者**` |
| `archi_decide`（MCP tool） | `plugins/dev-team/bin/src/mcp.ts` | 新增 | MCP tool：`name: 'archi_decide'`；input=`archiDecideInputSchema`；output=`archiDecideOutputSchema`；经 `jsonContent` 返回 | 单工具 + `action`；无 CLI 子命令 |

私有辅助（slugify、resolveAdrTemplatePath、renderAdrFromTemplate 等）不导出，不列入上表。

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `AdrStatus` | `archi-decide.schema.ts`（或 lib 从 schema 导出） | 新增 | `'proposed' \| 'accepted' \| 'deprecated' \| 'superseded'` |
| `ArchiDecideAlternative` | `archi-decide.schema.ts` | 新增 | `{ name: string; description?: string; pros?: string \| string[]; cons?: string \| string[] }` |
| `ArchiDecideCreateInput` | `archi-decide.schema.ts` | 新增 | create 分支字段（不含 `action`/`project_root` 的 lib 入参视图，或与 Zod 推断对齐） |
| `ArchiDecideUpdateInput` | `archi-decide.schema.ts` | 新增 | `{ file: string; status: AdrStatus; superseded_by?: string }` |
| `ArchiDecideCreateResult` | `archi-decide.schema.ts` | 新增 | `{ success: boolean; path?: string; filename?: string; error?: string }` |
| `ArchiDecideListResult` | `archi-decide.schema.ts` | 新增 | `{ adrs: ArchiDecideListEntry[]; count: number }` |
| `ArchiDecideListEntry` | `archi-decide.schema.ts` | 新增 | `{ filename; title; status; date; scope: string[]; path }` |
| `ArchiDecideUpdateResult` | `archi-decide.schema.ts` | 新增 | `{ success: boolean; path?: string; old_status?: string; new_status?: string; error?: string }` |
| `archiDecideInputSchema` | `archi-decide.schema.ts` | 新增 | `z.discriminatedUnion('action', [create, list, update])`，每支含必填 `project_root` |
| `archiDecideOutputSchema` | `archi-decide.schema.ts` | 新增 | 按 `action` 判别的输出联合（或等价可解析为 MCP structuredContent 的联合） |

### MCP 输入形状（契约摘要）

| `action` | 必填字段 | 可选字段 |
|----------|----------|----------|
| `create` | `project_root`, `action`, `title`, `background`, `decision` | `consequences`, `alternatives`, `scope`（`string[]`）, `status`（默认 `proposed`） |
| `list` | `project_root`, `action` | `status`（过滤） |
| `update` | `project_root`, `action`, `file`, `status` | `superseded_by`（`status=superseded` 时业务必填） |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| ADR 文件 | front matter 式元数据：`标题`（`# ADR: …`）、`日期`、`状态`、可选 `取代者`；正文：`## 背景` / `## 决策` / `## 后果`（含正面/负面小节）/ `## 备选方案` / `## 影响范围` | 影响范围条目引用模型元素 id/FQN（软引用，本次不校验 `archi_query`） | `<projectRoot>/openspec/architecture/decisions/YYYY-MM-DD-<kebab-title>.md` |
| list 条目（内存） | `filename`, `title`, `status`, `date`, `scope[]`, `path` | 自 ADR 文件解析 | 不单独持久化 |
| 插件模板 | `templates/adr.md` 章节骨架 | create 只读 | 插件根下只读文件；不写入用户项目 |

### create 渲染规则

1. 解析插件根上的 `templates/adr.md`；失败 → `{ success: false, error }`，**不**创建 decisions 文件。
2. `date =` 本地日历日 `YYYY-MM-DD`；`slug =` kebab(title)（小写、去非常用字符、空白→`-`）；`filename = `${date}-${slug}.md``。
3. 若目标路径已存在 → 失败。
4. 用入参填充标题、日期、状态、背景、决策；`consequences` 写入「后果」区（保留模板「正面/负面」小节结构，将正文填入合理位置，避免丢掉章节标题）；`alternatives` 渲染为 `### 方案 N：{name}` + 描述/优缺点；`scope` 渲染为 `-` 列表，空则 `- (none)` 或模板等价占位。
5. 写出 UTF-8 markdown；返回 `success` + `path`/`filename`。

### update 规则

- 仅匹配并替换 `**状态**: <word>`；`superseded` 且提供 `superseded_by` 时，若尚无 `**取代者**` 则插在状态行后。
- 非法 status / 缺文件 / `superseded` 无 `superseded_by` → 失败且不改文件。

---

<!-- 本变更不涉及 HTTP API；交付面为 MCP tool，见「公共函数 / API」。 -->

## 依赖

### 运行时依赖

- `zod`（已有）— MCP 入参/出参 schema
- `@modelcontextprotocol/sdk`（已有）— 工具注册
- Node.js `fs` / `path` — 读写 ADR 与模板
- 无新增 npm 包

### 构建/测试依赖

- 既有 `vp pack` / `pnpm -C plugins/dev-team run build` — 刷新三端产物与 manifest
- 测试依赖变更属测试阶段，此处不列

---

## 待决问题

（无阻塞项。模板根解析算法与 Zod 判别联合已在上文敲定。）

实现期可微调但不改契约的细节：

- consequences 在「正面/负面」子节中的具体填充排版（须保留 `## 后果` 与子节标题）
- `archiDecideOutputSchema` 用顶层 `action` 判别 vs 三支结果 schema 在 handler 内分支校验（对外 MCP 仍按 action 返回对应结构）
