# 框架识别与运行命令

## 优先级

1. **`package.json` 的 `scripts.test`** — 直接用，不改语义
2. 无 `scripts.test` 时，按下表推导命令，并在画像中标注"推断"

## 命令模板

| 框架 | 命令 | 说明 |
|---|---|---|
| vite-plus | `npm exec -- vp test` | 或 `pnpm exec vp test` / `pnpm test`（若脚本已定义） |
| vitest | `npm exec -- vitest run` | 必须带 `run`，否则进入 watch 模式 |
| jest | `npm exec -- jest` | 默认单次运行 |
| bun | `bun test` | 内置运行器 |

**必须带非交互标志。** vitest 不带 `run` 会挂起等待按键。

## 包管理器

按锁文件判断：

| 锁文件 | 命令前缀 |
|---|---|
| `pnpm-lock.yaml` | `pnpm` |
| `yarn.lock` | `yarn` |
| `bun.lockb` / `bun.lock` | `bun` |
| `package-lock.json` 或无 | `npm` |

在包目录内运行，或用包管理器的工作区语法（如 `pnpm -C <包路径> <命令>`）。

## 常见 `scripts.test` 形态

```json
{ "scripts": { "test": "vitest run" } }
{ "scripts": { "test": "vp test --sequence.shuffle" } }
{ "scripts": { "test": "jest --coverage" } }
{ "scripts": { "test": "pnpm run client:test && pnpm run server:test" } }
```

最后一种是聚合命令。若它同时包含 JS 与非 JS 套件（如 cargo），只用 JS 那部分：

```
pnpm -C <包路径> run client:test
```

画像中记录**实际要跑的**那条，不是聚合命令。并在扫描备注里说明拆分原因。

## 配置识别

| 配置文件 | 框架 | 关键字段 |
|---|---|---|
| `vite.config.{ts,js,mts}` | vite-plus / vitest | `test.include`、`test.environment`、`test.projects` |
| `vitest.config.{ts,js,mts}` | vitest | 同上 |
| `jest.config.{js,ts,mjs,cjs,json}` | jest | `testMatch`、`testEnvironment`、`projects` |

注意：`vite.config.ts` 可能只有构建配置而无 `test` 节——此时该文件不能证明项目用 vitest。以依赖或 `scripts.test` 为准。

## projects 识别

```js
// jest
projects: [{ displayName: 'web', testMatch: ['<rootDir>/web/**/*.test.ts'] }]
```

```ts
// vitest
test: { projects: [{ test: { name: 'web', include: ['web/**/*.test.ts'] } }] }
```

每个 project 单独成行。命名取 `displayName` → `name` → `rootDir`。

**不要**把 projects 合并成一行——否则会漏掉一部分测试目标。

## 失败时的读法

跑测试失败后，只读输出**尾部**（失败摘要部分），不要读完整输出。关注：

- 失败用例名（`✗` / `FAIL` 之后的行）
- 断言差异（expected / received）
- 文件与行号

据此判断是测试写错还是产码有 bug。
