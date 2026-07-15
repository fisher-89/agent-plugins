# 提案: split-test-scripts-by-platform

> **变更**: split-test-scripts-by-platform
> **日期**: 2026-07-15
> **状态**: 起草

---

## 问题

当前测试执行脚本（`test-runner.ts`）在 Windows 平台上依赖 Git Bash 提供的 POSIX shell 环境。所有测试命令和覆盖率清理命令均使用 Unix shell 语法（`;` 命令链、`rm -rf` 清理、`\n` 换行分隔），导致在仅提供 cmd.exe 的 Windows 环境中全部失败。

**数据流现状**:

```
FRAMEWORK_REGISTRY (test-framework.ts) → FrameworkConfig.test_cmd
    ↓
generateScript() (test-detect-frameworks.ts) → TestPlan.script (bash string)
    ↓
buildTestCommand() (test-runner.ts) → entry.script
    ↓
runCommand() (test-runner.ts) → resolveShell() → 在 Windows 上返回 bash，否则返回 undefined
    ↓
execSync(cmd, { shell: resolveShell() })
```

**核心问题**:
1. **`resolveShell()`** 在 Windows 上返回 `process.env.SHELL || 'bash'`，若 Git Bash 未安装则直接失败
2. **`test_cmd`** 模板字符串全部使用 Unix shell 语法（`;` 链式命令、`$_X` 退出码捕获），cmd.exe 无法解析
3. **`coverage_cleanup`** 使用 `rm -rf`，cmd.exe 依赖 `del` / `rmdir`
4. **`generateScript()`** 使用 `\n` 拼接命令，产生仅适用于 bash 的脚本

---

## 提案

采用 **双脚本模式**：为每个框架同时维护 Shell（bash/POSIX）和 Cmd（Windows cmd.exe）两套命令模板和脚本生成逻辑，在运行时根据 `process.platform` 自动选择。

### 统一二级嵌套结构

`FrameworkConfig` 和 `TestPlan.script` 均采用 `shell`/`cmd` 二级嵌套结构，保持一致：

```typescript
// FrameworkConfig — 原始命令模板
interface FrameworkConfig {
  framework: TestFrameworks;
  shell: {
    test_execution: string;      // Shell 测试命令模板
    coverage_cleanup: string[];  // Shell 覆盖率清理目录/文件
  };
  cmd: {
    test_execution: string;      // Windows cmd.exe 测试命令模板
    coverage_cleanup: string[];  // Windows cmd.exe 覆盖率清理目录/文件
  };
  // 平台无关的共享字段
  coverage_format: 'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py';
  coverage_output: string;
  coverage_artifacts: string[];
  default_glob: string;
  mutation_framework: string | null;
}

// TestPlan — 生成的执行脚本
interface TestPlan {
  // ...
  script: {
    shell: string;  // generateShellScript() 生成的 bash 脚本
    cmd: string;    // generateCmdScript() 生成的 cmd.exe 脚本
  };
}
```

**设计要点**:
- `FrameworkConfig.shell.test_execution` — 原 `test_cmd`
- `FrameworkConfig.cmd.test_execution` — Windows cmd.exe 版测试命令
- `FrameworkConfig.shell.coverage_cleanup` — 原 `coverage_cleanup`（`rm -rf`）
- `FrameworkConfig.cmd.coverage_cleanup` — Windows cmd.exe 版清理列表（`rmdir /s/q`, `del /q/f`）
- `TestPlan.script.shell` — 生成的 bash 脚本字符串
- `TestPlan.script.cmd` — 生成的 cmd.exe 脚本字符串
- 共享字段（`framework`、`coverage_format`、`coverage_output`、`coverage_artifacts`、`default_glob`、`mutation_framework`）保持在 `FrameworkConfig` 顶层

### 各层变更摘要

1. **`FrameworkConfig` 重构**（`test-framework.ts`）
   - 将 `test_cmd`、`coverage_cleanup` 移入 `shell: { test_execution, coverage_cleanup }`
   - 新增 `cmd: { test_execution, coverage_cleanup }` 子对象
   - `FRAMEWORK_REGISTRY` 中所有 8 个框架条目按新结构重写
   - `getFrameworkConfig()` 返回新结构

