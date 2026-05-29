# 测试设计: reorder-workflow-phases

> **变更**: reorder-workflow-phases
> **日期**: 2026-05-28
> **基于**: proposal.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | `workflow.ts` 中的 `PHASES` 数组顺序、`getPhaseIndex`、`getPriorPhases` 函数行为 | vite-plus/test (vitest-compatible, `import { describe, it, expect } from "vite-plus/test"`) | 100% 覆盖 `workflow.ts` 导出函数，验证新阶段顺序正确性 |
| 集成测试 | 文件系统结构（目录重命名、agent 文件名变更）、agent.md 输入声明、SKILL.md 引用、backtrack_to 引用、CLI eval-check 行为 | bats (shell 自动化测试) + 独立 shell 脚本 (`.sh`)，配合 `grep` 全量搜索验证 | 验证所有文件已正确重命名、引用已更新、CLI 命令行为与 spec 一致 |

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `tests/test_workflow_phases_order.test.ts` | 单元测试 | 功能验证 |
| AC-2 | `tests/test_directory_renames.sh` | 集成测试 | 结构验证 |
| AC-3 | `tests/test_directory_renames.sh` | 集成测试 | 结构验证 |
| AC-4 | `tests/test_agent_inputs.sh` | 集成测试 | 内容验证 |
| AC-5 | `tests/test_agent_inputs.sh` | 集成测试 | 内容验证 |
| AC-6 | `tests/test_skill_references.sh` | 集成测试 | 引用验证 |
| AC-7 | `tests/test_skill_references.sh` | 集成测试 | 引用验证 |
| AC-8 | `tests/test_backtrack_references.sh` | 集成测试 | 全量搜索验证 |
| AC-9 | `tests/test_eval_check_cli.bats` | 集成测试 | CLI 行为验证 |
| AC-10 | `tests/test_eval_check_cli.bats` | 集成测试 | CLI 行为验证 |

---

## 3. 测试策略

### 3.1 方法

本变更为纯结构性变更——不引入新逻辑，仅涉及重命名、重排序和引用更新。测试策略围绕三个维度展开：

1. **单元级别的 API 正确性**：`workflow.ts` 的导出函数行为在重排后是否正确，包括 PHASES 数组顺序、索引计算、前序阶段推导。
2. **文件系统完整性**：验证所有重命名操作已正确执行，旧文件不再存在，新文件存在，且内容符合要求。
3. **引用一致性**：验证所有跨文件引用（SKILL.md 中的 subagent_type、gate check phase 标识符、agent.md 中的 eval-log phase 参数、backtrack_to 引用）已同步更新，无遗漏。

### 3.2 测试分类

- **单元测试**: 针对 `plugins/dev-team/bin/src/lib/workflow.ts` 的三个导出（`PHASES`、`getPhaseIndex`、`getPriorPhases`），验证新顺序下的行为。基于现有 `workflow.test.ts` 修改扩展，遵循 `vite-plus/test` 框架。
- **集成测试**: 
  - 文件系统断言：测试目录存在性 (`skills/phase-dev-design/` 存在，`skills/phase-dev-proposal/` 不存在)、agent 文件存在性 (`agents/dev-design-planner.md` 存在、`agents/dev-proposal-planner.md` 不存在)。
  - 内容断言：使用 `grep`/`head` 解析 Markdown frontmatter 和正文，验证 agent.md 的 Input 章节、SKILL.md 的 phase/gate-check 引用。
  - 全量搜索断言：对全仓库执行 `grep -r "dev-proposal"` 和 `grep -r "02-test-design"` 验证无旧标识符残留（白名单除外）。
  - CLI 断言：使用 `dev-team eval-check --change <test-fixture> --phase <phase-id>` 验证前置阶段推导正确。

#### 测试分类说明

- **单元测试**: 不依赖外部服务/数据库的测试，测试独立函数、方法或组件的单一行为。使用 mock 模拟所有外部依赖。
- **集成测试**: 依赖 mock 外部服务的测试，验证多个模块/组件的协同工作。可以启动轻量级测试容器或使用内存数据库。

> **注意**: 不再使用端到端测试 (E2E)。所有测试均使用 mock，不依赖真实外部环境。

### 3.3 模拟策略

