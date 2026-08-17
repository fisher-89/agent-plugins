## Module Contract

### Module: lib/test-plan.ts (`resolveSuite` / `mutationCwd`)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/lib/test-plan.ts` |
| **Exports** | `resolveAllSuites`（及既有 `ResolvedSuite.mutationCwd`）；LCA 实现 MUST NOT 仅为测试而额外 export |
| **缺省** | `absMutationCwd = LCA(absRoot, absCwd, dirname(absConfig)?)`，再 `toPosixRelative(projectRoot, …)`；无 `suite.config` 时省略第三元 |
| **覆盖** | `suite.mutation.cwd` 一旦出现：`path.resolve(absRoot, suite.mutation.cwd)`，跳过 LCA |
| **上沿** | LCA 结果 MUST 位于 `projectRoot` 之内（含自身）；高于 `projectRoot` 则 clamp 到 `path.resolve(projectRoot)`，`mutationCwd` SHALL 为 `"."` |
| **跨盘** | 无共同祖先时 SHALL 抛错；MUST NOT 使用盘符根（如 `C:\`）作为 `mutation_cwd` |
| **Consumers** | `commands/test-detect-frameworks.ts` 将 `resolved.mutationCwd` 写入 `plan[].mutation_cwd` |

### Module: commands/test-detect-frameworks.ts (Plan `mutation_cwd`)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` |
| **Plan fields** | `mutation_cwd` 仍为相对 `projectRoot` 的 POSIX 路径（Stryker sandbox / 临时 config 根）；缺省来源从 `suite.mutation.cwd ?? suite.cwd` 改为 LCA（见上） |
| **Shared lib** | 仍只调用一次 `resolveAllSuites`；本模块不重复实现 LCA |

---

## MODIFIED Requirements

### Requirement: Plan built from tests array suites

**ID**: REQ-TDF-SUITE-1
**Priority**: MUST
**Description**: `runTestDetectFrameworks` SHALL read `config.tests`（而非 `config.test.framework` / `config.test.overrides`）构建执行计划。`plan` 与 `config.tests` **一一对应**：每个 suite 产生一条 `plan` 条目，不得因相同 `cwd` / `framework` 合并；不得再使用 `deriveWorkingDirectory(glob)`。调用方 SHALL 保证各 suite 测试范围互不重叠（重叠属配置错误，本模块不检测、不合并）。

对每个 suite：
1. `absRoot = projectRoot / suite.root`
2. `absCwd = absRoot / suite.cwd`
3. `absMutationCwd`：若 `suite.mutation.cwd` 已出现，则为 `absRoot / suite.mutation.cwd`；否则为 `LCA(absRoot, absCwd, dirname(absConfig)?)`（无 `suite.config` 时省略第三元），再按 REQ-TDF-MUT-CWD-1 clamp / 报错
4. `plan.cwd` = absCwd 相对于 `projectRoot` 的 POSIX 相对路径（运行目录）
5. `plan.root` = absRoot 相对于 `projectRoot` 的 POSIX 相对路径（测试范围根）
6. `plan.mutation_cwd` = absMutationCwd 相对于 `projectRoot` 的 POSIX 相对路径（Stryker sandbox / 临时 config 根）
7. `framework` / coverage 元数据 / mutation_framework 来自 `FRAMEWORK_REGISTRY`
8. `mutation_score` 来自该 suite 解析后的 `mutation.score`
9. include 匹配 glob = `suite.includes` 若存在，否则为框架 `default_glob`；匹配时相对 `suite.root`（实现可将 glob 拼为 projectRoot 相对形式 `root/includes`）

模块 SHALL 删除或停止调用 `deriveWorkingDirectory` 作为 cwd 来源。

执行时：CLI 路径过滤 `pathFilter = plan.root` 相对于 `plan.cwd`（二者相同时为 `"."`）。省略 `files`（未传列表）时，空 `{files}` SHALL 使用 `pathFilter`（`"."` 时展开为空，因 cwd 已等于 root）。显式 `files` 列表优先：SHALL 按 `plan.root` 过滤后再转为相对 `plan.cwd` 的路径；若某 plan 与显式列表无交集，CLI SHALL **跳过**该 plan（不得回落为全 suite 发现）。`{directory}`（go）SHALL 展开为 `./...` 或 `./<pathFilter>/...`。rust 的 `cargo test` 不支持任意子目录 CLI 裁剪，SHALL 保持 crate 级行为（`pathFilter` 不改变其 `test_execution`）。

Suite 路径解析（`resolveSuite` / `resolveAllSuites`）与文件归属（`isInSuiteScope`）SHALL 由 `lib/test-plan.ts` 提供；`runTestDetectFrameworks` SHALL 对 `config.tests` **只 resolve 一次**，再同时用于 `detected` 匹配与 `plan` 构建。

