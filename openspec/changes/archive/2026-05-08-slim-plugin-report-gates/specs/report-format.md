# Spec: Step Report Format

> **Change**: slim-plugin-report-gates
> **Capability**: report-format

## Overview

每个 SDD 步骤执行后产出的标准化报告格式，用于 Hook 层检查流程完整性。

## Report Schema

```json
{
  "schema": "openspec-step-report/v1",
  "change": "<change-name>",
  "task_id": "<task-number>",
  "step": "<step-name>",
  "status": "pass|fail|skip",
  "timestamp": "<ISO-8601-UTC>",
  "duration_ms": 150,
  "details": {}
}
```

## Step Names

| Step | 产出脚本 | 含义 |
|------|----------|------|
| `scope` | test-scope.py | 测试范围识别 |
| `skeleton` | test-generator.py | 测试骨架生成 |
| `lint` | lint-runner.py | 代码检查 |
| `scoped-test` | test-runner.py | 范围测试 |
| `full-test` | test-runner.py | 全量测试 |
| `code-review` | review-parser.py | 代码审查 |

## Report Chain Rules

每个 task 的标准链：`scope → skeleton → lint → scoped-test`

时间戳必须严格递增。

全量流程：`full-test → code-review`（所有 task 完成后）

## Storage

```
openspec/changes/<name>/reports/
├── task-<N>_<step>.json   (per-task)
├── full-test.json          (all tasks done)
└── code-review.json        (after full-test)
```
