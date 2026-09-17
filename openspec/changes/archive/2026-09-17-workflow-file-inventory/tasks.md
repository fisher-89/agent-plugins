# 任务: workflow-file-inventory

> 依赖顺序：基础层（schema/共享库）→ session 注册表 → hooks 层 → `change_files` 工具 → 消费方改造 → git 退场 → 提示词/模板层 → 版本与构建。测试用例由独立的 test 工作流阶段产出，此处不含任何测试任务。

## Phase 1: 清单基础层

- [x] 扩展 `plugins/dev-team/bin/src/schemas/workflow.schema.ts`：`workflowFileSchema` 增加可选 `files` 字段（`written: string[]`、`deleted: string[]`、`source?: Record<string, string>`），更新模块级 JSDoc（`files` 由 `change_create` 初始化、消费方对缺失硬报错）
- [x] 新增 `plugins/dev-team/bin/src/lib/file-inventory.ts`：定义 `FileInventory` / `FileOp` 类型；`readFileInventory(changeDir)`（workflow.json 读取 + schema 校验 + 缺 `files` 抛"该 change 创建于文件清单机制之前，请重建"硬错误）；`foldFileOps(inventory, ops)` 纯函数（write → `deleted -= P ; written += P`、delete → `written -= P ; deleted += P`、revert → 双移出；`source` 维护：带 `agentType` 覆写、不带清除、条目移除随之移除）；`writeFileInventory(changeDir, files)`（沿用 `eval-json` 写盘纪律：文件必须已存在否则抛错、保留 `workflow_type`/`created`/`eval`/未知键、2 空格缩进 + 末尾换行）
- [x] 修改 `plugins/dev-team/bin/src/commands/change-create.ts`：新建 `workflow.json` 写入 `files: { written: [], deleted: [] }`（键序 `workflow_type` → `created` → `files`；不写 `eval`、不建 `eval.json` / `.openspec.yaml` 的既有契约不变）
- [x] 修改 `plugins/dev-team/bin/src/commands/backtrack.ts`：清空规则——`validatePhaseTarget` 之后预检 `files` 存在（缺失抛硬错误且不改写评估条目，保证不部分生效）；`backtrack_to` 在同一 phase 表中的索引 ≤ `implement` 的索引时（`implement` 不在表内的工作流不触发），在既有 eval 写回后将 `files` 重置为 `{ written: [], deleted: [] }`

## Phase 2: session 注册表

- [x] 新增 `plugins/dev-team/bin/src/lib/session-registry.ts`：`bindSession(projectRoot, sessionId, change)` 与 `lookupChange(projectRoot, sessionId)`；存储于 `os.tmpdir()/dev-team-hooks/<sha256(projectRoot) 前 16 位>.json`，结构 `{ [session_id]: { change, updatedAt } }`；`bindSession` 写入时惰性清理 `updatedAt` 超 24h 的条目；读写失败不抛出（返回 `null` / 静默），保证 hook fail-open

## Phase 3: hooks 层（提取器、扩保护、记录器、注册链）

- [x] 修改 `plugins/dev-team/bin/src/hooks.ts`：将 `extractBashWriteTargets` / `extractPowerShellWriteTargets` 合并为模块内部 `extractFileOps(command: string): FileOp[]`——write（既有全部写正则 + `git restore --source=<commit>` / `git checkout <commit> -- <paths>`）、delete（`rm` / `git rm` / `rmdir` / `unlink` 逐路径参数、PowerShell `Remove-Item` / `del` / `rd`）、revert（`git restore <paths>` 无 `--source`、`git checkout -- <paths>`）、mv 双条目（bash `mv`、PowerShell `Move-Item` / `mv` / `ren`）；python/node 豁免与 fail-open 语义不变
- [x] 修改 `plugins/dev-team/bin/src/hooks.ts`：PreToolUse（`checkBashCommand` / `checkPowerShellCommand` / `evaluateToolAccess`）改按 `extractFileOps` 提取路径匹配保护 glob，write 与 delete 双分类命中即 deny（reason 沿用 `%s`/`%t` 占位符）；新增批量还原拦截——`git stash`（含 `pop`/`apply`/`branch`，`list`/`show` 放行）、`git clean`（无路径参数或任一路径命中 `openspec/**` 即 deny，`-n`/`--dry-run` 放行）、`git restore` / `git checkout --` 目标命中 `openspec/changes/**` 即 deny（源码目标放行，交记录器记 revert）
- [x] 修改 `plugins/dev-team/bin/src/hooks.ts`：新增 `runRecordFiles()` 子命令（`main()` switch 增 `record-files` 分支）——读 stdin 事件；`tool_name` 以 `mcp__` 开头且以 `phase_next` 结尾时 `bindSession(getProjectDir(), stdin.session_id, tool_input.change)` 后结束；其余工具按类型取路径（`Write`/`Edit` → `file_path`、`NotebookEdit` → `notebook_path`、`Bash`/`PowerShell` → `extractFileOps(command)` 并附事件 `agent_type`）；过滤 `openspec/**` 与 `workflow.json` 自身；绝对路径相对化到 `getProjectDir()`、越出项目根丢弃；`foldFileOps` → `writeFileInventory`；未绑定 session 或任何异常仅 stderr 诊断并 exit 0
- [x] 修改 `plugins/dev-team/hooks/hooks.canonical.json`：新增 `postToolUse` 条目——matchers.claude = `Write|Edit|NotebookEdit|Bash|PowerShell|__MCP:phase_next__`、matchers.cursor = `null`、commandTemplate = `node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" record-files`；`description` 补充记录器一句说明
- [x] 修改 `plugins/dev-team/build/hooks-profile.ts`：canonical Zod schema 增 `postToolUse` 数组（条目形状同 preToolUse）；`buildClaudeNested` 输出 `hooks.PostToolUse`；`buildCursorNative` 按 cursor matcher 输出 `postToolUse`（条目 cursor matcher 为 `null` 时整段省略）；既有 `preToolUse` / `subagentStop` 包装行为不变

