# 设计: workflow-file-inventory

> **变更**: workflow-file-inventory
> **日期**: 2026-09-17
> **提案与规格同步状态**: `proposal.md` 与 `specs/**`（10 个能力规格）已由提案阶段写入并同步，本设计不将其列为变更条目；`design.md` / `tasks.md` 的变更清单与任务只覆盖 proposal「变更范围 - 实现文件」及其设计连带文件。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 清单 schema | `workflow.json` 的 `files` 字段校验（written/deleted 两个路径列表 + 可选旁挂来源映射 `source`），缺字段不视为非法（由消费方硬报错） | `plugins/dev-team/bin/src/schemas/workflow.schema.ts` | zod v4 | Zod looseObject 扩展 |
| 清单存取与折叠库 | 读清单（缺失 `files` 硬报错 + 重建指引）、三态折叠（write/delete/revert 对称规则 + `source` 维护）、写回（沿用 `eval-json` 写盘纪律：2 空格缩进 + 末尾换行、保留其他键、绝不创建文件/目录） | `plugins/dev-team/bin/src/lib/file-inventory.ts`（新增） | `workflow.schema.ts`、`getChangeDir` | Node fs + 纯函数折叠 |
| session 注册表 | `session_id → change` 绑定的持久化与查询；bind 时按 TTL 清理过期条目 | `plugins/dev-team/bin/src/lib/session-registry.ts`（新增） | `node:os`、`node:crypto`、`node:fs` | JSON 文件落盘（os.tmpdir，按 projectRoot 哈希隔离） |
| hooks 三态提取器 | 将既有 bash/PowerShell 写目标提取泛化为 `extractFileOps(command) → FileOp[]`（write/delete/revert），PreToolUse 写/删保护与 PostToolUse 记录器共用（模块内部符号，不出 `hooks.ts`） | `plugins/dev-team/bin/src/hooks.ts` | `FileOp` 类型（file-inventory） | 正则单遍提取，python/node 豁免与 fail-open 不变 |
| PostToolUse 记录器 | hooks CLI 子命令 `record-files`：读 stdin 事件 → `phase_next` 事件建立 session 绑定 → 其余工具事件查表归账 → 路径归一化/自污染过滤 → `foldFileOps` → `writeFileInventory`；任何错误只走 stderr 诊断、exit 0 | `plugins/dev-team/bin/src/hooks.ts` | file-inventory、session-registry、`getProjectDir` | Node CLI 子命令（`main()` switch 分发） |
| PreToolUse 扩保护 | 提取路径 ∩ 保护 glob → deny（write 与 delete 双分类）；新增批量还原拦截（`git stash` / `git clean` / 对 `openspec/changes/**` 的 `git restore`） | `plugins/dev-team/bin/src/hooks.ts` | `extractFileOps`、`loadPatterns`/`matchGlob`（既有） | 既有 fail-open 保护框架扩展 |
| hooks 注册链 | canonical 注册 PostToolUse 记录器条目；build 组装 schema 扩展 `postToolUse` 数组并接入 claude / cursor 两种产物包装 | `plugins/dev-team/hooks/hooks.canonical.json` + `plugins/dev-team/build/hooks-profile.ts` | `apply-env-tokens`（`__MCP:phase_next__`、`__BIN:hooks__` token） | Zod schema + JSON 组装 |
| `change_files` 工具 | 文件清单手动补录（append）与净状态显式修正（set）的 MCP 工具，作非 Claude 环境与 hook 漏记兜底 | `plugins/dev-team/bin/src/schemas/change-files.schema.ts`（新增）+ `plugins/dev-team/bin/src/commands/change-files.ts`（新增）+ `plugins/dev-team/bin/src/mcp.ts` | file-inventory、`workflowFileSchema` | MCP tool（Zod schema） |
| 突变 scope 清单化 | `--change=<name>` 兼作突变 scope 唯一入口：读 `files.written` + 同位源文件反推 + HEAD 内容净归零去噪；`--mutation-diff-only` 删除；惰性解析 | `plugins/dev-team/bin/src/commands/test-execution.ts` + `plugins/dev-team/bin/src/cli.ts` | file-inventory、`deriveSourcePathFromTestFile`（既有） | `git show HEAD:<path>` 逐文件内容对比（只读观察） |
| test 路径清单模式 | `modules: "git-change"` 字面量替换为 `"change"` + 独立 `change` 参数；改读 `files.written`，失败进 `errors` | `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` + `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | file-inventory、`runTestDetectFrameworks`（既有） | 既有 resolver 管线复用 |
| `archi_check` 清单消费 | 被查文件集支持 change 清单模式（`files.written` 直通）；`staged` 降级为显式报错别名，`git diff --cached` 退场 | `plugins/dev-team/bin/src/lib/c4-cross-ref.ts` + `plugins/dev-team/bin/src/schemas/archi-check.schema.ts` + `plugins/dev-team/bin/src/mcp.ts` | file-inventory | 既有 cross-ref 管线复用 |
| backtrack 清空规则 | `backtrack_to` 索引 ≤ `implement`（同 phase 表）时重置 `files` 重记；预检 files 缺失硬报错先行，保证评估条目不部分生效 | `plugins/dev-team/bin/src/commands/backtrack.ts` | file-inventory、`getPhaseTable`（既有） | 既有写回通道 |
| change_create 初始化 | 新建 `workflow.json` 写入 `files: { written: [], deleted: [] }` | `plugins/dev-team/bin/src/commands/change-create.ts` | `workflowFileSchema` | fs 直写（既有模式） |
| git 退场 | `getGitDiffFiles` 与 `git add -N`/`git reset` hack 整体删除；`simple-git` 依赖移除 | `plugins/dev-team/bin/src/lib/git.ts`（删除）+ `plugins/dev-team/package.json` | — | 文件删除 |
| 提示词层 | evaluator 范围核对改三态对账、generator 规范还原动作、观察型 evaluator git diff 降级、skill 文案对齐 | `plugins/dev-team/agents/*.md`（6 个 + `architecture.md` 连带）、`plugins/dev-team/skills/phase-implement/SKILL.md`、`phase-test-gen/SKILL.md` | — | 提示词/文案 |
| 模板层 | design 模板补「删除文件」子节；proposal 模板「变更范围」补删除声明分组 | `plugins/dev-team/templates/artifacts/design.md.template`、`proposal.md.template` | — | Markdown 模板 |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/lib/file-inventory.ts` | 清单存取与折叠共享库：`FileInventory` / `FileOp` 类型、`readFileInventory`（缺 `files` 硬报错 + 重建指引）、`foldFileOps`（三条对称折叠规则 + `source` 旁挂来源维护）、`writeFileInventory`（写盘纪律同 `eval-json`）。被记录器、`change_files`、backtrack、test-execution、test-resolve-paths、c4-cross-ref 六处消费 |
| `plugins/dev-team/bin/src/lib/session-registry.ts` | `session_id → change` 持久化注册表：`bindSession` / `lookupChange`；存储于 `os.tmpdir()/dev-team-hooks/<sha256(projectRoot) 前 16 位>.json`，结构 `{ [session_id]: { change, updatedAt } }`；bind 时清理 `updatedAt` 超 24h 的条目（TTL 自清理，无需归档钩子） |
| `plugins/dev-team/bin/src/schemas/change-files.schema.ts` | `change_files` 输入/输出 Zod schema：`changeFilesInputSchema`（`change` 必填、`op: "append"|"set"`、`written`/`deleted` 至少其一、路径为相对项目根 POSIX 风格）、`changeFilesOutputSchema`（操作后净状态） |
| `plugins/dev-team/bin/src/commands/change-files.ts` | `runChangeFiles` 命令实现：校验 change 存在且 `workflow.json` 合法且含 `files`（缺失硬报错指引重建）；append 按折叠规则逐路径合并（去重、保留既有来源、新路径无来源）；set 整体覆写指定桶并清理被覆写路径的 `source` 条目；输出操作后净状态 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/lib/git.ts` | **整个文件删除**（REMOVED） | `getGitDiffFiles` 连同 `git add -N`/`git reset` index 改写 hack 移除；连带删除其测试文件 `plugins/dev-team/bin/src/lib/git.test.ts`（随模块移除，非新增测试） |
| `plugins/dev-team/bin/src/hooks.ts` | ① 合并 `extractBashWriteTargets` / `extractPowerShellWriteTargets` 为 `extractFileOps`（write/delete/revert 三态，模块内部符号）；② PreToolUse 改按提取路径匹配保护 glob（write + delete 双分类拦截）；③ 新增批量还原拦截；④ 新增 `runRecordFiles` 子命令（CLI 名 `record-files`） | 记录器流程：stdin 解析 → `tool_name` 以 `mcp__` 开头且以 `phase_next` 结尾则 `bindSession(session_id, tool_input.change)` 后输出；否则查 `lookupChange`，未命中静默丢弃（stderr 诊断通道）。路径来源：`Write`/`Edit` 取 `tool_input.file_path`，`NotebookEdit` 取 `tool_input.notebook_path`，`Bash`/`PowerShell` 走 `extractFileOps`。过滤：`openspec/**` 与 `workflow.json` 自身排除；绝对路径相对化到 `getProjectDir()`，越出项目根丢弃。归账：`foldFileOps` 携带事件 `agent_type`（存在时）→ `writeFileInventory`；全程任何异常只写 stderr、exit 0，不阻塞工具调用 |
| `plugins/dev-team/hooks/hooks.canonical.json` | 新增 `postToolUse` 条目；`description` 补充记录器说明 | matcher：claude 为 `Write\|Edit\|NotebookEdit\|Bash\|PowerShell\|__MCP:phase_next__`（token 展开为 MCP 全名 `mcp__plugin_dev-team_dev-team__phase_next`），cursor 为 `null`（Cursor PostToolUse 支持不全，记录器 Claude-only，与 `subagentStop` 条目同型）；commandTemplate：`node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" record-files` |
| `plugins/dev-team/build/hooks-profile.ts` | canonical schema 增 `postToolUse` 数组（条目形状同 preToolUse：`matchers` + `commandTemplate`）；`buildClaudeNested` 输出 `hooks.PostToolUse`；`buildCursorNative` 按 cursor matcher 输出（`null` 则整段省略） | 既有 `preToolUse` / `subagentStop` 包装行为不变 |
| `plugins/dev-team/bin/src/schemas/workflow.schema.ts` | `workflowFileSchema` 扩展可选 `files` 字段 | `files = { written: string[], deleted: string[], source?: Record<string, string> }`；`workflow_type` / `created` / `eval` / 未知键保留策略不变 |
| `plugins/dev-team/bin/src/schemas/index.ts` | 导出 `changeFilesInputSchema` / `changeFilesOutputSchema` 及推断类型 | 与既有单行 re-export 模式一致 |
| `plugins/dev-team/bin/src/schemas/archi-check.schema.ts` | `archiCheckInputSchema` 增可选 `change`（string，目标 change 名）；`staged` 描述改为"已废弃，传值将显式报错" | 既有 `files`（逗号分隔字符串）不变 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` | `modules` union 中 `"git-change"` 字面量替换为 `"change"`；新增可选 `change: string` 参数；description 同步 | 清单模式入参 = `modules: "change"` + `change: "<name>"`（后者缺省时由 schema refine 拒绝） |
| `plugins/dev-team/bin/src/commands/change-create.ts` | 写入对象增 `files: { written: [], deleted: [] }` | 键序 `workflow_type` → `created` → `files`；不写 `eval`、不建 `eval.json` / `.openspec.yaml` 等既有契约不变 |
| `plugins/dev-team/bin/src/commands/backtrack.ts` | 新增清空规则：`backtrack_to` 在同一 phase 表中的索引 ≤ `implement` 的索引时，重置 `files` 为空净状态 | 执行序：`validatePhaseTarget` → 预检 `files` 存在（缺失即抛"机制前旧 change 请重建"，评估条目尚未改写，保证不部分生效）→ 既有 eval 读改写回 → `writeFileInventory` 清空。`implement` 不在 phase 表中的工作流（test-only）不触发清空 |
| `plugins/dev-team/bin/src/commands/test-execution.ts` | `TestExecutionOptions` 删 `mutationDiffOnly`；`resolveMutationDiffFiles` 重写为清单解析（触发条件 `--change`、`noMutation` 时惰性跳过）；新增净归零去噪过滤 | scope = `readFileInventory(changeDir).written` → `expandMutationDiffWithInferredSources`（保留，测试文件反推同位源文件）→ 去噪（逐文件工作区内容 vs `git show HEAD:<path>` 对比，相同则剔除；HEAD 无该文件或读取失败则保留，漏斗只向 overstate 方向）。`files` 缺失 → 硬报错"该 change 创建于文件清单机制之前，请重建"。未传 `--change` → 突变不限圈（现状默认语义确认保留）。stdout 诊断行由 `--mutation-diff-only:` 前缀改为清单来源前缀（如 `mutation scope (change inventory): N files`）。MUST NOT 使用 `git diff HEAD` 发现文件 |
| `plugins/dev-team/bin/src/cli.ts` | 删 `--mutation-diff-only` 选项与 `mutationDiffOnly` 透传；`--change` 描述更新为"兼作突变 scope 入口（清单）" | `--skip-mutation` 不变（继续映射 `noMutation`）；不引入 `--mutation-scope` |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | `ResolveTestPathsParams` / `TestResolvePathsInput` 的 `modules: "git-change"` 分支重写为清单模式；`resolveEffectiveModules` 删 `execSync('git diff HEAD --name-only')` | 清单模式：`readFileInventory(getChangeDir(change, projectRoot)).written` 作为 effectiveModules，后续非空 modules 管线（test config 过滤 + exclude 过滤）不变；清单读取失败（change 不存在 / workflow.json 缺失非法 / 无 `files`）→ `errors` 增含重建指引的条目并 earlyReturn（不抛出，与该工具错误收集语义一致）；`written` 为空 → 空 `unit_tests`、无致命错误 |
| `plugins/dev-team/bin/src/lib/c4-cross-ref.ts` | `runCrossRefCheck` options 增 `change?: string`；`getChangedFiles` 数据源改造 | 被查文件集优先级：显式 `files` > `files.written`（清单直通，不做 test config 过滤）> 空。`staged === true` → 直接抛错（"staged 模式已由清单模式替代，请传 change 或 files"），MUST NOT 隐式执行 `git diff --cached`；清单模式下 `workflow.json` 无 `files` → 硬报错指引重建。`openspec/architecture/**` 模型文件只作参照系，不进被查文件集 |
| `plugins/dev-team/bin/src/mcp.ts` | 注册 `change_files` 工具（description 含补录漏记 + 修正净状态双用途，不含绕过写保护指引）；`archi_check` handler 增 `change` 传参与 `staged` 显式报错；`test_resolve_paths` description 的模式 3 文案改清单 | `change_files` 走 `withResolvedProjectRoot` + `jsonContent` 既有模式 |
| `plugins/dev-team/agents/implementation-evaluator.md` | frontmatter description、Tools/Workflow 段落与 I1 / I2 / I6 重写 | 范围核对权威改为 design 变更清单 × `workflow.json.files` × 文件系统三态对账（判定表对齐 workflow-file-inventory spec「消费方对账三态」：计划有 actual 无文件不存在 → 硬 fail；计划有 actual 无文件存在 → 良性漏记照常核对；actual 有计划无 → agent 判断、额外删除从严；声明删文件仍在 → 硬 fail）；`git diff` 仅作观察辅助；不要求读取 `files` 缺失的旧 change（按硬报错指引重建） |
| `plugins/dev-team/agents/test-gen-evaluator.md` | frontmatter description 与 Workflow 步骤重写 | 移除"via git diff"与 `git diff --stat/--name-only` 圈定被检文件的步骤；改为读 test-design 声明 + `files.written` 清单三态对账；内容核对独立于 actual |
| `plugins/dev-team/agents/test-gen-generator.md` | "The git diff of these uncommitted changes IS the artifact" 语义改写 | 产出契约 = 写盘的测试文件本身，范围以 test-design / design 声明与 `files.written` 清单核对；保留"代码即产物、无 JSON 报告"的其余语义 |
| `plugins/dev-team/agents/acceptance-evaluator.md` | `git diff`（全量变更集）降级为纯观察辅助 | AC 核对范围来自 proposal/design 声明 + `files` 清单 + 文件系统；"禁止设置 backtrack_to"约束不变 |
| `plugins/dev-team/agents/code-review-evaluator.md` | 同上（`git diff --stat` / 全量 diff 降级） | 范围判定读清单 + design + 文件系统；diff 仅用于查看修改/被删内容以评估质量 |
| `plugins/dev-team/agents/implementation-generator.md` | 新增规范还原动作说明 | 撤销某文件修改用 `git restore <path>`（PostToolUse 记录器识别为 revert 并折叠为净 untouched）；MUST NOT 指示重写文件内容为原样来"还原"（该形态折叠为 written，只能靠内容核对去噪） |
| `plugins/dev-team/agents/architecture.md` | validate 模式的 `archi_check` 调用形态更新：`archi_check({staged: true})` → `archi_check({change: "<change-name>"})`；独立（无 change 上下文）场景用 `files` 显式列表 | 设计连带文件（proposal 实现文件清单未列）：`archi-check.schema.ts` 移除 staged 的 git 语义后，若不同步本提示词，validate 模式会静默 no-op。工具能力清单行同步（"optional staged flag" → "change inventory mode"） |
| `plugins/dev-team/skills/phase-implement/SKILL.md` | frontmatter description（"evaluator inspects git diff"）与范围核对文案对齐清单机制 | 范围语义 = design 变更清单 × `files` × 文件系统对账；清单与实际不符时指引 MCP `change_files`（append/set），MUST NOT 指引直接编辑 `workflow.json`；不引入其他新行为 |
| `plugins/dev-team/skills/phase-test-gen/SKILL.md` | frontmatter description 与产出范围文案对齐 | 不含"git 工作区即产出"表述 |
| `plugins/dev-team/templates/artifacts/design.md.template` | 变更清单增「删除文件」子节 | 与「新增文件」「修改文件」并列；目录级删除以目录路径声明；注明无删除时整段省略、MUST NOT 强制为空表 |
| `plugins/dev-team/templates/artifacts/proposal.md.template` | 「变更范围」增「删除文件」分组 | 与实现文件 / 测试文件 / 不要修改并列 |
| `plugins/dev-team/package.json` | ① `dependencies` 移除 `simple-git`（仅 `lib/git.ts` 使用，已确认无其他引用）；② `version` bump | 版本升级遵循 CLAUDE.md 项目规则（改源头后 rebuild 产物） |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `readFileInventory` | `plugins/dev-team/bin/src/lib/file-inventory.ts` | 新增 | `readFileInventory(changeDir: string): FileInventory` | 读 `workflow.json` 并校验 `files` 字段；文件缺失 / JSON 非法 / 根非对象 / schema 不合法 / 无 `files` 均抛错（硬报错文案含"该 change 创建于文件清单机制之前，请重建"） |
| `foldFileOps` | `plugins/dev-team/bin/src/lib/file-inventory.ts` | 新增 | `foldFileOps(inventory: FileInventory, ops: FileOp[]): FileInventory` | 纯函数：按 `write(P) → deleted -= P ; written += P`、`delete(P) → written -= P ; deleted += P`、`revert(P) → written -= P ; deleted -= P` 折叠；路径单桶不变量；`source` 同步维护（带 `agentType` 覆写 / 不带清除 / 条目移除随之移除） |
| `writeFileInventory` | `plugins/dev-team/bin/src/lib/file-inventory.ts` | 新增 | `writeFileInventory(changeDir: string, files: FileInventory): void` | 写回 `workflow.json.files`；保留 `workflow_type` / `created` / `eval` / 未知键；2 空格缩进 + 末尾换行；文件不存在时抛错，绝不创建 |
| `bindSession` | `plugins/dev-team/bin/src/lib/session-registry.ts` | 新增 | `bindSession(projectRoot: string, sessionId: string, change: string): void` | 建立/更新 `session_id → change` 绑定并刷新 `updatedAt`；顺带清理 TTL（24h）过期条目 |
| `lookupChange` | `plugins/dev-team/bin/src/lib/session-registry.ts` | 新增 | `lookupChange(projectRoot: string, sessionId: string): string \| null` | 查绑定；未命中返回 `null`（调用方静默丢弃事件） |
| `runRecordFiles` | `plugins/dev-team/bin/src/hooks.ts` | 新增 | `runRecordFiles(): void` | hooks CLI 子命令 `record-files` 入口（PostToolUse 记录器），经 `main()` switch 分发；与 `runProtectFiles` / `runStaticCheck` 同型 |
| `runChangeFiles` | `plugins/dev-team/bin/src/commands/change-files.ts` | 新增 | `runChangeFiles(options: ChangeFilesInput): ChangeFilesOutput` | `ChangeFilesInput = { change: string; op: 'append' \| 'set'; written?: string[]; deleted?: string[]; project_root?: string }`；输出操作后净状态 `{ written: string[]; deleted: string[] }` |
| `runChangeCreate` | `plugins/dev-team/bin/src/commands/change-create.ts` | 修改 | 签名不变：`runChangeCreate(name: ChangeCreateInput['name'], projectRoot: ChangeCreateInput['project_root'], workflowType: ChangeCreateInput['workflow_type']): { name: string; path: string }` | 写入对象增 `files` 初始空净状态 |
| `runBacktrack` | `plugins/dev-team/bin/src/commands/backtrack.ts` | 修改 | 签名不变：`runBacktrack(options: BacktrackOptions): BacktrackResult` | 新增清空规则（见修改文件表）；输出结构不变 |
| `runTestExecution` | `plugins/dev-team/bin/src/commands/test-execution.ts` | 修改 | `runTestExecution(options: TestExecutionOptions): Promise<number>` | options 删 `mutationDiffOnly`；`change` 兼作清单突变 scope 入口 |
| `runTestResolvePaths` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 修改 | `runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult` | 入参字面量与 change 参数变更（见类型定义） |
| `runCrossRefCheck` | `plugins/dev-team/bin/src/lib/c4-cross-ref.ts` | 修改 | `runCrossRefCheck(projectRoot: string, options?: { staged?: boolean; files?: string[]; change?: string }): Promise<ArchiCheckResult>` | 增 `change` 清单模式；`staged: true` 改为显式抛错别名 |
| MCP 工具 `change_files` | `plugins/dev-team/bin/src/mcp.ts` | 新增 | 输入 `{ change: string, op: 'append' \| 'set', written?: string[], deleted?: string[], project_root: string }` → 输出 `{ written: string[], deleted: string[] }` | 注册于 `MCP_TOOLS`，走 `withResolvedProjectRoot` |
| CLI 子命令 `dev-team test-execution` | `plugins/dev-team/bin/src/cli.ts` | 修改 | 选项集：`--change`（职责扩大）、`--project-root`、`--files`、`--framework`、`--skip-mutation`（不变）；`--mutation-diff-only` 删除；不引入 `--mutation-scope` | — |

> 注：`extractFileOps`（`hooks.ts`）为模块内部共享符号（不出模块导出），按变更清单约定不列入上表，其覆盖面见下文「extractFileOps 提取覆盖面」。

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `WorkflowFile` | `plugins/dev-team/bin/src/schemas/workflow.schema.ts` | 修改 | `z.infer<typeof workflowFileSchema>`；增可选 `files` 字段 |
| `FileInventory` | `plugins/dev-team/bin/src/lib/file-inventory.ts` | 新增 | `{ written: string[]; deleted: string[]; source?: Record<string, string> }`；`source` 为旁挂来源映射（路径 → subagent `agent_type`），仅审计用途，消费方契约只依赖两个路径数组 |
| `FileOp` | `plugins/dev-team/bin/src/lib/file-inventory.ts` | 新增 | `{ op: 'write' \| 'delete' \| 'revert'; path: string; agentType?: string }`；`extractFileOps` 输出与 `foldFileOps` 输入 |
| `ChangeFilesInput` / `ChangeFilesOutput` | `plugins/dev-team/bin/src/schemas/change-files.schema.ts` | 新增 | `z.infer` 导出，经 `schemas/index.ts` re-export |
| `TestExecutionOptions` | `plugins/dev-team/bin/src/commands/test-execution.ts` | 修改 | 删 `mutationDiffOnly?: boolean`；`change?: string` 职责注释更新 |
| `TestResolvePathsInput` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 修改 | `modules: string[] \| 'change'`（原 `'git-change'` 移除）；增 `change?: string` |
| `ResolveTestPathsParams` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 修改 | 同步内部参数类型 |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `postToolUse` | `plugins/dev-team/hooks/hooks.canonical.json` | 新增 | 条目数组（1 条） | matchers.claude = `Write\|Edit\|NotebookEdit\|Bash\|PowerShell\|__MCP:phase_next__`；matchers.cursor = `null`；commandTemplate = `node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" record-files`。`__MCP:phase_next__` 经 `apply-env-tokens` 展开为各产物 MCP 全名 |
| `dependencies.simple-git` | `plugins/dev-team/package.json` | 修改（移除） | — | 随 `lib/git.ts` 删除 |
| `version` | `plugins/dev-team/package.json` | 修改 | — | 按项目规则 bump 并 rebuild 产物 |

> 不涉及 `openspec/config.json` 的 `write_protection` 配置结构变更（既有 glob 集合与 fail-open 语义不动，仅扩展拦截的分类与命令面）。

---

## 数据模型

### 1. `workflow.json.files`（change 文件清单净状态）

| 字段 | 类型 | 说明 |
|------|------|------|
| `written` | `string[]` | 净写入路径（新建 + 修改不分桶）；相对项目根 POSIX 风格；目录删除记录目录路径本身，消费方按前缀匹配 |
| `deleted` | `string[]` | 净删除路径；语义同上 |
| `source`（可选） | `Record<string, string>` | 旁挂来源映射：路径 → 事件 `agent_type`（如 `dev-team:implementation-generator`）。**单桶不变量**（write 移出 deleted、delete 移出 written、revert 双移出）保证路径键无歧义，无需按桶分映射。主会话事件与 `change_files` 补录不写该字段；同路径重写 last-writer-wins；条目被折叠移除时来源随之移除。仅审计用途，突变 scope / `test_resolve_paths` / `archi_check` / evaluator 对账 MUST NOT 读取 |

关系：change 与 `files` 1-1（同 `workflow_type` / `created` / `eval` 并列）；由 `change_create` 初始化为两个空数组；`backtrack`（≤ implement）整体重置。持久化：`openspec/changes/<name>/workflow.json`。

### 2. session 注册表文件

| 属性 | 值 |
|------|-----|
| 路径 | `path.join(os.tmpdir(), 'dev-team-hooks', sha256(normalizedProjectRoot).slice(0, 16) + '.json')` —— OS 临时目录避免向用户仓库引入 git-status 噪声；按 projectRoot 哈希隔离多项目 |
| 结构 | `{ [session_id]: { change: string, updatedAt: string /* ISO */ } }` |
| 写入方 | 仅 `bindSession`（`record-files` 观察 `phase_next` 事件时触发；每次 agent turn 的 `phase_next` 调用自动刷新绑定，故注册表丢失可自愈，只损一个 turn 内的漏记窗口） |
| 清理 | bind 时惰性清理 `updatedAt` 超 24h 的条目；不依赖 change 归档或会话结束钩子 |
| 并发 | 简单同步读写；多 session 并发写竞态本期标注不解决（工作流为单 session 串行，提案风险表既定） |

### 3. 记录器归账不变量

- 路径表示：清单内一律相对项目根 POSIX 风格；hook 事件的绝对路径相对 `getProjectDir()` 归一化，越出项目根的事件丢弃。
- 自污染排除：`openspec/**` 整体（proposal/design/reports/architecture 等工作流产物）与 `workflow.json` 自身不入清单——`archi_write` / `archi_decide` 产出全部落在排除范围内，因此不需要任何特判记录。
- 折叠方向保证：只多记（overstate）、不漏记危险操作；偏差由外部裁判兜底（delete 净状态裁判＝文件系统存在性；write 净状态裁判＝内容核对 + 突变去噪）。

---

## 路由 / API 设计

不涉及 HTTP API。本变更的接口面为 MCP 工具、hooks CLI 子命令与 CLI 命令选项：

### MCP 工具

| 工具 | 变更 | 输入 | 输出 | 认证 |
|------|------|------|------|------|
| `change_files` | 新增 | `{ change: string, op: 'append' \| 'set', written?: string[], deleted?: string[], project_root: string }`（`written`/`deleted` 至少其一） | `{ written: string[], deleted: string[] }`（操作后净状态） | 既有 MCP server 通道；写 `workflow.json` 与 `phase_log` / `backtrack` 同通道，不经 PreToolUse 文件写路径 |
| `archi_check` | 修改 | `{ project_root, staged?: boolean（废弃，传 true 显式报错）, files?: string, change?: string }` | 不变（`violations` / `warnings` / `matched` / `unmatched_files` / `status`） | 既有 |
| `test_resolve_paths` | 修改 | `{ project_root, modules: string[] \| 'change', change?: string }`（`modules === 'change'` 时 `change` 必填，schema refine 拒绝缺省） | 不变（`unit_tests` / `errors`） | 既有 |

`phase_next` 输入协议（必填 `run_id`、`(change, run_id)` anchor）不变；session 注册表仅在同一 MCP 调用的 PostToolUse 事件中借 `session_id` + `tool_input.change` 建绑，协议零改动。

### hooks CLI 子命令（`node <bin>/hooks.cjs <subcommand>`）

| 子命令 | 事件 | matcher（claude 产物） | 行为 |
|--------|------|------------------------|------|
| `protect-files`（既有，扩展） | PreToolUse | `Write\|Edit`、`Bash`、`PowerShell` | 提取路径精确匹配保护 glob（write + delete 双分类 deny）；批量还原拦截（`git stash` 含 pop/apply/branch、`git clean` 无路径或路径命中 `openspec/**`、`git restore` / `git checkout --` 目标命中 `openspec/changes/**` → deny；`git restore` 源码放行、`git stash list/show` 与 `git clean -n/--dry-run` 放行） |
| `record-files`（新增） | PostToolUse | `Write\|Edit\|NotebookEdit\|Bash\|PowerShell\|<MCP 全名 phase_next>` | `phase_next` 事件 → 建绑定；其余事件 → 查表归账 → 折叠 → 写清单；未绑定/异常静默丢弃（stderr 诊断、exit 0） |
| `static-check`（既有） | SubagentStop | 不变 | 不涉及本变更 |

### extractFileOps 提取覆盖面（三态分类）

| 分类 | bash / git | PowerShell | 备注 |
|------|-----------|------------|------|
| write | 既有 `>` / `>>` / `>\|` / `tee` / `>&`；`git restore --source=<commit>`、`git checkout <commit> -- <paths>` | 既有重定向、`Set-Content` / `Out-File` / `Add-Content` / `Export-Csv` / `Export-CliXml` / `Tee-Object`、`[System.IO.File]::WriteAll*` | 既有正则原样并入 |
| delete | `rm`、`git rm`、`rmdir`、`unlink`（逐路径参数提取，跳过 `-` 开头标志） | `Remove-Item`、`del`、`rd` | 命中保护 glob 时 PreToolUse 同步 deny |
| delete(旧) + write(新) 双条目 | `mv <old> <new>` | `Move-Item`、`mv`、`ren` / `Rename-Item` | 仅识别双参数形态；多源 `mv` 长尾不识别（漏记方向，由对账兜底，见待决问题） |
| revert | `git restore <paths>`（无 `--source`）、`git checkout -- <paths>` | — | revert 由 PreToolUse 放行（openspec 工作流产物除外），由 PostToolUse 折叠为净 untouched |

python/node 命令豁免（`^(python|python3|node)\s`）与 fail-open 语义保持不变。

---

## 依赖

### 运行时依赖

- `zod`（v4，既有）— 全部新增/扩展 schema
- `@modelcontextprotocol/sdk`（既有）— `change_files` 工具注册
- `node:os` / `node:crypto` / `node:fs` / `node:path`（内置）— session 注册表落盘与哈希、清单读写
- ~~`simple-git`~~ — **移除**（唯一使用者 `lib/git.ts` 删除）；git 操作仅剩去噪与还原识别所需的只读子进程调用（`git show HEAD:<path>` / `execSync`），无需客户端库

### 构建/测试依赖

- 既有 `pnpm -C plugins/dev-team run build` 组装链（`assemble.ts` + `hooks-profile.ts` + `apply-env-tokens.ts`）— 承载 `postToolUse` 注册；无新增构建依赖
- 既有 vitest / Stryker 测试设施 — 测试由独立工作流阶段处理；无新增测试依赖

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | 「change_create 初始化」组件 + 修改文件 `change-create.ts`（`files: { written: [], deleted: [] }`，键序与既有契约不变）；schema 扩展见 `workflow.schema.ts` 行 |
| AC-2 | 「PostToolUse 记录器」组件：`record-files` 子命令路径提取 / openspec 与 workflow.json 自污染排除 / 归账写清单；注册见 canonical `postToolUse` 条目 |
| AC-3 | 「session 注册表」组件 + `record-files` 的 `phase_next` 全名（`__MCP:phase_next__` token）建绑流程；session 隔离由注册表按 `session_id` 键控保证 |
| AC-4 | 「extractFileOps 提取覆盖面」表（write/delete/revert/mv 双条目）+ `foldFileOps` 三条折叠规则 + `rm -rf dir` 记录目录路径（数据模型 §3） |
| AC-5 | 「hooks CLI 子命令」表 `protect-files` 扩展行：stash/clean deny、`rm <受保护路径>` deny、`git restore` 对 `openspec/changes/**` deny 且源码放行 |
| AC-6 | `backtrack.ts` 修改行：清空规则 + 预检先行；`implement` 之后的 phase 不清空；test-only（无 `implement`）不触发 |
| AC-7 | 「突变 scope 清单化」组件：`--change` 触发、无 `--mutation-scope`、`--mutation-diff-only` 删除（cli.ts 行）、`--skip-mutation` 保留且惰性解析、净归零去噪、旧 change 硬报错、`lib/git.ts` 与 `git.test.ts` 删除、`simple-git` 移除 |
| AC-8 | 「test 路径清单模式」组件：`modules: "change"` + `change` 参数，`files.written` 经 test config 过滤推导，不执行 git |
| AC-9 | 「archi_check 清单消费」组件：`change` 入参、`files.written` 直通、显式 `files` 优先、`staged` 显式报错别名、`git diff --cached` 退场 |
| AC-10 | 「change_files 工具」组件：注册（mcp.ts）、append 折叠去重保留来源、set 覆写净状态、旧 change 硬报错；写保护不受影响（走 MCP 通道） |
| AC-11 | 提示词层修改行：implementation-evaluator / test-gen-evaluator 三态对账判定表；acceptance / code-review 的 git diff 纯观察辅助降级 |
| AC-12 | 模板层修改行（design 模板「删除文件」子节、proposal 模板删除分组）+ skills 两份 SKILL.md 文案 + `implementation-generator.md` 规范还原动作 |
| AC-13 | 数据模型 §1 `source` 旁挂映射（形态决议）：subagent 携带 / 主会话与补录省略 / last-writer-wins / 折叠随之移除 / 消费方不读取；记录器以事件 `stdin.agent_type` 传入 `FileOp.agentType` |

---

## 待决问题

- **subagent 事件的 `session_id` 共享性**：提案风险表既定——文档强指标、非明文契约。实现期先用临时 log hook 实测一轮完整工作流；若不共享，退化为"暂存-认领"模式（记录器按 `session_id` 暂存事件流，消费命令认领合并），session-registry 模块接口预留该演进。
- **zcode / cursor 平台的 PostToolUse hook 支持度**：canonical matcher 仅 `claude`/`cursor` 两键（zcode 产物复用 cursor 键）。本期记录器 `cursor: null`（Claude-only），zcode 暂无清单记录、依赖 `change_files` 兜底；若后续确认 zcode 运行时支持 PostToolUse，再扩 canonical matcher 键。
- **`extractFileOps` 长尾边界**：`rm a b c` 多路径参数、引号含空格路径、多源 `mv`、`Move-Item -Path a,b -Destination d` 等形态的识别粒度——实现期按"单遍正则 + 只多记不漏记"原则覆盖主流形态，剩余长尾由对账良性漏记 + `change_files` 补录兜底；具体正则在实现与测试阶段收敛。
- **突变去噪内容对比选型**：`git show HEAD:<path>` 内容直读 vs `git hash-object` / `git rev-parse HEAD:<path>` blob 哈希对比（后者免读全文且经行尾过滤器，但每文件两次 exec）。change 内文件量级下两者均可，实现期定；硬约束只有一条——MUST NOT 使用 `git diff HEAD` 发现文件。
- **`git stash pop/apply/branch` 拦截面细化**：设计已定为"凡改写工作区的形态 deny、只读形态（list/show）放行"，具体正则边界在实现期随测试收敛。
- **注册表并发写竞态**：多 session 同时 bind 同一注册表文件的写覆盖风险本期不解决（提案既定），`change_files` 的 `set` 语义可事后修复。
