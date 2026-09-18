# 提案: move-files-write-into-workflow-module

> **变更**: move-files-write-into-workflow-module
> **日期**: 2026-09-18
> **状态**: draft

---

## 问题

`workflow-files-query-api`（2026-09-17）确立 `plugins/dev-team/bin/src/modules/workflow/` 为 `workflow.json` 操作逻辑的模块边界，但该次只迁入了底层读写原语（`readFileInventory` / `foldFileOps` / `writeFileInventory`）与只读查询。**「写入 `workflow.json` 的 `files` 字段」的完整业务实现仍散在 `commands/` 层**：

- `commands/record-files.ts` 内联了归账管线的全部语义——`normalizeRecordedPath`（项目根相对化）、`isExcludedFromInventory`（自污染排除）、`collectRecordedOps`（agentType 盖章）与「读 → 折叠 → 写」落盘步骤；
- `commands/change-files.ts` 内联了 append/set 的净状态折叠语义（`applyAppend` / `applySet` / `dedupe`）。

即：模块出口上只有原语，**「什么算一次归账、如何折叠落盘」的写路径语义不在模块边界内**，与既定的收敛方向相悖。

同时，PostToolUse 记录器的排除规则只有 `openspec/**` 与 `workflow.json` 自身，**gitignore 命中的路径照单全收**：对本仓库根 `.gitignore`（`.*/`、`memory`、`node_modules`、`_stryker-tmp`、`plugins/*/.pack-staging/`、`*/dev-team/bin/*.cjs.map`）命中的路径（如 `.claude/agent-memory/**` 的记忆文件写入），每次都会进入 `files` 净状态。这些路径永不提交、不属于变更内容，却持续污染下游消费方的范围判定——突变测试 scope、`test_resolve_paths` 清单模式的模块列表、`archi_check` 被查文件集、evaluator 三态对账（永远构成「计划外改动」噪音）。

---

## 提案

1. **写路径实现移入 `modules/workflow`**：将 `commands/record-files.ts` 的归账管线（规范化 → 自污染排除 → 折叠 → 落盘）与 `commands/change-files.ts` 的 append/set 折叠语义整体迁入 `modules/workflow/files/`，经 `workflow/index.ts` 统一导出（拟定 `recordFileOps`、`appendFileOps`、`setFileBuckets`，确切签名由 design 定并回填 Module Contract）。命令层瘦身为纯适配：`record-files.ts` 只保留 stdin 解析、`phase_next` 绑定、`tool_input` 提取与错误吞并策略；`change-files.ts` 只保留输入校验与委托。纯移动、行为不变，不留 re-export shim（避免 knip 判 dead export）。

2. **记录器同步 gitignore 过滤**：新增 gitignore 过滤器（拟 `modules/workflow/files/gitignore.ts`），按 gitignore 语义匹配（注释/空行、`*` / `**`、尾随 `/` 目录模式、`!` 负模式、后匹配覆盖先匹配、被忽略目录子路径递归）。匹配范围为**层级 `.gitignore`**：项目根与目标路径祖先链上各目录的 `.gitignore` 均参与判定——深层文件规则优先于浅层文件；目录在任一层被排除后，其下路径一律忽略，且该目录内更深层 `.gitignore` 不再参与判定（与 git 对被排除目录不下降的语义一致）。记录器在归账时同步应用过滤——折叠落盘前丢弃被忽略路径。实现选型：`ignore` npm 包（^7，零传递依赖，ESLint/Prettier 同款）作为**单文件规则引擎**，层级应用（祖先链收集、深层优先、目录短路）在其上自实现——`ignore` 设计上不处理嵌套，亦无成熟嵌套封装库（`ignore-walk` 为目录遍历器形态，不适配单路径事件）；**不**用手写解析 + `picomatch` 转换（glob 与 gitignore 语义存在三处实质差异：无斜杠段任意层级生效、负模式有序求值、父目录被排除后子路径不可 re-include，手写转换易错）；**不**用 `git check-ignore` 子进程（PostToolUse 每事件起进程开销大，且本仓库已刻意去 git 化清单机制）。匹配不可判定时 fail-open（保留路径入清单），与折叠「只多记、不漏记」原则一致。`change_files` append/set 为人工补录/修正通道，MUST NOT 过滤；既有自污染排除规则不变、与其叠加。

3. **版本与产物**：按 CLAUDE.md 规则提升 `plugins/dev-team/package.json` `version` 并执行 `pnpm -C plugins/dev-team run build` 刷新 `claude-plugins/`、`cursor-plugins/`、`cursor-home-image/` 产物（Claude 与 Cursor 共用同一 hook 实现，过滤自动双平台生效）。

---

## 能力

### 新增能力

- 无（gitignore 过滤与写路径收敛分别并入既有 `workflow-file-inventory` 与 `change-files` 能力）

### 修改的能力

