## ADDED Requirements

### Requirement: matchGlob 使用 fast-glob 生态匹配单条路径

`plugins/dev-team/bin/src/lib/glob.ts` SHALL 导出纯函数 `matchGlob(filePath: string, pattern: string): boolean`。

该函数 SHALL 使用 `fast-glob` npm 包及其内置的 picomatch 匹配器判断 `filePath` 是否匹配 `pattern`，SHALL NOT 包含手写 glob-to-regex 转换逻辑。

匹配前 SHALL 将 `filePath` 与 `pattern` 中的反斜杠 `\` 归一化为正斜杠 `/`，以保证 Windows 与 POSIX 路径的跨平台一致性。

函数 SHALL 支持项目配置中实际使用的 glob 语法，至少包括：

- `*` — 单段通配
- `**` — 跨目录通配（含 `**/` 零段路径）
- `?` — 单字符通配
- `{a,b,c}` — 花括号备选列表

函数 SHALL 为确定性纯函数：相同输入始终产生相同布尔输出，不访问文件系统或网络。

#### Scenario: 匹配 vitest 默认测试文件 glob

- **WHEN** `matchGlob("src/utils/helper.test.ts", "**/*.{test,spec}.{js,ts,jsx,tsx}")`
- **THEN** 返回 `true`

#### Scenario: 匹配 rust 默认测试文件 glob

- **WHEN** `matchGlob("tests/integration/test_auth.rs", "**/tests/**/*.rs")`
- **THEN** 返回 `true`

#### Scenario: 无通配符精确路径匹配

- **WHEN** `matchGlob("plugins/dev-team/bin/src/foo.test.ts", "plugins/dev-team/bin")`
- **THEN** 返回 `true`

#### Scenario: 不匹配的路径返回 false

- **WHEN** `matchGlob("src/utils/helper.ts", "**/*.test.ts")`
- **THEN** 返回 `false`

#### Scenario: Windows 反斜杠路径与 POSIX 路径等价

- **WHEN** `matchGlob("src\\utils\\helper.test.ts", "**/*.test.ts")`
- **THEN** 返回 `true`
- **AND** `matchGlob("src/utils/helper.test.ts", "**/*.test.ts")` 亦返回 `true`

### Requirement: scanProjectFiles 使用 fast-glob 扫描项目文件

`plugins/dev-team/bin/src/lib/glob.ts` SHALL 导出函数 `scanProjectFiles(rootDir: string, patterns: string[], options?: ScanProjectFilesOptions): string[]`。

该函数 SHALL 使用 `fast-glob.sync()` 在 `rootDir` 下枚举匹配 `patterns` 中任一 glob 模式的文件。

函数 SHALL 返回绝对路径字符串数组（`absolute: true`），且仅包含文件（`onlyFiles: true`）。

函数 SHALL 默认排除以下目录名（通过 `ignore` 选项）：
`node_modules`、`.git`、`.claude`、`dist`、`build`、`target`、`.vp`、`coverage`、`.nyc_output`。

当 `patterns` 为空数组时，函数 SHALL 返回空数组。

结果 SHALL 按路径字典序排序并去重。

#### Scenario: 扫描匹配多个 glob 模式

- **WHEN** `scanProjectFiles` 在含 `src/a.test.ts` 与 `tests/foo.rs` 的项目根目录上收到 `patterns: ["**/*.test.ts", "**/tests/**/*.rs"]`
- **THEN** 返回的绝对路径列表包含上述两个文件

#### Scenario: 排除 node_modules 目录

- **WHEN** 项目根目录下存在 `node_modules/pkg/index.test.ts`
- **THEN** `scanProjectFiles` 返回列表不包含该路径

#### Scenario: patterns 为空时返回空数组

- **WHEN** `scanProjectFiles("/project", [])`
- **THEN** 返回 `[]`

## Module Contract

### Function: matchGlob

| Property      | Description                                                     |
| ------------- | --------------------------------------------------------------- |
| **Module**    | `lib/glob.ts`                                                   |
| **Signature** | `matchGlob(filePath: string, pattern: string): boolean`         |
| **Input**     | 文件路径（绝对或相对）；glob 模式字符串                         |
| **Output**    | 路径是否匹配模式的布尔值                                        |
| **Behavior**  | 路径分隔符归一化后使用 fast-glob/picomatch 匹配；纯函数、无 I/O |

### Function: scanProjectFiles

| Property      | Description                                                                                          |
| ------------- | ---------------------------------------------------------------------------------------------------- |
| **Module**    | `lib/glob.ts`                                                                                        |
| **Signature** | `scanProjectFiles(rootDir: string, patterns: string[], options?: ScanProjectFilesOptions): string[]` |
| **Input**     | 项目根目录绝对路径；glob 模式数组；可选扫描选项                                                      |
| **Output**    | 匹配文件的绝对路径数组（排序、去重）                                                                 |
| **Behavior**  | `fast-glob.sync` 遍历；默认排除常见非源码目录；仅返回文件                                            |

### Dependency: fast-glob

| Property       | Description                                                                    |
| -------------- | ------------------------------------------------------------------------------ |
| **Package**    | `fast-glob`                                                                    |
| **Location**   | `plugins/dev-team/bin/package.json` → `dependencies`                           |
| **Usage**      | `scanProjectFiles` 直接调用 `fg.sync()`；`matchGlob` 使用其 picomatch 匹配能力 |
| **Constraint** | SHALL 作为生产依赖（`dependencies`），非仅 `devDependencies`                   |