#### Scenario: cwd equals root when suite cwd is default

**WHEN** config has `tests: [{ root: "plugins/dev-team/bin", framework: "vite-plus" }]`
**AND** `runTestDetectFrameworks({})` is called
**THEN** `plan` SHALL contain one entry with `cwd: "plugins/dev-team/bin"`
**AND** `plan[0].root` SHALL be `"plugins/dev-team/bin"`
**AND** `plan[0].framework` SHALL be `"vite-plus"`
**AND** `plan[0].mutation_score` SHALL equal the suite schema default mutation score

#### Scenario: cwd resolves parent of suite root

**WHEN** config has `tests: [{ root: "plugins/dev-team/bin/src", cwd: "..", framework: "vite-plus" }]`
**AND** `runTestDetectFrameworks({})` is called
**THEN** `plan[0].cwd` SHALL be `"plugins/dev-team/bin"`
**AND** `plan[0].root` SHALL be `"plugins/dev-team/bin/src"`

#### Scenario: same cwd and framework with different roots yields two plans

**WHEN** config has `tests: [{ root: "pkg/a", cwd: "..", framework: "jest" }, { root: "pkg/b", cwd: "..", framework: "jest" }]`
**AND** `runTestDetectFrameworks({})` is called
**THEN** `plan` SHALL have length 2
**AND** `plan[0].cwd` and `plan[1].cwd` SHALL both be `"pkg"`
**AND** `plan[0].framework` and `plan[1].framework` SHALL both be `"jest"`
**AND** `plan[0].root` SHALL be `"pkg/a"`
**AND** `plan[1].root` SHALL be `"pkg/b"`

#### Scenario: omitted files falls back to pathFilter under parent cwd

**WHEN** config has `tests: [{ root: "plugins/dev-team/bin/src", cwd: "..", framework: "vite-plus" }]`
**AND** the plan entry is executed with `files` omitted (not an explicit empty filter after root matching)
**THEN** the substituted command SHALL include the path filter `src` in place of `{files}`
**AND** SHALL NOT discover tests outside that suite root solely due to a wider absCwd

#### Scenario: explicit files with no overlap skips plan

**WHEN** config has two suites with roots `pkg/a` and `pkg/b`
**AND** `dev-team test-execution --files pkg/a/foo.test.ts` runs
**THEN** the plan for `pkg/a` SHALL execute
**AND** the plan for `pkg/b` SHALL be skipped (no execute / no full-scope fallback)

#### Scenario: empty tests yields empty plan

**WHEN** config has `tests: []` or omits tests（parse 后为空数组）
**AND** `runTestDetectFrameworks({})` is called
**THEN** `plan` SHALL be `[]`

#### Scenario: multiple suites produce multiple plan entries

**WHEN** config has two suites with different roots/frameworks
**AND** `runTestDetectFrameworks({})` is called
**THEN** `plan.length` SHALL be 2
**AND** each entry's `cwd` SHALL reflect that suite's absCwd
**AND** each entry's `root` SHALL reflect that suite's absRoot

#### Scenario: default mutation_cwd follows LCA not suite cwd

**WHEN** config has `tests: [{ root: "pkg/src", cwd: "..", framework: "vitest" }]` with no `mutation.cwd`
**AND** `resolveAllSuites` / `runTestDetectFrameworks({})` is called
**THEN** `plan[0].mutation_cwd` SHALL be `"pkg"`（`LCA(absRoot, absCwd)`）
**AND** SHALL NOT equal `"pkg/src"` solely because that is `suite.root`

---

## ADDED Requirements

### Requirement: 缺省 mutation_cwd 为 LCA(root, cwd, dirname(config))

**ID**: REQ-TDF-MUT-CWD-1
**Priority**: MUST
**Description**: `lib/test-plan.ts` 的 `resolveSuite` SHALL 按下列规则计算 `absMutationCwd`，再经 `toPosixRelative(projectRoot, absMutationCwd)` 得到 `ResolvedSuite.mutationCwd` / plan `mutation_cwd`：

