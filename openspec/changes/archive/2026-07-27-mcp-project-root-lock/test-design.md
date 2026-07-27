# 测试设计: mcp-project-root-lock

> **日期**: 2026-07-27
> **回溯**: Mutation score 68.88% < 70%（约差 16 kills；detected 945 / undetected 427）。存活集中于 `test-resolve-paths`(84) / `test-detect-frameworks`(69+32 NoCoverage) / `hooks`(67) / `project-root`(43) / `test-execution`(44) / `mcp`(29)，突变类型以 StringLiteral / ConditionalExpression / LogicalOperator / Regex / NoCoverage 为主。本版**不降低 70% 阈值**；强化：① `project-root` 对 LITERAL/code-mapping Regex、完整 `message` 模板、`setLockFailure` 保留、fileUri 空/非 file 协议、timeout `clearTimeout` 的判别断言；② `mcp` 对全部 11 个 tool 的 **handler 成功执行**（杀死 ArrowFunction `() => undefined`）+ `registerTool` spy 精确 name/description；③ 为 resolve/detect/hooks/execution/config-get 增补可杀死 StringLiteral/Conditional/Logical/Regex/NoCoverage 的边界场景。`utils/index.ts` / `constant.ts` 已删除，相关 barrel 用例标记废弃。
> **T3 补强（本版）**: 为先前缺 异常 的新增突变对象补齐异常行——`registerTool` spy、server identity、全 11 handler、`shell·cmd` 字面量、`logSummary`、hooks `getProjectDir`/子命令调度、`collectFiles`、空 files auto-scan、`mutation_score`、既有透传；`archi_check` 的 `staged` bool 补齐 False / None（省略）边界（对照 Parameter Type Mapping）。

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `CLAUDE_PROJECT_DIR` 为已存在绝对路径时锁定该路径；字面量 `${...}` 被拒绝；roots 恰好 1 个可用时锁定；`WORKSPACE_FOLDER_PATHS` 按 `,`/`;` 解析后恰好 1 个时锁定 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | initProjectRootFromMcp — 启动锁定优先级 / LITERAL_ENV_PATTERN 正则判别 |
| AC-1 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-project-root-lock/mcp-project-root-lock.test.ts` | 唯一根锁定后工具可用 |
| AC-2 | 无可用唯一根时不回退 `cwd`；多路径列表不取 `[0]`；tool 返回可观测错误 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | initProjectRootFromMcp — 严格失败 / requireLockedProjectRoot — code 映射与完整 message |
| AC-2 | 同上 | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP 工具调用 — isError content 精确文本 |
| AC-2 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-project-root-lock/mcp-project-root-lock.test.ts` | 唯一根锁定后工具可用 |
| AC-3 | connect 后缓存一旦锁定，进程内不再因二次 init / 通知而改变 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | initProjectRootFromMcp — 进程内不可变 |
| AC-4 | 上述工具 Zod/MCP schema 不再接受 `project_root`；传入该字段被校验拒绝或忽略 | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — inputSchema 形状（无 project_root） |
| AC-4 | 同上 | 单元测试 | `plugins/dev-team/bin/src/commands/change-list.test.ts` | runChangeList — CLI vs MCP schema 契约 |
| AC-4 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-project-root-lock/mcp-project-root-lock.test.ts` | 无参 change_list 回显锁定根 |
| AC-5 | `phase_log` / `phase_next` / `backtrack` 及带参工具在未锁定时均失败，不写主目录 | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP 工具调用 — 未锁定入口校验 / withLockedProjectRoot 错误映射 / 全工具 handler 执行 |
| AC-5 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-project-root-lock/mcp-project-root-lock.test.ts` | 唯一根锁定后工具可用 |
| AC-6 | `change_list` 等仍可返回只读 `project_root` 字段等于锁定根 | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — outputSchema project_root 回显契约 / MCP 工具调用 — change_list 回显与锁定根接线 |
| AC-6 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-project-root-lock/mcp-project-root-lock.test.ts` | 无参 change_list 回显锁定根 |
| AC-7 | command 层显式 `project_root` / `projectRoot` fixture 仍可用；CLI 行为不强制 MCP 锁定 | 单元测试 | `plugins/dev-team/bin/src/commands/change-list.test.ts` | runChangeList -- project_root |
| AC-7 | 同上 | 单元测试 | `plugins/dev-team/bin/src/commands/config-get.test.ts` | runConfigGet — projectRoot 显式注入 |
| AC-7 | 同上 | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — project_root 注入与 getProjectDir 回退 |
| AC-7 | 同上 | 单元测试 | `plugins/dev-team/bin/src/commands/test-execution.test.ts` | runTestExecution — projectRoot / getProjectDir |
| AC-8 | 重装/指向本地 plugin 后，`change_list` 不传参返回工作区路径；断根时报错 | 不可测试 | — | Cursor 手工探针 |
| AC-9 | 宿主注入的 `CLAUDE_PROJECT_DIR` 生效 | 不可测试 | — | Claude Code 手工探针 |
| AC-10 | `getProjectDir` 定义于 `lib/project-root.ts`；`utils/constant.ts` 已删除；无残留对 `utils/constant` 的实现依赖 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | getProjectDir — 模块归并后公共 API |
| AC-10 | 同上 | 单元测试 | `plugins/dev-team/bin/src/utils/constant.test.ts` | （文件删除；既有用例迁入并标记废弃） |
| AC-11 | `plugins/dev-team/bin` 单测通过；`plugin.json` 版本已升级 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-project-root-lock/mcp-project-root-lock.test.ts` | 唯一根锁定后工具可用 |
| AC-11 | `plugin.json` 版本已升级 | 不可测试 | — | 人工检查版本号 |
| （突变） | `mcp.ts` registerTool name/description StringLiteral + handler ArrowFunction | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — registerTool spy 精确字面量（含异常） / server identity（含异常） / 全 11 工具 handler（含未锁定异常） / archi_check staged True·False·None |
| （突变） | `project-root` Regex/Conditional/StringLiteral/NoCoverage | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | LITERAL_ENV_PATTERN / code 映射正则 / setLockFailure 保留 / fileUri 边界 / timeout clearTimeout |
| （突变） | `test-resolve-paths` isTestFile Regex / extractErrorMessage / 扩展名 StringLiteral | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | isTestFile 正则判别 / extractErrorMessage stderr / SOURCE_EXTENSIONS 全扩展名 |
| （突变） | `test-detect-frameworks` expandConfigArgs / generateShell|Cmd NoCoverage / scope Conditional | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | {config_args} 精确展开 / shell·cmd 脚本字面量（含异常） / collectFiles（含异常） / 空 files auto-scan（含异常） / mutation_score（含异常） |
| （突变） | `hooks` Bash/PS Regex / 内置 glob·reason StringLiteral / isRecord Logical | 单元测试 | `plugins/dev-team/bin/src/hooks.test.ts` | detectBashWrite 正则判别 / 内置保护字面量 / isRecord 边界 / getProjectDir 接线（含异常） / 子命令调度（含异常） |
| （突变） | `test-execution` logResult status StringLiteral / mutationDiff startsWith | 单元测试 | `plugins/dev-team/bin/src/commands/test-execution.test.ts` | logResult 状态计数文案 / logSummary 文案（含异常） / mutationDiffOnly 路径过滤 / 既有透传（含异常） |

---

## 单元测试

### `plugins/dev-team/bin/src/lib/project-root.ts` -> `plugins/dev-team/bin/src/lib/project-root.test.ts`

#### 待测功能

