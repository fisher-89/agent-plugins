# wps-agent-plugin

## 安装

### wps_claude 安装

将本仓库配置为 Claude Code 插件源：

```bash
wps_claude plugin marketplace add https://ksogitlab.kso.net/zhangbohan3/wps-agent-plugin.git
```

安装插件

```bash
wps_claude plugin install dev-team@wps-ai
```

## 插件清单

### 插件列表

| 插件名 | 描述 |
|--------|------|
| `dev-team` | OpenSpec 集成的开发工作流增强插件 |

### dev-team

需求开发

```bash
/openspec-explore <需求描述>   # 【可选】深度分析需求

# 以下流程可以使用 /workflow-requirement 自动执行
/phase-proposal               # 创建需求提案
/phase-dev-design             # 技术实现设计
/phase-test-design            # 测试设计
/phase-implement              # 实现功能代码
/phase-test-gen               # 生成测试代码
/phase-test-execution         # 执行测试
/phase-code-review            # 代码审查
/phase-acceptance             # 流程验收

/openspec-archive-change      # 归档
```

生成测试用例
```bash
# 以下流程可以使用 /workflow-test-only 自动执行
/phase-proposal <需求描述>     # 创建需求提案
/phase-code-analyze           # 分析源码
/phase-test-design            # 测试设计
/phase-test-gen               # 生成测试代码
/phase-test-execution         # 执行测试

/openspec-archive-change      # 归档
```
