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