- `initProjectRootFromMcp(server: McpServerLike): Promise<void>`: connect 后按 env → roots 恰 1 → WORKSPACE 恰 1 尽力锁定；失败不 exit；已锁定则 no-op；不注册 `list_changed`；`listRoots` 有 2000ms 超时与 `clearTimeout`
- `requireLockedProjectRoot(): string`: 返回已锁定绝对路径；未锁定抛 `ProjectRootLockError`，按 `lockFailureReason` 经 Regex 映射 `code`；`message` 为固定两段模板（含 `Project root is not locked:` 与指导文案）
- `getProjectDir(): string`: CLI/command 辅助：优先锁定缓存 → 可用 `CLAUDE_PROJECT_DIR` → 恰好 1 个可用 `WORKSPACE_FOLDER_PATHS` → `process.cwd()`
- `getMcpCachedProjectRoot(): string | null`: 只读缓存
- `isProjectRootLockError(err: unknown): err is ProjectRootLockError`: 跨模块副本鸭类型识别（`instanceof` 与 `name+code` 双路径）
- `ProjectRootLockError`: 含 `code: 'not_locked' | 'multi_root' | 'literal_env' | 'invalid_path'`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| initProjectRootFromMcp — 启动锁定优先级 | 正向 | `CLAUDE_PROJECT_DIR` 为已存在绝对路径时锁定该路径，且 `listRootsCalls === 0`；即使同时设置可用 WORKSPACE 与可返回的 roots，缓存仍严格等于 CLAUDE 路径 (AC-1) | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 正向 | 无可用 CLAUDE 时 `listRoots` 返回恰好 1 个可用 `file://` 根则锁定为该路径；同时设置可用 WORKSPACE 时缓存仍等于 roots 路径而非 WORKSPACE (AC-1) | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 正向 | env/roots 均不可用时 `WORKSPACE_FOLDER_PATHS` 为单个绝对存在路径（`;` 分隔，含尾部分隔符）则锁定且 `getMcpCachedProjectRoot()` 严格等于该路径 (AC-1) | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 正向 | `WORKSPACE_FOLDER_PATHS` 为单个绝对存在路径（`,` 分隔）则锁定且严格等于该路径 (AC-1) | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 正向 | roots 条目为裸绝对本地路径（非 `file://`）且可用时锁定为该路径 (AC-1) | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 正向 | roots 含多个条目但仅 1 个可转换为可用绝对路径（其余为空 URI / `http://` / 相对路径 / 不存在路径）时锁定该唯一可用根，不得因总数 >1 失败 (AC-1) | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 异常 | `CLAUDE_PROJECT_DIR` 为 `${workspaceFolder}` 字面量时拒绝该通道；缓存等于后续 WORKSPACE 路径；`requireLocked` 不得再抛（已锁定）(AC-1) | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 异常 | `CLAUDE_PROJECT_DIR` 为相对路径时不锁定该通道，继续 WORKSPACE；缓存不等于相对字符串本身 (AC-1) | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 异常 | `CLAUDE_PROJECT_DIR` 为绝对但不存在路径时不锁定该通道，继续 WORKSPACE (AC-1) | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 异常 | `listRoots` 抛错 / `-32601` 时该通道失败并写 stderr，继续 WORKSPACE；init Promise resolve（不抛）且缓存等于 WORKSPACE；stderr 含精确前缀 `MCP roots/list failed:` (AC-1/AC-2) | 新增 |
| initProjectRootFromMcp — listRoots 超时 | 异常 | `listRoots` 永不 resolve 时，advance fake timers ≥ `2000ms` 后 init resolve；缓存等于后续 WORKSPACE；stderr 含 `timed out` 与 `2000`；`listRootsCalls === 1`；spy `clearTimeout` 被调用 ≥1 次 (AC-1/AC-2) | 新增 |
| initProjectRootFromMcp — listRoots 超时 | 边界 | advance timers 仅 `1999ms` 时 init 仍 pending；再 advance 到 `2000ms` 才完成并锁定 WORKSPACE（证明超时常量分支而非任意短超时）(AC-1/AC-2) | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 边界 | `CLAUDE_PROJECT_DIR` 为 `undefined`（`delete process.env`）时跳过 env 通道 | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 边界 | `CLAUDE_PROJECT_DIR` 为空字符串 `""` 时跳过 env 通道 | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 边界 | `CLAUDE_PROJECT_DIR` 为超长绝对路径（>1000 chars）且不存在时缓存保持 `null` | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 边界 | `CLAUDE_PROJECT_DIR` 含空格 / emoji 且路径存在时缓存严格等于该路径 | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 边界 | URI 含 `%20` 时解码后锁定路径与本地目录 `path.resolve` 一致 | 新增 |
| initProjectRootFromMcp — 启动锁定优先级 | 边界 | 客户端未声明 `roots` capability（`getClientCapabilities()` 无 `roots`）时不调用 `listRoots`（`listRootsCalls === 0`），继续 WORKSPACE | 新增 |
| initProjectRootFromMcp — LITERAL_ENV_PATTERN 正则判别 | 异常 | `CLAUDE_PROJECT_DIR='${ab}'`（花括号内 ≥2 字符）被拒绝为字面量；后续 WORKSPACE 单根可锁定。若突变为 `/\$\{[^}]\}/`（仅 1 字符）则不会拒绝 `${ab}`，本用例必须失败以杀死该 Regex 突变 (AC-1) | 新增 |
| initProjectRootFromMcp — LITERAL_ENV_PATTERN 正则判别 | 边界 | `CLAUDE_PROJECT_DIR='${}'`（空花括号）**不得**被当作字面量拒绝：若路径本身不存在则走 `does not exist`/`invalid_path`，证明 `/\$\{[^}]+\}/` 的 `+` 量词；`requireLocked.code` 不得为 `literal_env` (AC-1/AC-2) | 新增 |
| initProjectRootFromMcp — LITERAL_ENV_PATTERN 正则判别 | 异常 | `WORKSPACE_FOLDER_PATHS='${workspaceFolder}'` 单独失败时 `requireLocked.code === 'literal_env'`，且 `message` **字节级**含 `WORKSPACE_FOLDER_PATHS contains unexpanded literal:` 与原始 raw 串 (AC-2) | 新增 |
| initProjectRootFromMcp — fileUri / rootEntry 边界 | 异常 | roots 仅含 `http://example.com/x` 时不得锁定；stderr 含 `no usable file roots`；缓存 `null`（覆盖 `fileUriToPath` 非 `file:` 协议抛错路径）(AC-2) | 新增 |
| initProjectRootFromMcp — fileUri / rootEntry 边界 | 异常 | roots 仅含空字符串 URI `""` 时不得锁定（覆盖 `rootEntryToPath` / `fileUriToPath` 空串早退）(AC-2) | 新增 |
| initProjectRootFromMcp — fileUri / rootEntry 边界 | 边界 | roots 含 `file:` 非法 URI（如 `file:` 无 pathname）时该条目跳过；若另有 1 个可用绝对路径则锁定该路径 (AC-1) | 新增 |
| initProjectRootFromMcp — 严格失败 | 异常 | `listRoots` 返回 2+ 可用根时缓存为 `null`，且不等于任一 `roots[i]`；stderr **精确含** `refusing to guess` 与 usable 数量数字 (AC-2) | 新增 |
| initProjectRootFromMcp — 严格失败 | 异常 | `WORKSPACE_FOLDER_PATHS` 解析后 >1 个可用路径时缓存为 `null`，且不等于 split 后的 `[0]` (AC-2) | 新增 |
| initProjectRootFromMcp — 严格失败 | 异常 | 三通道均失败时缓存为 `null`，且 `getMcpCachedProjectRoot() !== process.cwd()` (AC-2) | 新增 |
| initProjectRootFromMcp — 严格失败 | 边界 | `listRoots` 返回 `{ roots: [] }` 时缓存保持 `null`；stderr 含精确串 `MCP roots/list returned no usable file roots` (AC-2) | 新增 |
| initProjectRootFromMcp — 严格失败 | 边界 | `WORKSPACE_FOLDER_PATHS` 为 `undefined` 时该通道失败 | 新增 |
| initProjectRootFromMcp — 严格失败 | 边界 | `WORKSPACE_FOLDER_PATHS` 为 `,,;;` 时缓存保持 `null` | 新增 |
| initProjectRootFromMcp — 严格失败 | 边界 | `WORKSPACE_FOLDER_PATHS` 含空白 trim 后恰好 1 个可用路径时锁定（证明 `.map(trim).filter(Boolean)`；去掉 trim 或 filter 突变必须失败） | 新增 |
| initProjectRootFromMcp — 严格失败 | 边界 | `WORKSPACE_FOLDER_PATHS` 超长（>1000 chars）双段相对/无效路径时不误锁定 | 新增 |
| initProjectRootFromMcp — 严格失败 | 边界 | `WORKSPACE_FOLDER_PATHS` 夹杂 `\n` / emoji 且两可用根时不得取 `[0]` | 新增 |
| initProjectRootFromMcp — 严格失败 | 边界 | `WORKSPACE_FOLDER_PATHS` 仅相对路径段时失败原因含精确前缀 `WORKSPACE_FOLDER_PATHS has no usable absolute paths:`（非 literal）(AC-2) | 新增 |
| initProjectRootFromMcp — setLockFailure 保留 | 异常 | 先设 `CLAUDE_PROJECT_DIR='${workspaceFolder}'`（literal），再经无 roots capability + `WORKSPACE` unset 的通用失败通道后：`requireLocked.code` 仍为 `literal_env`（通用原因不得覆盖先前具体失败）(AC-2) | 新增 |
| initProjectRootFromMcp — setLockFailure 保留 | 异常 | 先设相对路径 CLAUDE（`invalid_path`），再经 `WORKSPACE` unset：`code` 仍为 `invalid_path`，`message` 仍含 `not an absolute path`（保留 `an ` 可选量词语义）(AC-2) | 新增 |
| initProjectRootFromMcp — 进程内不可变 | 正向 | 首次锁定后二次 `init` 即使 env/roots 指向另一存在目录，缓存仍严格等于首次路径 (AC-3) | 新增 |
| initProjectRootFromMcp — 进程内不可变 | 正向 | 已锁定后二次 `init` 时 `listRootsCalls` 保持 0（no-op 短路）(AC-3) | 新增 |
| initProjectRootFromMcp — 进程内不可变 | 异常 | 首次失败（缓存 null）后二次 `init` 在新 CLAUDE 可用时可写入；写入后第三次改 env 不再改写 (AC-3) | 新增 |
| requireLockedProjectRoot | 正向 | 缓存已锁定时返回值 `===` 缓存路径，且 `path.isAbsolute` 为 true (AC-2/AC-5) | 新增 |
| requireLockedProjectRoot — code 映射与完整 message | 异常 | 缓存为 `null` 且无特定失败原因时：`code === 'not_locked'`；`message` **完整字节级相等**于 `` `Project root is not locked: ${reason}. Set CLAUDE_PROJECT_DIR to an existing absolute path, or provide exactly one MCP root / WORKSPACE_FOLDER_PATHS entry.` ``（`reason` 为 `no unique usable project root found` 或通道写入的原文；禁止仅 `/not locked/i`）(AC-2) | 新增 |
| requireLockedProjectRoot — code 映射与完整 message | 异常 | WORKSPACE 双可用根失败后：`code === 'multi_root'`；`message` 含 `multi-root ambiguity` 或 `usable paths`；不得为 `not_locked`。另用失败原因含 `> 1`（带空格）与 `>1`（无空格）各一用例，证明 `>\s*1` 的 `\s*`（杀死 `\s`/`\S*` 突变）(AC-2) | 新增 |
| requireLockedProjectRoot — code 映射与完整 message | 异常 | roots 返回 2+ 可用根后：`code === 'multi_root'`，`message` 含 `roots/list` 与数量 (AC-2) | 新增 |
| requireLockedProjectRoot — code 映射与完整 message | 异常 | 仅 `WORKSPACE_FOLDER_PATHS='${workspaceFolder}'`：`code === 'literal_env'`，`message` 含 `${` 与 `literal` (AC-2) | 新增 |
| requireLockedProjectRoot — code 映射与完整 message | 异常 | 仅 `CLAUDE_PROJECT_DIR='${workspaceFolder}'`：`code === 'literal_env'` (AC-2) | 新增 |
| requireLockedProjectRoot — code 映射与完整 message | 异常 | 仅相对路径 CLAUDE：`code === 'invalid_path'`；`message` 含完整片段 `is not an absolute path`（保留 `an `，杀死 `an\\s+` 被删的 Regex 突变）(AC-2) | 新增 |
| requireLockedProjectRoot — code 映射与完整 message | 异常 | 仅绝对不存在 CLAUDE：`code === 'invalid_path'`；`message` 含 `does not exist` 与路径原文 (AC-2) | 新增 |
| requireLockedProjectRoot — code 映射与完整 message | 边界 | 抛错后再次调用仍抛同一 `code` 与同一完整 `message`（失败原因不被清空） | 新增 |
| getProjectDir — 模块归并后公共 API | 正向 | 缓存已锁定时返回缓存路径，优先于随后改写的 env (AC-10) | 新增 |
| getProjectDir — 模块归并后公共 API | 正向 | 无缓存且可用 `CLAUDE_PROJECT_DIR` 时返回该路径（CLI 语义）(AC-7/AC-10) | 新增 |
| getProjectDir — 模块归并后公共 API | 正向 | 无缓存、无 CLAUDE，且 WORKSPACE 恰好 1 个可用路径时返回该路径 (AC-10) | 新增 |
| getProjectDir — 模块归并后公共 API | 边界 | 无缓存且 env 均不可用时返回 `process.cwd()`（仅 CLI 辅助）(AC-7) | 新增 |
| getProjectDir — 模块归并后公共 API | 边界 | `CLAUDE_PROJECT_DIR` 为 `undefined` 时继续 WORKSPACE/cwd | 新增 |
| getProjectDir — 模块归并后公共 API | 边界 | `CLAUDE_PROJECT_DIR=""` 视为未设置，继续 WORKSPACE/cwd | 新增 |
| getProjectDir — 模块归并后公共 API | 异常 | `CLAUDE_PROJECT_DIR` 为 `${...}` 时跳过，返回值不得等于该字面量 | 新增 |
| getProjectDir — 模块归并后公共 API | 边界 | WORKSPACE 双可用路径时 CLI `getProjectDir` 回退 `cwd`（不得取 `[0]`），且与 `requireLocked` 抛错语义区分 (AC-7/AC-2) | 新增 |
| getMcpCachedProjectRoot | 正向 | 锁定后返回非 null，且 `===` init 所用路径 | 新增 |
| getMcpCachedProjectRoot | 异常 | 三通道均失败后不抛异常，返回 `null`，且 `!== process.cwd()` | 新增 |
| getMcpCachedProjectRoot | 边界 | 未 init 时返回 `null` | 新增 |
| isProjectRootLockError — 跨模块副本鸭类型识别 | 正向 | 真实 `ProjectRootLockError` 实例返回 `true`（覆盖 `instanceof` 真分支；清空该分支 BlockStatement 突变必须失败） | 新增 |
| isProjectRootLockError — 跨模块副本鸭类型识别 | 正向 | 同名 `Error` + 字符串 `code`（模块副本）返回 `true`（覆盖非 instanceof 的 duck 路径） | 新增 |
| isProjectRootLockError — 跨模块副本鸭类型识别 | 异常 | 普通 Error / 仅改 name 无 code / `null` / 字符串 / 非 Error 对象 → 均返回 `false` | 新增 |
| isProjectRootLockError — 跨模块副本鸭类型识别 | 边界 | `name === 'ProjectRootLockError'` 但 `code` 为非 string（如 number）→ `false`（证明 `typeof code === 'string'`） | 新增 |
| initProjectRootFromMcp — roots 可用时缓存首个 root | 正向 | 多条 root 时仅使用 `roots[0].uri` | 废弃 |
| initProjectRootFromMcp — 缓存更新 | 正向 | 二次 init 返回新 URI 时更新缓存 | 废弃 |
| initProjectRootFromMcp — list_changed 注册 | 正向 | `roots.listChanged === true` 时注册 notification handler | 废弃 |
| initProjectRootFromMcp — listRoots 抛错 / 空 roots | 正向 | 失败后回退 env/cwd 并依赖旧 `constant.getProjectDir` 语义 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| MCP `McpServerLike` | 最小 mock：`getClientCapabilities()` 返回 `{ roots?: ... }` 或 `{}`；`listRoots()` resolve / reject / hang（never resolve）并计数调用次数；可控 URI 含 `file://` / 裸绝对 / `http://` / `""` / 非法 `file:` | init / requireLocked / 超时 / fileUri 全部 |
| `process.env` | `beforeEach`/`afterEach` 保存恢复；覆盖 `undefined` / `""` / 字面量 `${ab}` / `${}` / 多值 / 与其它通道并存 | 启动优先级与 getProjectDir / Regex 判别 |
| `fs.existsSync` / 真实 temp dir | 优先真实 `mkdtemp`；特殊场景可 spy 控制存在性 | 绝对路径存在性 |
| 模块级缓存 | `vi.resetModules()` + 重新 `import('./project-root')` | 全部用例隔离 |
| `process.stderr` | spy `write`，断言失败通道**精确子串**（`MCP roots/list failed:` / `refusing to guess` / `no usable file roots`） | listRoots 失败 / 超时 / 多根 |
| `vi.useFakeTimers` + `clearTimeout` spy | 精确推进 `1999`/`2000` ms；断言 `clearTimeout` 调用 | listRoots 超时 |

