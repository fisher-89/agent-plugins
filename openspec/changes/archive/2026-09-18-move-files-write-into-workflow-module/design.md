# 设计: move-files-write-into-workflow-module

> **变更**: move-files-write-into-workflow-module
> **日期**: 2026-09-18

---

## 提案与规格同步状态（信息性）

`proposal.md` 与 `specs/change-files/spec.md`、`specs/workflow-file-inventory/spec.md` 已由提案阶段写入并同步，本 design 不将它们列入变更清单与任务。`recordFileOps` / `appendFileOps` / `setFileBuckets` 的名称与签名在本 design 终定，与 change-files delta 的 Module Contract 拟定值一致；唯一扩展是 `loadGitignoreFilter` 增加可选 `onWarn` 回调参数（见待决问题），归档时随 Module Contract 一并注明。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| gitignore 层级过滤器 | 装载根 + 目标路径祖先链上各层 `.gitignore`，按 gitignore 语义判定单条路径（深层优先、目录短路、fail-open） | `plugins/dev-team/bin/src/modules/workflow/files/gitignore.ts`（新增） | `ignore`（^7） | TypeScript |
| 清单原语 + 人工写通道 | 既有读/折叠/写原语（不动），新增 `change_files` 的 append/set 读改写语义 | `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts`（修改） | `schemas/workflow.schema.ts`、`utils` | TypeScript |
| PostToolUse 归账管线 | 规范化 → 自污染排除 → gitignore 过滤 → 读 → 折叠 → 落盘 的事件归账入口 | `plugins/dev-team/bin/src/modules/workflow/files/record.ts`（新增） | `files/file-inventory.ts`、`files/gitignore.ts` | TypeScript |
| workflow barrel | 模块统一出口，扩充写路径导出 | `plugins/dev-team/bin/src/modules/workflow/index.ts`（修改） | 上方三个组件 | TypeScript |
| PostToolUse 记录器（命令层） | stdin 解析、`phase_next` 绑定识别、`tool_input` 提取、错误吞并 exit-0，委托归账管线 | `plugins/dev-team/bin/src/commands/record-files.ts`（修改） | workflow barrel、`lib/change`、`lib/project-root`、`lib/session-registry`、`lib/shell-file-ops` | TypeScript |
| `change_files` 命令（命令层） | options 校验、`resolveChangeDir`、委托 append/set 语义 | `plugins/dev-team/bin/src/commands/change-files.ts`（修改） | workflow barrel、`lib/change`、`lib/project-root`、`schemas` | TypeScript |

### 组件设计要点

#### 1. gitignore 层级过滤器（`files/gitignore.ts`）

单一文件规则引擎用 `ignore` npm 包，层级应用自实现（`ignore` 设计上不处理嵌套）。导出面：

```ts
export interface GitignoreFilter;   // 不透明句柄
export function loadGitignoreFilter(projectRoot: string, onWarn?: (message: string) => void): GitignoreFilter;
export function isGitIgnored(filter: GitignoreFilter, relPath: string): boolean;
```

- 句柄内持有：`projectRoot`、根层 `Ignore | null`（`loadGitignoreFilter` 时即刻解析）、`Map<目录前缀, Ignore | null>` 祖先链层缓存、`onWarn` 回调。
- **层级求值算法**（`isGitIgnored`，输入为根相对 POSIX 路径）：
  1. 空路径直接返回 `false`（fail-open）。
  2. 沿祖先链自浅入深行走：对每个目录前缀 `p`，先用**当前已激活层**判定 `p` 作为目录是否被忽略——是则直接返回 `true`（目录短路：被排除目录不再下探，`p/.gitignore` 不装载、不参与判定）；否则尝试装载 `p/.gitignore` 并入激活层集合（缓存；文件缺失 → 该层无规则；读取或解析异常 → `onWarn` 诊断 + 该层按无规则处理）。
  3. 对 `relPath` 自浅层至深层依次用各激活层测试「相对该层的路径后缀」，**取最后一个产生匹配（ignored 或 unignored）的层的结论**（深层文件规则优先于浅层文件）；无任何层匹配 → `false`。
  4. 整个判定包 try/catch，异常 → `onWarn` 诊断 + `false`（fail-open）。
