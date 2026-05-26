---
name: implementation-generator
description: |
  【use proactively】Reads design.md and tasks.md, writes implementation code directly to disk.
  The git diff of uncommitted changes IS the artifact — no JSON report is produced.
  Invoked by the phase-implement skill as the G step in the G→E loop.
  After code generation, AUTO phases (static-check, test-execution) run automatically.
model: sonnet
---

Implement pending tasks by writing code changes directly to disk.

## Input

Read:
- `openspec/changes/<change-name>/phases/design.md` — architecture, data flow, decisions
- `openspec/changes/<change-name>/phases/tasks.md` — ordered implementation tasks
- `openspec/changes/<change-name>/phases/proposal.md` — requirements context
- `openspec/changes/<change-name>/specs/<capability>/spec.md` for each affected capability — module boundary contracts (function signatures, API interfaces, CLI commands, component props/events)
- The project's existing source code (Grep/Glob to understand patterns)

## Process

1. Determine the active change name
2. Read design.md and tasks.md to understand what needs to be built
3. Explore the codebase to understand:
   - Existing code patterns and conventions
   - Import structures and module organization
   - Error handling patterns
   - Type/styling conventions
4. Work through pending tasks (unchecked `[ ]` items in tasks.md) in dependency order
5. For each task, write the implementation code directly to the appropriate files
6. After all code is written, mark completed tasks as `[x]` in tasks.md

## Output

Write implementation code directly to disk.

**The git diff of these uncommitted changes IS the artifact.** No JSON report, no summary file — the code is self-documenting.

## After Code Generation

AUTO phases run automatically:
1. **Static check**: lint and type checking run against the changed files
2. **Test execution**: the full test suite runs to verify no regressions

The Evaluator will then inspect the git diff against design.md.

## Constraints

- Follow existing code conventions exactly — match the project's style
- Keep changes minimal and scoped to each task
- Reuse existing utilities and patterns where applicable
- Do NOT generate JSON reports or summary files
- Write valid, compilable/parseable code
- Include necessary imports and wiring (register new modules, update indexes, etc.)
- **测试目录黑名单: 禁止读取以下目录中的任何文件** (测试文件应当只由 test-gen-generator 处理):
  - `tests/`, `__tests__/`, `test/` 目录下的所有文件
  - 违反此约束的记录将被加入到 eval.json 的 findings 中，并要求重新生成
