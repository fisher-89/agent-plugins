# 测试设计: stryker-mutation-cwd-lca

> **日期**: 2026-08-17

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | 人造 suite：root=`pkg`、cwd=`.`、config=`vite.config.ts` → `mutationCwd` 为 `pkg`（三者同层，与本仓库同构） | 单元测试 | `plugins/dev-team/bin/src/lib/test-plan.ts` |
| AC-2 | 人造 suite：root=`pkg/src`、cwd=`.`、config=`../vitest.config.ts` → `mutationCwd` 为 `pkg` | 单元测试 | `plugins/dev-team/bin/src/lib/test-plan.ts` |
| AC-3 | 人造 suite：root=`pkg/src`、cwd=`..`、无 config → `mutationCwd` 为 `pkg`（`LCA(root, cwd)`） | 单元测试 | `plugins/dev-team/bin/src/lib/test-plan.ts` |
| AC-4 | 上例加 `mutation.cwd='.'` 或 `'../..'` → `mutationCwd` 为覆盖解析值，不是 LCA | 单元测试 | `plugins/dev-team/bin/src/lib/test-plan.ts` |
| AC-5 | 仅 root + cwd → `mutationCwd` 为二者目录 LCA | 单元测试 | `plugins/dev-team/bin/src/lib/test-plan.ts` |
| AC-6 | mock `execSync`：命令含 `npx --prefix <绝对 cwd>`，且没有 `-p`；`options.cwd` 仍是 `entry.mutation_cwd`（可 ≠ prefix） | 单元测试、集成测试 | `plugins/dev-team/bin/src/lib/test-framework.ts`、`plugins/dev-team/bin/src/lib/test-runner.ts` |
| AC-7 | plan `mutation_cwd` 为 `'.'` 时仍原样传给 stryker cwd；`{prefix}` 仍是 `path.resolve(projectRoot, entry.cwd)` 的绝对路径 | 单元测试、集成测试 | `plugins/dev-team/bin/src/lib/test-runner.ts` |
| AC-8 | Zod / JSON schema 将 `mutation.cwd` 描述为覆盖自动 LCA；省略该字段时 parse 结果仍不写入默认字符串（消费者计算 LCA） | 单元测试 | `plugins/dev-team/bin/src/lib/test-plan.ts` |
| AC-9 | 真跑时命令为 `npx --prefix <abs plugins/dev-team> stryker run …`，process cwd 仍是 `plugins/dev-team`；不比今天更差 | 不可测试 | — |
| AC-10 | 缺省 LCA 绝对路径高于 `projectRoot`（例如 suite `root="."`、`cwd=".."`，未设 `mutation.cwd`）→ `mutationCwd` 为 `"."`；不得把 `projectRoot` 的父目录写入 plan `mutation_cwd` | 单元测试 | `plugins/dev-team/bin/src/lib/test-plan.ts` |
| AC-11 | `absRoot` / `absCwd` / `dirname(absConfig)` 不在同一盘符或无法计算共同祖先，且未设 `mutation.cwd` → SHALL 抛错；不得把盘符根（如 `C:\`）作为 `mutation_cwd` 返回 | 单元测试 | `plugins/dev-team/bin/src/lib/test-plan.ts` |

---

## 单元测试

### plugins/dev-team/bin/src/lib/test-plan.ts -> plugins/dev-team/bin/src/lib/test-plan.test.ts

#### 待测功能

- `resolveAllSuites(suites: TestSuite[], projectRoot: string): ResolvedSuite[]`: 对每个 suite 解析 `absRoot` / `absCwd` / `absConfig`；缺省 `mutationCwd` 为目录 LCA（再 `toPosixRelative`）；显式 `suite.mutation.cwd !== undefined` 时跳过 LCA 与 clamp；LCA 高于 `projectRoot` 则 clamp 为 `"."`；跨盘或无共同祖先抛 `Error`。`directoryLca` / `resolveSuite` 保持模块私有，经此入口观测。
- `ResolvedSuite.mutationCwd`: 相对 `projectRoot` 的 POSIX 沙箱根（空相对路径为 `"."`）。
- `configSchema.parse`（经 `../schemas` 再导出，非本文件 export）：锁 AC-8——省略 `mutation.cwd` 不写入默认字符串；`toJSONSchema` 描述覆盖自动 LCA。`findSuite` / `derivePlanId` / `pathFilterFromPlan` / `resolvePlanFiles` / `isInSuiteScope` 本变更不改行为，沿用既有用例。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| resolveAllSuites -- 缺省 LCA | 正向 | 人造 suite `root='pkg'`、`cwd='.'`、`config='vite.config.ts'`、未设 `mutation.cwd` → `mutationCwd==='pkg'` 且 `cwd==='pkg'`（AC-1，三者同层） | 新增 |
| resolveAllSuites -- 缺省 LCA | 正向 | 人造 suite `root='pkg/src'`、`cwd='.'`、`config='../vitest.config.ts'`、未设 `mutation.cwd` → `mutationCwd==='pkg'`（AC-2，config 探出 cwd） | 新增 |
| resolveAllSuites -- 缺省 LCA | 正向 | 人造 suite `root='pkg/src'`、`cwd='..'`、无 `config`、未设 `mutation.cwd` → `mutationCwd==='pkg'` 且不等于 `'pkg/src'`（AC-3 / AC-5，`LCA(absRoot, absCwd)`） | 新增 |
| resolveAllSuites -- 缺省 LCA | 正向 | `root='pkg/a'`、`cwd='..'` 与 `root='pkg/b'`、`cwd='..'` 两条 suite 均未设覆盖 → 各自 `mutationCwd==='pkg'`，互不合并 | 新增 |
| resolveAllSuites -- 缺省 LCA | 正向 | 同一输入连续调用两次 `resolveAllSuites` → `mutationCwd` 相同（纯函数、无副作用） | 新增 |
| resolveAllSuites -- 缺省 LCA | 异常 | `framework` 为 `'unknown'` / `''` / `'jest '` → 抛错，message 含 `Unknown framework`，不返回 `mutationCwd` | 新增 |
| resolveAllSuites -- 缺省 LCA | 异常 | `suites` 为 `undefined` / `null`（None）→ 抛 TypeError，不得返回盘符根 | 新增 |
| resolveAllSuites -- 缺省 LCA | 边界 | `suites=[]`（空数组）→ 返回 `[]` | 新增 |
| resolveAllSuites -- 缺省 LCA | 边界 | `suites` 仅单元素且 `root===cwd`（`root='pkg'`、`cwd='.'`、无 config）→ `mutationCwd==='pkg'` | 新增 |
| resolveAllSuites -- 缺省 LCA | 边界 | `suites` 超大列表（>100 条同构人造 suite）→ 每条 `mutationCwd` 独立正确，不抛 | 新增 |
| resolveAllSuites -- 缺省 LCA | 边界 | `projectRoot` 为空串 `''` → 解析不崩溃；`mutationCwd` 仍为相对 POSIX，不得为盘符根 | 新增 |
| resolveAllSuites -- 缺省 LCA | 边界 | `projectRoot` 超长（>1000 chars）或含 emoji / `\n` → 不崩溃；`mutationCwd` 为 POSIX 相对路径 | 新增 |
| resolveAllSuites -- 缺省 LCA | 边界 | `suite.config` 为 `undefined`（Optional None）→ 第三元不参与 LCA，行为同 AC-5 | 新增 |
| resolveAllSuites -- 缺省 LCA | 边界 | `root='pkg/a/b'`、`cwd='../..'`、`config='../x.ts'`（三目录不同层）→ `mutationCwd` 为三者目录 LCA 相对 `projectRoot` 的 POSIX 路径 | 新增 |
| resolveAllSuites -- 缺省 LCA | 边界 | `cwd` 含反斜杠 `'..\\..'` → 平台 `path.resolve` 后 LCA 结果与正斜杠等价（POSIX `mutationCwd`） | 新增 |
| resolveAllSuites -- 显式 mutation.cwd 覆盖 | 正向 | `root='pkg/src'`、`cwd='..'`、`mutation.cwd='.'` → `mutationCwd==='pkg/src'`，不是 LCA `'pkg'`（AC-4） | 新增 |
| resolveAllSuites -- 显式 mutation.cwd 覆盖 | 正向 | `root='pkg/src'`、`cwd='.'`、`mutation.cwd='../..'` → `mutationCwd` 为 `path.resolve(absRoot, '../..')` 再相对 `projectRoot` 的 POSIX 路径，不被改写为 LCA（AC-4 放大 sandbox） | 新增 |
| resolveAllSuites -- 显式 mutation.cwd 覆盖 | 异常 | 显式覆盖场景下 `framework` 非法 → 仍先因 `getFrameworkConfig` 抛错，不返回覆盖值 | 新增 |
| resolveAllSuites -- 显式 mutation.cwd 覆盖 | 边界 | `mutation.cwd===''`（空串，字段已出现）→ `absMutationCwd=path.resolve(absRoot, '')` 即 `absRoot`，跳过 LCA 与 clamp | 新增 |
| resolveAllSuites -- 显式 mutation.cwd 覆盖 | 边界 | `mutation.cwd` 超长（>1000 chars 的 `'../'.repeat`）→ 按 `path.resolve` 解析，不改写为 LCA | 新增 |
| resolveAllSuites -- 显式 mutation.cwd 覆盖 | 边界 | `mutation.cwd` 含特殊字符（`'\n'` / emoji）→ `path.resolve` 后相对化，不抛未捕获异常（非法路径由 Node `path` 处理） | 新增 |
| resolveAllSuites -- 显式 mutation.cwd 覆盖 | 边界 | `mutation.cwd` 为 `undefined`（Optional None）→ 走缺省 LCA，不得当作 `'undefined'` 字符串覆盖 | 新增 |
| resolveAllSuites -- 显式 mutation.cwd 覆盖 | 边界 | 显式 `mutation.cwd='..'` 使结果高于 `projectRoot` → **不 clamp**（clamp 仅约束缺省 LCA），`mutationCwd` 可为 `'..'` | 新增 |
| resolveAllSuites -- LCA 上沿 clamp | 正向 | `root='.'`、`cwd='..'`、未设 `mutation.cwd` → `mutationCwd==='.'`，不得为 `'..'`（AC-10） | 新增 |
| resolveAllSuites -- LCA 上沿 clamp | 正向 | 缺省 LCA 绝对路径为 `projectRoot` 自身 → `mutationCwd==='.'`（含自身视为在内，不误 clamp） | 新增 |
| resolveAllSuites -- LCA 上沿 clamp | 异常 | `projectRoot` 为 `undefined` / `null`（None）→ 抛错，不得把父目录或盘符根写入 `mutationCwd` | 新增 |
| resolveAllSuites -- LCA 上沿 clamp | 边界 | `cwd='../..'` 使 LCA 高于仓库根但尚未到文件系统根 → clamp 为 `'.'`，plan 不得含 `'../..'` | 新增 |
| resolveAllSuites -- LCA 上沿 clamp | 边界 | `cwd=0` 经 `String`/`path.resolve` 的非字符串（若强转）不作为合法覆盖；正常调用仅接受 string | 新增 |
| resolveAllSuites -- 跨盘抛错 | 正向 | win32：`projectRoot` 在 C: 盘、suite `cwd` 为另一盘符绝对路径（如 `D:\\no-common`）、未设 `mutation.cwd` → 抛 `Error`；posix：`cwd='/'` 使 LCA 为文件系统根 → 抛 `Error`（AC-11） | 新增 |
| resolveAllSuites -- 跨盘抛错 | 异常 | 同上跨盘 / 无共同祖先 → SHALL 抛错；`message` 含 `mutation_cwd` 且表明共同祖先或跨盘；`catch` 后不得读到盘符根（`C:\\` / `D:\\` / `'/'`）作为 `mutationCwd`（AC-11） | 新增 |
| resolveAllSuites -- 跨盘抛错 | 边界 | `suite.config` 为另一盘符（或 posix 下 `'/'`）上的绝对路径、`cwd` 仍在 `projectRoot` 内、未设覆盖 → 第三元导致无共同祖先时同样抛错 | 新增 |
| resolveAllSuites -- 跨盘抛错 | 边界 | 已设 `mutation.cwd` 时即使 `cwd`/`config` 跨盘 → **不走** `directoryLca`，不因跨盘抛错（逃生舱） | 新增 |
| configSchema -- mutation.cwd 省略与文档 | 正向 | `configSchema.parse({ schema:'spec-driven', tests:[{ root:'pkg', framework:'vitest' }] })` 后 `tests[0].mutation.cwd` 为 `undefined`，不得为 `'.'` 或 suite `cwd`；将该 suite 交给 `resolveAllSuites` → 走 LCA（AC-8） | 新增 |
| configSchema -- mutation.cwd 省略与文档 | 正向 | `configSchema.toJSONSchema({ io:'input' })` 中 `tests.items.properties.mutation.properties.cwd.description` 表明覆盖自动 LCA，且不含「默认等于cwd」或「默认等于 cwd」（AC-8） | 新增 |
| configSchema -- mutation.cwd 省略与文档 | 正向 | parse 显式 `mutation:{ cwd:'.', score:50 }` → `mutation.cwd==='.'` 且 `score===50`（字段出现即保留） | 新增 |
| configSchema -- mutation.cwd 省略与文档 | 异常 | `mutation.cwd` 为 `null` / `123`（非 string）→ `configSchema.parse` 抛 ZodError | 新增 |
| configSchema -- mutation.cwd 省略与文档 | 边界 | `mutation:{}`（空对象）→ `cwd` 仍为 `undefined`（无字符串 prefault） | 新增 |
| configSchema -- mutation.cwd 省略与文档 | 边界 | `mutation` 省略（Optional None）→ parse 后 `mutation` 因 `prefault({})` 存在，但 `cwd` 仍 `undefined` | 新增 |
| configSchema -- mutation.cwd 省略与文档 | 边界 | `mutation.cwd` 为空串 `''` → parse 成功且值为 `''`（optional string，非 nonempty） | 新增 |
| configSchema -- mutation.cwd 省略与文档 | 边界 | 多余未知字段（passthrough）不影响 `mutation.cwd` 省略语义 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无文件系统 / 无子进程 | `resolveAllSuites` 为纯 `path` 计算；`projectRoot` 用 `path.resolve('/virtual-project-root')` 或 `mkdtempSync` 仅在需要真实盘符时使用 | 缺省 LCA / 显式覆盖 / clamp |
| `process.platform` 与绝对路径 | 跨盘用例按平台构造：win32 用另一盘符绝对 `cwd`/`config`；posix 用 `'/'` 使 LCA 落在文件系统根。不 mock `path` 模块 | 跨盘抛错 |
| `configSchema` | 不 mock；真实 `parse` / `toJSONSchema` | mutation.cwd 省略与文档 |

---

### plugins/dev-team/bin/src/lib/test-framework.ts -> plugins/dev-team/bin/src/lib/test-framework.test.ts

#### 待测功能

- `getFrameworkConfig(framework: string): FrameworkConfig`: 浅拷贝返回注册表项。本变更将 jest / vitest / vite-plus 的 `shell.mutation_execution` 与 `cmd.mutation_execution` 精确改为 `npx --prefix "{prefix}" stryker run "{config}"`；继续禁止模板匹配 `/(^|\s)-p(\s|$)/`。其余框架仍无 `mutation_execution`。`detectFrameworkVersion` 本变更不改。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| getFrameworkConfig -- mutation_execution 模板 | 正向 | jest / vitest / vite-plus 的 `shell.mutation_execution('99.0.0')` 与 `cmd.mutation_execution('99.0.0')` 均精确等于 `npx --prefix "{prefix}" stryker run "{config}"`（AC-6） | 新增 |
| getFrameworkConfig -- mutation_execution 模板 | 正向 | 上述字符串 `.not.toMatch(/(^|\s)-p(\s|$)/)`；含 `--prefix` 不得被误判为 `-p`（AC-6） | 新增 |
| getFrameworkConfig -- mutation_execution 模板 | 正向 | `vi.resetModules` 后动态 `import('./test-framework')`，jest/vitest/vite-plus 的 mutation 模板仍精确含 `{prefix}` 且无 `-p` token | 新增 |
| getFrameworkConfig -- mutation_execution 模板 | 异常 | `getFrameworkConfig('unknown')` / `''` / `'jest '` 抛错，message 含 `Unknown framework` 及八框架名，不返回残缺 `mutation_execution` | 新增 |
| getFrameworkConfig -- mutation_execution 模板 | 异常 | `framework` 为 `undefined` / `null` 强转调用时抛错 | 新增 |
| getFrameworkConfig -- mutation_execution 模板 | 边界 | bun / rust / go / pytest / node-test 的 `shell.mutation_execution` 与 `cmd.mutation_execution` 均为 `undefined`（非 jest/vitest/vite-plus 仍无门控） | 新增 |
| getFrameworkConfig -- mutation_execution 模板 | 边界 | `framework` 超长（>1000 chars）或含 `\n` / emoji → 抛 `Unknown framework` | 新增 |
| getFrameworkConfig -- mutation_execution 模板 | 边界 | `mutation_execution` 传入 version=`''` / `'0.0.0'` / `'99.0.0'` → 返回值与 version 无关（常量模板） | 新增 |
| getFrameworkConfig -- mutation_execution 模板 | 边界 | 修改返回对象的 `shell.mutation_execution` 不影响再次 `getFrameworkConfig` 的模板（浅拷贝） | 新增 |
| getFrameworkConfig -- mutation_execution 模板 | 废弃 | jest/vitest/vite-plus 的 mutation 模板精确为 `npx stryker run "{config}"`（无 `{prefix}`） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | `getFrameworkConfig` 为纯注册表查找，不 mock 外部依赖 | mutation_execution 模板全部路径 |
| `vi.resetModules` + 动态 `import` | 解除模块缓存后重读字面量，锁静态变异 | resetModules 杀静态变异 |

---

### plugins/dev-team/bin/src/lib/test-runner.ts -> plugins/dev-team/bin/src/lib/test-runner.test.ts

#### 待测功能

- `executePlanEntry(entry: TestPlan, projectRoot: string, options: { files?: string[]; timeout?: number; noMutation?: boolean; mutationDiffFiles?: string[]; reportsDir: string }): ExecutionResult`: 变异阶段经私有 `genStrykerCommand` 将 `{prefix}` 换成 `path.resolve(projectRoot, entry.cwd)` 的绝对路径（不相对 `mutation_cwd`、不 `toForwardSlash`），将 `{config}` 换成临时配置路径；`execSync` / `runCommand` 的 `cwd` 仍为 `entry.mutation_cwd`（相对值如 `'.'` 原样传递）。命令 MUST NOT 匹配 `npx -p`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| executePlanEntry -- prefix 与 exec cwd | 正向 | plan `cwd='pkg/jest'`、`mutation_cwd='pkg'`；mock `execSync` 捕获 stryker 调用：命令含 `npx --prefix` 后接 `path.resolve(projectRoot, 'pkg/jest')` 的绝对路径；`options.cwd==='pkg'`；命令 `.not.toMatch(/(^|\s)-p(\s|$)/)`（AC-6） | 新增 |
| executePlanEntry -- prefix 与 exec cwd | 正向 | 既有「stryker 在 mutation_cwd 执行（可不同于 suite cwd）」用例保留，并追加断言 prefix 指向 suite abs cwd 且 ≠ `opts.cwd` | 新增 |
| executePlanEntry -- prefix 与 exec cwd | 异常 | mock `execSync` 在 stryker 命令上抛 `status=1` → `ExecutionResult.mutation===null` 且 `error` 含失败信息；捕获到的命令仍含绝对 `--prefix` 且无 `-p`；测试 `exitCode` 不因此变为 stryker 的 1 | 新增 |
| executePlanEntry -- prefix 与 exec cwd | 异常 | `mutation_script===null`（如 bun/go plan）→ 不调用 stryker `execSync`，无 `--prefix` 命令 | 新增 |
| executePlanEntry -- prefix 与 exec cwd | 边界 | `entry.cwd===''`（空串）→ `{prefix}` 为 `path.resolve(projectRoot, '')` 的绝对路径，不是相对 `mutation_cwd` | 新增 |
| executePlanEntry -- prefix 与 exec cwd | 边界 | `entry.cwd` 超长（>1000 chars 的嵌套 `p/`）→ prefix 仍为 `path.resolve(projectRoot, entry.cwd)` 绝对路径 | 新增 |
| executePlanEntry -- prefix 与 exec cwd | 边界 | `entry.cwd` 含空格 / emoji → 命令中 `--prefix` 仍带模板双引号包裹的绝对路径 | 新增 |
| executePlanEntry -- prefix 与 exec cwd | 边界 | win32 下 prefix 可含反斜杠与盘符；断言等于 `path.resolve(...)` 原文，**不得**被 `toForwardSlash` 成全正斜杠 | 新增 |
| executePlanEntry -- prefix 与 exec cwd | 边界 | `noMutation:true`（bool True）→ 不跑 stryker；`noMutation` 省略（None）且框架支持 mutation → 跑 stryker 并注入 prefix；`noMutation:false` → 与省略等价 | 新增 |
| executePlanEntry -- 相对 mutation_cwd 时 prefix 仍绝对 | 正向 | plan `mutation_cwd='.'`、`cwd='.'` → `execSync` 的 `cwd` option 精确为 `'.'`；`--prefix` 参数为 `path.resolve(projectRoot, '.')` 的绝对路径，SHALL NOT 使用相对 `'.'` 作为 prefix（AC-7） | 新增 |
| executePlanEntry -- 相对 mutation_cwd 时 prefix 仍绝对 | 异常 | `projectRoot` 为 `undefined` / `null`（None）时 `path.resolve` 行为导致抛错或落到 `process.cwd()`：断言不得把相对 `mutation_cwd` 当作 prefix | 新增 |
| executePlanEntry -- 相对 mutation_cwd 时 prefix 仍绝对 | 边界 | `mutation_cwd='.'` 且 `cwd='pkg'`（相对值 cwd ≠ `.`）→ exec cwd 仍为 `'.'`；prefix 为 `path.resolve(projectRoot, 'pkg')` 绝对路径 | 新增 |
| executePlanEntry -- 相对 mutation_cwd 时 prefix 仍绝对 | 边界 | `mutation_cwd` 为空串 `''` → 原样传给 `execSync` cwd；prefix 仍绝对 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `child_process.execSync` | `vi.mock` 捕获 `(cmd, options)`；stryker 调用时写入 `reportDir/mutation.json` 后返回，或抛带 `status` 的 Error | prefix 注入 / 相对 cwd / 失败路径 |
| 临时目录 `fs` | `os.tmpdir` 下构造 `openspec/config.json` + `pkg/` + `pkg/jest/`（不必安装 `@stryker-mutator`） | 抬根与相对 `mutation_cwd` |
| `console.log` | `vi.spyOn` 吞日志 | 全部分支 |

---

## 集成测试

### mutation_execution 模板 → execSync `--prefix` 与 `mutation_cwd` → `plugins/dev-team/bin/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/test-framework.ts` | 提供 jest/vitest/vite-plus 的 `mutation_execution` 模板（含未展开 `{prefix}` / `{config}`） |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 将 `{prefix}` 展开为 suite cwd 绝对路径，以 `entry.mutation_cwd` 为 `execSync` cwd 启动 Stryker |

**关联AC**: AC-6, AC-7

**关系描述**:

detect 产出的 `mutation_script` 仍是带占位符的模板；真正把 `{prefix}` 换成绝对 suite cwd、并把 process cwd 留在（可能已被 LCA 抬高的）`mutation_cwd` 上，发生在 runner 调用 `execSync` 的那一次跨模块组合。单元测试分别锁模板字面量与 `executePlanEntry` 的替换，但抬根时「prefix 指向子目录、sandbox cwd 指向父目录」只有把 `getFrameworkConfig` 的真实模板交给 `executePlanEntry` 才能暴露「相对 prefix 漂移」或把 `--prefix` 误写成 `-p` 的裂口。本关系用临时目录拼 `pkg/` + `pkg/jest/`，mock `execSync` 不启动真实 Stryker、不安装 `@stryker-mutator`。

#### 场景: 抬根时 prefix 指向子目录 cwd

验证抬根后层 1 npx 解析根与 Stryker sandbox 根可以分离。前置条件是临时项目含 `pkg/` 与 `pkg/jest/`，以及可读的 `openspec/config.json` 与至少一个会被 mutation 阶段当作源文件的 `*.ts`。输入是 `TestPlan`：`cwd='pkg/jest'`、`mutation_cwd='pkg'`（或相对 `projectRoot` 的等价 POSIX）、`mutation_script` 取自 `getFrameworkConfig('vitest')` 的真实模板。预期 `execSync` 的命令含 `npx --prefix` 后接 `path.resolve(projectRoot, 'pkg/jest')` 的绝对路径、不含 `-p` token，且 `options.cwd` 为 `'pkg'`（或测试写入 plan 的同一相对值）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 临时目录 `pkg/` + `pkg/jest/`；plan `mutation_cwd` 指向 `pkg`、`cwd` 指向 `pkg/jest`；mock `execSync` 捕获命令含绝对 `--prefix` 指向 `pkg/jest`，`cwd` 为 `pkg`，无 `npx -p`（AC-6） | 新增 |
| 异常 | 同上结构下 mock stryker `execSync` 抛 `status=1` → 返回 `mutation===null` 且 `error` 有值；失败前捕获的命令仍含绝对 `--prefix`；临时 `stryker.config.*` 仍被清理 | 新增 |
| 边界 | `pkg/jest` 目录存在但无 `node_modules` / 未安装 `@stryker-mutator` → 因 mock 不启动真实 CLI，仍只断言命令形态与 cwd，不因缺包失败 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `child_process.execSync` | `vi.mock`；识别命令含 `stryker` 时写入 `reportDir/mutation.json` 或抛错；记录 `cmd` 与 `opts.cwd` | 正向 / 异常 / 边界 |
| 真实临时目录 | `mkdtempSync` 创建 `pkg/` 与 `pkg/jest/`，用后 `rmSync` | 全部路径 |

#### 场景: mutation_cwd 为相对 '.' 时 prefix 仍绝对

验证 AC-7：exec cwd 可以是相对 `'.'`（与今天 detect 产出一致），但 npx `--prefix` 不得写成相对 `mutation_cwd` 的 `'.'`，以免 process cwd 与 exec cwd 不一致时漂。前置条件与既有 mutation-flow 夹具相同（临时项目根即 suite cwd）。输入 `mutation_cwd='.'`、`cwd='.'`（或 `cwd` 为相对子路径）。预期 `opts.cwd==='.'`，`--prefix` 等于 `path.resolve(projectRoot, entry.cwd)` 且 `path.isAbsolute` 为 true。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `mutation_cwd='.'`、`cwd='.'` → `execSync` cwd 为 `'.'`；`--prefix` 为 `path.resolve(projectRoot, '.')` 绝对路径（AC-7） | 新增 |
| 异常 | `mutation_script` 手工去掉 `{prefix}` 占位（残缺模板）→ 命令不含 `--prefix` 绝对路径，本场景用此断言锁「模板与 runner 必须同时在场」；生产路径不得走残缺模板 | 新增 |
| 边界 | `mutation_cwd='.'` 且 `cwd` 为相对子目录 `'src'` → exec cwd 仍为 `'.'`；prefix 为 `path.resolve(projectRoot, 'src')`，不得为 `'src'` 或 `'.'` | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `child_process.execSync` | 同上一场景，捕获 cmd 与 cwd | 正向 / 异常 / 边界 |

---

## 不可测试项

- AC-9 本仓库 vite-plus 真跑回归 — **原因**: proposal 明确主证据是人造 suite + mock `execSync`，真跑仅为「prefix 与 `mutation_cwd` 重合时不比今天更差」的可选观察；`openspec/config.json` 的 vite-plus suite **不得**为 dogfood 拆开 `root` / `cwd` / `config`，因此自动化套件锁不住「抬根」形态，也不在本变更引入真实 Stryker 嵌套 fixture。
- `plugins/dev-team/bin/src/schemas/config/config.schema.ts` 的 colocated 单测 — **原因**: `test_resolve_paths` 返回 `Not in test config scope`（`openspec/config.json` 的 `excludes` 含 `bin/src/schemas/**/*`）。AC-8 的 parse 语义与 Zod `describe` 改在 `test-plan.test.ts` 经 `configSchema.parse` / `toJSONSchema` 验证。
- `plugins/dev-team/bin/dev-team-config.schema.json` — **原因**: `test_resolve_paths` 返回 `Not a testable source file`。该文件由 `generateConfigJsonSchema()`（`configSchema.toJSONSchema({ io: 'input' })`）再生；文案与 Zod 的一致性由 AC-8 的 `toJSONSchema` 用例间接锁住，不另开 colocated JSON 测试。
- 私有 `directoryLca` / `isInsideProjectRoot` / `genStrykerCommand` / `MUTATION_EXECUTION` — **原因**: design 规定不为测试额外 export（knip / CLAUDE.md）；行为分别经 `resolveAllSuites` 与 `executePlanEntry` 观测。
- 层 2（sandbox 内被测 `import` / 子目录 `node_modules`、`inPlace`、手写 symlink、`symlinkNodeModules`）— **原因**: proposal / design 本刀明确不做，失败只记入 `ExecutionResult.mutation.error`。
- `commands/test-detect-frameworks.ts` 再挂一条 LCA CLI 集成 — **原因**: proposal「CLI 集成不必为 LCA 再挂一条；plan 字段由 detect 单测锁住」；本模块仍只调用一次 `resolveAllSuites`。
- `npx -p` / `--package` 自动安装、权威 `reportDir/mutation.json` 路径、`mutate` / vitest `configFile` 相对 `mutation_cwd` 的既有重写、非 jest/vitest/vite-plus 的 mutation 门控 — **原因**: 本变更不要修改；仅保留「命令不含 `-p` token」的回归断言。