2. **`TestPlan` Schema 重构**（`test-detect-frameworks.schema.ts`）
   - `script` 字段从 `z.string()` 改为 `z.object({ shell: z.string(), cmd: z.string() })`

3. **`generateScript()` 拆分**（`test-detect-frameworks.ts`）
   - `generateShellScript()` — 从 `frameworkConfig.shell` 读取，产生 bash 脚本
   - `generateCmdScript()` — 从 `frameworkConfig.cmd` 读取，产生 cmd.exe 脚本：
     - 使用 `cd /d <dir>` 代替 `cd <dir>`
     - 使用 `if exist <dir> rmdir /s/q <dir>` 代替 `rm -rf <dir>`
     - 使用 `if exist <file> del /q /f <file>` 代替 `rm -f <file>`
     - 使用 `&&` 链式命令 + `%errorlevel%` 代替 `; _X=$?; exit $_X`
   - `buildPlanFromMappings()` 构造 `script: { shell, cmd }` 对象

4. **`test-runner.ts` 平台感知选择**
   - `buildTestCommand()` — 根据平台选择 `entry.script.cmd`（Windows）或 `entry.script.shell`（Unix）
   - `resolveShell()` — 在 Windows 上返回 `process.env.SHELL || process.env.COMSPEC || 'cmd.exe'`，Unix 上返回 `undefined`
   - `runCommand()` — 根据选择的脚本类型传递对应的 shell 参数
   - `executeStrykerMutation()` — `npx stryker run` 使用相同的 shell 选择逻辑

5. **框架特定命令的 Windows 适配**
   - **简单命令**（jest, vitest, vite-plus, bun, node-test, go）— `cmd.test_execution` 与 `shell.test_execution` 一致
   - **链式命令**（rust, pytest）— 使用 `&&` 和 `%errorlevel%` 重新实现
   - **清理命令**— `shell.coverage_cleanup` 使用 `rm -rf`，`cmd.coverage_cleanup` 使用 `rmdir /s/q` / `del /q/f`

### 向后兼容

- Unix（Linux/macOS）行为完全不变
- Windows 上若 `process.env.SHELL` 存在（Git Bash 已安装），`resolveShell()` 仍可回退到 bash
- `TestPlan.script` 虽是对象但语义清晰，内部调用方只读不写

---

## 能力

### 新增能力

无。本次变更在现有 `test-detect-frameworks` 和 `cli-unit-test-execute` 能力基础上做增量修改。

### 修改的能力

- `test-detect-frameworks` — `FrameworkConfig` 接口重构为 `shell`/`cmd` 二级嵌套；`FRAMEWORK_REGISTRY` 按新结构重写；`TestPlan.script` 从字符串改为 `{ shell, cmd }` 对象；`generateScript()` 拆分为 `generateShellScript()` 和 `generateCmdScript()`。

- `cli-unit-test-execute` — `buildTestCommand()` 根据平台选择 `entry.script.cmd` 或 `entry.script.shell`；`resolveShell()` 增加 cmd.exe 选择逻辑。

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/lib/test-framework.ts` — `FrameworkConfig` 接口重构为 `shell`/`cmd` 二级结构；`FRAMEWORK_REGISTRY` 按新结构重写
- `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` — `script` 字段从 `z.string()` 改为 `z.object({ shell: z.string(), cmd: z.string() })`
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` — `generateScript()` 拆分为 `generateShellScript()` 和 `generateCmdScript()`；`buildPlanFromMappings()` 构造 `{ shell, cmd }` 对象
- `plugins/dev-team/bin/src/lib/test-runner.ts` — `buildTestCommand()` 读取 `entry.script.cmd` / `entry.script.shell`；`resolveShell()` 更新策略

### 测试文件