## Phase 4: change_files MCP 工具

- [x] 新增 `plugins/dev-team/bin/src/schemas/change-files.schema.ts`：`changeFilesInputSchema`（`change` 必填、`op: 'append' | 'set'`、`written`/`deleted` 可选但至少其一、路径为相对项目根 POSIX 风格并拒绝绝对路径与穿越）与 `changeFilesOutputSchema`（`{ written, deleted }`）；在 `plugins/dev-team/bin/src/schemas/index.ts` 增补 re-export
- [x] 新增 `plugins/dev-team/bin/src/commands/change-files.ts`：`runChangeFiles(options)`——校验 change 存在、`workflow.json` 通过 schema、含 `files`（缺失抛硬错误指引重建）；`append` 按折叠规则逐路径合并（去重、保留既有 `source`、新路径不写 `source`）；`set` 整体覆写指定桶（未提供的桶不动）并清理被覆写路径的 `source` 条目；保留其他字段逐字节、输出操作后净状态
- [x] 修改 `plugins/dev-team/bin/src/mcp.ts`：注册 `change_files` 工具（走 `withResolvedProjectRoot` + `jsonContent` 既有模式；description 说明补录漏记与修正净状态双用途，不含绕过写保护的指引）

## Phase 5: 消费方改造

- [x] 修改 `plugins/dev-team/bin/src/commands/test-execution.ts`：`TestExecutionOptions` 删 `mutationDiffOnly`；`resolveMutationDiffFiles` 重写为清单解析——由 `options.change` 触发、`noMutation` 时惰性跳过（不读清单不报错）；scope = `files.written` → `expandMutationDiffWithInferredSources`（保留）→ 净归零去噪（逐文件与 `git show HEAD:<path>` 对比，相同剔除；HEAD 无该文件或读取失败保留）；`files` 缺失抛硬错误（含重建指引）；未传 `--change` 时突变不限圈；stdout 诊断行改清单来源前缀；移除对 `lib/git.ts` 的 import
- [x] 修改 `plugins/dev-team/bin/src/cli.ts`：删除 `--mutation-diff-only` 选项与 `mutationDiffOnly` 透传；`--change` 选项描述更新为兼作突变 scope 入口；`--skip-mutation` 不变；不引入 `--mutation-scope`
- [x] 修改 `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts`：`modules` union 的 `"git-change"` 字面量替换为 `"change"`；增可选 `change: string` 参数并 refine（`modules === 'change'` 时必填）；description 同步清单语义
- [x] 修改 `plugins/dev-team/bin/src/commands/test-resolve-paths.ts`：`ResolveTestPathsParams` / `TestResolvePathsInput` 类型同步；`resolveEffectiveModules` 的清单分支改读 `readFileInventory(getChangeDir(change, projectRoot)).written`（删除 `execSync('git diff HEAD --name-only')`）；清单读取失败（change 不存在 / workflow.json 缺失非法 / 无 `files`）→ `errors` 增含重建指引条目并 earlyReturn；`written` 为空 → 空 `unit_tests` 无致命错误；后续非空 modules 管线不变
- [x] 修改 `plugins/dev-team/bin/src/schemas/archi-check.schema.ts`：增可选 `change: string`；`staged` 描述改为"已废弃，传 true 将显式报错"
- [x] 修改 `plugins/dev-team/bin/src/lib/c4-cross-ref.ts`：`runCrossRefCheck` options 增 `change?: string`；`getChangedFiles` 数据源改造——优先级 显式 `files` > 清单 `files.written` 直通（不做 test config 过滤）> 空；`staged === true` 抛错指引 `change` / `files`，删除 `git diff --cached` 执行路径；清单模式下 `workflow.json` 无 `files` 抛硬错误；模型文件不进被查文件集
- [x] 修改 `plugins/dev-team/bin/src/mcp.ts`：`archi_check` handler 增 `change` 传参并把 `staged === true` 映射为显式错误；`test_resolve_paths` 工具 description 的模式 3 文案从 git diff 改为 change 清单