- **fail-open 语义**：任一层 `.gitignore` 缺失视为该层无规则（正常路径，非告警）；读取失败、`ignore` 解析/判定抛错一律按「不忽略」处理并经 `onWarn` 留诊断，MUST NOT 抛错——与记录器 exit-0 吞错策略一致、与折叠「只多记、不漏记」原则一致。
- `ignore` 实例只承担单文件语义（负模式、目录递归、后匹配覆盖、父目录排除后子路径不可 re-include 均由库负责，不逐项自测）；层级序（祖先链、深层优先、目录短路）由上述算法控制。
- 该 API 仅供归账管线内部消费，**不进 workflow barrel**（见要点 4）。

#### 2. 清单原语 + 人工写通道（`files/file-inventory.ts`）

既有 `readFileInventory` / `foldFileOps` / `writeFileInventory` 与 `FileInventory` / `FileOp` 类型不动。追加迁入 `commands/change-files.ts` 的两个读改写入口：

- `appendFileOps(changeDir, paths)` = 原 `applyAppend` + `dedupe`：桶内折叠合并去重，**保留既有 `source`**（已存在路径去重后 source 原样，新并路径无 source）。
- `setFileBuckets(changeDir, paths)` = 原 `applySet`：所提供桶整桶覆写、未提供桶保持原样，覆写条目清除 `source`，净状态中已不存在的路径的 `source` 条目删除。

设计约束（防实现漂移）：`appendFileOps` **刻意不复用** `foldFileOps`——`foldFileOps` 对无 `agentType` 的 op 会清除该路径 `source`，与 append「保留既有 source」语义不同；AC-4 既有断言锁定此差异。两者持久化一律经 `readFileInventory` + `writeFileInventory`（`workflow_type` / `created` / `eval` 保留纪律不变）。此文件**不引入** gitignore 过滤（`change_files` 是人工补录/修正通道，MUST NOT 过滤，AC-5）。

#### 3. 归账管线（`files/record.ts`）

```ts
export function recordFileOps(
  changeDir: string,
  ops: FileOp[],
  context: { projectRoot: string; agentType?: string },
): void;
```

- `ops` 为 hook 事件提取的**原始** ops（路径可能为绝对/反斜杠形态），管线内部按序处理：`normalizeRecordedPath`（项目根相对化，越界丢弃）→ `isExcludedFromInventory`（`openspec/**` 与 `workflow.json` 自污染排除）→ gitignore 过滤（`isGitIgnored` 命中即丢弃）→ `readFileInventory` → `foldFileOps` → `writeFileInventory`。规范化与自污染排除函数自命令层迁入并转为私有。
- 过滤时机：折叠落盘前、同一 hook 调用内同步应用；`GitignoreFilter` 每次 `recordFileOps` 调用惰性构建一次（仅在存在通过规范化与自污染排除的候选路径时构建），满足「单 hook 调用内缓存已解析实例」；`onWarn` 诊断由本组件经 `process.stderr.write` 输出（前缀 `record-files:`，与其余诊断一致）。
- 既有语义原样保留：`files` 缺失的 legacy change 抛错并向上传播（由 `runRecordFiles` 兜底吞并）；过滤对 write/delete/revert 统一生效，不做存量回溯清洗（历史 ignored 路径经 `change_files set` 人工修正）。

#### 4. barrel 与 knip 防护

