# 提案: stryker-mutation-cwd-lca

> **变更**: stryker-mutation-cwd-lca
> **日期**: 2026-08-17
> **状态**: 草稿

---

## 问题

Stryker sandbox 只能看见 process cwd 树内的文件。`mutate` 与 vitest `configFile` 已相对 `strykerRoot`（即 plan `mutation_cwd`）重写。当 `suite.config` 或 `suite.root` 探出 `suite.cwd` 时，缺省把 `mutation.cwd` 绑在 `suite.cwd` 上会让沙箱看不到配置或源码。

`resolveSuite` 现状是 `absMutationCwd = absRoot / (suite.mutation.cwd ?? suite.cwd)`；命令模板是 `npx stryker run "{config}"`；`execSync` 的 cwd 是 `entry.mutation_cwd`。npx 只从 process cwd **向上**找 `node_modules`，不会看子目录。抬根之后会出现第二道裂口：mutation.cwd 是父目录、真正的 `@stryker-mutator/*` 在 suite.cwd 时，`cd 父目录 && npx stryker` 找不到 CLI / 插件包。

本仓库 `openspec/config.json` 的 vite-plus suite 把 `root` / 默认 `cwd` / `config` 都放在 `plugins/dev-team`，LCA 与现状重合，dogfood **验不到「抬根」**。不得为了 dogfood 去拆开该 suite，以免改变本产品自己的变异沙箱范围。

---

## 提案

两刀一起落地，层 2（sandbox 内 vitest 解析被测 `import`）本刀不做：

```
mutation.cwd = LCA(absRoot, absCwd, dirname(absConfig)?)   ← Stryker 沙箱根（≥ 三者最高点）
npx --prefix <abs(suite.cwd)>                              ← 只解决层 1：找到 CLI / @stryker-mutator/*
```

1. **缺省 `mutation_cwd`**：`resolveSuite` 在未写 `suite.mutation.cwd` 时，取 `absRoot`、`absCwd`、以及（若有 `suite.config`）`dirname(absConfig)` 的目录 LCA，再 `toPosixRelative(projectRoot, …)` 写入 plan。无 config 时为 `LCA(absRoot, absCwd)`。由构造可知结果等于或高于 cwd，不会落到 cwd 里面。
2. **显式覆盖**：`suite.mutation.cwd` 一旦出现，仍 `path.resolve(absRoot, suite.mutation.cwd)`，**跳过 LCA**。用于故意放大 sandbox，或 LCA 不合适时的逃生舱。现有「root=pkg、cwd=jest、mutation.cwd=.」在不写覆盖时 LCA 已是 pkg。
3. **LCA 上沿**：结果 MUST 落在 `projectRoot` 之内（含自身）；若三者 LCA 高于 `projectRoot`，clamp 到 `path.resolve(projectRoot)`。跨盘（或无法计算共同祖先）MUST 抛错，不得落到盘符根（如 `C:\`）。
4. **`--prefix` 绝对路径**：`MUTATION_EXECUTION` 改为 `npx --prefix "{prefix}" stryker run "{config}"`。`genStrykerCommand` 将 `{prefix}` 换成 `path.resolve(projectRoot, entry.cwd)` 的绝对路径。`execSync` 的 cwd 仍是 `entry.mutation_cwd`（可 ≠ prefix）。禁止相对 mutation_cwd 的前缀（process cwd 与 exec cwd 可能不一致会漂）。仍禁止 `npx -p` 自动安装；`--prefix` ≠ `-p`。

层 2 先观察：Stryker 默认 ignore `**/node_modules`，`symlinkNodeModules` 只链 `<mutation.cwd>/node_modules`。依赖若只在子目录 cwd，sandbox 可能没有包。本刀不配 inPlace、不手写 symlink、不因层 2 风险拒绝抬根。实现 LCA + `--prefix` 后若真跑裂了再补第三刀。

验收主证据是人造 suite + mock `execSync`，不是本仓库 vite-plus mutation。本仓库真跑只作为「prefix 与 mutation.cwd 重合时不比今天更差」的可选回归。

---

## 能力

### 新增能力

- （无）

### 修改的能力

- `test-detect-frameworks` — `resolveSuite` 缺省 `mutation_cwd` 改为 LCA(root, cwd, dirname(config))；显式 `mutation.cwd` 仍覆盖
- `mutation-testing` — Stryker 命令注入绝对 `--prefix`（suite cwd）；`execSync` cwd 仍为 `mutation_cwd`；禁止 `npx -p`
- `config-schema` — `tests[].mutation.cwd` 保留；文档从「默认等于 suite cwd」改为「覆盖自动 LCA」

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/lib/test-plan.ts` — `resolveSuite` 缺省 LCA；显式 `mutation.cwd` 跳过 LCA；clamp / 跨盘报错
- `plugins/dev-team/bin/src/lib/test-framework.ts` — `MUTATION_EXECUTION` 改为 `npx --prefix "{prefix}" stryker run "{config}"`
- `plugins/dev-team/bin/src/lib/test-runner.ts` — `genStrykerCommand` 替换 `{prefix}` 为 `path.resolve(projectRoot, entry.cwd)`
- `plugins/dev-team/bin/src/schemas/config/config.schema.ts` — `mutation.cwd` 的 Zod `describe` 改为覆盖自动 LCA
- `plugins/dev-team/bin/dev-team-config.schema.json` — 同步 `mutation.cwd` description