## Phase 6: git 退场

- [x] 删除 `plugins/dev-team/bin/src/lib/git.ts`（`getGitDiffFiles` 与 `git add -N`/`git reset` hack 整体移除），并连带删除其测试文件 `plugins/dev-team/bin/src/lib/git.test.ts`（随模块移除）
- [x] 修改 `plugins/dev-team/package.json`：`dependencies` 移除 `simple-git`（已确认仅 `lib/git.ts` 引用）

## Phase 7: 提示词与模板层

- [x] 修改 `plugins/dev-team/agents/implementation-evaluator.md`：frontmatter description、Tools/Workflow 段落与 I1 / I2 / I6 重写为 design 变更清单 × `workflow.json.files` × 文件系统三态对账（判定表：计划有 actual 无文件不存在 → 硬 fail；计划有 actual 无文件存在 → 良性漏记照常核对；actual 有计划无 → agent 判断、额外删除从严；声明删文件仍在 → 硬 fail）；`git diff` 降为观察辅助；不要求读取 `files` 缺失的旧 change
- [x] 修改 `plugins/dev-team/agents/test-gen-evaluator.md`：移除"via git diff"表述与 `git diff --stat` / `git diff --name-only` 圈定被检文件的步骤；范围核对改三态对账（test-design 声明 + `files.written` + 文件系统），内容核对独立于 actual
- [x] 修改 `plugins/dev-team/agents/test-gen-generator.md`：改写 "The git diff of these uncommitted changes IS the artifact"——产出契约 = 写盘的测试文件本身，范围以 test-design / design 声明与 `files.written` 清单核对；保留"代码即产物、无 JSON 报告"语义
- [x] 修改 `plugins/dev-team/agents/acceptance-evaluator.md` 与 `plugins/dev-team/agents/code-review-evaluator.md`：`git diff` 降级为纯观察辅助（查看修改与被删内容），范围判定读 proposal/design 声明 + `files` 清单 + 文件系统；"禁止设置 backtrack_to"约束不变
- [x] 修改 `plugins/dev-team/agents/implementation-generator.md`：新增规范还原动作说明——撤销文件修改用 `git restore <path>`（可被记录器折叠为净 untouched），MUST NOT 指示重写内容为原样来还原
- [x] 修改 `plugins/dev-team/agents/architecture.md`：validate 模式调用从 `archi_check({staged: true})` 改为 `archi_check({change: "<change-name>"})`（独立场景用 `files` 列表）；工具能力清单行同步
- [x] 修改 `plugins/dev-team/skills/phase-implement/SKILL.md` 与 `plugins/dev-team/skills/phase-test-gen/SKILL.md`：frontmatter description 与范围/产出核对文案对齐清单机制（design 清单 × `files` × 文件系统）；清单修正指引走 MCP `change_files`（append/set），不得指引直接编辑 `workflow.json`
- [x] 修改 `plugins/dev-team/templates/artifacts/design.md.template`：变更清单增「删除文件」子节（目录路径合法、无删除时整段省略注记，不强制空表）
- [x] 修改 `plugins/dev-team/templates/artifacts/proposal.md.template`：「变更范围」增「删除文件」分组（与实现文件 / 测试文件 / 不要修改并列）

## Phase 8: 版本与构建产物

- [x] bump `plugins/dev-team/package.json` 的 `version`，执行 `pnpm -C plugins/dev-team run build` 刷新 `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`zcode-plugins/dev-team/`、`cursor-home-image/dev-team/` 产物（确认产物 hooks.json 含 PostToolUse 记录器、无 `--mutation-diff-only` 残留文案）