`workflow/index.ts` 扩充导出 `recordFileOps`（自 `files/record.ts`）、`appendFileOps` / `setFileBuckets`（自 `files/file-inventory.ts`）。**gitignore API 不进 barrel**——这是对 proposal「变更范围」中 barrel 扩充条目（含「gitignore 过滤器」）的有意收窄：其唯一消费方是 `files/record.ts`（模块内相对导入），barrel re-export 将无外部导入方，knip 判 dead export 违反质量门禁；AC-1 只要求封装存在于 `modules/workflow/files/` 下，AC-3 只要求归账管线与 append/set 语义经 barrel 导出，均不受影响。两个 delta spec 的 Module Contract 亦将 gitignore 函数定位为「归账管线内部消费」，与该收窄一致。无 re-export shim：命令层旧实现直接删除，`lib/file-inventory.ts` 保持删除状态。

#### 5. 命令层瘦身后形态

- `commands/record-files.ts` 保留：`runRecordFiles`（stdin 读取 + 全量 catch-all，stderr + exit 0）、`recordFilesEvent`（JSON 解析、`tool_name` / `tool_input` / `session_id` / `agent_type` 提取、`isPhaseNextCall` 绑定识别、`lookupChange`）、`extractOpsFromToolInput`、`bindFromPhaseNextCall`。事件流程收敛为：提取 raw ops（空则提前返回）→ `resolveChangeDir` → `recordFileOps(changeDir, ops, { projectRoot, agentType })`。
- `commands/change-files.ts` 保留：`ChangeFilesOptions` 类型（MCP handler 消费，签名不变）、`runChangeFiles` 的 `resolveChangeDir` 与校验前置，主体委托 `setFileBuckets` / `appendFileOps` 并透传 `{ written, deleted }` 返回投影。

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/modules/workflow/files/gitignore.ts` | 基于 `ignore` 包的层级 `.gitignore` 过滤器：`loadGitignoreFilter` / `isGitIgnored` / `GitignoreFilter`（文件名与导出名按 proposal 授权由本 design 终定） |
| `plugins/dev-team/bin/src/modules/workflow/files/record.ts` | PostToolUse 归账管线 `recordFileOps`：规范化 → 自污染排除 → gitignore 过滤 → 读 → 折叠 → 落盘（proposal 授权的拆分新文件） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/modules/workflow/index.ts` | barrel 扩充：`files/file-inventory.ts` 导出集增补 `appendFileOps` / `setFileBuckets`，新增 `recordFileOps` 导出；gitignore API 不进 barrel | 单一出口原则保持；knip 防护见组件设计要点 4 |
| `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts` | 追加 `appendFileOps` / `setFileBuckets` 与私有 `dedupe`（自 `commands/change-files.ts` 迁入）；既有原语与类型不动 | 人工写通道语义归位模块；append 不复用 `foldFileOps`（source 保留差异）；本文件不接 gitignore 过滤 |
| `plugins/dev-team/bin/src/commands/record-files.ts` | 删除 `normalizeRecordedPath` / `isExcludedFromInventory` / `collectRecordedOps` 与读折叠写步骤；`recordFilesEvent` 改为提取 raw ops 后委托 `recordFileOps` | 命令层瘦身为纯协议适配；stdin 解析、`phase_next` 绑定、错误吞并策略不变；无 shim |
| `plugins/dev-team/bin/src/commands/change-files.ts` | 删除 `applyAppend` / `applySet` / `dedupe`；`runChangeFiles` 校验后委托 `appendFileOps` / `setFileBuckets` | 命令层仅输入校验与委托；append/set 语义不变且不过滤（AC-5） |
| `plugins/dev-team/package.json` | `dependencies` 新增 `"ignore": "^7.0.0"`；`version` 2.10.38 → 2.10.39；随后 `pnpm -C plugins/dev-team run build` 刷新 `claude-plugins/` / `cursor-plugins/` / `cursor-home-image/` | 运行时依赖 + 版本提升；产物由 build 刷新，不手改 |