### 测试文件

- `plugins/dev-team/bin/src/lib/test-plan.test.ts` — 层 A：人造 suite 锁 LCA 缺省 / 显式覆盖 / 无 config / clamp（LCA 高于 `projectRoot` → `mutationCwd` 为 `"."`）/ 跨盘抛错且不得为盘符根（经 `resolveAllSuites`，不启动 Stryker）
- `plugins/dev-team/bin/src/lib/test-framework.test.ts` — 模板精确为带 `{prefix}` 的 npx 命令，且继续断言没有 `-p`
- `plugins/dev-team/bin/src/lib/test-runner.test.ts` — 层 B：mock `execSync`；命令含绝对 `--prefix`；`options.cwd` 仍是 `mutation_cwd`；相对 `mutation_cwd`（如 `'.'`）时 prefix 仍绝对
- `plugins/dev-team/bin/__tests__/mutation-execution-flow/mutation-execution-flow.test.ts` — 层 B 集成：抬根时 prefix 指向子目录 cwd、process cwd 为 mutation_cwd（临时目录拼 `pkg/` + `pkg/jest/`，不必真装 `@stryker-mutator`）
- 既有 config schema 测试（若有断言「默认等于 cwd」的文案或语义）— 改为「省略字段；消费者走 LCA」

### 不要修改

