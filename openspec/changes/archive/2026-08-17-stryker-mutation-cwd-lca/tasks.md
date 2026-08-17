# 任务: stryker-mutation-cwd-lca

> **变更**: stryker-mutation-cwd-lca
> **日期**: 2026-08-17

---

## 阶段 1: Schema 文档（mutation.cwd 覆盖 LCA）

- [x] 修改 `plugins/dev-team/bin/src/schemas/config/config.schema.ts`：`mutationConfigSchema.cwd` 的 `.describe(...)` 改为说明该字段相对 `root`、出现时覆盖 detect 侧自动 LCA（`LCA(absRoot, absCwd, dirname(absConfig)?)`）；删除「默认等于cwd」或等价表述
- [x] 保持 `cwd` 为 `z.string().optional()`，不得加字符串 `prefault`；省略时 parse 结果不写入 `mutation.cwd`
- [x] 同步 `plugins/dev-team/bin/dev-team-config.schema.json` 中 `properties.tests.items.properties.mutation.properties.cwd.description` 与 Zod 文案一致（手改或调用 `generateConfigJsonSchema()`）

## 阶段 2: resolveSuite 缺省 LCA

- [x] 在 `plugins/dev-team/bin/src/lib/test-plan.ts` 增加模块内私有目录 LCA 辅助（例如 `directoryLca`、`isInsideProjectRoot`），**不 export**；使用平台 `path`（`path.resolve` / `path.dirname` / `path.parse(p).root` / `path.relative`），两两归约
- [x] `resolveSuite`：`absRoot` / `absCwd` / `absConfig` 计算保持现状；若 `suite.mutation.cwd !== undefined`，`absMutationCwd = path.resolve(absRoot, suite.mutation.cwd)` 并跳过 LCA 与 clamp
- [x] 缺省路径：`dirs = [absRoot, absCwd]`，有 `absConfig` 时再加入 `path.dirname(absConfig)`；`absMutationCwd = directoryLca(dirs)`
- [x] 跨盘或无法得到共同祖先时抛 `Error`（文案标明 `mutation_cwd` / 共同祖先 / 跨盘）；不得把盘符根（如 `C:\`）当作 `mutation_cwd` 返回
- [x] 仅对缺省 LCA：若结果不在 `path.resolve(projectRoot)` 之内（`relative` 以 `..` 开头或为另一盘符绝对路径），clamp 到 `path.resolve(projectRoot)`，使 `mutationCwd` 为 `"."`
- [x] 继续用既有 `toPosixRelative(projectRoot, absMutationCwd)` 写入 `ResolvedSuite.mutationCwd`；更新该字段注释。拆分私有函数以遵守 `max-lines-per-function` 50
- [x] 不修改 `commands/test-detect-frameworks.ts`：仍只调用一次 `resolveAllSuites`，把 `resolved.mutationCwd` 写入 `plan.mutation_cwd`

## 阶段 3: npx --prefix 模板

- [x] 修改 `plugins/dev-team/bin/src/lib/test-framework.ts`：将 `MUTATION_EXECUTION` 改为 `npx --prefix "{prefix}" stryker run "{config}"`（精确该字符串）
- [x] 确认 jest / vitest / vite-plus 的 `shell.mutation_execution` 与 `cmd.mutation_execution` 仍返回该常量；其余框架继续省略 `mutation_execution`
- [x] 更新邻近注释：使用 `--prefix` 定位项目本地包；继续禁止 `npx -p` / `--package` 自动安装（`--prefix` ≠ `-p`）

## 阶段 4: genStrykerCommand 展开绝对 prefix

- [x] 修改 `plugins/dev-team/bin/src/lib/test-runner.ts` 的 `genStrykerCommand`：增加 `projectRoot: string` 参数；在既有 `{config}` 替换之外，将 `{prefix}` 替换为 `path.resolve(projectRoot, entry.cwd)` 的绝对路径（不要相对 `mutation_cwd`，不要 `toForwardSlash`）
- [x] 让 `executeStrykerMutation`（及调用链上的 `runMutationPhase`）把已有的 `projectRoot` 传入 `genStrykerCommand`
- [x] 保持 `runCommand(strykerCmd, strykerRoot, …)` 的 `strykerRoot` 为 `entry.mutation_cwd`（相对路径如 `'.'` 原样传递）；不改 `resolveStrykerConfig`、权威 `reportDir/mutation.json`、层 2 symlink / `inPlace`

## 阶段 5: 版本与产物

- [x] 按项目规则将 `plugins/dev-team/package.json` 的 `version` patch bump（`2.10.30` → `2.10.31`）
- [x] 在 `plugins/dev-team` 执行构建（`pnpm -C plugins/dev-team run build`），刷新 `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/`
- [x] **不要**修改 `openspec/config.json` 的 vite-plus suite（不要为 dogfood 拆开 `root` / `cwd` / `config`）