- `change-files` — 「workflow.json 文件清单逻辑统一由 modules/workflow 导出」要求扩展：归账管线与 append/set 折叠语义迁入模块，命令层仅保留协议适配；模块出口与 Module Contract 扩充
- `workflow-file-inventory` — 新增「记录器同步 gitignore 过滤」要求：根 `.gitignore` 语义匹配、fail-open 兜底、与自污染排除叠加、`change_files` 通道不过滤

---

## 变更范围

### 实现文件

- 修改 `plugins/dev-team/bin/src/modules/workflow/index.ts` — barrel 扩充导出（归账管线、append/set 语义、gitignore 过滤器）
- 新建 `plugins/dev-team/bin/src/modules/workflow/files/gitignore.ts` — 基于 `ignore` 包的层级 `.gitignore`（根 + 祖先链）解析与语义匹配封装（文件名/导出名 design 可调）
- 修改 `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts` — 并入归账管线与 append/set 折叠语义（或按 design 拆分新文件，如 `files/record.ts`）
- 修改 `plugins/dev-team/bin/src/commands/record-files.ts` — 删除内联的 `normalizeRecordedPath` / `isExcludedFromInventory` / `collectRecordedOps` 与读折叠写步骤，改委托模块
- 修改 `plugins/dev-team/bin/src/commands/change-files.ts` — 删除 `applyAppend` / `applySet` / `dedupe` 本地实现，改委托模块
- `plugins/dev-team/package.json` — 新增 `ignore` 运行时依赖（^7）；`version` 提升；`pnpm -C plugins/dev-team run build` 刷新产物

### 测试文件

- 新建 `plugins/dev-team/bin/src/modules/workflow/files/gitignore.test.ts` — 只验证自研层：各层 `.gitignore` 规则准确装载（以本仓库根 `.gitignore` 实样 `.*/` + `!.claude-plugin` 的判定结果作代表用例）、子目录规则覆盖根规则（深层优先）、被排除目录内 `.gitignore` 不生效（目录短路）、某层缺失视为无规则、读取失败 fail-open；gitignore 语法/匹配语义不逐项验证（`ignore` 包自行负责）
- 修改 `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.test.ts` — 归账管线与 append/set 用例随迁移扩充
- 修改 `plugins/dev-team/bin/src/hooks.test.ts` — record-files 场景：gitignore 命中路径不入清单、未忽略路径照常归账、无 `.gitignore` 行为不变
- 修改 `plugins/dev-team/bin/src/commands/change-files.test.ts` — 委托后 append/set 语义断言保持；新增「ignored 路径经 change_files 仍可补录」

### 删除文件

- 无（纯就地移动与修改，无整文件删除）

### 不要修改