| 依赖 | 模拟方式 | 使用场景 |
|------|----------|----------|
| `openspec/changes/` 文件系统 | 单元测试中不依赖；集成测试使用预置 fixture 目录（创建最小化 `eval.json`、`proposal.md` 等） | `test_eval_check_cli.bats` 需要模拟已通过的前置阶段记录 |
| `dev-team MCP eval_check` | 不模拟——CLI 测试直接调用真实 `dev-team eval-check` 命令，使用 fixture eval.json 文件控制前置条件 | AC-9, AC-10 验证 `getPriorPhases` 在真实 CLI 管道的表现 |

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| `getPriorPhases` 查询旧阶段标识符 | 调用 `getPriorPhases("03-dev-proposal")`（旧 dev-proposal 标识符） | 返回 `[]`（fault-tolerant，不抛出异常） | `tests/test_workflow_phases_order.test.ts` |
| `getPriorPhases` 查询旧阶段标识符 02-test-design | 调用 `getPriorPhases("02-test-design")`（旧 test-design 标识符） | 返回 `[]`（fault-tolerant） | `tests/test_workflow_phases_order.test.ts` |
| `getPhaseIndex` 查询旧标识符 | 调用 `getPhaseIndex("03-dev-proposal")` | 返回 `-1`（未找到，不抛出异常） | `tests/test_workflow_phases_order.test.ts` |
| eval-check --phase 02-dev-design 且 eval.json 中 requirements 未通过 | eval.json 只有 entry 但 verdict="fail" | eval-check 退出码非 0，提示 01-requirements 未通过 | `tests/test_eval_check_cli.bats` |
| eval-check --phase 03-test-design 且 eval.json 缺失 dev-design 记录 | eval.json 只有 requirements pass 记录 | eval-check 退出码非 0，提示 02-dev-design 未执行 | `tests/test_eval_check_cli.bats` |
| `phase-dev-proposal` 和 `phase-dev-design` 两个目录同时存在（重命名冲突） | 文件系统同时包含旧目录和新目录 | 测试断言 `phase-dev-proposal` 应完全不存在；若存在则标记为残留清理项 | `tests/test_directory_renames.sh` |
| 旧 agent 文件 `dev-proposal-planner.md` 和 `dev-proposal-evaluator.md` 作为残留文件存在 | 文件系统同时包含新旧 agent.md | 测试断言旧 agent 文件不存在；验证只有新文件存在 | `tests/test_directory_renames.sh` |
| 全量 grep 搜索发现合法旧标识符引用（如 spec 文件中的历史记录） | `grep -r "dev-proposal"` 在 `openspec/specs/` 的旧文档中找到匹配 | 仅允许白名单目录（`openspec/changes/archive/`、`openspec/specs/`）中的旧标识符引用；非白名单目录出现旧标识符则测试失败 | `tests/test_backtrack_references.sh` |
| eval-check --phase 09-acceptance 验证 dev-design 在 test-design 之前的传递依赖 | eval.json 包含所有前置阶段 pas | eval-check 返回 exit code 0（代表整个链路的 gate check 正常） | `tests/test_eval_check_cli.bats` |

---

## 5. 测试数据

所有集成测试依赖的 fixture 数据存放在 `tests/fixtures/` 目录下：

| Fixture 文件 | 用途 | 结构 |
|-------------|------|------|
| `tests/fixtures/eval_requirements_pass.json` | 仅有 01-requirements 通过的 eval.json | `[{"phase":"01-requirements","verdict":"pass"}]` |
| `tests/fixtures/eval_dev_design_pass.json` | 01-requirements + 02-dev-design 通过的 eval.json | `[{"phase":"01-requirements","verdict":"pass"},{"phase":"02-dev-design","verdict":"pass"}]` |
| `tests/fixtures/eval_requirements_fail.json` | 01-requirements 未通过的 eval.json | `[{"phase":"01-requirements","verdict":"fail"}]` |
| `tests/fixtures/eval_empty.json` | 空的 eval.json | `[]` |
| `tests/fixtures/eval_old_identifiers.json` | 使用旧标识符的 eval.json（向后兼容性验证） | `[{"phase":"01-requirements","verdict":"pass"},{"phase":"02-test-design","verdict":"pass"}]` |

集成测试在执行时，会创建一个临时 change 目录并软链或复制对应 fixture 作为 `eval.json`，确保测试不会污染真实变更数据。

---

## 6. 不可测试项

- eval.json 历史数据向后兼容性（旧标识符条目与新标识符条目混合）—— **原因**: 当前无活跃变更，无法构造真实混合场景。proposal 声明"手动迁移或丢弃"，不属于自动化测试范围。
- 用户从 `phase-dev-proposal` 到 `phase-dev-design` 的迁移体验 —— **原因**: 这是开发流程变更，通过 proposal 文档和 release notes 通知用户，非代码行为。
- `eval-check.ts` / `eval-log.ts` 核心逻辑的完整性 —— **原因**: proposal.md 明确声明"不要修改"，即核心逻辑不变。如果该模块已有自身的测试，本次不增加新测试。
