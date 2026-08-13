# wps-agent-plugin

## 安装

### Claude Code 安装

将本仓库配置为 Claude Code 插件源：

```bash
claude plugin marketplace add https://github.com/fisher-89/agent-plugins.git
```

安装插件

```bash
claude plugin install dev-team@wps-ai
```

### Cursor 安装

#### 方式一：Marketplace 插件（推荐）

进入 Cursor Desktop，输入：

```
/add-plugin fisher-89/agent-plugins/cursor-plugins/dev-team
```

#### 方式二：Home 镜像（团队禁用插件通道时）

当 Cursor 策略关闭第三方 / 本地插件（`userLocal=false`）时，改用用户级 `~/.cursor` 镜像安装：

```bash
git clone https://github.com/fisher-89/agent-plugins.git
cd agent-plugins
node cursor-home-image/dev-team/install.mjs
```

安装完成后在 Cursor 执行 **Developer: Reload Window**，以加载 skills / agents / hooks / mcp。

## 使用插件

### dev-team

按顺序执行执行 `/phase-*` 命令，或执行 `/workflow-*` 命令启动流程。以下是目前支持的场景：

**需求开发**

```bash
/openspec-explore <需求描述>   # 【可选】深度分析需求

# 以下流程可以使用 /workflow-requirement 自动执行
/phase-proposal               # 创建需求提案（创建change目录 /openspec/changes/xxx/，产物 proposal.md | specs)
/phase-dev-design             # 技术实现设计（产物 design.md | task.md)
/phase-test-design            # 测试设计（产物 test-design.md)
/phase-implement              # 实现功能代码
/phase-test-gen               # 生成测试代码
/phase-test-execution         # 执行测试（产物 reports/test/summary.json）
/phase-code-review            # 代码审查
/phase-acceptance             # 流程验收

/openspec-archive-change      # 归档
```

**生成测试用例**

```bash
# 以下流程可以使用 /workflow-test-only 自动执行
/phase-proposal <需求描述>     # 创建需求提案
/phase-code-analyze           # 分析源码
/phase-test-design            # 测试设计
/phase-test-gen               # 生成测试代码
/phase-test-execution         # 执行测试

/openspec-archive-change      # 归档
```

## 项目配置（`openspec/config.json`）

在项目根目录创建 `openspec/config.json`，配好后，测试执行、静态分析、文件保护会更贴近你的仓库。

可在文件顶部加上 `$schema`，编辑器会给出补全与校验：

```json
{
  "$schema": "https://raw.githubusercontent.com/fisher-89/agent-plugins/master/plugins/dev-team/bin/dev-team-config.schema.json"
}
```

### 新手先配这几项

| 字段 | 作用 | 要不要配 |
|------|------|----------|
| `tests` | 测试套件：框架、范围、覆盖率门槛 | 要跑测试时必配 |
| `static_analysis` | 静态检查命令（lint / typecheck 等） | 按需 |
| `write_protection` | 禁止 Agent 改写的文件 | 可选 |

### 最小可用示例

单包、Vitest 项目可以这样起步：

```json
{
  "$schema": "https://raw.githubusercontent.com/fisher-89/agent-plugins/master/plugins/dev-team/bin/dev-team-config.schema.json",
  "tests": [
    {
      "root": ".",
      "framework": "vitest",
      "config": "vitest.config.ts"
    }
  ],
  "static_analysis": "pnpm lint"
}
```

说明：

- `tests[].root`：该套件覆盖的源码根目录（相对项目根，**不要**写 `*` 等通配符）
- `tests[].framework`：常用值如 `vitest`、`jest`、`vite-plus`、`pytest`、`go`、`rust` 等
- `tests[].config`：框架配置文件路径（相对 `root`）
- 未写 `coverage` 时，默认门槛约为行 80% / 分支 70% / 函数 75%

### 常用进阶（按需添加）

**缩小测试范围、提高覆盖率门槛：**

```json
{
  "tests": [
    {
      "root": "packages/api",
      "framework": "vitest",
      "config": "vitest.config.ts",
      "includes": ["src/**/*.{ts,tsx}"],
      "excludes": ["src/**/*.stories.ts"],
      "coverage": { "lines": 90, "branches": 80, "functions": 85 }
    }
  ]
}
```

**保护敏感或不希望被 Agent 改写的文件**（`openspec/config.json` 与各 change 下的 `eval.json` 已有内置保护，一般不用重复写）：

```json
{
  "write_protection": {
    "files": [{ "glob": "secrets/**" }]
  }
}
```

配好后保存即可；下次执行 `/phase-test-execution` 或相关 workflow 时会读取该配置。