<!-- 删除文件：无（纯就地移动与修改，无整文件删除），省略此子节 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `recordFileOps` | `plugins/dev-team/bin/src/modules/workflow/files/record.ts` | 新增 | `function recordFileOps(changeDir: string, ops: FileOp[], context: { projectRoot: string; agentType?: string }): void` | 归账管线入口：规范化 → 自污染排除 → gitignore 过滤 → 读 → 折叠 → 落盘；经 barrel 导出 |
| `appendFileOps` | `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts` | 新增 | `function appendFileOps(changeDir: string, paths: { written?: string[]; deleted?: string[] }): FileInventory` | append 读改写：桶内折叠合并去重，保留既有 `source`；经 barrel 导出 |
| `setFileBuckets` | `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts` | 新增 | `function setFileBuckets(changeDir: string, paths: { written?: string[]; deleted?: string[] }): FileInventory` | set 读改写：所提供桶整桶覆写、覆写条目清 `source`；经 barrel 导出 |
| `loadGitignoreFilter` | `plugins/dev-team/bin/src/modules/workflow/files/gitignore.ts` | 新增 | `function loadGitignoreFilter(projectRoot: string, onWarn?: (message: string) => void): GitignoreFilter` | 构建层级过滤器：根层即刻解析，祖先链按需惰性解析并缓存；缺失层视为无规则；解析异常 fail-open + `onWarn`；仅供管线内部消费，不进 barrel |
| `isGitIgnored` | `plugins/dev-team/bin/src/modules/workflow/files/gitignore.ts` | 新增 | `function isGitIgnored(filter: GitignoreFilter, relPath: string): boolean` | 层级判定：深层优先、目录短路、负模式后匹配覆盖；不可判定返回 `false`（fail-open）；不进 barrel |
| `runRecordFiles` | `plugins/dev-team/bin/src/commands/record-files.ts` | 修改 | `function runRecordFiles(): void`（签名不变） | 主体流程改委托 `recordFileOps`；exit-0 吞错策略不变 |
| `runChangeFiles` | `plugins/dev-team/bin/src/commands/change-files.ts` | 修改 | `function runChangeFiles(options: ChangeFilesOptions): ChangeFilesOutput`（签名不变） | 主体改委托模块函数；IO 契约不变 |

<!-- 既有导出 `readFileInventory` / `foldFileOps` / `writeFileInventory` / `getChangedFiles` 签名不变，不列。私有函数（`normalizeRecordedPath`、`isExcludedFromInventory`、`dedupe` 等）迁移不列入。 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `GitignoreFilter` | `plugins/dev-team/bin/src/modules/workflow/files/gitignore.ts` | 新增 | 不透明过滤器句柄：持 `projectRoot`、根层 `Ignore \| null`、祖先链层缓存 `Map`、`onWarn` 回调；消费方不探其内部 |
| `ChangeFilesOptions` | `plugins/dev-team/bin/src/commands/change-files.ts` | 修改（位置不变，定义不动） | 命令层 options 类型，MCP handler 继续消费；随文件瘦身保留 |

<!-- `FileInventory` / `FileOp` / `WorkflowFile` 均不变（schema 不动），不列。 -->

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `dependencies.ignore` | `plugins/dev-team/package.json` | 新增 | `"^7.0.0"` | gitignore 单文件规则引擎，零传递依赖（精确 minor 以安装时最新 7.x 为准） |
| `version` | `plugins/dev-team/package.json` | 修改 | `"2.10.39"` | 版本提升，随后 build 刷新三端产物 |

<!-- hooks.json / settings.json / plugin.json 无键变更：PostToolUse 记录器入口与 matcher 不变。 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `FileInventory`（`workflow.json.files`，不变） | `written: string[]`、`deleted: string[]`、`source?: Record<string, string>`（路径 → 子代理 `agent_type`，仅审计） | `workflow.json` 的可选 `files` 字段；`schemas/workflow.schema.ts` 定义不变 | `<changeDir>/workflow.json`，2 空格缩进 + 尾换行；写路径一律经 `writeFileInventory` 保留 `workflow_type` / `created` / `eval` 与未知键 |
| `GitignoreFilter`（新增，内存态） | `projectRoot`、根层 `Ignore \| null`、层缓存 `Map<目录前缀, Ignore \| null>`、`onWarn` | 由 `recordFileOps` 每次 hook 调用构建一次，跨事件不共享 | 不持久化（进程内句柄） |

