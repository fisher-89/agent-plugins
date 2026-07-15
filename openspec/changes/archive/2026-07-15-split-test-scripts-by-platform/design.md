# 设计: split-test-scripts-by-platform

> **变更**: split-test-scripts-by-platform
> **日期**: 2026-07-15

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Framework Registry | 硬编码的框架测试/覆盖率命令模板注册表 | `plugins/dev-team/bin/src/lib/test-framework.ts` | `schemas/index` (TestFrameworks 类型) | TypeScript |
| Test Detect Frameworks Schema | Zod 校验 schema 定义 TestPlan 和 MCP 输入/输出 | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | `zod/v4` | Zod |
| Test Detect Frameworks Command | MCP 工具：检测文件所属框架并生成执行计划 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `test-framework`, `test-detect-frameworks.schema`, `config`, `glob`, `test-exclude` | TypeScript |
| Test Runner | 执行单个 plan entry 的测试命令，处理模板替换、覆盖率解析、变异测试 | `plugins/dev-team/bin/src/lib/test-runner.ts` | `schemas`, `config`, `test-exclude`, `test-parser/*`, `child_process` | TypeScript / Node.js |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
  公共函数仅列模块级导出函数、CLI 子命令、HTTP 端点，私有函数不列入。
-->

### 新增文件

<!-- 如无新增文件，省略此子节 -->

本变更不涉及新增文件。所有改动在现有 4 个实现文件和 3 个测试文件内完成。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/lib/test-framework.ts` | FrameworkConfig 接口重构为 shell/cmd 二级嵌套；FRAMEWORK_REGISTRY 全部 8 个条目按新结构重写 | 将 `test_cmd` 和 `coverage_cleanup` 移入 `shell` 子对象，新增 `cmd` 子对象 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | script 字段从 `z.string()` 改为 `z.object({ shell: z.string(), cmd: z.string() })`；移除 `test_cmd` 和 `coverage_cleanup` 字段 | TestPlan.script 支持双脚本，移除冗余字段（D2） |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | generateScript() 拆分为 generateShellScript() 和 generateCmdScript()；buildPlanFromMappings() 构造 `{ shell, cmd }` 对象，不再写入 `test_cmd` 和 `coverage_cleanup` | 两平台独立脚本生成逻辑（D2） |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | buildTestCommand 增加平台选择；resolveShell() 更新 Windows 选择策略；executeStrykerMutation() 改为从 FrameworkConfig 读取平台对应的 mutation_execution 模板并替换 {config} 占位符 | 运行时根据平台选择对应脚本和 shell，变异命令不再硬编码 |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | 更新测试用例以匹配新的二级嵌套结构 | 验证升级后结构和值正确性 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | 更新测试用例以匹配双脚本 plan 输出 | 验证 script 字段类型改为对象 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | 更新测试用例 mock 的 entry.script 为对象结构 | 验证平台选择逻辑 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `getFrameworkConfig` | `test-framework.ts` | 修改 | `function getFrameworkConfig(framework: string): FrameworkConfig` | 返回类型 `FrameworkConfig` 结构变更（返回类型签名不变，但接口定义已变） |
| `runTestDetectFrameworks` | `test-detect-frameworks.ts` | 修改 | `function runTestDetectFrameworks(options: TestDetectFrameworksOptions): TestDetectFrameworksResult` | `plan[].script` 类型从 `string` 变为 `{ shell: string; cmd: string }`；`plan[]` 移除 `test_cmd` 和 `coverage_cleanup` 字段（D2） |
| `executePlanEntry` | `test-runner.ts` | 修改 | `function executePlanEntry(entry: TestPlan, projectRoot: string, options?: { files?: string[]; timeout?: number; noMutation?: boolean; mutationDiffFiles?: string[] }): ExecutionResult` | `entry.script` 类型从 `string` 变为 `{ shell: string; cmd: string }`，`buildTestCommand` 根据平台选择 `.cmd` 或 `.shell` |

<!-- 以下函数为模块内部私有函数（以下划线前缀或模块作用域内），不列入本表：
     generateShellScript, generateCmdScript, buildPlanFromMappings, buildTestCommand, resolveShell, runCommand, executeStrykerMutation
-->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `FrameworkConfig` | `test-framework.ts` | 修改 | `framework: TestFrameworks` + `shell: { test_execution: string; coverage_cleanup: string[] }` + `cmd: { test_execution: string; coverage_cleanup: string[] }` + 共享字段 `coverage_format`, `coverage_output`, `coverage_artifacts`, `default_glob`, `mutation_framework` |
| `TestPlan` | `test-detect-frameworks.schema.ts` | 修改 | `script` 字段类型从 `string` 改为 `{ shell: string; cmd: string }`；移除 `test_cmd` 和 `coverage_cleanup` 冗余字段（决策 D2） |

### 配置

<!-- 本变更不涉及配置文件键变更，省略此子节 -->

本变更不涉及 `plugin.json`、`config.json`、`hooks.json`、`settings.json` 等配置文件的结构或键变更。

---

## 数据模型

### FrameworkConfig（重构后）

```typescript
interface FrameworkConfig {
  framework: TestFrameworks;           // 共享 — 框架标识
  shell: {
    test_execution: string;            // Shell (bash/POSIX) 测试命令模板
    coverage_cleanup: string[];        // Shell 覆盖率清理路径列表
    mutation_execution?: string;       // Shell 变异测试命令模板，{config} 占位符指向 Stryker 配置路径
  };
  cmd: {
    test_execution: string;            // Windows cmd.exe 测试命令模板
    coverage_cleanup: string[];        // Windows cmd.exe 覆盖率清理路径列表
    mutation_execution?: string;       // Windows cmd.exe 变异测试命令模板
  };
  coverage_format: 'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py';  // 共享
  coverage_output: string;             // 共享
  coverage_artifacts: string[];        // 共享
  default_glob: string;                // 共享
  mutation_framework: string | null;   // 共享 — 变异框架名称
}
```

**设计要点**:
- 平台相关字段（测试命令、清理路径、变异命令）分别放在 `shell` / `cmd` 子对象内
- 平台无关字段（格式、输出路径、globs、mutation 框架名）保持在顶层
- `mutation_execution` 为可选字段：仅 `mutation_framework` 非 null 时提供（jest/vitest/vite-plus 提供，其余 5 个框架不提供）
- `mutation_execution` 模板使用 `{config}` 占位符指代 Stryker 临时配置文件路径
- 简单框架的 `cmd.mutation_execution` 与 `shell.mutation_execution` 相同（均为 `npx stryker run "{config}"`）
- `FRAMEWORK_REGISTRY` 所有 8 个框架条目均按此结构重写

### TestPlan（重构后）

```typescript
interface TestPlan {
  directory: string;
  framework: string;
  coverage_format: 'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py';
  coverage_output: string;
  coverage_artifacts?: string[];
  mutation_framework?: string | null;
  mutation_config?: { score: number } | null;
  mutation_score?: number | null;
  script: {
    shell: string;                      // generateShellScript() 输出
    cmd: string;                        // generateCmdScript() 输出
  };
}
```

### 数据流

```
FRAMEWORK_REGISTRY (test-framework.ts)
  ├── shell: { test_execution, coverage_cleanup, mutation_execution? }  → generateShellScript()
  └── cmd: { test_execution, coverage_cleanup, mutation_execution? }    → generateCmdScript()
       ↓
