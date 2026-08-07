# 测试设计: cursor-home-image

> **日期**: 2026-08-07

---

## 策略说明

本变更验收以**手动构建与产物/安装目检**为主（AC-1～AC-5、AC-7 的产品侧断言）。**不要**为「完整 `vp pack` + assemble×3 后断言三产物树」编写自动化集成测试。

### 突变测试范围（用户收窄后）

| 项 | 值 |
|----|-----|
| includes | `bin/src/**/*.{ts,tsx}` |
| excludes | `bin/src/schemas/**/*`、`build/**/*` |
| 最近分数 | 69.96%（threshold 70%；gap ≈ 0.04pp；killed 748 / survived 290 / ignored 176） |
| 高存活文件 | `test-runner.ts`（129）、`hooks.ts`（57）、`workflow.ts`（51，多为 static）、`phase-next.ts`（29）、`stryker-config.ts`（24） |

本轮回溯目标：用**少量高杀伤力单元用例**把 mutation score 推过 70%。优先杀非 static 存活体；对 `workflow.ts` 的 static 存活体用精确 `agent_type` / 依赖表断言（运行时读表即可杀）。`build/**` 单测可保留作 AC-2/3/5 回归，但**不计入**本轮突变补强范围。

自动化范围：

| 自动化对象 | 目的 |
|-----------|------|
| `hooks.ts` protect-files | AC-6 + 杀 Shell/StrReplace 路由周边、bash/PowerShell 正则、`isRecord`、reason 字面量存活体 |
| `workflow.ts` | AC-2：`__AGENT:…__` 占位符与 DEFAULT_WORKFLOW / 依赖表精确断言 |
| `test-runner.ts` | 杀 clearDirectory / placeholder / redirect / ExecError 等高存活分支 |
| `stryker-config.ts` | 杀盘符路径、空路径、vitest/jest overlay 存活体 |
| `phase-next.ts` | 杀 interpolate / done message / backtrack / retry 过滤存活体 |
| `build/*`（突变范围外） | AC-2/3/5 纯函数回归；不为本轮 mutation 加用例 |

测试框架：`test_detect_frameworks` → **vite-plus**（`cwd: plugins/dev-team`）。

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------------|
| AC-1 | 手动执行插件 build 后，`claude-plugins/dev-team`、`cursor-plugins/dev-team`、`cursor-home-image/dev-team` 均存在且 version 与 `plugins/dev-team/package.json` 一致；staging 含未展开 token 的 CJS；三产物内名称类 token 无残留（`cursorHome` 可残留路径 token） | 手动验收 | `pnpm -C plugins/dev-team run build` 产物树（见「不可测试项」） |
| AC-2 | 源码 skills/agents/bin 字符串使用 `__<KIND>:<id>__`；plugin 产物展开为现网插件前缀；home 产物 skill/agent/bin 带 `dev-team_`，MCP 为 `mcp__user-dev-team_mcp__…`；无 MCP 服务器级无 tool id 引用 | 单元测试 + 手动验收 | `plugins/dev-team/bin/src/lib/workflow.ts`；`plugins/dev-team/build/apply-env-tokens.ts`（突变范围外）；产物前缀目检（手动） |
| AC-3 | Claude/Cursor 插件产物为 `claudeNested` 落 `hooks/hooks.json`；home 为 `cursorNative` 落根 `hooks.json`；源码不为 Claude 成品 JSON 权威 | 单元测试 + 手动验收 | `plugins/dev-team/build/hooks-profile.ts`（突变范围外）；产物 hooks 路径目检（手动） |
| AC-4 | 镜像含前缀化 skills/agents、`bin`/`templates`/`utils`、`install.mjs`、`manifest.json`、mcp merge 片段；路径 token 仍为 `__DEV_TEAM_*__` | 手动验收 | `cursor-home-image/dev-team/`（见「不可测试项」） |
| AC-5 | 对干净或已有用户内容的 `~/.cursor`（或 `--root`）执行 `install.mjs` 后：路径已绝对化；仅 upsert/删除 `dev-team_` 托管 hooks/mcp 项；用户其它 server/条目保留；Reload 提示可见 | 单元测试 + 手动验收 | `plugins/dev-team/build/home-install.mjs`（突变范围外）；实机/`--root` 安装（手动） |
| AC-6 | 单测：stdin `tool_name` 为 `Shell` / `StrReplace` 时与 `Bash` / `Edit` 等价保护行为；既有 Write/Edit/Bash/PowerShell 用例仍通过 | 单元测试 | `plugins/dev-team/bin/src/hooks.ts` |
| AC-7 | `cursor-home-image/` 未被 gitignore；与双产物一样可 commit，clone 后可装 | 手动验收 | `.gitignore` + git 状态（见「不可测试项」） |