1. `absRoot = path.resolve(projectRoot, suite.root)`
2. `absCwd = path.resolve(absRoot, suite.cwd)`
3. `absConfig = suite.config ? path.resolve(absRoot, suite.config) : null`
4. **显式覆盖**：若 `suite.mutation.cwd` 字段已出现（schema 解析后为 string），`absMutationCwd = path.resolve(absRoot, suite.mutation.cwd)`，SHALL NOT 再计算 LCA
5. **缺省 LCA**：否则 `absMutationCwd` SHALL 为目录 `absRoot`、`absCwd`、以及（当 `absConfig` 非 null）`path.dirname(absConfig)` 的最长共同祖先。无 config 时为 `LCA(absRoot, absCwd)`
6. **上沿**（proposal AC-10）：若 LCA 结果不在 `projectRoot` 之内（`path.relative(projectRoot, lca)` 以 `..` 开头或为另一盘符绝对路径），SHALL clamp 到 `path.resolve(projectRoot)`，因此 `mutationCwd` SHALL 为 `"."`
7. **跨盘**（proposal AC-11）：输入路径无共同祖先时 SHALL 抛错；MUST NOT 将盘符根（如 `C:\`）写入 `mutation_cwd`

由构造可知：缺省 LCA 等于或高于 `absCwd`，SHALL NOT 落到 `absCwd` 的子目录。本规则由 `resolveAllSuites` 单测锁住即可；CLI 集成不必为 LCA 再挂一条。

#### Scenario: 三者同层（本仓库同构）

**WHEN** suite 为 `{ root: "pkg", cwd: ".", framework: "vitest", config: "vite.config.ts" }`
**AND** 未设置 `mutation.cwd`
**AND** `resolveAllSuites` 针对该 suite 被调用
**THEN** `mutationCwd` SHALL 为 `"pkg"`

#### Scenario: config 探出 cwd

**WHEN** suite 为 `{ root: "pkg/src", cwd: ".", framework: "vitest", config: "../vitest.config.ts" }`
**AND** 未设置 `mutation.cwd`
**AND** `resolveAllSuites` 被调用
**THEN** `mutationCwd` SHALL 为 `"pkg"`

#### Scenario: cwd 在 root 之上且无 config

**WHEN** suite 为 `{ root: "pkg/src", cwd: "..", framework: "vitest" }`
**AND** 未设置 `config` 与 `mutation.cwd`
**AND** `resolveAllSuites` 被调用
**THEN** `mutationCwd` SHALL 为 `"pkg"`
**AND** SHALL 等于 `LCA(absRoot, absCwd)`，不是 `"pkg/src"`

#### Scenario: 显式 mutation.cwd 覆盖 LCA

**WHEN** suite 为 `{ root: "pkg/src", cwd: "..", framework: "vitest", mutation: { cwd: "." } }`
**AND** `resolveAllSuites` 被调用
**THEN** `mutationCwd` SHALL 为 `"pkg/src"`（`path.resolve(absRoot, ".")`）
**AND** SHALL NOT 为 LCA 结果 `"pkg"`

#### Scenario: 显式 mutation.cwd 放大 sandbox

**WHEN** suite 为 `{ root: "pkg/src", cwd: ".", framework: "vitest", mutation: { cwd: "../.." } }`
**AND** `resolveAllSuites` 被调用
**THEN** `mutationCwd` SHALL 为覆盖解析后相对 `projectRoot` 的 POSIX 路径
**AND** SHALL NOT 被改写为 `LCA(absRoot, absCwd)`

#### Scenario: 无 config 时 LCA(root, cwd)

**WHEN** suite 仅含 `root` 与 `cwd`（无 `config`、无 `mutation.cwd`）
**AND** `absRoot` 与 `absCwd` 不相同
**AND** `resolveAllSuites` 被调用
**THEN** `mutationCwd` SHALL 为二者目录 LCA 相对 `projectRoot` 的 POSIX 路径

#### Scenario: LCA 高于 projectRoot 时 clamp 为 "."

**WHEN** `projectRoot` 为某仓库根，suite 为 `{ root: ".", cwd: "..", framework: "vitest" }`（或缺省 LCA 绝对路径为 `projectRoot` 的父目录的等价人造 suite）
**AND** 未设置 `mutation.cwd`
**AND** `resolveAllSuites` 被调用
**THEN** `absMutationCwd` SHALL 为 `path.resolve(projectRoot)`（clamp，不是父目录）
**AND** `mutationCwd` SHALL 为 `"."`
**AND** plan `mutation_cwd` SHALL NOT 为 `projectRoot` 之上的相对路径（如 `".."`）

#### Scenario: 跨盘无共同祖先时抛错且不得为盘符根

**WHEN** 缺省 LCA 的输入中 `absRoot` / `absCwd` / `dirname(absConfig)` 至少两者不在同一盘符（例如 `absRoot` 在 `C:\...`、`absCwd` 在 `D:\...`），或无法计算共同祖先
**AND** 未设置 `suite.mutation.cwd`
**AND** `resolveAllSuites` 被调用
**THEN** SHALL 抛错（不得返回成功的 `ResolvedSuite` / plan）
**AND** SHALL NOT 将盘符根（如 `C:\` 或 `D:\`）作为 `mutationCwd` / plan `mutation_cwd` 返回