---

### `plugins/dev-team/bin/src/mcp.ts` -> `plugins/dev-team/bin/src/mcp.test.ts`

#### 待测功能

- `connectToServer(transport: Transport): Promise<McpServer>`: 构造 `McpServer({ name: 'dev-team', version: '2.8.11' })`；注册全部 11 个 tool（精确 name + description + inputSchema + outputSchema + handler）；connect 后 `initProjectRootFromMcp`；各 handler 经 `withLockedProjectRoot` 调用 `requireLockedProjectRoot()`；捕获锁错误经 `lockErrorResult` 返回 `{ isError: true, content: [{ type: 'text', text: err.message }] }`；非锁错误重新抛出；有参工具将锁定根传入 command（不再读 `args.project_root`）；不注册 `list_changed`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| MCP 注册 — 工具 name 精确断言 | 正向 | `listTools()` 返回的 name 集合经 sort 后 **严格等于** `['archi_check','archi_query','archi_validate','archi_write','backtrack','change_list','config_get','phase_log','phase_next','test_detect_frameworks','test_resolve_paths']`（逐元素 `===`，长度恰好 11） | 新增 |
| MCP 注册 — 工具 name 精确断言 | 正向 | 对每个预期 name 单独 `expect(names).toContain('<exact>')` | 新增 |
| MCP 注册 — 工具 name 精确断言 | 异常 | name 集合**不得**包含：`phaseLog` / `Phase_Log` / `list_changed` / `roots` / `changeList` / `config-get` / `""` | 新增 |
| MCP 注册 — registerTool spy 精确字面量 | 正向 | spy `McpServer.prototype.registerTool`（或构造后实例方法）：按注册顺序对每次调用断言 `args[0] === EXPECTED_NAME` 且 `args[1].description === EXPECTED_DESCRIPTION`（**字节级 toBe**，与 `listTools` 双通道；专杀 `ignoreStatic`/归因导致 listTools 未杀的 StringLiteral） | 新增 |
| MCP 注册 — registerTool spy 精确字面量 | 正向 | 第 11 次注册后 `registerTool` 调用次数恰好 11；每次 `args[2]`（handler）为 `typeof === 'function'`（非 undefined） | 新增 |
| MCP 注册 — registerTool spy 精确字面量 | 异常 | 任一注册调用的 `args[0]` 不得为 `""` / `undefined` / camelCase 别名（如 `changeList`）；`args[1]` 不得缺 `description` 或为非 string（spy 记录中断言每项均通过） | 新增 |
| MCP 注册 — registerTool spy 精确字面量 | 异常 | 若某次 `args[2]` 为 `undefined` / 非函数，该用例必须失败（专杀 handler 被替换为空箭头或省略第三参） | 新增 |
| MCP 注册 — registerTool spy 精确字面量 | 边界 | 注册顺序严格等于源码 TOOLS 数组顺序；打乱顺序或少注册 1 个时 `toEqual` 失败 | 新增 |
| MCP 注册 — 工具 description 精确字面量 | 正向 | 11 个工具 description 与源码注册串逐一 `toBe`（含 `phase_log` 尾部空格；`test_resolve_paths` 含 `"git-change"`；`test_detect_frameworks` 含 `tests suite mappings`） | 新增 |
| MCP 注册 — 工具 description 精确字面量 | 异常 | 任一工具 description 不得为空串 / `undefined`；截断或改写关键词必须使 `toBe` 失败 | 新增 |
| MCP 注册 — inputSchema 形状（无 project_root） | 正向 | `change_list` / `config_get` / `archi_*` / `test_*` 的 `inputSchema.properties` 不含 `project_root`/`projectRoot` (AC-4) | 新增 |
| MCP 注册 — inputSchema 形状（无 project_root） | 正向 | `change_list` 的 input 为空 object；`config_get` 必填 `key`；`test_detect_frameworks` 可含 `files`；`test_resolve_paths` 可含 `modules` (AC-4) | 新增 |
| MCP 注册 — inputSchema 形状（无 project_root） | 正向 | `phase_log` / `phase_next` / `backtrack` 亦不得出现 `project_root` | 新增 |
| MCP 注册 — inputSchema 形状（无 project_root） | 异常 | `changeListInputSchema.safeParse({ project_root: '/x' })`：strip 则 data 无该键，strict 则 `success === false` (AC-4) | 新增 |
| MCP 注册 — outputSchema project_root 回显契约 | 正向 | 无参成功 `change_list`：含键 `project_root` 且值 `===` 锁定根；同时含 `changes`/`count` 且 `count === changes.length` (AC-6) | 新增 |
| MCP 注册 — outputSchema project_root 回显契约 | 正向 | `changeListOutputSchema.safeParse(成功体)` 成功；shape 含 `project_root` (AC-6) | 新增 |
| MCP 注册 — outputSchema project_root 回显契约 | 异常 | 未锁定时不得返回成功 JSON 的 cwd `project_root`；`isError === true` (AC-6) | 新增 |
| MCP 注册 — server identity | 正向 | `getServerVersion()`：`name === 'dev-team'` 且 `version === '2.8.11'`（字节级） | 新增 |
| MCP 注册 — server identity | 异常 | `name` 不得为 `""` / `DevTeam` / `dev_team` / `undefined`；`version` 不得为 `""` / `0` / 缺字段——任一偏差使 `toBe` 失败 | 新增 |
| MCP 注册 — server identity | 边界 | `version` 字符串长度与精确字面量 `2.8.11` 一致（禁止仅 `/^2\./` 宽松匹配，以免漏杀 StringLiteral 突变） | 新增 |
| MCP 注册 — 无 list_changed handler | 异常 | 服务端未注册 `notifications/roots/list_changed` | 新增 |
| MCP schema — 无 input project_root | 正向 | 同「inputSchema 形状」；保留为 AC-4 映射别名 | 新增 |
| MCP schema — 无 input project_root | 异常 | `callTool('change_list', { project_root: otherDir })`：成功则回显锁定根≠otherDir，失败则 `isError` (AC-4) | 新增 |
| MCP 工具调用 — 未锁定入口校验 | 异常 | `phase_log` / `phase_next` / `backtrack` 均 `isError === true`，cwd 目录列表不变 (AC-5) | 新增 |
| MCP 工具调用 — 未锁定入口校验 | 异常 | 未锁定时 `change_list` / `config_get` / `test_resolve_paths` / `test_detect_frameworks` / `archi_query` 均 `isError === true` 且 text 非空 (AC-5) | 新增 |
| MCP 工具调用 — isError content 精确文本 | 异常 | mock `requireLocked` 抛固定 MSG：`isError === true`、`content.length === 1`、`content[0].type === 'text'`、`content[0].text === MSG`（字节级）(AC-2/AC-5) | 新增 |
| MCP 工具调用 — isError content 精确文本 | 异常 | `multi_root` / `literal_env` code 仍返回原文 message，不按 code 改写 (AC-2) | 新增 |
| MCP 工具调用 — withLockedProjectRoot 错误映射 | 异常 | 鸭类型锁错误 → `isError` 且 text===原文；无 code 不得吞为锁错误；普通 `Error('boom')` 不得走 `lockErrorResult` 成功包装 (AC-5) | 新增 |
| MCP 工具调用 — change_list 回显与锁定根接线 | 正向 | fixture 锁定根 `dir !== cwd`：`project_root === dir`，`count === changes.length` (AC-6) | 新增 |
| MCP 工具调用 — change_list 回显与锁定根接线 | 正向 | spy `runChangeList`：以锁定根**字符串**调用恰好一次（签名为 `runChangeList(projectRoot: string)`）(AC-6) | 新增 |
| MCP 工具调用 — change_list 回显与锁定根接线 | 异常 | 未锁定：`isError`；text 无 cwd 成功 JSON (AC-6) | 新增 |
| MCP 工具调用 — 有参工具改用锁定根 | 正向 | spy `runConfigGet`：`projectRoot === lockedRoot !== cwd` (AC-4) | 新增 |
| MCP 工具调用 — 有参工具改用锁定根 | 正向 | spy `runTestResolvePaths`：`project_root === lockedRoot` (AC-4) | 新增 |
| MCP 工具调用 — 有参工具改用锁定根 | 正向 | spy `runTestDetectFrameworks`：`{ files, projectRoot: lockedRoot }` (AC-4) | 新增 |
| MCP 工具调用 — 有参工具改用锁定根 | 异常 | 未锁定时 command spy 调用次数为 0 (AC-4/AC-5) | 新增 |
| MCP 工具调用 — 全 11 工具 handler 非 undefined | 正向 | 锁定后对 11 个工具各 `callTool` 一次（archi_* / phase_* / backtrack / config_get / test_* / change_list 用既有 mock/fixture）：**每一个** `isError === false`（或 archi mock 成功），且 `content[0].text` 非空 JSON；专杀 handler ArrowFunction `() => undefined` 突变（该突变会使 callTool 无有效 structuredContent） | 新增 |
| MCP 工具调用 — 全 11 工具 handler 非 undefined | 正向 | 对 `phase_log` / `backtrack` / `phase_next` / `archi_query` / `archi_validate` / `archi_write` / `archi_check` 分别 spy 底层 command/lib，断言各 spy 调用次数 ≥1（证明 handler 体被执行，而非空箭头） | 新增 |
| MCP 工具调用 — 全 11 工具 handler 非 undefined | 异常 | 未锁定时对全部 11 个工具各 `callTool` 一次：每一个 `isError === true`，且对应 command/lib spy 调用次数均为 0（证明入口短路，非空箭头静默成功）(AC-5) | 新增 |
| MCP 工具调用 — 全 11 工具 handler 非 undefined | 异常 | 锁定后若任一 handler 返回 `undefined` / 缺 `content`：该用例必须失败（与 SDK 成功包装对照） | 新增 |
| MCP 工具调用 — 全 11 工具 handler 非 undefined | 边界 | 11 次成功调用后 spy 计数表长度恰好 11（少 1 个工具未覆盖则失败） | 新增 |
| MCP 工具调用 — archi_check files/staged | 正向 | `archi_check` 传入 `files: 'a.ts, b.ts'` 与 `staged: true` 时 lib 收到 trim 后数组与 `staged === true`（杀死 files split/filter 与 `!!args.staged` 相关突变） | 新增 |
| MCP 工具调用 — archi_check files/staged | 边界 | `staged: false` 时 lib 收到 `staged === false`（非 `true`/`undefined`；证明 bool False 边，杀死恒 `true` 突变） | 新增 |
| MCP 工具调用 — archi_check files/staged | 边界 | 省略 `staged`（None/undefined）时 lib 收到 `staged === false`（`!!undefined === false`；与显式 `true` 对照） | 新增 |
| MCP 工具调用 — archi_check files/staged | 异常 | `files: ''` / `','` /仅空白：lib 收到 `files === undefined` 或空过滤后无有效项（不得传入含空串的数组）；非法时不得把 cwd 当根 | 新增 |
| MCP 工具调用 — archi_check files/staged | 异常 | 未锁定时 `archi_check`（含 `staged: true`）`isError`，且 `runCrossRefCheck` spy 次数为 0 | 新增 |
| MCP 注册 — 工具清单 | 正向 | 已被 name 精确断言取代 | 废弃 |
| MCP 注册 — backtrack 工具 | 正向 | description 不为空（已被精确字面量取代） | 废弃 |
| MCP 注册 -- test_resolve_paths description (AC-5) | 正向 | 已被精确字面量取代 | 废弃 |
| MCP 注册 — test_detect_frameworks description | 正向 | 已被精确字面量取代 | 废弃 |
| MCP 工具调用 — config_get / test_* / change_list 传 project_root | 正向 | arguments 含 `project_root` | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `./lib/project-root` | mock `init` / `requireLocked` / `isProjectRootLockError`；精确文本场景抛固定 message | 未锁定 / isError / 接线 |
| `./commands/*` 与 archi lib | `vi.spyOn` 断言锁定根参数与调用次数；全工具 handler 场景全部 spy | handler 执行 / 短路 |
| `McpServer.prototype.registerTool` | spy 记录 name/description/handler | registerTool 精确字面量 |
| 临时文件系统 | `setupTempProject`；锁定根 `!== cwd` | config_get / test_* / change_list |
| InMemoryTransport + Client | 真实协议 `listTools` / `callTool` | 全部协议级用例 |

