# 测试环境探测

首次调用时扫描仓库，生成 `openspec/test-profile.md`；之后每次直接读该文件。

## 首轮扫描

```
① 找包
   glob **/package.json，排除 node_modules / dist / build / target / .git / .next / coverage
   ├─ 一个 → 单包
   └─ 多个 → 多包（并列；不假设 workspace 根）

② 每包内找测试入口（按可靠度排序，命中即停）
   a. package.json scripts.test
   b. devDependencies / dependencies 里的框架包
   c. 配置文件存在性与内容
   d. projects 字段
```

### ① 找包

不要读 `pnpm-workspace.yaml` / `lerna.json` / `nx.json` 来推导包列表——很多仓库（包括本仓库）没有真正的 workspace 根，只有各包自己的配置。直接扫 `package.json` 最可靠。

多个包时**并列列出**，不推断父子关系。

### ② 找测试入口

按顺序尝试，**命中即停**：

**a. `scripts.test`**

```json
{ "scripts": { "test": "vitest run" } }
```

最可靠——这是项目作者写下的、确实能跑的命令。直接用，不改语义。

若 `scripts.test` 聚合了非 JS 套件（如 `pnpm run client:test && pnpm run server:test`，后者是 cargo），只取 JS 那部分并记入画像：

```
pnpm -C <包路径> run client:test
```

，并在扫描备注里说明拆分原因。聚合命令本身不要直接记入画像——它会把非 JS 套件一起跑起来。

**b. 依赖里的框架包**

在 `dependencies` + `devDependencies` 中查找：

| 包名 | 框架 |
|---|---|
| `vite-plus` | vite-plus |
| `vitest`、`@vitest/coverage-v8` | vitest |
| `jest`、`ts-jest`、`babel-jest` | jest |
| `bun-types`、或 `bun test` 出现在脚本中 | bun |

**c. 配置文件**

| 文件 | 提取 |
|---|---|
| `vite.config.{ts,js,mts}` | `test.include` → 测试文件 glob；`test.environment` → 环境 |
| `vitest.config.{ts,js,mts}` | `test.include` / `test.environment` |
| `jest.config.{js,ts,mjs,cjs,json}` | `testMatch` / `testEnvironment` |

vite 与 vitest 共用 `vite.config.ts`，其中 `test` 节即 vitest 配置。

**d. `projects` 字段**

一个包可能拆成多个测试域：

```js
// jest.config.js
export default {
  projects: [
    { displayName: 'web', testMatch: ['<rootDir>/web/**/*.test.ts'] },
    { displayName: 'lib', testMatch: ['<rootDir>/lib/**/*.test.ts'] },
  ],
}
```

```ts
// vite.config.ts
export default defineConfig({
  test: {
    projects: [
      { test: { name: 'web', include: ['web/**/*.test.ts'] } },
    ],
  },
})
```

每个 project 记为一行测试域，**不要合并**。

测试域命名降级顺序：

1. `displayName`（jest）/ `name`（vitest project）
2. project 对象所在的键或索引
3. project 的 `rootDir`

### ③ 记录字段

每个测试域记录：

| 字段 | 来源 | 缺失时 |
|---|---|---|
| 测试域 | 见上方降级规则 | 用包名 |
| 包 | `package.json` 的 `name` | 目录名 |
| 路径 | 包所在目录（相对项目根，POSIX） | — |
| 测试命令 | `scripts.test`，或按框架推导 | 按框架表推导并标注"推断" |
| 框架 | 依赖或配置 | — |
| 配置文件 | 命中的配置路径 | 留空 |
| 测试文件 glob | 配置的 include / testMatch | 用 `**/*.test.{ts,tsx,js,jsx}` |
| 突变配置 | `stryker.config.*` 是否存在 | 留空 |

**不记录版本号**——版本易变，命令稳定。命令表见 `frameworks.md`。

### ④ 四级全不中

询问用户，不要凭文件扩展名猜框架。用户回答后照样写入画像并标注为"用户指定"。

## 画像格式

```markdown
# 测试环境画像

> 由 js-unit-test 首轮扫描生成。后续执行直接读取，不再扫描。
> 手动修改本文件即视为新的确认状态。

## 包与测试域

| 测试域 | 包 | 路径 | 测试命令 | 框架 | 配置文件 | 测试文件 glob | 突变配置 |
|---|---|---|---|---|---|---|---|
| desktop | desktop | packages/desktop | pnpm -C packages/desktop run client:test | vite-plus | packages/desktop/vite.config.ts | src/**/*.test.{ts,tsx} | packages/desktop/stryker.config.json |

## 扫描备注

- 无根 package.json，未检测到 workspace 根；上表为并列包
- 命令取自各包 `scripts.test`（非推断）
- 未做框架版本探测

## 变更记录

- YYYY-MM-DD 首次生成，用户确认
```

生成后**必须展示给用户确认**，确认后才作为后续运行的依据。

## 过期判定

每次读取画像后执行（不产生扫描成本）：

```
画像记录的每个包路径仍存在？
  否 → 提示过期，询问是否重扫
任一 package.json 的 mtime 晚于画像文件的 mtime？
  是 → 提示可能过期，询问是否重扫
  否 → 直接使用
```

**永不自动重扫。** 自动重扫即回到"每次执行都扫描"，违背本 skill 的设计目标。

用户要求重扫时，重走首轮扫描并覆盖画像，再次请求确认。用户选择不重扫时，按现有画像继续。

## 多包时的目标选择

画像可能列出多个测试域。根据用户给定的范围判断命中哪一个，路径前缀匹配优先；无法判断时询问用户。
