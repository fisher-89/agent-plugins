# 突变测试配置

**只产出配置，不执行。** 突变测试耗时长（数分钟到数小时），由用户手动运行并按需验收。

## 先查既有配置

在项目根与包根查找：

```
stryker.config.json
stryker.config.js
stryker.config.mjs
stryker.config.cjs
stryker.config.ts
```

**找到 → 直接引用**，在 `test-profile.md` 的「突变配置」列记录路径，不再生成。阶段 7 告知用户用现有配置。

找不到才按下节生成。

## 生成位置

`openspec/changes/<change 名>/mutation/stryker.config.json`

## 模板

### vitest / vite-plus

```json
{
  "$schema": "node_modules/@stryker-mutator/core/schema/stryker-schema.json",
  "mutate": [
    "src/foo.ts",
    "src/bar.ts",
    "!src/**/*.test.ts",
    "!src/**/*.test.tsx"
  ],
  "testRunner": "vitest",
  "plugins": ["@stryker-mutator/vitest-runner"],
  "vitest": {
    "configFile": "vite.config.ts"
  },
  "ignoreStatic": true,
  "reporters": ["html", "clear-text", "progress"],
  "thresholds": { "high": 80, "low": 60, "break": 50 }
}
```

### jest

```json
{
  "$schema": "node_modules/@stryker-mutator/core/schema/stryker-schema.json",
  "mutate": ["src/foo.ts", "src/bar.ts", "!src/**/*.test.ts"],
  "testRunner": "jest",
  "plugins": ["@stryker-mutator/jest-runner"],
  "jest": {
    "configFile": "jest.config.js"
  },
  "ignoreStatic": true,
  "reporters": ["html", "clear-text", "progress"],
  "thresholds": { "high": 80, "low": 60, "break": 50 }
}
```

## 字段填写

| 字段 | 填法 |
|---|---|
| `mutate` | 本次模块清单的源文件。**相对 Stryker 的运行目录**（即包根），不是相对配置文件位置 |
| `testRunner` | `vitest`（vite-plus 也用 vitest）或 `jest` |
| `plugins` | vitest → `@stryker-mutator/vitest-runner`；jest → `@stryker-mutator/jest-runner` |
| `<runner>.configFile` | 画像中记录的配置文件路径，**相对运行目录** |
| `thresholds` | 沿用 `{ "high": 80, "low": 60, "break": 50 }`，除非项目已有约定 |
| `reporters` | 保留 `html` 便于人工查看；`clear-text` 与 `progress` 给终端 |

`mutate` 用显式文件列表而非 glob——本次只关心补测试的那几个文件，全项目突变耗时不可控。

路径基准很容易搞错：Stryker 以 **cwd** 解析 `mutate` 与 `configFile`，与 `--config` 指向的文件在哪个目录无关。既有配置多以包根为基准（本仓库 `packages/desktop/stryker.config.json` 写 `"src/**/*.ts"`，运行目录就是 `packages/desktop`）。生成时按同一基准。

## 依赖检查

生成后检查项目是否装了 Stryker：

```
node_modules/@stryker-mutator/core
```

未安装时在简报中提示：

```
npm i -D @stryker-mutator/core @stryker-mutator/vitest-runner
```

（jest 对应 `@stryker-mutator/jest-runner`。）**不要自动安装。**

## 运行命令

写在简报里，不执行：

```
npx --prefix <包根> stryker run <配置路径>
```

示例：

```
npx --prefix packages/desktop stryker run openspec/changes/my-change/mutation/stryker.config.json
```

产物默认落在配置文件的 `htmlReporter.fileName` 或项目根的 `reports/mutation/`；提示用户打开 HTML 查看存量存活突变。

## 不做的事

| 不做 | 原因 |
|---|---|
| 不执行 Stryker | 耗时不可控，由用户决定何时跑 |
| 不设定强制分数线 | 补测试初期分数必然偏低，硬性门槛会产生噪音 |
| 不生成 Rust / 其他语言配置 | 本 skill 只覆盖 JS/TS |