---

### `plugins/dev-team/bin/src/commands/change-list.ts` -> `plugins/dev-team/bin/src/commands/change-list.test.ts`

#### 待测功能

- `runChangeList(projectRoot: string): ChangeListResult`: CLI/command 层接受显式项目根字符串（**非** options 对象）；扫描 `openspec/changes/`；输出回显 `project_root`；`count === changes.length`；不强制 MCP 锁定 (AC-7)

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runChangeList -- project_root | 正向 | 显式传入 fixture 根：`result.project_root ===` 传入值（字节级），且可扫描到该根下 change (AC-7) | 新增 |
| runChangeList -- project_root | 正向 | 显式根含活跃 change：列表含该 change；`result.project_root !== process.cwd()` (AC-7) | 新增 |
| runChangeList -- project_root | 异常 | 显式根指向不存在路径：`{ changes: [], count: 0 }`，`project_root` 仍为传入值（不得改写 cwd）(AC-7) | 新增 |
| runChangeList -- project_root | 边界 | 显式超长根（>1000 chars）不崩溃；`project_root` 仍为该串；`count === 0` | 新增 |
| runChangeList -- project_root | 边界 | 显式含 emoji 的根：`project_root` 严格等于传入值且 `!== cwd` | 新增 |
| runChangeList -- project_root | 边界 | 显式根与 `process.cwd()` 不同时仍以传入为准，且 `count === changes.length` | 新增 |
| runChangeList — CLI vs MCP schema 契约 | 正向 | `changeListInputSchema` shape 不含 `project_root`；同时 `runChangeList(dir)` 仍可用 (AC-4/AC-7) | 新增 |
| runChangeList — CLI vs MCP schema 契约 | 正向 | `changeListOutputSchema.safeParse(runChangeList(...))` 成功，且 `project_root` 等于命令使用的根 (AC-6/AC-7) | 新增 |
| runChangeList — CLI vs MCP schema 契约 | 异常 | `changeListInputSchema.safeParse({ project_root: '/x' })`：strip 则无该键 / strict 则失败 (AC-4) | 新增 |
| runChangeList -- project_root | 正向 | spy `getProjectDir`：显式 options 时不调用（旧 options 对象 API） | 废弃 |
| runChangeList -- project_root | 边界 | `project_root: null` / `""` 经 `\|\|` 回退（旧 API） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时文件系统 | `createTempProject` / `writeChange` | 正向 / 异常 |
| Zod schema | 直接 import input/output schema | CLI vs MCP 契约 |

