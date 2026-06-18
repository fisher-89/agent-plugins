# 测试设计: use-fast-glob

> **日期**: 2026-06-18

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `plugins/dev-team/bin/package.json` 包含 `fast-glob` 作为 `dependencies` 条目 | 不可测试项 | — | 见「不可测试项」 |
| AC-2 | `lib/glob.ts` 导出 `matchGlob` 与 `scanProjectFiles`，且不包含手写 glob-to-regex 转换逻辑 | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `glob.ts` 模块导出 |
| AC-3 | `matchGlob` 正确匹配项目使用的 glob 模式：`**/*.{test,spec}.{js,ts,jsx,tsx}`、`**/tests/**/*.rs`、`plugins/dev-team/bin`（无通配符精确路径）、`**/e2e/**` | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 项目 glob 模式正向匹配 |
| AC-3 | `matchGlob` 正确匹配项目使用的 glob 模式：`**/*.{test,spec}.{js,ts,jsx,tsx}`、`**/tests/**/*.rs`、`plugins/dev-team/bin`（无通配符精确路径）、`**/e2e/**` | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 不匹配路径 |
| AC-3 | `matchGlob` 正确匹配项目使用的 glob 模式：`**/*.{test,spec}.{js,ts,jsx,tsx}`、`**/tests/**/*.rs`、`plugins/dev-team/bin`（无通配符精确路径）、`**/e2e/**` | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — glob 语法边界 |
| AC-3 | `matchGlob` 正确匹配项目使用的 glob 模式：`**/*.{test,spec}.{js,ts,jsx,tsx}`、`**/tests/**/*.rs`、`plugins/dev-team/bin`（无通配符精确路径）、`**/e2e/**` | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 输入边界 |
| AC-3 | `matchGlob` 正确匹配项目使用的 glob 模式：`**/*.{test,spec}.{js,ts,jsx,tsx}`、`**/tests/**/*.rs`、`plugins/dev-team/bin`（无通配符精确路径）、`**/e2e/**` | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 多模式扫描 |
| AC-3 | `matchGlob` 正确匹配项目使用的 glob 模式：`**/*.{test,spec}.{js,ts,jsx,tsx}`、`**/tests/**/*.rs`、`plugins/dev-team/bin`（无通配符精确路径）、`**/e2e/**` | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 无通配符目录扫描 |
| AC-3 | `matchGlob` 正确匹配项目使用的 glob 模式：`**/*.{test,spec}.{js,ts,jsx,tsx}`、`**/tests/**/*.rs`、`plugins/dev-team/bin`（无通配符精确路径）、`**/e2e/**` | 集成测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `detectFrameworks -- single framework` |
| AC-3 | `matchGlob` 正确匹配项目使用的 glob 模式：`**/*.{test,spec}.{js,ts,jsx,tsx}`、`**/tests/**/*.rs`、`plugins/dev-team/bin`（无通配符精确路径）、`**/e2e/**` | 集成测试 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_glob_matching/test_detect_frameworks_glob_matching.test.ts` | `test_detect_frameworks` — glob 模式匹配 |
| AC-4 | `matchGlob` 对 Windows 反斜杠路径与 POSIX 正斜杠路径产生一致匹配结果 | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 路径分隔符归一化 |
| AC-4 | `matchGlob` 对 Windows 反斜杠路径与 POSIX 正斜杠路径产生一致匹配结果 | 集成测试 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_glob_matching/test_detect_frameworks_glob_matching.test.ts` | `test_detect_frameworks` — Windows 路径 |
| AC-5 | `scanProjectFiles` 自动扫描排除 `node_modules`、`.git`、`dist`、`build`、`target`、`.vp`、`coverage`、`.nyc_output` 目录 | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 默认排除目录 |
| AC-5 | `scanProjectFiles` 自动扫描排除 `node_modules`、`.git`、`dist`、`build`、`target`、`.vp`、`coverage`、`.nyc_output` 目录 | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 结果排序去重 |
| AC-5 | `scanProjectFiles` 自动扫描排除 `node_modules`、`.git`、`dist`、`build`、`target`、`.vp`、`coverage`、`.nyc_output` 目录 | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 仅返回文件 |
| AC-5 | `scanProjectFiles` 自动扫描排除 `node_modules`、`.git`、`dist`、`build`、`target`、`.vp`、`coverage`、`.nyc_output` 目录 | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 输入边界 |
| AC-5 | `scanProjectFiles` 自动扫描排除 `node_modules`、`.git`、`dist`、`build`、`target`、`.vp`、`coverage`、`.nyc_output` 目录 | 单元测试 | `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 可选 ignore |
| AC-6 | `test_detect_frameworks` 首匹配规则不变：文件同时匹配多个 glob 时返回 `frameworks` 数组中第一个匹配的框架 | 集成测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `detectFrameworks -- glob first-match (AC-4)` |
| AC-6 | `test_detect_frameworks` 首匹配规则不变：文件同时匹配多个 glob 时返回 `frameworks` 数组中第一个匹配的框架 | 集成测试 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_first_match/test_detect_frameworks_first_match.test.ts` | `test_detect_frameworks` — 首匹配规则 |
| AC-7 | `test_detect_frameworks` 无 `files` 参数时自动扫描行为与替换前等价（匹配文件集合一致） | 集成测试 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_auto_scan/test_detect_frameworks_auto_scan.test.ts` | `test_detect_frameworks` — 自动扫描匹配文件集合 |
| AC-8 | `test_detect_frameworks` 的 `detected`、`frameworks`、`plan` 输出结构与 schema 不变 | 单元测试 | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testDetectFrameworksOutputSchema` — plan 正向测试 |
| AC-8 | `test_detect_frameworks` 的 `detected`、`frameworks`、`plan` 输出结构与 schema 不变 | 单元测试 | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testDetectFrameworksOutputSchema` — plan 异常测试 |
| AC-8 | `test_detect_frameworks` 的 `detected`、`frameworks`、`plan` 输出结构与 schema 不变 | 单元测试 | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testDetectFrameworksOutputSchema` — plan 边界测试 |
| AC-8 | `test_detect_frameworks` 的 `detected`、`frameworks`、`plan` 输出结构与 schema 不变 | 集成测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `runTestDetectFrameworks -- 向后兼容 (AC-11)` |
| AC-8 | `test_detect_frameworks` 的 `detected`、`frameworks`、`plan` 输出结构与 schema 不变 | 集成测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `runTestDetectFrameworks -- plan 内容正确性 (AC-6)` |
| AC-8 | `test_detect_frameworks` 的 `detected`、`frameworks`、`plan` 输出结构与 schema 不变 | 集成测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `runTestDetectFrameworks -- plan 包含 script 字段 (AC-7, AC-9)` |
| AC-8 | `test_detect_frameworks` 的 `detected`、`frameworks`、`plan` 输出结构与 schema 不变 | 集成测试 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_output/test_detect_frameworks_plan_output.test.ts` | `test_detect_frameworks` — plan 输出结构 |
| AC-9 | `deriveWorkingDirectory` 行为不变 | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `deriveWorkingDirectory` — 正向测试 |
| AC-9 | `deriveWorkingDirectory` 行为不变 | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `deriveWorkingDirectory` — 异常测试 |
| AC-9 | `deriveWorkingDirectory` 行为不变 | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `deriveWorkingDirectory` — 边界测试 |
| AC-10 | `plugins/dev-team/bin` 全部单元测试通过（`vp test`） | 不可测试项 | — | 见「不可测试项」 |
| AC-11 | 插件版本号已递增 | 不可测试项 | — | 见「不可测试项」 |

---

## 单元测试

### 用例

#### `glob.test.ts` — `matchGlob` 正向匹配（AC-3）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 项目 glob 模式正向匹配 | 正向 | `matchGlob("src/utils/helper.test.ts", "**/*.{test,spec}.{js,ts,jsx,tsx}")` 返回 `true` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 项目 glob 模式正向匹配 | 正向 | `matchGlob("src/util.spec.js", "**/*.{test,spec}.{js,ts,jsx,tsx}")` 返回 `true` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 项目 glob 模式正向匹配 | 正向 | `matchGlob("tests/integration/test_auth.rs", "**/tests/**/*.rs")` 返回 `true` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 项目 glob 模式正向匹配 | 正向 | `matchGlob("plugins/dev-team/bin/src/foo.test.ts", "plugins/dev-team/bin")` 返回 `true`（无通配符目录前缀语义） | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 项目 glob 模式正向匹配 | 正向 | `matchGlob("tests/e2e/test_app.ts", "**/e2e/**")` 返回 `true` | 新增 |

#### `glob.test.ts` — `matchGlob` 路径分隔符归一化（AC-4）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 路径分隔符归一化 | 正向 | `matchGlob("src\\utils\\helper.test.ts", "**/*.test.ts")` 与 `matchGlob("src/utils/helper.test.ts", "**/*.test.ts")` 均返回 `true` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 路径分隔符归一化 | 正向 | `matchGlob("plugins\\dev-team\\bin\\src\\foo.test.ts", "plugins/dev-team/bin")` 返回 `true` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 路径分隔符归一化 | 边界 | pattern 含反斜杠 `**\\*.test.ts` 与 `**/*.test.ts` 对同一路径产生相同结果 | 新增 |

#### `glob.test.ts` — `matchGlob` 不匹配与 glob 语法边界（AC-3）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 不匹配路径 | 异常 | `matchGlob("src/utils/helper.ts", "**/*.test.ts")` 返回 `false` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 不匹配路径 | 异常 | `matchGlob("src/readme.md", "**/*.{test,spec}.{js,ts,jsx,tsx}")` 返回 `false` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — glob 语法边界 | 边界 | `**/` 零段路径：`matchGlob("foo.test.ts", "**/*.test.ts")` 返回 `true` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — glob 语法边界 | 边界 | `?` 单字符通配：`matchGlob("tests/unit/test.ts", "tests/?nit/*.test.ts")` 返回 `true` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — glob 语法边界 | 边界 | 花括号备选：`matchGlob("lib/utils.test.ts", "{src,lib}/*.test.ts")` 返回 `true` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — glob 语法边界 | 边界 | 无通配符精确路径不匹配：`matchGlob("other/foo.test.ts", "plugins/dev-team/bin")` 返回 `false` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — glob 语法边界 | 边界 | 无通配符目录自身匹配：`matchGlob("plugins/dev-team/bin", "plugins/dev-team/bin")` 返回 `true` | 新增 |

#### `glob.test.ts` — `matchGlob` 输入边界

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 输入边界 | 边界 | `filePath` 为空字符串 `""` 时返回 `false`（或确定性布尔值，与 picomatch 行为一致） | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 输入边界 | 边界 | `pattern` 为空字符串 `""` 时返回确定性结果 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 输入边界 | 边界 | 超长路径（>1000 字符）+ 标准 glob 模式不产生异常 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `matchGlob` — 输入边界 | 边界 | 路径含空格与 Unicode：`matchGlob("src/my test/测试.test.ts", "**/*.test.ts")` 返回 `true` | 新增 |

#### `glob.test.ts` — `scanProjectFiles` 正向扫描（AC-3、AC-5）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 多模式扫描 | 正向 | 临时项目含 `src/a.test.ts` 与 `tests/foo.rs`，`patterns: ["**/*.test.ts", "**/tests/**/*.rs"]` 返回两者绝对路径 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 无通配符目录扫描 | 正向 | 临时项目含 `plugins/dev-team/bin/src/foo.test.ts`，`patterns: ["plugins/dev-team/bin"]` 返回该文件绝对路径 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 结果排序去重 | 正向 | 多 pattern 重叠匹配同一文件时，结果去重且按字典序排序 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 仅返回文件 | 正向 | 扫描结果不含目录节点，仅含 `onlyFiles: true` 的文件路径 | 新增 |

#### `glob.test.ts` — `scanProjectFiles` 排除目录（AC-5）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 默认排除目录 | 边界 | `node_modules/pkg/index.test.ts` 不在返回列表中 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 默认排除目录 | 边界 | `.git/hooks/pre-commit.test.ts` 不在返回列表中 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 默认排除目录 | 边界 | `dist/bundle.test.js` 不在返回列表中 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 默认排除目录 | 边界 | `build/output.test.js` 不在返回列表中 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 默认排除目录 | 边界 | `target/debug/deps/test.rs` 不在返回列表中 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 默认排除目录 | 边界 | `.vp/cache.test.ts` 不在返回列表中 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 默认排除目录 | 边界 | `coverage/lcov.test.ts` 不在返回列表中 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 默认排除目录 | 边界 | `.nyc_output/temp.test.ts` 不在返回列表中 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 默认排除目录 | 边界 | `.claude/cache.test.ts` 不在返回列表中 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 默认排除目录 | 正向 | `src/valid.test.ts` 在排除目录之外时正常返回 | 新增 |

#### `glob.test.ts` — `scanProjectFiles` 输入边界

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 输入边界 | 边界 | `patterns` 为空数组 `[]` 时返回 `[]` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 输入边界 | 边界 | 项目根目录无匹配文件时返回 `[]` | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `scanProjectFiles` — 可选 ignore | 边界 | 通过 `options.ignore` 追加排除模式后，对应路径被过滤 | 新增 |

#### `glob.test.ts` — 模块契约（AC-2）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `glob.ts` 模块导出 | 正向 | 从 `lib/glob.ts` 可导入 `matchGlob` 与 `scanProjectFiles` 且为函数类型 | 新增 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `glob.ts` 模块导出 | 正向 | `matchGlob` 对相同输入多次调用结果一致（纯函数确定性） | 新增 |

#### `test-detect-frameworks.test.ts` — `deriveWorkingDirectory` 回归（AC-9）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `deriveWorkingDirectory` — 正向测试 | 正向 | 无通配符路径、`**`、`{}`、`?`、Windows 反斜杠等现有用例全部通过 | 废弃 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `deriveWorkingDirectory` — 异常测试 | 异常 | 空字符串、`null`/`undefined` 防御用例全部通过 | 废弃 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `deriveWorkingDirectory` — 边界测试 | 边界 | 超长路径、纯通配符、连续分隔符等现有用例全部通过 | 废弃 |

#### `test-detect-frameworks.schema.test.ts` — 输出 schema 回归（AC-8）

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testDetectFrameworksOutputSchema` — plan 正向测试 | 正向 | 完整 `detected` + `frameworks` + `plan` 输出通过 Zod 校验 | 废弃 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testDetectFrameworksOutputSchema` — plan 异常测试 | 异常 | 缺失必填字段、非法枚举值被拒绝 | 废弃 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | `testDetectFrameworksOutputSchema` — plan 边界测试 | 边界 | 空 plan 数组、passthrough 多余字段通过验证 | 废弃 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/lib/glob.test.ts` | 文件系统 | 使用 `fs.mkdtempSync` 创建临时目录树，写入测试文件与排除目录内容；`scanProjectFiles` 测试结束后 `fs.rmSync` 清理。不 mock `fast-glob`，使用真实库调用 | 所有 `scanProjectFiles` 用例 |
| `plugins/dev-team/bin/src/lib/glob.test.ts` | `fast-glob` / `picomatch` | 不 mock，直接调用生产依赖验证真实匹配语义 | 所有 `matchGlob` 用例 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` — `deriveWorkingDirectory` | 无 | 纯函数，无外部依赖 | 所有 `deriveWorkingDirectory` 回归用例 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` | 无 | 直接调用 Zod `.safeParse()` | 所有 schema 回归用例 |

