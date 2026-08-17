# 设计: stryker-mutation-cwd-lca

> **变更**: stryker-mutation-cwd-lca
> **日期**: 2026-08-17

---

## 架构组件

本变更在既有 detect → execute 管线内改两处：缺省 `mutation_cwd` 的计算，以及 Stryker 启动命令的 npx 解析根。不新增模块、不改 C4 模型、不碰层 2（sandbox 内被测 `import` / `node_modules`）。

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Suite path resolver | 解析 `absRoot` / `absCwd` / `absConfig`；缺省 `mutationCwd` 为目录 LCA（clamp / 跨盘报错）；显式 `suite.mutation.cwd` 跳过 LCA | `plugins/dev-team/bin/src/lib/test-plan.ts` | `path`、`../schemas`（`TestSuite`）、`./test-framework`、`./glob` | TypeScript |
| Framework registry | jest / vitest / vite-plus 的 `mutation_execution` 模板改为带 `{prefix}` 的 npx 命令；继续禁止 `npx -p` | `plugins/dev-team/bin/src/lib/test-framework.ts` | `../schemas`（`TestFramework`） | TypeScript |
| Test runner | `genStrykerCommand` 将 `{prefix}` 换成 suite cwd 绝对路径；`execSync` cwd 仍为 `entry.mutation_cwd` | `plugins/dev-team/bin/src/lib/test-runner.ts` | `child_process.execSync`、`path`、`TestPlan`、`stryker-config` | TypeScript |
| Config schema | `tests[].mutation.cwd` 仍 optional、无字符串 prefault；Zod describe 改为覆盖自动 LCA | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | `zod/v4`、`./defaults` | TypeScript |
| Config JSON schema | 与 Zod describe 同步的 JSON Schema 镜像 | `plugins/dev-team/bin/dev-team-config.schema.json` | `configSchema.toJSONSchema`（`build/build-config-schema.ts`） | JSON Schema |

### 组件协作

```
config.tests[]
    → resolveAllSuites / resolveSuite
        → 显式 mutation.cwd？ path.resolve(absRoot, mutation.cwd)
        → 否则 directoryLca(absRoot, absCwd, dirname(absConfig)?)
              → 跨盘 / 无共同祖先 → throw
              → 高于 projectRoot → clamp 到 path.resolve(projectRoot)
        → mutationCwd = toPosixRelative(projectRoot, absMutationCwd)
    → runTestDetectFrameworks → plan.mutation_cwd / mutation_script（模板含 {prefix}、{config}）
    → executePlanEntry → executeStrykerMutation
        → genStrykerCommand：{prefix} = path.resolve(projectRoot, entry.cwd)
        → runCommand(cmd, cwd = entry.mutation_cwd)
```

抬根时：`mutation_cwd` 为父目录（沙箱根），`--prefix` 指向子目录 suite cwd（层 1：找到 CLI / `@stryker-mutator/*`）。二者可以不相等。

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
  公共函数仅列模块级导出函数、CLI 子命令、HTTP 端点，私有函数不列入。
-->