---

### `plugins/dev-team/bin/src/commands/config-get.ts` -> `plugins/dev-team/bin/src/commands/config-get.test.ts`

#### 待测功能

- `runConfigGet(options: ConfigGetOptions): ConfigGetResult`: 读取 `openspec/config.json` 点路径；`projectRoot` 显式优先，否则 `getProjectDir()`（`\|\|` 链）；确保配置文件存在

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runConfigGet — projectRoot 显式注入 | 正向 | 显式 `projectRoot` 指向含 `openspec/config.json` 的 fixture：`exists === true` 且 `value` 等于写入值；`key` 回显等于入参 (AC-7) | 新增 |
| runConfigGet — projectRoot 显式注入 | 正向 | spy `getProjectDir`：显式非空 `projectRoot` 时调用次数为 0（杀死始终 `getProjectDir()` 突变）(AC-7) | 新增 |
| runConfigGet — projectRoot 显式注入 | 边界 | 省略 `projectRoot` / `projectRoot: ""` / `undefined`：均调用 `getProjectDir`，三者行为一致（证明 `\|\|` 非 `??`）(AC-7) | 新增 |
| runConfigGet — projectRoot 显式注入 | 异常 | 键不存在：`exists === false`，`value` 按实现为 `undefined`/`null`，不得抛 | 新增 |
| runConfigGet — projectRoot 显式注入 | 边界 | `key` 为空串 / 超长 / 含 `.` 嵌套路径：行为明确且不崩溃 | 新增 |
| runConfigGet — projectRoot 显式注入 | 边界 | 显式根 `!== cwd` 时读取的是该根配置，而非 cwd 配置 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时文件系统 | 写入 `openspec/config.json` fixture | 全部 |
| `getProjectDir` | `vi.spyOn` 断言 `\|\|` 回退 | 省略 / 空串 |

---

### `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` -> `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts`

#### 待测功能

- `runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult`: 三种 modules 模式（空数组自动扫描 / 非空过滤 / `"git-change"`）；`project_root` 可选，缺省 `getProjectDir()`；派生 colocated 单测路径；排除 integration 字段

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runTestResolvePaths — project_root 注入与 getProjectDir 回退 | 正向 | 显式 `project_root` 指向 fixture 时 unit_tests 相对该根；spy `getProjectDir` 调用次数 0 (AC-7) | 新增 |
| runTestResolvePaths — project_root 注入与 getProjectDir 回退 | 边界 | `project_root: null` / 省略 / `""`：均回退 `getProjectDir`（`\|\|`/`??` 语义以源码为准，断言 spy 被调用）(AC-7) | 新增 |
| runTestResolvePaths — SOURCE_EXTENSIONS 全扩展名 | 正向 | 分别传入 `.ts` / `.tsx` / `.js` / `.jsx` / `.mjs` / `.cjs` / `.py` / `.go` / `.rs` 源文件：均进入 `unit_tests` 且派生路径精确为 `*.test.ts` / `*.test.tsx` / … / `test_*.py` / `*_test.go` / `*_test.rs`（任一扩展名 StringLiteral 被清空则对应用例失败） | 新增 |
| runTestResolvePaths — SOURCE_EXTENSIONS 全扩展名 | 异常 | `.md` / `.json` / `.yaml` / 无扩展名 → 进入 `errors`，不进 `unit_tests` | 新增 |
| runTestResolvePaths — isTestFile 正则判别 | 异常 | 传入 `foo.test.ts` / `test_foo.py` / `foo_test.go` / `foo_test.rs` / `foo_tests.rs` → 均进 `errors`（已是测试文件） | 新增 |
| runTestResolvePaths — isTestFile 正则判别 | 边界 | `testX.py`（无下划线）**不是**测试文件约定，应按源文件派生 `test_testX.py` 或进 errors（以实现为准）；必须与 `test_foo.py` 行为不同，杀死 `/^test_.\\.py$/`（单字符）突变 | 新增 |
| runTestResolvePaths — isTestFile 正则判别 | 边界 | `foo_tests.rs` 与 `foo_test.rs` 均识别为测试文件；`foo_tes.rs` 不得误判（证明 `tests?`） | 新增 |
| runTestResolvePaths — extractErrorMessage stderr | 异常 | `modules: "git-change"` 且 `execSync` 抛 `Error`：errors.message === `error.message` | 新增 |
| runTestResolvePaths — extractErrorMessage stderr | 异常 | 抛非 Error 但 `{ stderr: '  boom  ' }`：errors.message === `'boom'`（trim 后；trim 被删则含首尾空格导致失败） | 新增 |
| runTestResolvePaths — extractErrorMessage stderr | 异常 | 抛 `{ stderr: '   ' }`（仅空白）：errors.message === fallback 非空串（证明 `trim() \|\| fallback`，`&&` 突变必须失败） | 新增 |
| runTestResolvePaths — extractErrorMessage stderr | 异常 | 抛 `{ stderr: { toString() { throw new Error('x') } } }`：走 catch 后仍有非空 fallback（覆盖 catch BlockStatement） | 新增 |
| runTestResolvePaths — extractErrorMessage stderr | 边界 | 抛 `null` / 数字 / 无 stderr 对象：使用 fallback，不抛未捕获异常 | 新增 |
| runTestResolvePaths — 路径穿越与范围内绝对路径 | 异常 | `../../../outside.ts` → errors；绝对路径在项目内 → 正常 unit_tests | 新增 |
| runTestResolvePaths — git-change / 空 modules / 过滤 | 正向 | 保留既有：空 modules 扫描、范围过滤、git-change、去重、exclude（既有用例继续有效，标记不废弃） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时文件系统 | 多扩展名源文件 + openspec/config.json tests[] | 扩展名 / 过滤 / 扫描 |
| `child_process.execSync` | stub 抛 Error / `{stderr}` / toString 抛错 | extractErrorMessage |
| `getProjectDir` / `runTestDetectFrameworks` | spy | project_root 回退 / 空 modules |

---

### `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` -> `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts`

#### 待测功能