新增数据约束：gitignore 过滤只作用于归账入桶方向，不改变 `files` 的字段结构、不回溯清洗既有净状态。

---

## 路由/API 设计

本变更不涉及 HTTP API；两个 MCP tool 的 IO 契约均不变：

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP tool | `change_files` | append/set 清单修正（实现改委托模块；**不应用** gitignore 过滤） | `{ change: string, op: "append"\|"set", written?: string[], deleted?: string[] }` | `{ written: string[], deleted: string[] }` | 无（本地 MCP） |
| MCP tool | `workflow_files` | 只读净状态查询（不变；忠实返回，不做 gitignore 过滤） | `{ change: string, project_root?: string }` | `{ written: string[], deleted: string[] }` | 无（本地 MCP） |

---

## 依赖

### 运行时依赖

- `ignore`（^7，新增） — gitignore 单文件规则引擎（负模式、目录递归、后匹配覆盖）；零传递依赖、MIT，层级应用在其实例之上自实现
- 既有 `cac` / `picomatch` / `zod` / `@likec4/language-services` — 不变

### 构建/测试依赖

- 无新增；`typescript` / `vite-plus` / `knip` / `@stryker-tmp`（stryker）等既有构建与质量工具不变

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | `files/gitignore.ts` 新增文件行 + 组件设计要点 1（`ignore` 包单文件语义、自研层级求值、fail-open、`onWarn` 诊断）；代表用例与层级专项由测试阶段按 proposal 测试文件清单承接 |
| AC-2 | `files/record.ts` 管线内过滤时机（折叠落盘前、惰性单次构建、`onWarn` → stderr）+ `commands/record-files.ts` 委托形态（组件设计要点 3、5）；祖先链全无 `.gitignore` 时各层视为无规则，行为与现状一致且不阻塞 |
| AC-3 | barrel 扩充（修改文件行）+ `recordFileOps` / `appendFileOps` / `setFileBuckets` 公共函数行；两命令文件的删除内容逐项列出，无 re-export shim（组件设计要点 4） |
| AC-4 | 纯移动约束：`appendFileOps` 保持 `applyAppend` 语义且不复用 `foldFileOps`（source 保留差异）、`setFileBuckets` = `applySet` 原样、归账管线步骤原序迁移、`workflow_type` / `created` / `eval` 保留纪律经 `writeFileInventory` 不变 |
| AC-5 | `file-inventory.ts` 明确不接过滤器（组件设计要点 2）；`change-files.ts` 委托路径无过滤调用；gitignore API 仅 `record.ts` 内部消费，结构性排除误接 |
| AC-6 | `package.json` 配置行（`ignore` 依赖 + `version` 2.10.39）+ 阶段四任务执行 `pnpm -C plugins/dev-team run build` 刷新三端产物 |

---

## 待决问题

- `loadGitignoreFilter` 的可选 `onWarn` 参数是对 change-files delta Module Contract 签名的唯一扩展（spec 表仅列 `projectRoot: string`），归档（archive-change）时随 Module Contract 一并注明；本 design 阶段不改规格文件。
- `ignore` 依赖的精确版本（`^7.0.0` 范围内的最新 minor）以安装时解析结果为准，`pnpm-lock.yaml` 随之更新。
- gitignore 过滤器未来是否复用于其它通道（`workflow_files` 查询输出、`test_resolve_paths` 模块清单等）——沿用 proposal 决策：均不做，保持「源头过滤、消费方忠实」；本设计已通过「仅 `record.ts` 内部消费、不进 barrel」将该边界结构化。