<!-- 无新增源文件：LCA / clamp / 跨盘逻辑落在 test-plan.ts 的模块内私有函数；不新增文件、不为测试额外 export（knip / CLAUDE.md）。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/lib/test-plan.ts` | `resolveSuite` 缺省 `absMutationCwd` 改为 LCA；显式 `suite.mutation.cwd !== undefined` 时 `path.resolve(absRoot, suite.mutation.cwd)` 并跳过 LCA；LCA 高于 `projectRoot` 则 clamp；跨盘 / 无共同祖先抛 `Error`；抽出私有 `directoryLca` / `isInsideProjectRoot`（或等价）以遵守 `max-lines-per-function` 50 | AC-1–AC-5、AC-10、AC-11；覆盖 REQ-TDF-MUT-CWD-1 |
| `plugins/dev-team/bin/src/lib/test-framework.ts` | 将 `MUTATION_EXECUTION` 改为 `` `npx --prefix "{prefix}" stryker run "{config}"` ``；jest / vitest / vite-plus 的 `shell` 与 `cmd` 仍引用该常量；注释继续强调禁止 `npx -p` | AC-6；REQ-MT-PREFIX-1 模板 |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | `genStrykerCommand` 增加 `projectRoot` 参数；在既有 `{config}` 替换之外将 `{prefix}` 换成 `path.resolve(projectRoot, entry.cwd)`（平台绝对路径，不相对 `mutation_cwd`、不 `toForwardSlash`）；`executeStrykerMutation` / `runMutationPhase` 传入 `projectRoot`；`runCommand` 的 `cwd` 仍为 `entry.mutation_cwd`（相对值如 `'.'` 原样传递） | AC-6、AC-7；REQ-MT-PREFIX-1 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | `mutationConfigSchema.cwd` 的 `.describe(...)` 改为覆盖自动 LCA；保持 `z.string().optional()`、无字符串 `prefault` | AC-8；REQ-CS-MUT-CWD-1 |
| `plugins/dev-team/bin/dev-team-config.schema.json` | `tests.items.properties.mutation.properties.cwd.description` 与 Zod describe 一致；不得再写「默认等于cwd」 | AC-8；可由 `generateConfigJsonSchema()` 再生或手工同步文案 |
| `plugins/dev-team/package.json` | patch bump `version`（当前 `2.10.30`） | 项目规则：改插件源码后升版并重建产物 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `resolveAllSuites` | `plugins/dev-team/bin/src/lib/test-plan.ts` | 修改 | `function resolveAllSuites(suites: TestSuite[], projectRoot: string): ResolvedSuite[]` | 签名不变；`ResolvedSuite.mutationCwd` 缺省语义从 `suite.cwd` 改为 LCA（再相对 `projectRoot` 的 POSIX）；显式 `mutation.cwd` 仍覆盖。LCA 实现不 export |
| `getFrameworkConfig` | `plugins/dev-team/bin/src/lib/test-framework.ts` | 修改 | `function getFrameworkConfig(framework: string): FrameworkConfig` | 签名不变；jest / vitest / vite-plus 的 `shell.mutation_execution` 与 `cmd.mutation_execution` 返回值精确为 `npx --prefix "{prefix}" stryker run "{config}"` |
| `executePlanEntry` | `plugins/dev-team/bin/src/lib/test-runner.ts` | 修改 | `function executePlanEntry(entry: TestPlan, projectRoot: string, options: { files?: string[]; timeout?: number; noMutation?: boolean; mutationDiffFiles?: string[]; reportsDir: string }): ExecutionResult` | 签名不变；变异阶段展开后的命令含绝对 `--prefix`，`execSync` options.cwd 仍为 `entry.mutation_cwd` |

私有符号（`resolveSuite`、`directoryLca`、`isInsideProjectRoot`、`genStrykerCommand`、`executeStrykerMutation`、`MUTATION_EXECUTION`）不列入；经上表公共入口的可观测行为验收。

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `ResolvedSuite` | `plugins/dev-team/bin/src/lib/test-plan.ts` | 修改（语义） | 字段形状不变。`mutationCwd` 注释改为：相对 `projectRoot` 的 POSIX 沙箱根；缺省为 LCA，显式 `suite.mutation.cwd` 则为其解析值 |
| `TestPlan` | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | 既有（语义） | `mutation_cwd` / `cwd` 字段名不变；本变更不改该 schema 文件。`mutation_script` 模板新增未展开占位符 `{prefix}`（detect 仍不替换） |

### 配置

| 配置键 | 所在文件 | 类型 | 值类型 | 默认值 | 说明 |
|--------|----------|------|--------|--------|------|
| `tests[].mutation.cwd` | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` 与 `plugins/dev-team/bin/dev-team-config.schema.json` | 修改（文档） | `string`（optional） | 无（省略时 parse 结果不写入该键） | 相对 `root` 的可选覆盖，跳过自动 LCA。SHALL NOT 再描述为「默认等于 cwd」。消费者 `resolveSuite` 在字段省略时计算 LCA |
| `version` | `plugins/dev-team/package.json` | 修改 | `string` | `2.10.30` → 下一 patch | 插件源码变更后升版 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `TestSuite` | `root`、`cwd`（prefault `"."`）、`config?`、`mutation.cwd?`、`mutation.score` | 输入；`mutation` 对象因 schema `prefault({})` 始终存在，但 `mutation.cwd` 省略时为 `undefined` | `openspec/config.json`（本仓库 vite-plus suite **不改**） |
| 路径三元组（运行时） | `absRoot = path.resolve(projectRoot, suite.root)`；`absCwd = path.resolve(absRoot, suite.cwd)`；`absConfig = suite.config ? path.resolve(absRoot, suite.config) : null` | 缺省 LCA 的输入；第三元仅在 `absConfig !== null` 时取 `path.dirname(absConfig)`（目录，不是配置文件本身） | 内存 |
| `ResolvedSuite.mutationCwd` / `TestPlan.mutation_cwd` | 相对 `projectRoot` 的 POSIX 路径（`''` → `'.'`） | detect 写入 plan；runner 用作 Stryker `rootPath` / `execSync` cwd | 命令 / MCP 输出，不写入 config |
| `TestPlan.cwd` | 相对 `projectRoot` 的 POSIX suite 执行目录 | runner 将其 `path.resolve(projectRoot, entry.cwd)` 注入 `{prefix}` | 同上 |
| npx 命令 | `npx --prefix "<absPrefix>" stryker run "<configPath>"` | `absPrefix` 可 ≠ `mutation_cwd`（抬根）；权威 mutation JSON 仍是 `reportDir/mutation.json` | 进程命令行；临时 `stryker.config.*` 仍落在 `mutation_cwd` |

### 缺省 `mutation_cwd` 算法

`resolveSuite` 在算出 `absRoot` / `absCwd` / `absConfig` 后：