- `runTestDetectFrameworks(options: TestDetectFrameworksOptions)`: 基于 `tests[]` suite 映射检测框架；省略 `files` 时 auto-scan；产出 `detected` + `plan`（含 shell/cmd script、`{config_args}` 展开、`mutation_score`）；`projectRoot` 可选

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runTestDetectFrameworks — projectRoot | 正向 | 显式 `projectRoot` 读该根 config；spy `getProjectDir` 次数 0 (AC-7) | 新增 |
| runTestDetectFrameworks — projectRoot | 边界 | 省略 / `""` 回退 `getProjectDir` (AC-7) | 新增 |
| runTestDetectFrameworks — projectRoot | 异常 | 显式根指向不存在目录：不得崩溃；detected/plan 为空或错误可观测，且不得误读 cwd 配置 | 新增 |
| runTestDetectFrameworks — {config_args} 精确展开 | 正向 | suite 含 config 且 vite-plus：`script.shell` **含** `--config` 与相对 absCwd 的 POSIX 路径；**不得**残留字面量 `{config_args}`；空 configArgs 时不得在命令中留下双空格粘连（证明 `/\s*\{config_args\}/g`） | 新增 |
| runTestDetectFrameworks — {config_args} 精确展开 | 异常 | suite 含 config 但框架 `config_flag === null`（pytest/rust）：抛错或 plan 失败，message **精确含** `does not support config injection` 与 `config_flag is null` | 新增 |
| runTestDetectFrameworks — {config_args} 精确展开 | 异常 | suite 含 config 但 absConfig 无法解析：message 含 `absConfig could not be resolved` | 新增 |
| runTestDetectFrameworks — shell·cmd 脚本字面量 | 正向 | plan[0].script.shell：以 `cd <directory>\n` 开头（directory≠`.` 时）、含 `rm -rf` 清理项、以 `\n` 结尾；directory 为 `.` 时**不得**出现 `cd .` 行 | 新增 |
| runTestDetectFrameworks — shell·cmd 脚本字面量 | 正向 | plan[0].script.cmd：单行 `&` 连接；含 `cd /d`；cleanup 项包裹在 `(if exist ...)`；directory 含空格时路径被双引号包裹 | 新增 |
| runTestDetectFrameworks — shell·cmd 脚本字面量 | 异常 | shell/cmd 均不得残留未展开占位符 `{config_args}`（`{files}` 为执行期约定占位可保留）；若 `rm -rf` / `cd /d` / `(if exist` 任一关键字被清空则用例失败 | 新增 |
| runTestDetectFrameworks — shell·cmd 脚本字面量 | 异常 | directory 为非法非字符串路径导致 generate* 守卫抛 `TypeError` 时，公共 API 不得产出残缺 script（或 plan 失败可观测）；不得静默返回空串 script | 新增 |
| runTestDetectFrameworks — shell·cmd 脚本字面量 | 边界 | directory 含 tab/空格：cmd 分支引号逻辑生效；无空格时不包引号 | 新增 |
| runTestDetectFrameworks — shell·cmd 脚本字面量 | 边界 | `coverage_cleanup: []` 时空清理列表：shell 仍含 `cd`/`test_execution` 行，且不得出现孤立的 `rm -rf` 空参行 | 新增 |
| runTestDetectFrameworks — suite scope 边界 | 正向 | 文件恰好等于 suite.root（无尾部 `/`）视为 in-scope；`root + '/' + child` in-scope；`root + 'x'`（前缀假匹配）out-of-scope（杀死 `startsWith(root)` 无 `/` 分隔突变） | 新增 |
| runTestDetectFrameworks — suite scope 边界 | 异常 | excludes 命中时不得进入 detected；includes: [] 时显式 files 标 unknown | 新增 |
| runTestDetectFrameworks — suite scope 边界 | 边界 | 省略 includes 用 default_glob；多 suite 数组顺序优先 | 新增 |
| runTestDetectFrameworks — 空 files auto-scan | 正向 | 省略 `files`：扫描 suite 范围源文件填入 detected（覆盖 `options.files` 缺省分支；`files.length === 0` 与省略应对齐或按实现文档） | 新增 |
| runTestDetectFrameworks — 空 files auto-scan | 边界 | `files: []`：与省略行为对照断言（杀死 `length === 0` / `!== 0` Equality 突变） | 新增 |
| runTestDetectFrameworks — 空 files auto-scan | 异常 | suite root 无匹配源文件时 detected 为空数组且不抛；不得把 cwd 外文件误扫入 | 新增 |
| runTestDetectFrameworks — 空 files auto-scan | 异常 | `files: null`（若类型允许到达）或非法非数组：按实现拒绝或等同省略；不得崩溃且不得误用 `getProjectDir` 外路径 | 新增 |
| runTestDetectFrameworks — collectFiles 不可读目录 | 边界 | suite root 下放置不可读子目录（或 spy `readdirSync` 抛错）：walk catch 后仍返回其它可读文件，不抛未捕获（覆盖 NoCoverage BlockStatement） | 新增 |
| runTestDetectFrameworks — collectFiles 不可读目录 | 异常 | spy `readdirSync` 对根目录本身抛 `EACCES`：返回 `[]`（或仅已收集项），公共 API 不抛未捕获异常；不得把错误吞成「扫描到 cwd」 | 新增 |
| runTestDetectFrameworks — collectFiles 不可读目录 | 异常 | 空目录 / 仅含子目录无文件：返回 `[]`，detected 为空且 plan 行为明确（无崩溃） | 新增 |
| runTestDetectFrameworks — mutation_score / 空 tests | 正向 | 保留既有 mutation.score 0/100/默认 70、tests: []、旧 test 键 breaking 用例 | 新增 |
| runTestDetectFrameworks — mutation_score / 空 tests | 异常 | `mutation.score` 为非法类型（字符串 `"70"` / `null`）时按实现回退默认 70 或校验失败，不得产出 `NaN` 写入 plan | 新增 |
| runTestDetectFrameworks — mutation_score / 空 tests | 边界 | score `0` / `100` / 省略（默认 70）三值对照；`tests: []` 时 plan 为空且返回可观测空结果 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时文件系统 | 多 suite / config / 不可读目录 fixture | 全部 |
| `fs.readdirSync` | 局部 spy 抛 EACCES | collectFiles catch |
| `getProjectDir` | spy | projectRoot 回退 |

---

### `plugins/dev-team/bin/src/hooks.ts` -> `plugins/dev-team/bin/src/hooks.test.ts`

#### 待测功能

- `runProtectFiles(): void` / `runStaticCheck(): void` / `main(): void` / `captureStderr()`: PreToolUse 写保护与 SubagentStop 静态检查；内置保护 `eval.json` / `config.json`；Bash/PowerShell 写探测；`getProjectDir` 解析项目根

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| hooks — 内置保护字面量 | 正向 | Write `openspec/changes/x/eval.json` → deny；reason **同时含**精确片段 `phase_log MCP` 与 glob 语义；Write `openspec/config.json` → deny；reason 含 `自行操作` 或 config 保护文案（杀死内置 glob/reason StringLiteral 清空） | 新增 |
| hooks — 内置保护字面量 | 正向 | 内置 glob 必须匹配 `**/openspec/changes/**/eval.json` 与 `**/openspec/config.json`：对 `openspec/changes/foo/eval.json` deny、对 `openspec/changes/foo/proposal.md` allow | 新增 |
| hooks — 内置保护字面量 | 异常 | Write 目标为非保护路径（如 `src/foo.ts`）→ allow；reason 不得误含 `phase_log MCP` / config 保护文案 | 新增 |
| hooks — detectBashWrite 正则判别 | 异常 | `echo x > eval.json` / `>>` / `>|` / `tee eval.json` / `tee -a eval.json` / `>& eval.json` → 均 deny | 新增 |
| hooks — detectBashWrite 正则判别 | 边界 | `foo -> eval.json`（箭头，非重定向）→ allow（证明 `(?:^|[^-])>{1,2}`；去掉 `[^-]` 会误杀） | 新增 |
| hooks — detectBashWrite 正则判别 | 边界 | 命令以 `tee file` 开头（无前导空白）仍 deny（证明 `^|` 分支；突变去掉 `^` 则失败） | 新增 |
| hooks — detectBashWrite 正则判别 | 正向 | `^(python\|python3\|node)\s` 豁免：`python script.py > eval.json` / `node x.js` / `python3 x` → allow；`python3x`（无空格）不得误豁免 | 新增 |
| hooks — detectPowerShellWrite 正则判别 | 异常 | Set-Content / Out-File / Add-Content / Export-Csv / Export-CliXml / Tee-Object / `>` / `>>` / `*>` / WriteAllText / AppendAllText / WriteAllLines / WriteAllBytes → deny | 新增 |
| hooks — detectPowerShellWrite 正则判别 | 正向 | `python`/`node` 豁免；Get-Content 只读 allow | 新增 |
| hooks — isRecord 边界 | 边界 | `tool_input` 为数组 / `null` / 数字 / 字符串 → allow（fail-open）；证明 `value != null && typeof === 'object' && !Array.isArray`（`\|\|` 或去掉 `!Array` 突变必须失败） | 新增 |
| hooks — getProjectDir 接线 | 正向 | spy `getProjectDir`：protect-files / static-check 使用其返回根读取 config（MCP 锁定缓存优先时返回缓存） | 新增 |
| hooks — getProjectDir 接线 | 异常 | spy `getProjectDir` 抛错时 protect-files / static-check 不得未捕获崩溃；须 fail-open allow 或按既有 catch 输出 block JSON（以实现为准，断言可观测且非空响应） | 新增 |
| hooks — getProjectDir 接线 | 异常 | `getProjectDir` 返回不存在路径时：内置保护仍按该根拼接 glob 匹配；不得静默改读 `process.cwd()` 下的 config | 新增 |
| hooks — getProjectDir 接线 | 边界 | MCP 缓存已锁定且 env 指向另一路径时：hooks 经 `getProjectDir` 优先缓存根（与 CLI 语义一致） | 新增 |
| hooks — 子命令调度 | 正向 | 保留既有：`protect-files` / `static-check` / 未知子命令 exit(1) / 空 argv | 新增 |
| hooks — 子命令调度 | 异常 | `argv[2]` 为未知子命令（含 `""` / `Protect-Files` 大小写变体 / 随机串）：stderr 含 `Unknown subcommand:` + 原文，`process.exit(1)` | 新增 |
| hooks — 子命令调度 | 异常 | `argv[2]` 为 `undefined`（空 argv）：走 default 分支 exit(1)，不得误入 protect-files | 新增 |
| hooks — 子命令调度 | 边界 | 恰好 `protect-files` / `static-check` 字面量（字节级）调度到对应函数；多余 argv 后缀不改变子命令选择 | 新增 |
| hooks — WORKSPACE roots 静态检查 | 边界 | static-check 在多 WORKSPACE / 空 roots 时行为明确（与 project-root 严格失败语义不冲突；断言不读错 cwd 配置） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| stdin JSON | 注入 PreToolUse / SubagentStop 事件 | protect / static-check |
| 临时项目 + config | 内置与用户 write_protection.files | 模式加载 |
| `getProjectDir` / `runStaticAnalysis` | spy | 接线 |
| `process.exit` / stdout | mock 捕获 JSON 输出 | 调度 |