buildPlanFromMappings() → TestPlan.script: { shell, cmd }
       ↓
┌─ buildTestCommand() → 平台选择 entry.script.cmd / entry.script.shell ─┐
│       ↓                                                                │
│  runCommand() → resolveShell() → execSync(cmd, { shell })              │
│                                                                        │
├─ executeStrykerMutation() → 平台选择 mutation_execution ──────────────┤
│       ↓                                                                │
│  读取 entry.framework → getFrameworkConfig()                           │
│    → shell.mutation_execution / cmd.mutation_execution                 │
│    → 替换 {config} 占位符                                              │
│       ↓                                                                │
│  runCommand() → resolveShell() → execSync(cmd, { shell })              │
└────────────────────────────────────────────────────────────────────────┘
```


### 8 个框架的 test_execution + mutation_execution 概览

| 框架 | shell.test_execution | cmd.test_execution | shell.mutation_execution | cmd.mutation_execution | 说明 |
|------|---------------------|--------------------|--------------------------|------------------------|------|
| jest | `npx jest...` | 与 shell 相同 | `npx stryker run "{config}"` | 与 shell 相同 | StrykerJS，npx 跨平台 |
| vitest | `npx vitest run...` | 与 shell 相同 | `npx stryker run "{config}"` | 与 shell 相同 | StrykerJS，npx 跨平台 |
| vite-plus | `vp test...` | 与 shell 相同 | `npx stryker run "{config}"` | 与 shell 相同 | StrykerJS，npx 跨平台 |
| bun | `bun test...` | 与 shell 相同 | — (无) | — (无) | mutation_framework = null |
| node-test | `node --test...` | 与 shell 相同 | — (无) | — (无) | mutation_framework = null |
| go | `go test...` | 与 shell 相同 | — (无) | — (无) | mutation_framework = null |
| rust | `cargo test; _X=$?...` | 多行脚本（详见 generateCmdScript） | — (无) | — (无) | mutation_framework = null，`%errorlevel%` 利用逐行解析捕获 |
| pytest | `pytest -v {files}; _X=$?...` | 多行脚本（详见 generateCmdScript） | — (无) | — (无) | mutation_framework = null，同上 |

### executeStrykerMutation 改造

**现状**：`npx stryker run "${normalizedConfigPath}"` 硬编码在 `test-runner.ts` 中

**改造后**：
```
executeStrykerMutation(entry, absoluteDirectory, filteredSources)
  → getFrameworkConfig(entry.framework)  // 获取框架配置
  → resolveStrykerConfig(absoluteDirectory, filteredSources, entry.framework)  // 生成临时配置
  → 平台选择:
    ├── win32 && !process.env.SHELL → config.cmd.mutation_execution 或 fallback config.shell.mutation_execution
    └── else → config.shell.mutation_execution
  → 替换 {config} 占位符为 normalizedConfigPath
  → runCommand(mutationCmd, absoluteDirectory, 1200000)  // 自动继承 resolveShell()