- `openspec/config.json` 的 vite-plus suite（不要为 dogfood 拆开 root / cwd / config）
- Stryker sandbox 层 2：`inPlace`、手写 `node_modules` symlink、`symlinkNodeModules` 配置、因缺依赖而拒绝抬根
- `npx -p` 自动安装策略（继续禁止）
- 权威 mutation JSON 路径（仍是 `reportDir/mutation.json`）
- `mutate` / vitest `configFile` 相对 `mutation_cwd` 的既有重写
- 非 jest/vitest/vite-plus 框架的 mutation 门控（仍无 `mutation_execution`）
- CLI 集成不必为 LCA 再挂一条；plan 字段由 detect 单测锁住

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `test-plan.ts` 缺省 LCA | 人造 suite：root=`pkg`、cwd=`.`、config=`vite.config.ts` → `mutationCwd` 为 `pkg`（三者同层，与本仓库同构） |
| AC-2 | config 探出 cwd | 人造 suite：root=`pkg/src`、cwd=`.`、config=`../vitest.config.ts` → `mutationCwd` 为 `pkg` |
| AC-3 | cwd 在 root 之上且无 config | 人造 suite：root=`pkg/src`、cwd=`..`、无 config → `mutationCwd` 为 `pkg`（`LCA(root, cwd)`） |
| AC-4 | 显式 `mutation.cwd` 覆盖 | 上例加 `mutation.cwd='.'` 或 `'../..'` → `mutationCwd` 为覆盖解析值，不是 LCA |
| AC-5 | 无 config 的 LCA | 仅 root + cwd → `mutationCwd` 为二者目录 LCA |
| AC-6 | `--prefix` 注入 | mock `execSync`：命令含 `npx --prefix <绝对 cwd>`，且 **没有** `-p`；`options.cwd` 仍是 `entry.mutation_cwd`（可 ≠ prefix） |
| AC-7 | 相对 mutation_cwd | plan `mutation_cwd` 为 `'.'` 时仍原样传给 stryker cwd；`{prefix}` 仍是 `path.resolve(projectRoot, entry.cwd)` 的绝对路径 |
| AC-8 | schema 文档 | Zod / JSON schema 将 `mutation.cwd` 描述为覆盖自动 LCA；省略该字段时 parse 结果仍不写入默认字符串（消费者计算 LCA） |
| AC-9 | 本仓库回归（可选，非主证据） | 真跑时命令为 `npx --prefix <abs plugins/dev-team> stryker run …`，process cwd 仍是 `plugins/dev-team`；不比今天更差 |
| AC-10 | `test-plan.ts` LCA 上沿 clamp | 对象：`resolveAllSuites` / `ResolvedSuite.mutationCwd`。条件：缺省 LCA 绝对路径高于 `projectRoot`（例如 suite `root="."`、`cwd=".."`，未设 `mutation.cwd`）→ `mutationCwd` 为 `"."`（clamp 到 `path.resolve(projectRoot)` 后再 `toPosixRelative`）；不得把 `projectRoot` 的父目录写入 plan `mutation_cwd` |
| AC-11 | `test-plan.ts` 跨盘抛错 | 对象：`resolveAllSuites`。条件：`absRoot` / `absCwd` / `dirname(absConfig)` 不在同一盘符或无法计算共同祖先，且未设 `mutation.cwd` → SHALL 抛错；不得把盘符根（如 `C:\`）作为 `mutation_cwd` 返回 |

层 2「抬根后 sandbox 里找不到依赖」**先不验**。

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 抬根后 sandbox 无子目录 `node_modules`，vitest 解析被测 import 失败 | 嵌套包 suite 的 mutation 执行失败 | 中 | 本刀明确不做层 2；失败记入 `ExecutionResult.mutation.error`，不阻塞测试退出码；裂了再补 inPlace / symlink / 拒绝抬根 |
| `--prefix` 与 `mutation_cwd` 分离后 npx 仍找不到包 | CLI / `@stryker-mutator/*` 解析失败 | 低 | `{prefix}` 使用 suite cwd 绝对路径；单测锁命令形态与「无 `-p`」 |
| LCA 跨盘或逃出仓库落到盘符根 | sandbox 扫到无关文件系统 | 低 | clamp 到 `projectRoot`；无共同祖先则抛错 |
| 本仓库 dogfood 验不到抬根 | 误把「prefix == mutation.cwd」当成抬根已验 | 高 | 验收分层：LCA 靠 `test-plan` 人造 suite；prefix 靠 mock；真跑只作回归 |
| 显式 `mutation.cwd` 被 LCA 覆盖 | 故意放大 sandbox 的配置失效 | 低 | 字段一旦出现即跳过 LCA；单测锁覆盖值 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| sandbox `node_modules`（层 2） | 本刀不指定 inPlace / 手写 symlink / 拒绝抬根 | `--prefix` 只保证层 1；层 2 等真跑打脸再补，避免过早绑 Stryker 内部行为 | 第一刀就配 inPlace 或拒绝探出 cwd 的 LCA |
| 显式 `tests[].mutation.cwd` | 保留，且跳过 LCA | 逃生舱与故意放大 sandbox；不写覆盖时 LCA 已覆盖「root=pkg、cwd=jest」 | 删除覆盖字段，一律 LCA |
| `--prefix` 路径形态 | 绝对路径 = `path.resolve(projectRoot, entry.cwd)` | exec cwd 与 CLI process cwd 可能不一致，相对前缀会漂 | 相对 `mutation_cwd` 的前缀 |
| `npx -p` | 继续禁止 | 不得自动安装；`--prefix` ≠ `-p` | 允许 `-p` 兜底缺包 |
| LCA 上沿 | clamp 到 `projectRoot` | sandbox 不得逃出仓库 | 允许高于 projectRoot；或一律抛错 |
| 跨盘无共同祖先 | 抛错，不得使用盘符根 | 落到 `C:\` 会让 Stryker 看见整盘 | clamp 到 projectRoot（跨盘时 LCA 根本不存在） |
| 本仓库 `openspec/config.json` | 不改 suite 拆分 | 改了会改变本产品变异沙箱范围，且仍验不到抬根 | 人为把 root/cwd/config 拆开做 dogfood |
| 真 Stryker 嵌套 fixture | 第一刀不做 | 慢、要装依赖；层 2 未拍板 | 本刀就加嵌套包 fixture |

### 待决问题

- 层 2 若在真实嵌套 `node_modules` 下裂开：补 inPlace、手写 symlink，还是配置非法时拒绝抬根——等本刀落地后的真跑证据再拍板。
