# wps-claude-plugin

Claude Code 插件，集成 OpenSpec 工作流。

## 插件集成前后对比

```mermaid
flowchart LR
    subgraph 无插件时
        A1[用户提问] --> A2[直接编码]
        A2 --> A3[手动测试]
        A3 --> A4[git commit]
    end 

    subgraph 集成插件后
        B1[用户提问] --> B2[Hook: OpenSpec状态提示]
        B2 --> B3{选择命令}
        B3 --> B4["/"opsx:explore 探索思考]
        B4 --> B5["/"opsx:propose 生成提案]
        B3 --> B6["/"opsx:apply 实现任务]
        B5 --> B6
        B6 --> B7[Hook: TDD测试模板]
        B7 --> B8[完成实现]
        B8 --> B9[git commit]
        B9 --> B10[Hook: 代码审查]
        B10 --> B11["/"opsx:archive 归档]
    end
```

### Commands

| 命令 | 用途 |
|------|------|
| `/opsx:explore` | 探索问题，不写代码 |
| `/opsx:propose` | 创建变更提案 |
| `/opsx:apply` | 实现任务清单 |
| `/opsx:archive` | 归档变更 |
| `/code-review` | 代码审查 |
