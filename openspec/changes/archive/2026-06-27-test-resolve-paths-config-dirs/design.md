# 设计: test-resolve-paths-config-dirs

> **变更**: test-resolve-paths-config-dirs
> **日期**: 2026-06-26
> **基于**: proposal.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Schema定义 | 定义 `test_resolve_paths` 的输入/输出 Zod schema；`modules` 支持 `string[]` 和 `"git-change"` 两种类型 | `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` | zod/v4 | TypeScript |
| 路径解析器 | 解析源文件到测试路径的映射；统一入口根据 `modules` 类型分发：空数组→config扫描、`"git-change"`→git diff、非空数组→config过滤 | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | `runTestDetectFrameworks`, `child_process.execSync`, `path` | TypeScript |
| 框架检测器 | 读取 `config.json` 的 `test.framework` 和 `test.overrides`，返回执行计划（`plan`）和文件检测结果（`detected`） | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `readConfig`, `runTestGetFrameworkConfig` | TypeScript |
| MCP注册 | 向 MCP server 注册 `test_resolve_paths` 工具，更新描述文本 | `plugins/dev-team/bin/src/mcp.ts` | schema, command | TypeScript |
| 配置读取 | 读取 `openspec/config.json` 返回规范化配置对象 | `plugins/dev-team/bin/src/lib/config.ts` | `configSchema` | TypeScript |

### 组件图

```
┌──────────────────────────────────────────────────────────────────┐
│                      test_resolve_paths                          │
│                                                                   │
│  ┌────────────────────┐    ┌──────────────────────────────────┐  │
│  │   test-resolve-     │    │   test-resolve-                   │  │
│  │   paths.schema.ts   │───▶│   paths.ts                         │  │
│  │   (union 输入校验)   │    │   (核心解析 + 分发逻辑)            │  │
│  └────────────────────┘    └───────┬────────────────────────────┘  │
│                                    │                               │
│           ┌────────────────────────┼───────────────────────────┐   │
│           │                        │                           │   │
│           ▼                        ▼                           ▼   │
│  modules: "git-change"    modules: []           modules: string[]  │
│           │                        │                           │   │
│           ▼                        ▼                           ▼   │
│  ┌────────────────┐  ┌──────────────────────┐  ┌─────────────┐   │
│  │ git diff HEAD  │  │ runTestDetect        │  │ runTest     │   │
│  │ --name-only    │  │ Frameworks({})       │  │ Detect      │   │
│  │ → file[]       │  │ → plan[].directory   │  │ Frameworks  │   │
│  └───────┬────────┘  │ → 扫描 + 推导        │  │ ({files})   │   │
│          │           └──────────────────────┘  │ → detected  │   │
│          │                                     │ → 过滤+推导 │   │
│          └───────────┬─────────────────────────┘             │   │
│                      ▼                                        │   │
│             统一汇聚: unit_tests + integration_tests + errors  │   │
└──────────────────────────────────────────────────────────────────┘
```

---

## 数据流

### 流程描述

#### 场景 A: modules 非空（config-driven 过滤）

```
test_resolve_paths({ modules: ["src/config.ts", "scripts/util.ts"], ... })
  │
  ▼
runTestDetectFrameworks({ files: modules, projectRoot })
  │
  ├─ 返回 detected: [{ file: "src/config.ts", framework: "vite-plus" }]
  │   scripts/util.ts 不在任何 test config 覆盖范围内 → 不出现在 detected 中
  │
  ▼
对 detected 中的每个文件:
  ├─ isSourceFile() 校验
  ├─ deriveUnitTestPath()
  └─ addUnitTest()
  │
  ▼
对未被 detected 的文件 → errors 添加提示
  │
  ▼
resolveIntegrationTests(params, collectedSources, errors)
  │
  ▼
返回 { unit_tests, integration_tests, errors }
```

关键变更：原来直接对 modules 中的每个文件推导测试路径；现在先通过 `runTestDetectFrameworks({ files })` 过滤。

#### 场景 B: modules 为空（config-driven 目录扫描）