---

### `plugins/dev-team/bin/src/commands/test-execution.ts` -> `plugins/dev-team/bin/src/commands/test-execution.test.ts`

#### 待测功能

- `runTestExecution(options: TestExecutionOptions): Promise<number>`: 检测 plan → 按 framework 执行 → 写报告；`projectRoot \|\| getProjectDir()`；`mutationDiffOnly` 时 git diff 过滤；日志含 passed/failed/skipped 计数文案

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runTestExecution — projectRoot / getProjectDir | 正向 | 显式 `projectRoot` 时 spy `getProjectDir` 次数 0 (AC-7) | 新增 |
| runTestExecution — projectRoot / getProjectDir | 边界 | 省略 / `""` 时调用 `getProjectDir` (AC-7) | 新增 |
| runTestExecution — projectRoot / getProjectDir | 异常 | 显式根不存在且 detect plan 为空：返回 0，日志含 `Configure tests in openspec/config.json`，不得未捕获抛错 | 新增 |
| runTestExecution — logResult 状态计数文案 | 正向 | mock `executePlanEntry` 返回 testCases：`passed`×2 + `failed`×1 + `skipped`×1 时，stdout **精确匹配**片段 `` `${n} tests, ` `` / `2 passed` / `1 failed` / `1 skipped`（status 字符串 `'passed'`/`'failed'`/`'skipped'` 被清空或改为 `!==` 比较则失败） | 新增 |
| runTestExecution — logResult 状态计数文案 | 异常 | status 为未知值（非 passed/failed/skipped）时不得计入三计数器；文案中对应计数保持 0（或忽略该项，行为明确） | 新增 |
| runTestExecution — logResult 状态计数文案 | 边界 | 全 passed / 全 failed / 空 testCases：计数均为 0 或全量，文案仍含三个状态词 | 新增 |
| runTestExecution — logSummary 文案 | 正向 | summary `conclusion: 'pass'` 时 stdout 含 `Summary: PASS`（`toUpperCase`）；并含 `Total:` / `Passed:` / `Failed:` / `Skipped:` / `Duration:` 精确标签 | 新增 |
| runTestExecution — logSummary 文案 | 正向 | `problems.length > 0` 时打印 `Problems (N):` 与 `[type] framework: message`；`problems: []` 时不得打印 Problems 块（杀死 `length > 0` → `>= 0` 突变） | 新增 |
| runTestExecution — logSummary 文案 | 异常 | `conclusion: 'fail'` 时 stdout 含 `Summary: FAIL`（不得误印 `PASS`）；标签行仍完整 | 新增 |
| runTestExecution — logSummary 文案 | 异常 | `problems` 含单条时打印 `Problems (1):` 且行格式为 `[type] framework: message`；缺 type/framework 时不得崩溃（或以空串占位，行为明确） | 新增 |
| runTestExecution — logSummary 文案 | 边界 | `total/passed/failed/skipped/duration_seconds` 均为 `0`：仍打印五条标签且数值为 `0`（证明 Number 边界与模板字面量） | 新增 |
| runTestExecution — mutationDiffOnly 路径过滤 | 正向 | `mutationDiffOnly: true` 且 `options.files` 含跨 directory 的 `../x.ts`：传给 `executePlanEntry` 的 `files` **过滤掉** `startsWith('..')` 项；同 directory 相对路径保留（`endsWith('..')` 突变不得误过滤） | 新增 |
| runTestExecution — mutationDiffOnly 路径过滤 | 异常 | `mutationDiffOnly: true` 时 stdout 含精确前缀 `--mutation-diff-only:` 与文件数量 | 新增 |
| runTestExecution — 无配置 / framework 过滤 | 正向 | plan 空：日志含 `Configure tests in openspec/config.json`（精确，非 `test.framework`）；返回 0 | 新增 |
| runTestExecution — 无配置 / framework 过滤 | 异常 | `--framework` 无匹配：日志含 `No plan entries found for framework "` + 名称 + `"`；返回 0 | 新增 |
| runTestExecution — 既有透传 | 正向 | 保留 noMutation / mutationDiffOnly / change 目录 / 多 framework 既有用例 | 新增 |
| runTestExecution — 既有透传 | 异常 | `noMutation: true` 时不得调用 mutation 执行路径；framework 名拼写错误时返回 0 且日志含无匹配文案 | 新增 |
| runTestExecution — 既有透传 | 边界 | `files: []` / 省略 files：与 detect 透传语义一致；`projectRoot: undefined` 回退 `getProjectDir` | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `runTestDetectFrameworks` / `executePlanEntry` / 报告生成 | stub 可控 plan 与 testCases | logResult / 主流程 |
| `getGitDiffFiles` | stub 文件列表 | mutationDiffOnly |
| `console.log` | spy 断言精确文案 | 日志 StringLiteral |
| `getProjectDir` | spy | projectRoot 回退 |

---

### `plugins/dev-team/bin/src/utils/constant.ts` -> `plugins/dev-team/bin/src/utils/constant.test.ts`

#### 待测功能

- （源文件删除）原 `getProjectDir()` 职责迁入 `lib/project-root.ts`；本测试文件删除 (AC-10)

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| getProjectDir — 无 MCP 缓存时 env/cwd 回退链 | 正向 | 缓存为 null 且 `CLAUDE_PROJECT_DIR` 已设置时返回该 env 值 | 废弃 |
| getProjectDir — 无 MCP 缓存时 env/cwd 回退链 | 边界 | 无 MCP 缓存且两 env 均未设置时返回 `process.cwd()` | 废弃 |
| getProjectDir — env 空字符串 | 边界 | `CLAUDE_PROJECT_DIR=""` 视为未设置 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| — | 文件删除；无需保留 mock | 废弃全部 |

---

### `plugins/dev-team/bin/src/utils/index.ts` -> `plugins/dev-team/bin/src/utils/index.test.ts`

#### 待测功能

- （源文件已删除）barrel 不再存在；command 层改为直接 `from '../lib/project-root'` 导入 `getProjectDir` (AC-10)

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| utils barrel — re-export getProjectDir | 正向 | 从 `utils/index` 导入与 `lib/project-root` 同一引用 | 废弃 |
| utils barrel — re-export getProjectDir | 异常 | 动态 import `./constant` 失败 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| — | 文件已删；改由 `project-root.test.ts` 断言 `utils/constant.ts` 路径不存在 + 源码无 `from './constant'` / `from '../utils/constant'` 残留 | 废弃全部 |

---

## 集成测试

> 框架：`vite-plus`（vitest）；集成测试文件扩展名 `.test.ts`，置于 `plugins/dev-team/bin/__tests__/`。

### env/roots/WORKSPACE 启动锁定 → MCP tool 入口强制校验 → `plugins/dev-team/bin/__tests__/mcp-project-root-lock/mcp-project-root-lock.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/project-root.ts` | 启动锁定 resolve/cache；`requireLockedProjectRoot` 抛结构化错误 |
| `plugins/dev-team/bin/src/mcp.ts` | connect 后 init；全部 tool handler 经 `withLockedProjectRoot` 映射 MCP 错误 |

**关联AC**: AC-1, AC-2, AC-5, AC-11

**关系描述**:

集成测试不 mock `project-root` 模块，通过真实 `connectToServer` + InMemoryTransport 验证：宿主侧 env / mock roots 决定锁定结果后，无参工具与有参工具在未锁定时一律失败且不向 `process.cwd()` 写副作用；唯一根可用时工具可正常执行且输出根等于锁定路径。变异存活的典型模式是：handler 空箭头、未锁定静默回退 cwd、仅有参工具校验、或多根取 `[0]`。集成侧抽样断言 `listTools` 精确 name（如 `change_list`）与未锁定时 `content[0].text` 以 `Project root is not locked:` 开头，并与单元精确断言互补。