---

## 集成测试

### 用例

#### `test-detect-frameworks.test.ts` — 首匹配与文件检测回归（AC-6、AC-8）

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-6 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `detectFrameworks -- glob first-match (AC-4)` | 应按 default + overrides 将文件映射到对应框架 | 废弃 |
| AC-6 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `detectFrameworks -- glob first-match (AC-4)` | 文件同时匹配 default 与 override 时应用首匹配规则 | 废弃 |
| AC-6 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `runTestDetectFrameworks -- 向后兼容 (AC-11)` | 添加 plan 后首匹配规则不变 | 废弃 |
| AC-8 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `runTestDetectFrameworks -- 向后兼容 (AC-11)` | `detected` 字段结构和内容不变 | 废弃 |
| AC-8 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `runTestDetectFrameworks -- 向后兼容 (AC-11)` | `frameworks` 字段结构和内容不变 | 废弃 |
| AC-8 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `runTestDetectFrameworks -- plan 内容正确性 (AC-6)` | 单框架与多框架 plan 条目 directory/framework/coverage 字段正确 | 废弃 |
| AC-8 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `runTestDetectFrameworks -- plan 包含 script 字段 (AC-7, AC-9)` | plan 条目 script 与 `generateScript()` 输出一致 | 废弃 |

#### `test-detect-frameworks.test.ts` — 无通配符 override 与 glob 集成（AC-3、AC-6）

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-3, AC-6 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `detectFrameworks -- single framework` | vitest 默认 glob 匹配 `.test.ts`/`.spec.js`，不匹配 `helper.ts` 返回 `unknown` | 废弃 |
| AC-3, AC-6 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `runTestDetectFrameworks -- plan 内容正确性 (AC-6)` | override `plugins/dev-team/bin/**` 时 `plan[1].directory` 为 `plugins/dev-team/bin` | 废弃 |
| AC-3 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `detectFrameworks -- glob first-match (AC-4)` | 新增：override 使用无通配符路径 `plugins/dev-team/bin` 时，`plugins/dev-team/bin/src/foo.test.ts` 检测为对应框架 | 新增 |