- `plugins/dev-team/bin/src/lib/test-framework.test.ts` — 验证二级结构
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` — 验证双脚本生成和 plan 输出
- `plugins/dev-team/bin/src/lib/test-runner.test.ts` — 验证平台感知选择

### 不要修改

- `plugins/dev-team/bin/src/commands/test-execution.ts`
- `plugins/dev-team/bin/src/schemas/index.ts`
- `plugins/dev-team/bin/src/lib/test-parser/`
- 项目 `config.json` 结构
- `plugins/dev-team/bin/src/lib/test-exclude.ts`
- 非 `plugins/dev-team/bin/src/` 目录下的任何文件

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `FrameworkConfig` 二级嵌套 | `FrameworkConfig` 包含 `shell: { test_execution, coverage_cleanup }` 和 `cmd: { test_execution, coverage_cleanup }` 子对象；`FRAMEWORK_REGISTRY` 所有 8 个框架按新结构提供 |
| AC-2 | `TestPlan.script` 改为对象 | `testPlanSchema` 中 `script` 为 `z.object({ shell: z.string(), cmd: z.string() })` |
| AC-3 | Shell 脚本生成 | `generateShellScript()` 从 `frameworkConfig.shell` 读取，输出与原 `generateScript()` 一致 |
| AC-4 | Cmd 脚本生成 | `generateCmdScript()` 从 `frameworkConfig.cmd` 读取，使用 `cd /d`、`rmdir /s/q` 等 cmd.exe 兼容语法 |
| AC-5 | Plan 输出 | `buildPlanFromMappings()` 返回 `script: { shell: "...", cmd: "..." }` |
| AC-6 | `buildTestCommand` 平台选择 | Windows 选 `entry.script.cmd`，Unix 选 `entry.script.shell` |
| AC-7 | `resolveShell` 平台感知 | Windows: `process.env.SHELL \|\| process.env.COMSPEC \|\| 'cmd.exe'`；Unix: `undefined` |
| AC-8 | Stryker 命令 | `npx stryker run` 继承 `resolveShell()` 的平台感知 |
| AC-9 | Unix 向后兼容 | Unix 上所有测试行为与变更前完全一致 |
| AC-10 | Windows bash 回退 | `process.env.SHELL` 存在时行为不变 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| cmd.exe 命令语法与预期不符 | Windows 测试执行失败 | 中 | Windows CI 验证；链式命令单元测试 |
| `cmd.test_execution` 维护成本 | 新增框架多一步 | 低 | 简单命令两平台相同；注册表变更有测试保护 |
| `generateCmdScript()` 跨驱动器切换 | 切换失败 | 低 | `cd /d` 已处理；目录通常为项目内相对路径 |
| `TestPlan.script` 结构变更影响消费者 | 下游编译错误 | 低 | 所有消费者在本次 refactor 中一并更新；外部通过 `buildTestCommand()` 间接消费 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| `FrameworkConfig` 结构 | `shell`/`cmd` 二级嵌套，平台字段在子对象内，共享字段在顶层 | 结构清晰，一眼看出哪些是平台相关、哪些共享 | (A) 扁平字段 `test_cmd`/`test_cmd_win`；(B) 完全分裂为两个对象 |
| `TestPlan.script` 结构 | `script: { shell: string; cmd: string }` 对象 | 与 `FrameworkConfig.shell`/`cmd` 呼应，消除 `script`/`script_win` 两个独立字段，消费者只需读一个对象 | (A) 保持 `script: string` + `script_win: string` 两个字段 |
| 子对象字段命名 | `test_execution`（非 `test_cmd`） | 语义更准确，"执行"而非"命令" | `test_cmd` — 与旧名一致但 `shell.test_cmd` 显冗余 |
| Shell 选择策略 | Windows 优先 `cmd.exe`，有 `SHELL` 时回退 bash | 默认系统自带 shell；对 Git Bash 用户向后兼容 | (A) 始终 cmd.exe；(B) 用户配置 |
| 脚本生成函数 | `generateShellScript()` + `generateCmdScript()` 两个独立函数 | 两平台逻辑差异大，合并会产生过多分支 | 一个函数加 `platform` 参数 |

### 待决问题

- Rust `shell.test_execution` 链式命令的 cmd.exe 等价语法（`cargo test; _X=$?; cargo llvm-cov ...` → `%errorlevel%` ）
- pytest 同理
