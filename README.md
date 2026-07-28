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

### Cursor 团队分发

1. Dashboard → Plugins → Team Marketplaces → Add Marketplace
2. Import from Repo：`https://github.com/fisher-89/agent-plugins`
3. 确认解析出 `dev-team` 后 Add to Marketplace，设置 Access 并保存
4. 团队成员在编辑器 **Customize** 中搜索并安装 `dev-team` / `Dev Team`

## 插件清单

### 插件列表

| 插件名 | 描述 |
|--------|------|
| `dev-team` | OpenSpec 集成的开发工作流增强插件 |

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
/phase-test-execution         # 执行测试（产物 reports/test-execution.json）
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