#### `__tests__/test_detect_frameworks_auto_scan` — 自动扫描（AC-7）

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-7 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_auto_scan/test_detect_frameworks_auto_scan.test.ts` | `test_detect_frameworks` — 自动扫描匹配文件集合 | 临时项目写入 `src/app.test.ts` 与 `tests/auth.rs`，省略 `files` 参数，返回 detected 包含两文件且框架归属正确 | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_auto_scan/test_detect_frameworks_auto_scan.test.ts` | `test_detect_frameworks` — 自动扫描匹配文件集合 | 临时项目仅含 `readme.md`（非测试文件），省略 `files` 参数，detected 为空或 frameworks 为空 | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_auto_scan/test_detect_frameworks_auto_scan.test.ts` | `test_detect_frameworks` — 自动扫描匹配文件集合 | 临时项目 `node_modules/pkg/index.test.ts` 不被自动扫描纳入 detected | 新增 |
| AC-7 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_auto_scan/test_detect_frameworks_auto_scan.test.ts` | `test_detect_frameworks` — 自动扫描匹配文件集合 | 自动扫描结果仅含匹配 glob 的文件，不含 `"unknown"` 非测试文件（验证 D5 语义变更） | 新增 |

#### `__tests__/test_detect_frameworks_first_match` — 首匹配端到端（AC-6）

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-6 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_first_match/test_detect_frameworks_first_match.test.ts` | `test_detect_frameworks` — 首匹配规则 | 配置 `[{glob: "**/*.ts", framework: "vitest"}, {glob: "**/e2e/**", framework: "vite-plus"}]`，`tests/e2e/test_app.ts` 归属 vitest（数组首匹配） | 新增 |
| AC-6 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_first_match/test_detect_frameworks_first_match.test.ts` | `test_detect_frameworks` — 首匹配规则 | 交换 override 顺序后框架归属随之改变，验证顺序敏感性 | 新增 |

