# phase-agents Specification

## ADDED Requirements

### Requirement: evaluator 经查询 API 获取文件清单

`plugins/dev-team/agents/implementation-evaluator.md`、`plugins/dev-team/agents/code-review-evaluator.md` 与 `plugins/dev-team/agents/acceptance-evaluator.md` 的范围圈定步骤 SHALL 改为调用 `__MCP:workflow_files__`（输入目标 change 名）获取文件清单净状态，MUST NOT 指示直接 Read `workflow.json` 读取 `files` 清单。范围核对的三态对账判定语义不变——权威仍为 design 变更清单 × `files` 清单 × 文件系统（判定表见 `workflow-file-inventory` 规格「消费方对账三态」），本要求仅改变清单的获取通道。提示词 MUST NOT 依赖查询输出中不存在的字段（如 `source` 来源审计映射）。

#### Scenario: 提示词不再指示直接读

- **WHEN** 检查三个 evaluator md 的范围圈定步骤
- **THEN** 清单获取方式为调用 `__MCP:workflow_files__`
- **AND** 不存在「Read `workflow.json`」的 `files` 清单读取指令

#### Scenario: 对账语义不变

- **WHEN** evaluator 按新通道取得清单后执行范围核对
- **THEN** 判定仍按 design 清单 × `files` × 文件系统三态对账执行
- **AND** 判定结果与清单获取通道无关

## Module Contract

### 位置: `plugins/dev-team/agents/*.md`

| 位置 | Contract |
|------|----------|
| `plugins/dev-team/agents/implementation-evaluator.md` | 范围圈定经 `__MCP:workflow_files__` 获取清单，不直接 Read `workflow.json` |
| `plugins/dev-team/agents/code-review-evaluator.md` | 同上 |
| `plugins/dev-team/agents/acceptance-evaluator.md` | 范围权威表述引用查询 API，不直接 Read `workflow.json` |

### Function/API/CLI/Component

无可导出函数；无 API 接口；无 CLI 命令；无前端组件定义。
