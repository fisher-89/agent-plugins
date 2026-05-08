# Spec: Report Chain Check

> **Change**: slim-plugin-report-gates
> **Capability**: report-chain-check

## Overview

Hook 层通过检查报告文件的存在和时间顺序，验证 SDD 流程是否被严格执行。

## Check Points

### Commit Gate (pre-tool-commit-review.py)

当前 task 的报告链检查：
1. `task-<N>_scope.json` 存在？
2. `task-<N>_lint.json` 存在？
3. `task-<N>_scoped-test.json` 存在？
4. `scope.timestamp < lint.timestamp < scoped-test.timestamp`？

任一不满足 → deny + 提示缺失步骤

### Archive Gate (pre-tool-skill.py)

所有 task 的报告链 + 最终报告：
1. 每个 completed task 的 scope → lint → scoped-test 链完整？
2. `full-test.json` 存在且 status=pass？
3. `code-review.json` 存在且 status!=fail？
4. `full-test.timestamp < code-review.timestamp`？

任一不满足 → deny

## Error Recovery

报告缺失时，Hook 不会自动生成报告，而是 deny 并提示应执行的命令。