#### `__tests__/test_detect_frameworks_glob_matching` — glob 模式端到端（AC-3、AC-4）

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-3 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_glob_matching/test_detect_frameworks_glob_matching.test.ts` | `test_detect_frameworks` — glob 模式匹配 | 通过 `runTestDetectFrameworks` 验证 `{}`、`**/`、无通配符目录前缀三类 pattern 的文件检测正确 | 新增 |
| AC-4 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_glob_matching/test_detect_frameworks_glob_matching.test.ts` | `test_detect_frameworks` — Windows 路径 | 以反斜杠形式传入 `files` 参数，检测结果与正斜杠形式一致 | 新增 |

#### `__tests__/test_detect_frameworks_plan_output` — plan 输出完整性（AC-8）

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-8 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_output/test_detect_frameworks_plan_output.test.ts` | `test_detect_frameworks` — plan 输出结构 | 字符串简写 `"vitest"` 配置时 plan 含完整字段且通过 `testDetectFrameworksOutputSchema` 校验 | 新增 |
| AC-8 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_output/test_detect_frameworks_plan_output.test.ts` | `test_detect_frameworks` — plan 输出结构 | 无 `test.framework` 配置时 plan 为空数组，detected 均为 `unknown` | 新增 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | 文件系统 | `createTempProject()` 辅助函数：`fs.mkdtempSync` + 写入 `openspec/config.json`；测试结束 `cleanup()` 删除临时目录 | 所有 `runTestDetectFrameworks` 回归用例 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `readConfig` / `runTestGetFrameworkConfig` | 不 mock，从临时项目真实读取配置与框架注册表 | 所有 `runTestDetectFrameworks` 回归用例 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `lib/glob.ts` | 不 mock，集成测试验证 `matchGlob`/`scanProjectFiles` 与命令层协作 | 文件检测与自动扫描用例 |
| `plugins/dev-team/bin/__tests__/test_detect_frameworks_*/*.test.ts` | 文件系统 | 沿用 `createTempProject` 模式，额外写入实体测试文件以驱动自动扫描 | 所有新增 `__tests__` 集成场景 |
| `plugins/dev-team/bin/__tests__/test_detect_frameworks_*/*.test.ts` | `glob.ts` | 不 mock，端到端验证 fast-glob 替换后 MCP 工具输出 | 自动扫描、首匹配、glob 匹配、plan 输出场景 |