```
test_resolve_paths({ modules: [], ... })
  │
  ▼
runTestDetectFrameworks({ projectRoot })  ← 不传 files，触发 auto-scan
  │
  ├─ readConfig(projectRoot) → 读取 config.json
  ├─ normalizeFrameworks(framework, overrides) → 构建映射表
  │   └─ 优先级: test.overrides[].file → test.framework → 空
  ├─ buildPlanFromMappings() → plan[]
  │   └─ 每项包含: directory, framework, coverage_cmd, ...
  └─ 返回 { detected, frameworks, plan }
  │
  ▼ (仅使用 plan[].directory)
提取唯一目录列表
  │
  ├─ plan 为空 → No test configuration → errors 添加指导消息
  └─ plan 非空 → 进入扫描流程
       │
       ▼
    对每个 directory:
      ├─ resolve(directory, projectRoot) → 绝对路径
      ├─ collectFiles(absDir) → 递归收集所有文件
      │   └─ 排除: node_modules, .git, .claude, dist, build, target, .vp, coverage, .nyc_output
      └─ filter(isSourceFile) → 仅保留可测试源文件
       │
       ▼
    跨目录去重源文件列表
       │
       ▼
    deriveUnitTestPath() → 生成单元测试路径
       │
       ▼
    resolveIntegrationTests() → 生成集成测试路径
       │
       ▼
    返回 { unit_tests, integration_tests, errors }
```

#### 场景 C: modules 为 "git-change"

```
test_resolve_paths({ modules: "git-change", ... })
  │
  ▼
execSync('git diff HEAD --name-only', { cwd: projectRoot })
  │
  ├─ 成功 → 获取变更文件列表（按行分割，过滤空行）
  └─ 失败 → errors 添加 git 错误消息，返回空结果
  │
  ▼
以变更文件列表作为 modules（等价于场景 A 的处理）
  │
  ├─ runTestDetectFrameworks({ files: changedFiles, projectRoot })
  ├─ 过滤 → detected 中的文件推导测试路径
  └─ 返回 { unit_tests, integration_tests, errors }
```

#### 场景 D: 无 test 配置（指导性错误）

```
runTestDetectFrameworks({...}) → plan: [] (无 framework, 无 overrides)
  │
  ▼
resolveTestPaths() → 无扫描目录 且 无文件被 detected
  │
  ▼
errors: [{
  path: "config",
  message: "No test configuration found. Please configure 'test.framework' or 'test.overrides' in openspec/config.json"
}]
unit_tests: []
```

### 数据模型

#### TestResolvePathsInput (MCP 输入)

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `modules` | `string[] \| "git-change"` | 是 | 模块路径列表（可为空数组），或 `"git-change"` 触发 git diff |
| `integration_scenarios` | `string[]` | 否 | 集成测试场景名 |
| `extension` | `string` | 否 | 扩展名覆盖 |
| `integration_root` | `string` | 否 | `__tests__/` 父目录 |
| `project_root` | `string` | 否 | 项目根目录 |

#### ResolveTestPathsParams (内部参数对象)

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `projectRoot` | `string` | 是 | 项目根目录绝对路径 |
| `modules` | `string[] \| "git-change"` | 是 | 同 MCP 输入 |
| `integrationScenarios` | `string[]` | 否 | 集成测试场景名 |
| `extension` | `string` | 否 | 扩展名覆盖 |
| `integrationRoot` | `string` | 否 | `__tests__/` 父目录 |

#### ResolveTestPathsResult (返回结果)

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `unit_tests` | `{ source: string, test_file: string }[]` | 是 | 单元测试推导结果 |
| `integration_tests` | `{ scenario: string, test_file: string }[]` | 是 | 集成测试推导结果 |
| `errors` | `{ path: string, message: string }[]` | 是 | 错误列表 |

---

## 路由/API 设计

### MCP 工具: test_resolve_paths

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| `test_resolve_paths` | MCP tool | 从模块列表或 git 变更推导单元和集成测试文件路径。modules 为空时自动从 config.json 推导扫描目录；为 `"git-change"` 时读取 git diff 变更文件。所有路径均受 test config 过滤 | `TestResolvePathsInput` | `ResolveTestPathsResult` | 无 |

#### 输入 (TestResolvePathsInput)

| 字段 | 类型 | 必填 | 修改 | 说明 |
|------|------|------|------|------|
| `modules` | `string[] \| "git-change"` | 是 | **union 类型重写** | 模块路径列表（可为空数组），或 `"git-change"` 触发 git diff HEAD --name-only |
| `integration_scenarios` | `string[]` | 否 | 不变 | 集成测试场景名 |
| `extension` | `string` | 否 | 不变 | 扩展名覆盖 |
| `integration_root` | `string` | 否 | 不变 | `__tests__/` 父目录 |
| `project_root` | `string` | 否 | 不变 | 项目根目录 |