- `plugins/dev-team/bin/src/lib/shell-file-ops.ts` — `extractFileOps` 为 PreToolUse/PostToolUse 共享提取器，非 workflow.json 逻辑
- `plugins/dev-team/bin/src/lib/eval-json.ts` 与 `commands/phase-log.ts` / `change-create.ts` / `backtrack.ts` — eval 与元数据写路径按既定节奏由后续 change 增量迁移
- `plugins/dev-team/bin/src/lib/workflow.ts` — PGE phase 配置，与 workflow.json 文件操作无关
- `plugins/dev-team/bin/src/schemas/workflow.schema.ts` — `files` 数据模型不变
- PreToolUse 写保护（protect-files）行为与 `workflow.json` 保护规则
- `workflow_files` 查询语义 — 忠实返回净状态，不做 gitignore 过滤
- `claude-plugins/` / `cursor-plugins/` / `cursor-home-image/` — 由 build 刷新，不手改

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | gitignore 过滤器实现 | `modules/workflow/files/` 下存在基于 `ignore` 包的封装；gitignore 语法与匹配语义由 `ignore` 包负责、不做逐项验证；单测验证自研层：各层 `.gitignore` 规则准确装载（以本仓库根 `.gitignore` 实样 `.*/` + `!.claude-plugin` 组合对 `.claude/x`（忽略）与 `.claude-plugin/marketplace.json`（保留）的判定作提取准确性代表用例）、层级应用（子目录规则覆盖根规则、被排除目录内 `.gitignore` 不生效）、某层缺失视为无规则、读取失败 fail-open（`gitignore.test.ts`） |
| AC-2 | 记录器接入同步过滤 | 已绑定 session 时：写 `.claude/**` 等 gitignore 命中路径不入 `files`；写 `src/a.ts` 照常入清单；祖先链全无 `.gitignore` 时记录行为与现状一致且 hook 不阻塞（`hooks.test.ts`） |
| AC-3 | 写路径收敛模块 | `workflow/index.ts` 导出归账管线与 append/set 语义函数；`commands/record-files.ts` / `commands/change-files.ts` 无内联折叠/落盘实现（grep `foldFileOps` / `writeFileInventory` / `applyAppend` / `applySet` 在两文件无实现残留）；无 re-export shim（`index.test.ts` + tsc） |
| AC-4 | 行为回归不变 | 除新增过滤场景外，`hooks.test.ts` 与 `change-files.test.ts` 既有断言全部保持（折叠规则、source 审计 last-writer-wins、`workflow_type`/`created`/`eval` 保留、硬报错语义）；全量 vitest + tsc 通过 |
| AC-5 | change_files 通道不过滤 | `change_files` append/set 对 gitignore 命中路径仍生效入桶（`change-files.test.ts`） |
| AC-6 | 版本与产物 | `plugins/dev-team/package.json` `version` 提升，build 成功且 `claude-plugins/` / `cursor-plugins/` / `cursor-home-image/` 刷新（build 输出 + git status） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `ignore` 库判定与预期偏差或异常输入抛错导致误过滤 | 真实变更路径漏记，消费方范围缺失 | 低 | fail-open 兜底（解析/判定抛错即保留路径入清单）；AC-1 实样代表用例锁定提取与组装行为 |
| 新增运行时依赖 `ignore`（打包体积、供应链面） | hook bundle 变大、依赖审计面扩大 | 低 | 零传递依赖、MIT、ESLint/Prettier 同款且活跃维护；vp pack 打包后体积增量几十 KB 级 |
| 层级求值实现不完整（深层优先、目录短路、被排除目录内 `.gitignore` 不生效）导致误过滤 | 真实变更路径漏记，消费方范围缺失 | 低 | fail-open 兜底（不可判定即保留）；层级专项用例（深层覆盖根规则、目录短路）；`ignore` 实例只承担单文件语义，层级序由自实现控制 |
| 迁移触碰 `hooks.test.ts` / `change-files.test.ts` 大量既有断言 | 回归面广 | 低 | 纯移动不改行为，测试断言随迁不改语义；全量测试 + tsc + 既有集成测试（inventory-backtrack-preserve、archi-check-inventory）回归 |
| 每次事件按祖先链尝试读取 `.gitignore` 的开销 | hook 延迟增加 | 低 | 祖先链深度有界（多数层为 ENOENT miss）；单 hook 调用内缓存已解析实例；hook 本就每事件短进程 |
| 迁移残留导致 knip 判 dead export | 质量门禁失败 | 低 | 命令层旧实现删除、不留 shim；barrel 与 `index.test.ts` 同步更新 |
| 已入清单的历史 ignored 路径不被清理 | 旧 change 的净状态仍有噪音 | 高（预期内） | 本次不做回溯清洗（增量边界）；可用 `change_files set` 显式修正 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 写路径管线归属 | 整体迁入 `modules/workflow/files/`，命令层只做协议适配 | 用户确立的模块边界方向（上一 change 待决问题的下一步切分）；「归账/折叠语义」是 workflow.json 业务逻辑而非 hook 协议 | 保留在 `commands/`（现状双出口） |
| gitignore 匹配实现 | 引入 `ignore` npm 包（^7，零传递依赖）作单文件规则引擎，层级应用（祖先链、深层优先、目录短路）自实现 | 完整 gitignore 语义由成熟实现保证（负模式、目录递归、后匹配覆盖、父目录排除后不可 re-include），消除手写 glob→gitignore 转换的三处语义差异风险；`ignore` 不支持嵌套且无成熟嵌套封装库，层级求值逻辑有界可测；过滤正确性直接决定真实路径是否漏记，优于省一个零依赖库 | 纯 JS 解析 + `picomatch` 转换（手写语义易错）；`git check-ignore` 子进程（每事件起进程，清单机制已刻意去 git 化） |
| 嵌套 `.gitignore` 是否纳入 | 纳入：根 + 目标路径祖先链全层级 | 用户明确要求；hook 运行于插件使用方的任意项目，仅根匹配对含子目录 `.gitignore` 的项目会误放行 | 仅根 `.gitignore`（本仓库现状只有根文件，但插件面向任意项目） |
| 过滤不可判定的语义 | fail-open（保留路径入清单） | 折叠原则「只多记、不漏记」；hook MUST NOT 阻塞工具调用 | fail-closed（丢弃） |
| `change_files` 是否同步过滤 | 不过滤 | 它是补录 hook 漏记与显式修正净状态的兜底通道，过滤会使兜底失效 | 同步过滤 |
| 已记录的历史 ignored 路径 | 不回溯清理 | 增量功能边界；净状态可经 `change_files set` 人工修正 | 迁移脚本一次性清洗存量 |

### 待决问题

- `recordFileOps` 等拟定导出名与签名由 design 终定，需回填 change-files 规格 Module Contract。
- gitignore 过滤器未来是否复用于其它通道（如 `workflow_files` 查询输出过滤、`test_resolve_paths` 模块清单过滤）——当前均不做，保持「源头过滤、消费方忠实」。