---

## 不可测试项

- **AC-1：`package.json` 包含 `fast-glob` 依赖** — **原因**: 依赖声明属于构建/清单文件审查，无对应单元测试文件；通过代码审查检查 `plugins/dev-team/bin/package.json` 与 `pnpm-lock.yaml`。
- **AC-2：`globToRegex` 在 `dev-team/bin` 源码中不存在** — **原因**: 否定性静态约束（「某函数不得存在」）无法通过正向运行时测试直接断言；通过代码审查与实现后 `rg globToRegex plugins/dev-team/bin/src` 零匹配间接验证。模块导出契约由 `glob.test.ts` 正向测试覆盖。
- **AC-10：`plugins/dev-team/bin` 全部单元测试通过** — **原因**: 全量测试套件门禁属于 CI/本地 `pnpm test`（`vp test`）流程验证，非单条用例可表达。
- **AC-11：插件版本号已递增** — **原因**: `dev-team/.claude-plugin/plugin.json` 版本号为发布元数据，通过代码审查确认 `2.6.20` → `2.6.21`（或后续递增）。
- **`fast-glob` 打包后 `dev-team-mcp.cjs` bundle 体积可接受** — **原因**: 体积对比需构建产物与人工/CI 阈值判断，不在 Vitest 单元测试范围内；实现阶段通过 `pnpm build` 前后对比 bundle 大小。
- **MCP 跨进程调用 `test_detect_frameworks` 工具** — **原因**: 本变更内部实现替换，MCP 契约不变；`runTestDetectFrameworks` 函数级集成测试已等价覆盖工具行为，无需启动 MCP 服务器进程。