1. **显式覆盖**（AC-4）：若 `suite.mutation.cwd !== undefined`（schema 解析后为 `string`，含空串），则 `absMutationCwd = path.resolve(absRoot, suite.mutation.cwd)`，**不再计算 LCA、不 clamp**。用于故意放大 sandbox 或 LCA 不合适时的逃生舱。
2. **缺省 LCA**（AC-1、AC-2、AC-3、AC-5）：`dirs = [absRoot, absCwd]`，若 `absConfig !== null` 再 `dirs.push(path.dirname(absConfig))`。`absMutationCwd = directoryLca(dirs)`。
3. **跨盘**（AC-11）：`directoryLca` 对每个输入做 `path.resolve` 后比较 `path.parse(p).root`。根不一致或上溯至文件系统根仍无法得到共同祖先时 **抛 `Error`**，不得返回盘符根（如 `C:\`）作为 `mutation_cwd`。错误文案须表明无法计算 `mutation_cwd` 的共同祖先 / 跨盘，便于 detect 调用方捕获。
4. **上沿 clamp**（AC-10）：仅作用于缺省 LCA 结果。若 `path.relative(path.resolve(projectRoot), lca)` 以 `..` 开头，或为另一盘符绝对路径（`path.isAbsolute(rel)`），则 `absMutationCwd = path.resolve(projectRoot)`，因此 `mutationCwd === '.'`。不得把 `projectRoot` 的父目录写入 plan。
5. `mutationCwd = toPosixRelative(projectRoot, absMutationCwd)`（既有辅助，空相对路径为 `'.'`）。

由构造：缺省 LCA 等于或高于 `absCwd`，不会落到 `absCwd` 的子目录。

`directoryLca` 用平台 `path`（非 `path.posix`）：两两归约——从路径 A 沿 `path.dirname` 上溯，直到 B 落在候选目录内（`relative` 为空或不以 `..` 开头且非绝对路径）。LCA 实现保持模块私有。

显式覆盖与缺省 LCA 的对照：

| 场景 | suite（相对 `projectRoot`） | `mutationCwd` |
|------|------------------------------|---------------|
| 三者同层（本仓库同构） | `root=pkg`、`cwd=.`、`config=vite.config.ts` | `pkg` |
| config 探出 cwd | `root=pkg/src`、`cwd=.`、`config=../vitest.config.ts` | `pkg` |
| cwd 在 root 之上、无 config | `root=pkg/src`、`cwd=..` | `pkg` |
| 显式 `mutation.cwd='.'` | 上例 + `mutation.cwd='.'` | `pkg/src`（不是 LCA `pkg`） |
| 显式放大 | `mutation.cwd='../..'` 等 | `path.resolve(absRoot, 覆盖值)` 再相对 `projectRoot`，不改写成 LCA |
| LCA 高于仓库根 | `root=.`、`cwd=..`、未设覆盖 | `'.'`（clamp） |

### `{prefix}` 展开

- 模板常量精确为：`npx --prefix "{prefix}" stryker run "{config}"`（`shell` 与 `cmd` 相同）。
- 使用 `--prefix` 全称；禁止 `npx -p` / `--package`。`--prefix` ≠ `-p`。既有「无 `-p`」断言继续用 token 正则 `/(^|\s)-p(\s|$)/`，避免把 `--prefix` 误判为 `-p`。
- `{prefix}` = `path.resolve(projectRoot, entry.cwd)` 的**绝对路径**（Windows 可含盘符与反斜杠）。禁止相对 `entry.mutation_cwd` 的前缀（process cwd 与 exec cwd 可能不一致会漂）。
- `{config}` 替换保持现状（临时配置路径，反斜杠已在调用处归一）。
- `execSync` / `runCommand` 的 `cwd` 仍是 `entry.mutation_cwd`：相对路径（如 `'.'`）原样传递（AC-7）；抬根时可为父目录，而 prefix 指向子目录 abs cwd（AC-6）。
- 非 jest / vitest / vite-plus 仍无 `mutation_execution`；mutation JSON 权威路径、`mutate` / vitest `configFile` 相对 `mutation_cwd` 的重写均不改。

---

## 依赖

### 运行时依赖

- 无新增。继续使用 Node 内置 `path` / `child_process`，以及既有 `zod`、`@stryker-mutator/*`（由 suite cwd 的 `node_modules` 提供，经 `--prefix` 解析）。

### 构建/测试依赖

- 无新增。JSON Schema 仍由 `plugins/dev-team/build/build-config-schema.ts` 的 `generateConfigJsonSchema`（`configSchema.toJSONSchema({ io: 'input' })`）在 `vp pack` 完成时写入。

---

## 待决问题

- 层 2：抬根后 Stryker sandbox 默认 ignore `**/node_modules`，`symlinkNodeModules` 只链 `<mutation.cwd>/node_modules`。若依赖只在子目录 suite cwd，vitest 解析被测 `import` 可能失败。本刀不配 `inPlace`、不手写 symlink、不因层 2 风险拒绝抬根。失败仍记入 `ExecutionResult.mutation.error`，不阻塞测试退出码。等本刀落地后的真跑证据再在 inPlace / 手写 symlink / 配置非法时拒绝抬根之间拍板。
- 显式 `mutation.cwd` 解析结果若高于 `projectRoot`：本设计按 spec 不 clamp（clamp 仅约束缺省 LCA）。若后续要把覆盖值也关进仓库，另开变更。