#### 场景: 唯一根锁定后工具可用

前置：将 `CLAUDE_PROJECT_DIR`（或 WORKSPACE / mock roots 恰 1）指向临时项目根并 connect。输入：不传 `project_root` 调用 `change_list` / `phase_next`。预期：成功路径使用锁定根，错误路径 `isError` 且无主目录写入。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `CLAUDE_PROJECT_DIR` 指向 temp 项目时无参 `change_list` 成功：`project_root === dir`，且能列出该根下 change (AC-1) | 新增 |
| 正向 | roots 恰 1 且无 env 时锁定后：`change_list.project_root === dir` 且 `phase_next` 可读 change 目录 (AC-1/AC-5) | 新增 |
| 正向 | 仅 `WORKSPACE_FOLDER_PATHS` 单值时 `change_list.project_root` 严格等于该路径 (AC-1) | 新增 |
| 异常 | 三通道均失败时 `phase_log`/`backtrack`/`config_get` 均 `isError === true`，`content[0].type === 'text'`，text 以 `Project root is not locked:` 开头并含 `CLAUDE_PROJECT_DIR`；cwd 无副作用目录 (AC-2/AC-5) | 新增 |
| 边界 | `WORKSPACE_FOLDER_PATHS` 含 2 个绝对路径时工具失败，成功体不得回显 `[0]` (AC-2) | 新增 |
| 边界 | `${workspaceFolder}` 字面量 env 时 tool 错误可观测，不得成功 (AC-2) | 新增 |
| 边界 | 两 env 均为 `undefined` 且 roots 不可用时 `change_list`/`config_get`/`phase_next` 全部 `isError` (AC-2) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| MCP Client capabilities / listRoots | InMemory 客户端声明 roots；可控 roots 列表（不 mock project-root 业务函数） | roots 通道 |
| 临时文件系统 | `mkdtemp` 真实目录与 openspec 夹具 | 全部 |
| `process.env` | 隔离 CLAUDE / WORKSPACE | env 场景 |
| `vi.resetModules` | 每对连接前重置，避免锁定污染 | 全部 |

---

### MCP schema 删除 project_root → 锁定根驱动有参工具 → `plugins/dev-team/bin/__tests__/mcp-project-root-lock/mcp-project-root-lock.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/schemas/change-list.schema.ts`（及 config-get / archi_* / test_* schemas） | input 删除 `project_root`；`change_list` output 保留只读回显 |
| `plugins/dev-team/bin/src/mcp.ts` | 用 `requireLockedProjectRoot()` 结果调用 command |
| `plugins/dev-team/bin/src/commands/change-list.ts` | 接受 MCP 传入的锁定根字符串；output 回显 |

**关联AC**: AC-4, AC-6

**关系描述**:

验证 MCP 对外契约与 command 层注入分离：模型侧无法再通过 tool arguments 覆盖项目根，但锁定根仍能驱动 `runChangeList` / `runConfigGet` 等并在输出中回显为锁定路径。出错模式是 schema 仍可选接受 `project_root`、handler 仍优先 args，或输出回显变成 cwd。本关系用 `listTools` 确认 inputSchema 无 `project_root`，且成功 `change_list` JSON 必含回显字段。

#### 场景: 无参 change_list 回显锁定根

前置：锁定缓存指向 temp 项目（`dir !== cwd`）。输入：`change_list` arguments 为 `{}` 或误传 `project_root`。预期：JSON 中 `project_root` 严格等于锁定根；误传不得覆盖。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 无参 `change_list` 返回 `project_root ===` 锁定根，且 `count === changes.length`；顶层键含 `project_root`/`changes`/`count` (AC-6) | 新增 |
| 正向 | 无参 `config_get` 读锁定根配置成功；`listTools` 中 `config_get.inputSchema.properties` 无 `project_root` (AC-4) | 新增 |
| 异常 | 传入 `project_root: otherDir`：失败或忽略；若成功则回显锁定根≠otherDir (AC-4) | 新增 |
| 异常 | 未锁定无参 `change_list`：`isError`，不得成功回显 cwd (AC-6) | 新增 |
| 边界 | 锁定根≠cwd 时 `change_list`/`config_get` 均针对锁定根而非 cwd (AC-6) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 临时文件系统 | temp 含 config/changes；`otherDir` 为另一 temp | 全部 |
| `process.env` | 唯一可用 `CLAUDE_PROJECT_DIR`；未锁定场景删除两 env 并禁用 roots | 全部 |

---

### getProjectDir 命令链 → CLI 显式根与 MCP 锁定分离 → `plugins/dev-team/bin/__tests__/mcp-project-root-lock/mcp-project-root-lock.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/project-root.ts` | `getProjectDir` / 锁定缓存 |
| `plugins/dev-team/bin/src/commands/config-get.ts` | CLI `projectRoot \|\| getProjectDir()` |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | CLI `project_root` 注入 |
| `plugins/dev-team/bin/src/hooks.ts` | hooks 经 `getProjectDir` 读配置 |

**关联AC**: AC-7, AC-10

**关系描述**:

验证 CLI/hooks 路径在无 MCP 锁定时仍可通过显式根或 `getProjectDir` cwd/env 回退工作，与 MCP tool 强制锁定分离。突变风险是 command 层误接 `requireLockedProjectRoot` 导致 CLI 单测全挂，或 hooks 忽略显式项目根。本关系在同一临时项目上串联 `runConfigGet` / `runTestResolvePaths` / protect-files 解析，确认显式根生效且不要求 MCP 缓存。

#### 场景: CLI 显式根不依赖 MCP 锁定

前置：`vi.resetModules` 后不调用 `initProjectRootFromMcp`（缓存 null）；准备 temp 项目。输入：各 command 显式传入 temp 根。预期：成功读配置/解析路径；hooks 以 `getProjectDir` 或 env 指向 temp 时能匹配内置保护。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 无 MCP 缓存时 `runConfigGet({ key, projectRoot: dir })` 与 `runTestResolvePaths({ modules, project_root: dir })` 均成功 (AC-7) | 新增 |
| 正向 | 设置可用 `CLAUDE_PROJECT_DIR=dir` 后 hooks protect-files 对 `dir/openspec/config.json` 的 Write 返回 deny (AC-7/AC-10) | 新增 |
| 异常 | 无缓存且无显式根、env 不可用时 `getProjectDir() === process.cwd()`，但 MCP `change_list` 仍 `isError`（对照 AC-2/AC-7 分离） | 新增 |
| 边界 | 显式根≠cwd 时 CLI 读显式根配置，hooks 若 `getProjectDir` 被 stub 为显式根则保护命中该根路径 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 临时文件系统 | 共享 temp 项目 | 全部 |
| stdin / env | hooks 事件与 CLAUDE_PROJECT_DIR | hooks 路径 |
| InMemory MCP | 仅对照未锁定 MCP 失败 | 异常对照 |

---

## 不可测试项

- AC-8（Cursor 探针：`change_list` 返回工作区路径 / 断根报错）— **原因**: 依赖 Cursor 宿主注入 `WORKSPACE_FOLDER_PATHS` / roots 的真实 MCP 子进程；vitest 无法完整复现 Cursor 双进程与插件缓存路径。
- AC-9（Claude Code 探针：`CLAUDE_PROJECT_DIR` 生效）— **原因**: 依赖 Claude Code 宿主注入；单测已覆盖绝对存在路径锁定逻辑，宿主注入需手工探针。
- AC-11 中 `plugin.json` 版本升级 — **原因**: 清单字段变更，无运行时行为断言；实施时确认版本自基线递增。
- `plugins/dev-team/bin/dev-team-mcp.cjs` 重建 — **原因**: 构建产物一致性靠 `pnpm build`，不属于 vite-plus 用例表。
- 共享 `.mcp.json` 禁止 `${workspaceFolder}` — **原因**: 跨宿主配置约束，属「不要修改」与运维约定。
- 真正的 `notifications/roots/list_changed` 热刷新 — **原因**: proposal 明确不实现；旧用例已废弃。
- `plugins/dev-team/bin/src/schemas/*.schema.ts` 独立 colocated 单测路径 — **原因**: `test_resolve_paths` 报错 `Not in test config scope`；契约由 `mcp.test.ts` 与 `change-list.test.ts` / `config-get.test.ts` 覆盖。
- `mcp.ts` 的 `require.main === module` 启动日志分支 — **原因**: 已 `Stryker disable all`；stdio 入口不在 InMemory 覆盖范围。
- `project-root.ts` 中 `if (!lockFailureReason)` 防御性汇总分支（约 L331）— **原因**: 现有三通道失败路径均会写入 `lockFailureReason`，该分支在公共 API 下为 NoCoverage 死代码；强行覆盖需测试专用导出（违反「No test-only exports」）。
- `test-detect-frameworks.ts` 中 `generateShellScript`/`generateCmdScript` 对 `frameworkConfig === null` 的 TypeError 守卫 — **原因**: 公共 API 经 `getFrameworkConfig` 后难以传入 null；属防御性 NoCoverage，优先通过 script 字面量与 `{config_args}` 场景杀死相邻 StringLiteral/Conditional 突变。
- `utils/index.ts` barrel 单测 — **原因**: 实现已删除该文件；AC-10 改由 `project-root.test.ts` 断言 `getProjectDir` 归属与 `utils/constant.ts` 不存在覆盖。