```

### 脚本生成函数对比

| 方面 | generateShellScript() | generateCmdScript() |
|------|----------------------|---------------------|
| 输入 | `frameworkConfig.shell` | `frameworkConfig.cmd` |
| cd 命令 | `cd <dir>` | `cd /d <dir>` |
| 清理命令 | `rm -rf <item>` | `if exist <item> (rmdir /s /q <item> 2>nul & del /f /q <item> 2>nul)` |
| 退出码捕获 | `; _X=$?; ... exit $_X` | 多行脚本，逐行解析自动捕获 `%errorlevel%` |
| 行分隔符 | `\n` | `\r\n` |
| 输出 | bash/POSIX 兼容脚本 | cmd.exe 兼容脚本 |

> **cmd.exe 退出码捕获原理**：不使用 `&` 链式（`%errorlevel%` 在整行解析时展开，无法捕获运行时退出码），而是生成多行脚本。cmd.exe 逐行解析执行，第 N+1 行的 `%errorlevel%` 正确反映第 N 行的退出码。
>
> **清理命令原理**：`rmdir /s /q` 只作用于目录，对文件静默失败；`del /f /q` 只作用于文件，对目录静默失败。二者组合 (`&`) 确保无论目标是文件还是目录都能被清理。

---

## 路由/API 设计

<!-- 本变更不涉及 HTTP API，省略此节 -->

本变更涉及两个 MCP 工具的增量修改：
- `test-detect-frameworks` — 输出 schema 中 `plan[].script` 从 `string` 改为 `{ shell: string; cmd: string }`，移除 `plan[].test_cmd` 和 `plan[].coverage_cleanup`（D2）
- `cli-unit-test-execute` — 内部执行逻辑平台感知，无 schema 变化

均通过 MCP 工具注册表（`plugins/dev-team/bin/src/commands/`）调用，不涉及 HTTP API。

---

## 依赖

### 运行时依赖

- `child_process` (Node.js 内置) — `execSync` 执行测试命令，使用 `resolveShell()` 动态选择 shell
- `zod/v4` — schema 校验，`testPlanSchema.script` 类型变更

### 构建/测试依赖

- `vite-plus/test` — 测试框架，用于运行单元测试
- 无新增依赖

---

## 决策记录

### D1: cmd.exe 退出码捕获 — 采用多行脚本（问题 1-2 已决）

**结论**：`&` 链式 + `%errorlevel%` 不可行。cmd.exe 在整行解析时展开 `%errorlevel%`，无法捕获运行时退出码。

**方案**：`generateCmdScript()` 生成多行脚本，每行一条命令。cmd.exe 逐行解析执行，后续行的 `%errorlevel%` 正确反映前一行的退出码。

```
cargo test
if errorlevel 1 set _X=%errorlevel%
cargo llvm-cov --json --output-path coverage/coverage-summary.json
exit /b %_X%
```

`if errorlevel 1` 语法在 cmd.exe 中表示 "errorlevel ≥ 1"，语义正确。

### D2: 移除 TestPlan 上的 `test_cmd` 和 `coverage_cleanup`（问题 4 已决）

**结论**：移除。这两个字段在设计中被标记为"向后兼容"，但验证确认无下游消费者：

- `test-runner.ts`（唯一运行时消费者）：零引用，`buildTestCommand()` 仅使用 `entry.script`
- `test-execution.ts`、`test-resolve-paths.ts`：零引用
- 拆分后 `test_cmd` 仅指向 shell 版本，与"双平台"设计语义矛盾
- Schema 中已是 `.optional()`，移除不破坏兼容性
- 信息完全包含在 `script: { shell, cmd }` 中

### D3: cmd.exe 清理命令 — 组合 `rmdir` + `del`（问题 3 已决）

**结论**：`rmdir /s /q` 只作用于目录，对文件会失败。8 个框架的 `coverage_cleanup` 路径中混合了文件（`test-stderr.txt`, `coverage.out`, `.coverage`）和目录（`coverage`, `.nyc_output`, `target/llvm-cov`, `htmlcov`）。

**方案**：`generateCmdScript()` 对每个 cleanup 项使用组合命令：

```cmd
if exist "<item>" (rmdir /s /q "<item>" 2>nul & del /f /q "<item>" 2>nul)
```

`rmdir` 对文件静默失败，`del` 对目录静默失败，二者之一必然成功。