---

## 单元测试

### `plugins/dev-team/bin/src/hooks.ts` -> `plugins/dev-team/bin/src/hooks.test.ts`

#### 待测功能

- `runProtectFiles(): void`: PreToolUse 入口；读 stdin JSON，按 `tool_name` 路由保护逻辑并写 stdout 决策
- `runStaticCheck(): void`: SubagentStop 入口；解析 `workspace_roots` 后跑静态检查
- `captureStderr(): [() => string, () => void]`: 捕获 stderr 文本
- `main(): void`: 按 `process.argv[2]` 分发子命令
- （行为）`Shell` 与 `Bash` 共用 `checkBashCommand`；`StrReplace` 与 `Edit`/`Write` 共用 `matchProtectedPath`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `runProtectFiles` / Shell | 正向 | `tool_name: 'Shell'` 且 `command` 含 `> openspec/changes/test/eval.json` 时 `permissionDecision` 为 `deny`，与同等 `Bash` 用例一致 | 新增 |
| `runProtectFiles` / Shell | 正向 | `tool_name: 'Shell'` 且 `>>` 追加到受保护路径时 `deny`（杀 `{1,2}`→单次 `>` 正则存活体） | 新增 |
| `runProtectFiles` / Shell | 正向 | `tool_name: 'Shell'` 且 command 以 `> openspec/changes/test/eval.json` 开头（无前置命令）时 `deny`（杀 `^|[^-]` 锚点弱化存活体） | 新增 |
| `runProtectFiles` / Shell | 正向 | `tool_name: 'Shell'` 且 `echo x >\| openspec/changes/test/eval.json`（noclobber）时 `deny`；断言 `permissionDecisionReason` 含路径且含 `detected via`（杀 reason 内工具名/空串存活体） | 新增 |
| `runProtectFiles` / Shell | 正向 | `tool_name: 'Shell'` 且 `echo x \| tee -a openspec/changes/test/eval.json` 时 `deny`（杀 tee 可选 flag 组被掏空的存活体） | 新增 |
| `runProtectFiles` / Shell | 正向 | `tool_name: 'Shell'` 且 `echo x >& openspec/changes/test/eval.json`（`>&` 后零或多个空白）时 `deny` | 新增 |
| `runProtectFiles` / Shell | 正向 | `tool_name: 'Shell'` 且 command 为只读（如 `cat openspec/changes/test/eval.json`）时返回 `allow` | 新增 |
| `runProtectFiles` / Shell | 正向 | `tool_name: 'Shell'` 且 command 以 `python` / `python3` / `node` + 空白开头写入受保护路径时返回 `allow`（杀豁免正则去掉 `^` 或 `\s` 的存活体） | 新增 |
| `runProtectFiles` / Shell | 异常 | `tool_name: 'Shell'` 但 `tool_input.command` 缺失 / 非字符串时 fail-open 返回 `allow` | 新增 |
| `runProtectFiles` / Shell | 边界 | `tool_name: 'Shell'` 且 `command` 为 `""` 时返回 `allow` | 新增 |
| `runProtectFiles` / Shell | 边界 | `tool_name: 'Shell'` 且 `command` 为超长字符串（>1000 chars）含受保护路径重定向时仍 `deny` 且不崩溃 | 新增 |
| `runProtectFiles` / Shell | 边界 | `tool_name: 'Shell'` 且 command 含特殊字符（`\n`、emoji）与受保护路径时行为与 Bash 一致且不崩溃 | 新增 |
| `runProtectFiles` / Shell | 边界 | `command` 为 `echo x>openspec/changes/test/eval.json`（`>` 后无空白）时 `allow`（杀 `\s+`→`\s` 误匹配存活体：正确实现要求空白） | 新增 |
| `runProtectFiles` / Shell | 边界 | `command` 含 `-> openspec/changes/test/eval.json`（箭头而非重定向）时 `allow`（杀去掉 `[^-]` 负向约束的存活体） | 新增 |
| `runProtectFiles` / StrReplace | 正向 | `tool_name: 'StrReplace'` 且 `file_path` 为受保护路径时 `deny`，与同等 `Edit` 用例一致；`permissionDecisionReason` 含字面量 `StrReplace`（杀 `%t` 替换为空串存活体） | 新增 |
| `runProtectFiles` / StrReplace | 正向 | `tool_name: 'StrReplace'` 且 `file_path` 为未受保护路径时 `allow` | 新增 |
| `runProtectFiles` / StrReplace | 异常 | `tool_name: 'StrReplace'` 但缺失 `file_path` 或非字符串时 fail-open 返回 `allow` | 新增 |
| `runProtectFiles` / StrReplace | 边界 | `tool_name: 'StrReplace'` 且 `file_path` 为 `""` 时返回 `allow`（杀 `isProtected` 空路径条件被恒 false 的存活体） | 新增 |
| `runProtectFiles` / StrReplace | 边界 | `file_path` 为超长路径（>1000 chars）匹配保护 glob 时仍 `deny` 且不崩溃 | 新增 |
| `runProtectFiles` / StrReplace | 边界 | `file_path` 含 `\` / `/` 混用时匹配行为与 `Edit` 一致 | 新增 |
| `runProtectFiles` / isRecord | 边界 | `tool_input` 为数组且额外挂上 `file_path` 属性指向受保护路径时仍 `allow`（杀 `isRecord` 去掉 `!Array.isArray` 导致数组被当 record 的存活体） | 新增 |
| `runProtectFiles` / isRecord | 边界 | `tool_input` 为 `null` / 原始字符串 / 数字时 `allow`（配合 Shell/Write） | 新增 |
| `runProtectFiles` / PowerShell | 正向 | `tool_name: 'PowerShell'` 且 `2> openspec/changes/test/eval.json` 或 `*> …` 时 `deny`（杀 `\d*` / `\*` 正则存活体） | 新增 |
| `runProtectFiles` / PowerShell | 正向 | `python3` / `node` 开头写入受保护路径时 `allow`；非行首 `python …` 不豁免 | 新增 |
| `runProtectFiles` / 路由 | 异常 | `tool_name` 为未知字符串且 `tool_input.command` 为 PowerShell 写受保护路径时仍 `allow`（杀 `toolName === 'PowerShell'` 被替换为恒 `true` 的存活体） | 新增 |
| `runProtectFiles` / 既有工具名 | 正向 | 既有 `Write` / `Edit` / `Bash` / `PowerShell` 受保护用例仍为 `deny`；`Write` 写 `openspec/config.json` 时 reason 含内置中文模板关键句（杀 L66 空 reason 存活体） | 新增 |
| `runProtectFiles` / 既有工具名 | 边界 | `tool_name` 为未知字符串时 fail-open `allow` | 新增 |
| `runProtectFiles` / loadPatterns | 边界 | `readConfig` 返回无 `write_protection.files`（`undefined`）时仅内置 glob 生效；对非内置路径 `allow`（杀 `?? []` 被替换为非空假数组的存活体） | 新增 |
| `runProtectFiles` / evaluateToolAccess | 边界 | stdin 为仅空白 / 仅 `\t\n` 时 `allow`（杀 `!raw \|\| !raw.trim()` 逻辑算子存活体） | 新增 |
| `runProtectFiles` / evaluateToolAccess | 异常 | `tool_name` 为 `0` / `false` 等非字符串 truthy 时 `allow`（杀 `!toolName` 逻辑弱化存活体） | 新增 |
| `runStaticCheck` / parseWorkspaceRoot | 正向 | `workspace_roots: ['D:\\\\proj']` 时使用该 root（POSIX 化）调用分析 | 新增 |
| `runStaticCheck` / parseWorkspaceRoot | 边界 | `workspace_roots: []` / 非数组 / `[123]` / `['']` 时回退 `getProjectDir()`（杀 `length > 0`→`>= 0` 与类型守卫存活体） | 新增 |
| `captureStderr` | 正向 | 写入 string 与 `Uint8Array` 后 `getCaptured()` 拼接为完整文本；`restore` 后不再捕获（杀 `chunks.join('')` 分隔符被改写的存活体） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `node:fs` `readFileSync`（fd `0` / stdin） | 沿用现有 `hooks.test.ts`：`vi.mock` 后 `mockReturnValue` 注入 PreToolUse / SubagentStop JSON | 全部 `runProtectFiles` / `runStaticCheck` 用例 |
| `readConfig` / 项目根 | 固定无自定义 `write_protection` 或显式 `files: []`；需要用户 pattern 时再注入 | Shell/StrReplace/loadPatterns |
| `process.stdout.write` | 捕获 stdout 再 `JSON.parse`；对 deny 用例断言 `permissionDecisionReason` 子串而非仅 decision | 全部决策断言 |
| `runStaticAnalysis` | mock 返回 exitCode 0/非 0 | `runStaticCheck` |

---

### `plugins/dev-team/bin/src/lib/workflow.ts` -> `plugins/dev-team/bin/src/lib/workflow.test.ts`

#### 待测功能

- `getPhaseTable(workflowType: string): PhaseDefinition[]`: 按 workflow 返回阶段表；未知 key 回落 `DEFAULT_WORKFLOW`（`'requirement'`）
- `getDependents(phaseId: string, workflowType: string): string[]`: 由前置依赖表反查下游阶段

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `getPhaseTable` / agent tokens | 正向 | `requirement` 全阶段非 null 的 `executor.agent_type` / `evaluator.agent_type` 精确等于 `__AGENT:<logical-id>__` 清单（含 `proposal-planner`、`proposal-evaluator`、`dev-design-planner`、`dev-design-evaluator`、`test-design-planner`、`test-design-evaluator`、`implementation-generator`、`implementation-evaluator`、`test-gen-generator`、`test-gen-evaluator`、`test-execution-executor`、`test-execution-evaluator`、`code-review-evaluator`、`acceptance-evaluator`）（杀 static 空串 / 表项被掏空存活体） | 新增 |
| `getPhaseTable` / agent tokens | 正向 | `test-only` 的 `code-analyze` / `test-design` / `test-gen` / `test-execution` agent_type 精确等于对应 `__AGENT:…__` | 新增 |
| `getPhaseTable` / agent tokens | 正向 | `bug-fix` / `refactor` 各阶段 agent_type 与 design 一致，且全部匹配 `/^__AGENT:[a-z0-9-]+__$/`（禁止空串或非占位符） | 新增 |
| `getPhaseTable` / explore handoff | 正向 | `requirement` 与 `test-only` 的 proposal executor prompt 同时包含 `explore.md`、`merge`、`Do not expect inline EXPLORE_CONTEXT_SUMMARY`（杀 `PROPOSAL_EXPLORE_HANDOFF` 分段空串存活体） | 新增 |
| `getPhaseTable` / DEFAULT_WORKFLOW | 正向 | `getPhaseTable('UNKNOWN')` / `getPhaseTable('')` 的 phase id 列表与 agent_type 列表均深度等于 `getPhaseTable('requirement')`（杀 `DEFAULT_WORKFLOW` 空串导致回落失败的存活体） | 新增 |
| `getPhaseTable` | 正向 | 四张表 key：`requirement` / `bug-fix` / `refactor` / `test-only` 均非空；`refactor` id 序列等于 `requirement` | 新增 |
| `getDependents` | 正向 | `requirement`：`proposal`→`['dev-design','test-design','acceptance']`；`dev-design`→`['test-design','implement','acceptance']`；`test-design`→`['test-gen']`；`implement`→含 `test-gen`/`test-execution`/`code-review`/`acceptance`；叶子阶段 `[]` | 新增 |
| `getDependents` | 正向 | `bug-fix`：`proposal`→含 `dev-design`；`implement`→含 `test-execution`/`code-review`；`acceptance` 依赖链含 `code-review`（杀 bug-fix 前置表被清空的存活体） | 新增 |
| `getDependents` | 正向 | `test-only`：`proposal`→`['code-analyze','test-design']`；`code-analyze`→`['test-design']`；`test-design`→`['test-gen']`；`test-gen`→`['test-execution']` | 新增 |
| `getDependents` | 异常 | 未知 phase / 空 phase → `[]` | 新增 |
| `getDependents` | 边界 | 空/`UNKNOWN` workflowType 回落 requirement 依赖结果 | 新增 |
| `getDependents` | 边界 | 对 `requirement` 每个 phase id，用「逆映射」重算 prerequisites 与表一致（杀单元素前置数组被替换为 `[]` 或假元素的存活体） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 直接调用导出函数；不断言 assemble 展开后的最终 agent 名 | 全部 |

---

### `plugins/dev-team/bin/src/lib/test-runner.ts` -> `plugins/dev-team/bin/src/lib/test-runner.test.ts`

#### 待测功能

- `executePlanEntry(…): ExecutionResult`: 准备占位符、清 reportDir、拼命令、exec、解析结果/coverage/mutation

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `executePlanEntry` / clearDirectory | 正向 | 既有 planDir 含旧文件时执行前被清空；兄弟 plan 目录不受影响（杀 `existsSync` 恒 true / `rmSync` `recursive:false` 存活体） | 新增 |
| `executePlanEntry` / clearDirectory | 边界 | planDir 原本不存在时仍 `mkdirSync` 成功且可写结果（杀跳过 mkdir 的存活体） | 新增 |
| `executePlanEntry` / cleanupTempPaths | 正向 | bun 临时 bunfig 在结束后删除；用户 `bunfig.toml` 不变（杀 cleanup `existsSync` 恒 true / `force:false` 存活体） | 新增 |
| `executePlanEntry` / placeholders | 正向 | 捕获 cmd：`results_file` / `coverage_file` / `coverprofile_file` / `mutation_file` / `report_dir` 均为非空绝对 POSIX 路径且文件名精确（`results.json`/`coverage-summary.json`/`coverage.out`/`mutation.json`）（杀空串文件名存活体） | 新增 |
| `executePlanEntry` / config_args | 正向 | 无 suite config 时剥离 `{config_args}` 不留双空格；有 config 时替换为带引号绝对路径（杀 `/\s*\{config_args\}/g` 与替换串存活体） | 新增 |
| `executePlanEntry` / redirect | 正向 | SHELL 下 go：重定向插在 `go test…` 与 `;` 之间；win32 cmd：插在 `&` 之前；精确子串匹配（杀 go redirect 正则存活体） | 新增 |
| `executePlanEntry` / redirect | 正向 | SHELL 下 pytest：重定向在第一段 `;` 之前；win32：在 `&&` 之前（杀 pytest 分隔符正则存活体） | 新增 |
| `executePlanEntry` / redirect | 正向 | rust：`cargo test > "…"` 后仍保留 llvm-cov 段（杀 `cargo test\b` 正则存活体） | 新增 |
| `executePlanEntry` / ExecError | 异常 | `execSync` 抛 `null` / 原始字符串 / `{stdout: Buffer, stderr: Buffer, status: 1}` 时均不崩溃；Buffer 被解码为 string 写入 error/解析路径（杀 `isExecError` 与 Buffer 分支存活体） | 新增 |
| `executePlanEntry` / mutation | 正向 | 启用 mutation 时只读 `reportDir/mutation.json`；临时 stryker config 删除 | 新增 |
| `executePlanEntry` | 异常 | 空命令 → `exitCode=-1` 且 error 含 `Empty test command` | 新增 |
| `executePlanEntry` | 边界 | `timeout: 0` / `-1` / 省略（默认 60000）原样或默认传递 | 新增 |
| `executePlanEntry` | 边界 | `files=[]` 且 `scope='.'` → `{files}` 空串；`scope=src` → 回落 `src` | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `node:child_process` `execSync` | `vi.mock` 捕获 cmd/options，可抛 ExecError | 全部 |
| `fs` 临时目录 | `mkdtemp` 作为 projectRoot / reportsDir | clearDirectory / cleanup |
| `readConfig` / framework 表 | 固定 suite 与 mutation_execution | mutation / config_args |

---

### `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` -> `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts`

#### 待测功能

- `resolveStrykerConfig(…): { configPath; cleanup; … }`: 生成临时 Stryker JSON（mutate 路径归一、runner overlay、`jsonReporter.fileName`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `resolveStrykerConfig` / normalize | 正向 | 相对路径 `src/foo.ts` → mutate 为正斜杠相对路径 | 新增 |
| `resolveStrykerConfig` / normalize | 正向 | 绝对路径位于 root 下 → 相对 POSIX；输入含 `\` 时输出仅 `/` | 新增 |
| `resolveStrykerConfig` / normalize | 边界 | `sourceFiles` 含 `''` 时 mutate 保留空串条目且不抛（杀空串早退 block 被掏空 / 返回假串的存活体） | 新增 |
| `resolveStrykerConfig` / normalize | 边界 | 形如 `C:/abs/file.ts` 的盘符路径在非 win `path.isAbsolute` 为 false 时仍走盘符分支并相对化（杀 `/^[A-Za-z]:/` 与 `\|\|`→`&&` 存活体） | 新增 |
| `resolveStrykerConfig` / overlay | 正向 | `frameworkConfigPath` 为盘符绝对路径时 vitest overlay 仅含 `configFile`（相对 projectRoot、正斜杠） | 新增 |
| `resolveStrykerConfig` / overlay | 正向 | jest + `planRoot: '.'` → `testMatch: ['**/*.test.ts?(x)']`；`planRoot: 'src'` → `**/src/**/*.test.ts?(x)`；`planRoot: './pkg'` 先剥 `./`（杀 `^\.\/` 与 `normalizedPlan === '.'` 存活体） | 新增 |
| `resolveStrykerConfig` / overlay | 边界 | 未传 `frameworkConfigPath` → 无 vitest/jest 块 | 新增 |
| `resolveStrykerConfig` | 正向 | 写入 JSON：`ignoreStatic: true`、`timeoutMS: 10000`、`jsonReporter.fileName` 等于 `reportDir/mutation.json` 绝对 POSIX；`utf-8` 可读回 | 新增 |
| `resolveStrykerConfig` | 异常 | `framework` 为 `bun`/`pytest`/`go`/`''` 时抛错且 message 列出 `jest, vitest, vite-plus` | 新增 |
| `resolveStrykerConfig` | 边界 | `sourceFiles=[]` 仍生成配置；多文件顺序保留 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时 sandbox | `mkdtemp` 作 projectRoot；测后删除 temp config | 全部 |
| `path.isAbsolute` | 不 mock；用盘符字符串在当前平台覆盖 `\|\| /^[A-Za-z]:/` 分支 | 盘符用例 |

---

### `plugins/dev-team/bin/src/commands/phase-next.ts` -> `plugins/dev-team/bin/src/commands/phase-next.test.ts`

#### 待测功能

- `runPhaseNext(options: PhaseNextOptions): PhaseNextResult`: 计算下一阶段、插值 prompt、处理 backtrack / retry / done

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `runPhaseNext` / agent tokens | 正向 | 首跑 `executor.agent_type` 精确为 `__AGENT:proposal-planner__`，`evaluator.agent_type` 精确为 `__AGENT:proposal-evaluator__`（与 workflow 表一致；杀空串/假串） | 新增 |
| `runPhaseNext` / interpolate | 正向 | executor/evaluator prompt 中 `<change>` 被替换为实际 change；evaluator 中 `<phase>` 被替换为 phase id；断言无残留 `<change>`/`<phase>`（杀 `interpolatePrompt` 跳过 phase 替换的存活体） | 新增 |
| `runPhaseNext` / explore | 正向 | proposal executor prompt 含 `explore.md` 与 merge 语义 | 新增 |
| `runPhaseNext` / done | 正向 | 全部 phase pass 时 `done: true`，`message` 精确等于 `All phases have passed evaluation. Ready for archiving.`（杀 message 空串存活体） | 新增 |
| `runPhaseNext` / done | 正向 | done 响应 `error: null` 且 `last_result` 在未提供时为 `null`（杀 `lastResult ?? null` → `lastResult && null` 存活体） | 新增 |
| `runPhaseNext` / error | 异常 | max retries 时 `done: false`（非 true）、`error: 'max_retries_exceeded'`（杀 error 响应 `done: true` 存活体） | 新增 |
| `runPhaseNext` / backtrack | 正向 | 最新条目 `backtrack_to: ''` 视为无回溯，继续正常推进（杀 `!== ''` 被弱化存活体） | 新增 |
| `runPhaseNext` / backtrack | 正向 | `backtrack_reason: null` / 缺失字段 → prompt 不拼 `⚠️ 回溯原因`；`reason: ''` 仍拼接（当前 truthy 语义） | 新增 |
| `runPhaseNext` / backtrack | 异常 | 无效 backtrack 目标 → `invalid_backtrack_target`，message 含目标 JSON（杀 message 模板空串存活体） | 新增 |
| `runPhaseNext` / retry | 异常 | 连续 5 次同 phase `verdict: 'fail'` 触发 max retries；同 phase 但 `verdict: 'pass'` 不计入 fail 次数（杀 `phase===id \|\| verdict==='fail'` 存活体） | 新增 |
| `runPhaseNext` / hasPhasePassed | 正向 | `skipped: true` 且非 stale 视为已通过；`stale: true` 的 pass 忽略 | 新增 |
| `runPhaseNext` | 异常 | `options.change` 为 `''` / 缺失时抛错或返回校验错误（杀 change 空串守卫存活体） | 新增 |
| `runPhaseNext` | 边界 | 回溯 reason 长度 500 时 prompt 完整包含 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `eval.json` / change 目录 | 内存或临时目录写入 entries | 全部 |
| `workflow.getPhaseTable` | 默认真实实现（保留 token 断言） | agent tokens |

---

### `plugins/dev-team/build/apply-env-tokens.ts` -> `plugins/dev-team/build/apply-env-tokens.test.ts`

> 突变范围外（`excludes: build/**/*`）。保留作 AC-2 回归，不为本轮 mutation 加杀伤用例。

#### 待测功能

- `applyEnvTokens(text: string, env: ProductEnv, options?: { pathTokens?: boolean }): string`: 展开名称类 / 路径 token

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `applyEnvTokens` | 正向 | `claude`/`cursor`：`__MCP:phase_log__` → `mcp__plugin_dev-team_dev-team__phase_log` | 新增 |
| `applyEnvTokens` | 正向 | `cursorHome`：`__MCP:phase_log__` → `mcp__user-dev-team_mcp__phase_log`；`__SKILL:`/`__BIN:`/`__AGENT:`/`__SKILL_SLASH:` 按 design 表展开 | 新增 |
| `applyEnvTokens` | 正向 | `cursorHome` + `{ pathTokens: false }`：名称展开、路径 token 保留 | 新增 |
| `applyEnvTokens` | 异常 | `text`/`env` 为 `null`/`undefined` 时抛错或确定性拒绝 | 新增 |
| `applyEnvTokens` | 边界 | `""` / 超长 / 特殊字符 / 残缺占位 / 无 token 原样返回 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `ProductEnv` 夹具 | `getEnv(...)` 或最小手写 env | 全部 |

---

### `plugins/dev-team/build/env.ts` -> `plugins/dev-team/build/env.test.ts`

> 突变范围外。

#### 待测功能

- `getEnv(key: ProductEnvKey): ProductEnv`
- `PRODUCT_ENV_KEYS`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `getEnv` | 正向 | 三行 key/`mcpToolPrefix`/`namePrefix`/`hooksProfile`/`pathReplacePhase`/`outDir` 与 design 锁定表一致 | 新增 |
| `PRODUCT_ENV_KEYS` | 正向 | 长度为 3 且仅含 `claude`/`cursor`/`cursorHome` | 新增 |
| `getEnv` | 异常 | 非法 key / `null` / `""` 抛错 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 直接断言常量表 | 全部 |

---

### `plugins/dev-team/build/hooks-profile.ts` -> `plugins/dev-team/build/hooks-profile.test.ts`

> 突变范围外。

#### 待测功能

- `buildHooksFile(canonical: HooksCanonical, env: ProductEnv): string`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `buildHooksFile` | 正向 | `claudeNested` nested 形态；`cursorNative` 含 `Shell`/`StrReplace` matcher | 新增 |
| `buildHooksFile` | 正向 | SubagentStop matcher = `namePrefix + agentLogicalId`；`__BIN:hooks__` 展开 | 新增 |
| `buildHooksFile` | 正向 | `cursorHome` 路径 token 可保留 | 新增 |
| `buildHooksFile` | 异常 | `canonical`/`env` 为 `null` 抛错 | 新增 |
| `buildHooksFile` | 边界 | 空事件数组产出合法 JSON；两 profile 结构字段不同 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `HooksCanonical` 夹具 | 最小 JSON | 结构用例 |

---

### `plugins/dev-team/build/assert-no-tokens.ts` -> `plugins/dev-team/build/assert-no-tokens.test.ts`

> 突变范围外。

#### 待测功能

- `assertNoNameTokens(rootDir: string, env: ProductEnv): void`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `assertNoNameTokens` | 正向 | 无名称 token 不抛；`cursorHome` 允许路径 token | 新增 |
| `assertNoNameTokens` | 异常 | 残留 `__MCP:…__` 抛错；`rootDir` 不存在抛错 | 新增 |
| `assertNoNameTokens` | 边界 | 空目录不抛；plugin 残留路径 token 失败 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时目录 | `mkdtemp` fixture | 全部 |

---

### `plugins/dev-team/build/scan-files.ts` -> `plugins/dev-team/build/scan-files.test.ts`

> 突变范围外。

#### 待测功能

- 宽 include / 窄 exclude 枚举待替换文本文件（导出名以实现为准）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `scanFiles` | 正向 | skills/agents/bin/templates/utils 文本被枚举 | 新增 |
| `scanFiles` | 异常 | `rootDir` 不存在 / `null` 行为确定 | 新增 |
| `scanFiles` | 边界 | 排除 `openspec-bundled.js`、`.map`；空目录 `[]` | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时目录 fixture | `mkdtemp` | 全部 |

---

### `plugins/dev-team/build/home-install.mjs` -> `plugins/dev-team/build/home-install.test.mjs`

> 突变范围外。

#### 待测功能

- `expandHomePathTokens` / `mergeManagedHooks` / `mergeManagedMcp`（或等价命名导出）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `expandHomePathTokens` | 正向 | 双路径 token → 绝对 root；非 JSON `\`→`/` | 新增 |
| `mergeManagedHooks` | 正向 | 保留用户项、更新/删除托管项 | 新增 |
| `mergeManagedMcp` | 正向 | 仅动 `dev-team_` 前缀 server；保留其它 | 新增 |
| 上述 | 异常/边界 | `null`/`{}`/空串确定性行为 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 纯 JSON 夹具 | 内存对象；不写 `~/.cursor` | merge |

---

## 集成测试

<!-- 本变更不设计自动化集成测试：assemble×3、三产物树、home 镜像布局、installer 实装均改为手动验收（见下节）。
     `applyEnvTokens`↔`getEnv`、`buildHooksFile`↔`applyEnvTokens` 的组合在对应单元测试中用真实函数调用覆盖，不另建 `__tests__/` 集成套件。 -->

---

## 不可测试项

> 下列项**刻意不**做成自动化「构建产物树」测试；验收时按步骤手动执行并勾选。

### 手动验收：三产物构建（AC-1、AC-2 产物侧、AC-3 落盘路径）

1. 执行 `pnpm -C plugins/dev-team run build`。
2. 确认存在：
   - `claude-plugins/dev-team/`
   - `cursor-plugins/dev-team/`
   - `cursor-home-image/dev-team/`
3. 三处 version（plugin.json / manifest.json）与 `plugins/dev-team/package.json` 的 `version` 一致。
4. 确认 `plugins/dev-team/.pack-staging/bin/*.cjs`（或约定 staging 路径）仍含未展开名称类 token。
5. 在三产物内检索名称类 token（`__SKILL:` / `__AGENT:` / `__MCP:` / `__SKILL_SLASH:` / `__BIN:`）应无残留；`cursor-home-image` 允许 `__DEV_TEAM_ROOT__` / `__DEV_TEAM_RUNTIME_ROOT__`。
6. plugin 产物：hooks 位于 `hooks/hooks.json`（`claudeNested`）；bin 为 `mcp.cjs` / `cli.cjs` / `hooks.cjs`。
7. home 产物：根 `hooks.json`（`cursorNative`）；bin 为 `dev-team_*.cjs`；MCP 前缀抽样为 `mcp__user-dev-team_mcp__`。

- **原因**: proposal / 用户指引明确禁止为完整 build 产物树设计自动化断言；构建断言已在 assemble 进程内以 `assertNoNameTokens` 等方式 fail-fast，外加目检即可。

### 手动验收：cursor-home-image 布局（AC-4）

1. 检查 `cursor-home-image/dev-team/` 含：前缀化 `skills/`、`agents/`、`bin/`、`templates/`、`utils/`、`hooks.json`、`mcp.dev-team.json`、`install.mjs`、`manifest.json`。
2. 抽样打开 hooks/mcp/agents，确认路径仍为 `__DEV_TEAM_*__` token。

- **原因**: 布局是 assemble 文件系统产物，适合目检而非再写一套树快照测试。

### 手动验收：安装器（AC-5）

1. 准备临时目录作为 `--root`（干净树 + 含用户自定义 `hooks.json`/`mcp.json` 的脏树各一次）。
2. 执行 `node cursor-home-image/dev-team/install.mjs --root <tmpdir>`。
3. 确认路径 token 已变为该 root 的绝对路径；托管 hooks/mcp 已 upsert；用户其它条目仍在；stdout 含 Reload Window 提示；进程 exit `0`。

- **原因**: 依赖本机目录与用户内容共存；merge 规则已由纯函数单测覆盖，端到端安装保持手动。

### 手动验收：产物可提交（AC-7）

1. 确认 `.gitignore` 忽略 `.pack-staging/`（或等价），**不** ignore `cursor-home-image/`。
2. `git check-ignore -v cursor-home-image/dev-team/manifest.json`（或等价）应显示未被忽略；变更可加入提交。

- **原因**: gitignore / 可提交性属于仓库策略，用一次命令目检即可，无需测试套件。

### 不自动化的编排模块

- `plugins/dev-team/build/assemble.ts`（`assemble` / `assembleAll`）— **原因**: 编排复制/rename/写盘即「产物树验证」；按指引不设计自动化套件。
- `plugins/dev-team/vite.config.ts` 的 pack×1 + `build:done` 接线 — **原因**: 由手动 build 冒烟确认；不为 staging/outDir 树写集成测试。
- 源码占位符化全量（skills/agents 海量 md）— **原因**: 由 assemble 宽扫 + 构建断言 + 抽样目检保证；不为每个 md 写快照测试。