**修改后 schema**:
```typescript
export const testResolvePathsInputSchema = z.object({
  modules: z.union([
    z.array(z.string()).describe('Module paths (files or directories, relative to project_root). When empty, directories are auto-detected from config.json test configuration.'),
    z.literal('git-change').describe('Read git diff HEAD --name-only to get changed files, then resolve test paths filtered by test config.'),
  ]).describe('Module paths or "git-change" to auto-detect from git diff'),
  integration_scenarios: z.array(z.string()).optional().describe('Integration test scenario names'),
  extension: z.string().optional().describe('Integration test file extension (e.g. "ts", "py"; leading dot optional)'),
  integration_root: z.string().optional().describe('Parent directory of __tests__/ (relative to project_root)'),
  project_root: z.string().optional().nullable().describe('Project root directory (defaults to cwd)'),
});
```

#### 输出 (ResolveTestPathsResult)

无结构性变更。`errors` 数组可能包含新增类型的错误：
- `{ path: "git", message: "..." }` — git diff 失败
- `{ path: "config", message: "..." }` — 无 test 配置
- `{ path: "<file>", message: "Not in test config scope" }` — 文件不在测试覆盖范围

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 空 `modules` 时调用 `runTestDetectFrameworks({})` 获取扫描目录 | 复用现有 `normalizeFrameworks()` 链和 `buildPlanFromMappings()` 逻辑，无需重复 config 读取 | **备选 A**: 直接读取 config.json 推导目录 — 需复制 `deriveWorkingDirectory` 和 `buildPlanFromMappings` 逻辑 |
| D2 | 非空 `modules` 时调用 `runTestDetectFrameworks({ files })` 过滤 | 确保输出始终在测试配置范围内；`test_detect_frameworks` 已有 `files` 参数支持按文件检测框架 | **备选 B**: 直接推导不过滤 — 违背"测试配置是唯一真相来源"原则 |
| D3 | `"git-change"` 使用 `git diff HEAD --name-only` | 覆盖 staged + unstaged 全部变更，是大多数工作流场景的预期行为 | **备选 C**: `git diff --cached --name-only` — 遗漏 unstaged 变更；`git status --porcelain` — 解析更复杂 |
| D4 | `"git-change"` 失败时写入 errors 而非抛出 | 保持与现有 `errors` 收集模式一致，不中断调用方 | **备选 D**: 抛异常 — 与现有错误处理风格不一致 |
| D5 | `collectFiles` 在 `test-resolve-paths.ts` 中本地实现 | 避免修改 `test-detect-frameworks.ts`；函数体仅约 20 行 | **备选 E**: 提取到共享工具库 — 需修改 `test-detect-frameworks.ts` 导入 |
| D6 | 使用 `plan[].directory` 作为扫描根而非 `plan[].glob` | `directory` 已经是 `deriveWorkingDirectory()` 处理过的精确目录路径 | **备选 F**: 使用原始 glob 字段 — 需重复 `deriveWorkingDirectory` 逻辑 |
| D7 | 不新增 config 配置项 | 现有 `test.framework` 和 `test.overrides` 已提供完整目录推导链 | **备选 G**: 新增 `test.directories` 配置项 — 增加维护负担 |

---

## 依赖

### 运行时依赖

- zod/v4 — schema 校验（union 类型验证）
- `runTestDetectFrameworks` — 获取配置计划和文件过滤（同一进程内函数调用）
- `child_process.execSync` — 执行 `git diff` 命令（`"git-change"` 模式）

### 构建/测试依赖

- TypeScript 类型定义无需新增外部包

---

## 实现步骤

1. 修改 `test-resolve-paths.schema.ts`：`modules` 从 `z.array(z.string()).min(1)` 改为 `z.union([z.array(z.string()), z.literal("git-change")])`
2. 修改 `test-resolve-paths.ts`：
   a. 处理 `modules === "git-change"` → 执行 `git diff HEAD --name-only` 获取文件列表
   b. 处理 `modules: []` → 调用 `runTestDetectFrameworks({})` 获取 plan 并扫描目录
   c. 处理 `modules: string[]` 非空 → 调用 `runTestDetectFrameworks({ files: modules })` 过滤后推导
3. 修改 `mcp.ts`：更新 `test_resolve_paths` 工具的 description

---

## 待决问题

- 无
