# 设计: test-exclude-source-globs

> **变更**: test-exclude-source-globs
> **日期**: 2026-07-14

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| test-exclude | 提供 `isFileExcluded` 和 `getExcludeGlobs` 工具函数，判断源文件是否被 `test.exclude` 或 `test.overrides[].exclude` 排除 | `plugins/dev-team/bin/src/lib/test-exclude.ts` | `lib/glob.ts` (`matchGlob`), `schemas/` (`OpenSpecConfig` 类型) | TypeScript |
| config-schema | Zod schema 中 `test` 和 `test.overrides` 对象各增加可选 `exclude: string[]` 字段 | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | zod/v4 | TypeScript |
| config-json-schema | JSON Schema 中 `test` 和 `test.overrides` 对象各增加可选 `exclude: string[]` 字段 | `plugins/dev-team/bin/dev-team-config.schema.json` | JSON Schema | JSON |
| test-detect-frameworks | 在 `detectFrameworksForFiles` 中调用 `isFileExcluded` 过滤，排除的文件不进入 `detected[]` | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `lib/test-exclude.ts`, `lib/config.ts` | TypeScript |
| test-resolve-paths | 在 `processNonEmptyModules` 和 `processEmptyModules` 中调用 `isFileExcluded` 过滤，排除的文件不进入 `unit_tests[]` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | `lib/test-exclude.ts`, `lib/config.ts` | TypeScript |
| test-runner | 在 `runMutationPhase` 中调用 `isFileExcluded` 过滤源文件，排除的文件不进入 StrykerJS 变异目标列表 | `plugins/dev-team/bin/src/lib/test-runner.ts` | `lib/test-exclude.ts`, `lib/config.ts` | TypeScript |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
  公共函数仅列模块级导出函数、CLI 子命令、HTTP 端点，私有函数不列入。
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/lib/test-exclude.ts` | 共享排除过滤工具模块，导出 `isFileExcluded` 和 `getExcludeGlobs` 函数 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | 在 `test` 的 Zod object 中增加 `exclude: z.array(z.string()).optional()`；在 `test.overrides` 条目 object 中增加同样字段 | 使 `test.exclude` 和 `test.overrides[].exclude` 通过类型检查和 Zod 验证 |
| `plugins/dev-team/bin/dev-team-config.schema.json` | 在 `test.properties` 中增加 `exclude` 属性（array of string）；在 `test.properties.overrides.items.properties` 中增加同样属性 | 使 JSON Schema 验证器识别 `exclude` 字段 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 在 `detectFrameworksForFiles` 中增加 exclude 过滤逻辑；调用处传入 config | 被排除的文件不在 `detected[]` 中出现 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 在 `processNonEmptyModules` 和 `processEmptyModules` 中增加 exclude 过滤逻辑 | 被排除的文件不在 `unit_tests[]` 中出现 |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 在 `runMutationPhase` 中增加 exclude 过滤逻辑 | 被排除的源文件不进入 StrykerJS 变异目标列表 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `isFileExcluded` | `plugins/dev-team/bin/src/lib/test-exclude.ts` | 新增 | `function isFileExcluded(filePath: string, config: OpenSpecConfig): boolean` | 判断文件是否被 `test.exclude` 或 `test.overrides[].exclude` 排除。全局 exclude 直接匹配；override-level exclude 仅当文件在该 override 的 `file` glob 范围内时匹配 |
| `getExcludeGlobs` | `plugins/dev-team/bin/src/lib/test-exclude.ts` | 新增 | `function getExcludeGlobs(config: OpenSpecConfig): string[]` | 返回全局 + 所有 override-level 的 exclude glob 并集，用于排查和信息展示 |
| `runTestDetectFrameworks` | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 修改 | `function runTestDetectFrameworks(options: TestDetectFrameworksOptions): TestDetectFrameworksResult` | 行为变更：内部调用 `isFileExcluded` 过滤，排除的文件不进入 `detected[]`；返回类型不变 |
| `runTestResolvePaths` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 修改 | `function runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult` | 行为变更：内部调用 `isFileExcluded` 过滤，排除的文件不进入 `unit_tests[]`；返回类型不变 |
| `executePlanEntry` | `plugins/dev-team/bin/src/lib/test-runner.ts` | 修改 | `function executePlanEntry(entry: TestPlan, projectRoot: string, options?: {...}): ExecutionResult` | 行为变更：`runMutationPhase` 内部调用 `isFileExcluded` 过滤源文件；返回类型不变 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `OpenSpecConfig` | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | 修改（自动推导） | `test.exclude` 和 `test.overrides[].exclude` 成为可选的 `string[]` 字段。由 Zod schema 自动推导，无需手写类型变更 |

<!-- 配置子表不省略：test.exclude 和 test.overrides[].exclude 是配置变更的核⼼内容 -->

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `test.exclude` | `openspec/config.json` | 新增 | `undefined`（可选） | `string[]` | 全局排除的源文件 glob 模式列表。匹配的文件在文件-框架检测、路径解析、变异测试中均被跳过 |
| `test.overrides[].exclude` | `openspec/config.json` | 新增 | `undefined`（可选） | `string[]` | override 级别的排除 glob 模式列表。仅在该 override 的 `file` glob 范围内生效。与全局 exclude 以并集方式合并 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `OpenSpecConfig` (`openspec/config.json`) | `test.exclude?: string[]` — 全局排除 glob 列表； `test.overrides[].exclude?: string[]` — override 级别排除 glob 列表 | 不依赖其他模型 | `openspec/config.json` 文件，通过 `readConfig` / `configSchema.decode` 加载 |

数据模型仅涉及配置结构扩展，无新增持久化实体。

---

<!-- 本变更不涉及 HTTP API，省略路由/API 设计节 -->

---

## 依赖

### 运行时依赖

- 无新增运行时依赖。`isFileExcluded` 和 `getExcludeGlobs` 复用已有的 `matchGlob`（基于 `picomatch`）实现 glob 匹配。

### 构建/测试依赖

- 无新增构建/测试依赖。

---

## 待决问题

- 无
