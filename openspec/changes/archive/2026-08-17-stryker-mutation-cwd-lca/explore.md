# Stryker mutation.cwd = LCA(root, cwd, config) + npx --prefix

探索时间：2026-08-17

无进行中 change。现状：`resolveSuite` 里 `mutation.cwd` 缺省等于 `suite.cwd`；命令模板为 `npx stryker run "{config}"`；`execSync` 的 cwd 为 `entry.mutation_cwd`。npx 只从 process cwd **向上**找 `node_modules`，不会看子目录。

本仓库 `openspec/config.json` 的 vite-plus suite：`root` / 默认 `cwd` / `config` 都在 `plugins/dev-team`，LCA 与现状重合，**dogfood 验不到「抬根」**。

---

## 问题形状

Stryker sandbox 只看得见 process cwd 树内文件。`mutate`、vitest `configFile` 已相对 `strykerRoot` 重写。当 `suite.config` 或 `suite.root` 探出 `suite.cwd` 时，mutation 绑在 cwd 上会裂。

抬根之后 npx 的盲区：mutation.cwd 是父目录、真·node_modules 在 suite.cwd 时，`cd 父目录 && npx stryker` 找不到包。

```
mutation.cwd = LCA(root, cwd, dirname(config))   ← Stryker 沙箱根（≥ 三者最高点）
npx --prefix <abs(suite.cwd)>                    ← 只解决「找到 CLI / 插件包」
```

LCA 由构造可知：等于或高于 cwd，不会落到 cwd 里面。

---

## 已拍板

### 1. sandbox `node_modules`（层 2）先不指定

`--prefix` 只保证层 1（Stryker CLI + `@stryker-mutator/*` 解析）。

层 2：sandbox 里 vitest 解析被测代码 `import`。Stryker 默认 ignore `**/node_modules`，`symlinkNodeModules` 只链 `<mutation.cwd>/node_modules`。若依赖在子目录 cwd，sandbox 可能没有包。

**先不配 inPlace / 手写 symlink / 拒绝抬根。** 实现 LCA + `--prefix` 后看 Stryker 自己能否跑通；裂了再补第三刀。

### 2. 保留 `tests[].mutation.cwd` 覆盖

- **缺省**：`LCA(absRoot, absCwd, dirname(absConfig)?)`，再 `toPosixRelative(projectRoot, …)` 写入 plan
- **显式 `mutation.cwd`**：仍 `path.resolve(absRoot, suite.mutation.cwd)`，**跳过 LCA**
- 无 config：LCA(root, cwd)

现有「root=pkg、cwd=jest、mutation.cwd=.」场景，不写覆盖时 LCA 已是 pkg；覆盖主要用于故意放大 sandbox，或 LCA 算出来不合适时的逃生舱。

### 3. `--prefix` 用绝对路径

模板从 `npx stryker run "{config}"` 改为带 `{prefix}`（或 execute 时注入）。`{prefix}` = `path.resolve(projectRoot, entry.cwd)` 的绝对路径。

不用相对 mutation.cwd 的相对路径：`execSync` cwd 与 CLI process cwd 可能不一致，相对前缀会漂。

仍禁止 `npx -p` 自动安装；`--prefix` ≠ `-p`。现有「精确等于 `npx stryker run "{config}"`」断言要改，并继续断言没有 `-p`。

---

## 落点（未实现，仅对照）

| 处 | 改什么 |
|----|--------|
| `test-plan.ts` `resolveSuite` | 缺省 LCA；显式 `mutation.cwd` 覆盖 |
| `test-framework.ts` `MUTATION_EXECUTION` | `npx --prefix "{prefix}" stryker run "{config}"` |
| `test-runner.ts` `genStrykerCommand` | 替换 `{prefix}` 为 abs(cwd)；已有 `{config}` |
| schema `mutation.cwd` | 保留；描述改为覆盖自动 LCA |

建议（未拍板）：LCA 上沿 clamp 到 `projectRoot`；跨盘无共同祖先则报错，不要落到 `C:\`。

---

## 4. 验收分层（本仓库 dogfood 不够）

本仓库 suite 不变 ⇒ 真正的 vite-plus mutation **只覆盖「prefix == mutation.cwd == 包目录」的回归**，覆盖不了抬根 / 覆盖项 / 嵌套 node_modules。

验收应拆成三层，不要为了 dogfood 去改 `openspec/config.json` 把 root/cwd/config 拆开（那会改变本产品自己的变异沙箱范围）。

### 层 A — LCA 纯路径（detect）

落在已有 `test-plan.test.ts` 的 `resolveAllSuites`。不启动 Stryker。

人造 suite 即可，例如：

| 场景 | suite | 期望 `mutationCwd` |
|------|--------|-------------------|
| 三者同层（本仓库同构） | root=`pkg`, cwd=`.`, config=`vite.config.ts` | `pkg` |
| config 探出 cwd | root=`pkg/src`, cwd=`.`, config=`../vitest.config.ts` | `pkg` |
| cwd 在 root 之上（已有用例的变体） | root=`pkg/src`, cwd=`..` | `pkg`（无 config 时 LCA(root,cwd)） |
| 显式覆盖 | 上例 + `mutation.cwd='.'` 或 `'../..'` | 覆盖值，不是 LCA |
| 无 config | root + cwd | LCA(root, cwd) |

CLI 集成不必为 LCA 再挂一条；plan 字段由 detect 单测锁住即可。

### 层 B — `--prefix` 注入（execute，mock execSync）

现有 `test-runner.test.ts` / `mutation-execution-flow` 已 mock `execSync`。补：

- 命令含 `npx --prefix <绝对 cwd>`，且 **没有** `-p`
- `execSync` options.cwd 仍是 `entry.mutation_cwd`（可 ≠ prefix）
- 相对 `mutation_cwd`（如 `'.'`）「原样传给 stryker cwd」的旧行为：prefix 仍必须是绝对的（相对的是 sandbox cwd，不是 prefix）

「抬根 + prefix 指向子目录」用临时目录拼 `pkg/` + `pkg/jest/` 即可，**不必真装 @stryker-mutator**。

### 层 C — 真 Stryker（本仓库 dogfood）

`dev-team test-execution`（或现有 mutation 流程）在本仓库上：

- 命令变为 `npx --prefix <abs plugins/dev-team> stryker run …`
- process cwd 仍是 `plugins/dev-team`
- 这是层 2「先看 Stryker 自己能否适配」的**最小真实探针**：prefix 与 mutation.cwd 重合时不应比今天更差

它**不能**代替：

- LCA 抬根（config/root 在 cwd 外）
- 显式 `mutation.cwd` 覆盖
- 层 2 的危险形态：mutation.cwd 在上、node_modules 只在子目录 cwd

后者等拍板 1 被打脸时再加「真跑 Stryker 的嵌套包 fixture」（慢、要装依赖）。第一刀不要做。

### 结论

| 要验的行为 | 靠什么 |
|------------|--------|
| LCA 缺省 / 显式覆盖 | `test-plan` 人造 suite |
| `--prefix` 绝对路径、cwd 分离 | mock `execSync` 的 runner/flow 测试 |
| prefix 不破坏现有包内 mutation | 本仓库真跑（可选，作回归而非主证据） |
| 抬根后 sandbox 里找不到依赖 | **先不验**；裂了再补层 2 |

主证据是 fixture + mock，不是本仓库 vite-plus mutation。
