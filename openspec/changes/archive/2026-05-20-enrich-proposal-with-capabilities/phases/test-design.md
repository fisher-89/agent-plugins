# 测试设计: enrich-proposal-with-capabilities

> **变更**: enrich-proposal-with-capabilities
> **日期**: 2026-05-20
> **基于**: proposal.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | `openspec-cli.sh` 新增的 `openspec_spec_list()` 函数：正常调用返回 JSON 数组、CLI 不可用时返回 `[]`、输出格式校验 | bats (Bash Automated Testing System) v6+ | 覆盖 `openspec_spec_list()` 的所有逻辑分支，确保 CLI 调用成功/失败场景下的输出正确性 |
| 集成测试 | 修改后的五个文件之间的交互：template 不再被注入到 Planner prompt、Planner 调用 spec list 并区分 New/Modified、Evaluator 检查 Capabilities 章节、CLI 故障时 fallback 行为 | bats（Bash Automated Testing System）v6+ | 验证每个变更点与其上下游组件之间的协作正确，包括 prompt 构造、agent 步骤执行、checklist 检查 |
| 端到端测试 | 完整的 phase-requirements 流程：从技能触发到 proposal.md 产出（含 Capabilities 章节）再到 evaluator 检查通过写入 eval.json 的全程 | bats（Bash Automated Testing System）v6+ | 验证完整 P→E 工作流在引入 Capabilities 章节后仍然正确运行，最终 eval.json verdict 为 pass |

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_proposal_template_structure.sh` | 集成测试 | 正向功能 — 模板包含 New Capabilities 和 Modified Capabilities 子章节 |
| AC-2 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_prompt_no_template_injection.sh` | 集成测试 | 正向功能 — SKILL.md 不再从 CLI 注入 template 字段 |
| AC-2 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_openspec_spec_list.bats` | 单元测试 | 回归 — 移除 template 注入后现有 CLI 指令注入逻辑不受影响 |
| AC-3 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_openspec_spec_list.bats` | 单元测试 | 正向功能 — `openspec_spec_list()` 正常返回 JSON 数组 |
| AC-3 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_openspec_spec_list.bats` | 单元测试 | 异常处理 — CLI 不可用时返回 `[]` |
| AC-3 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_openspec_spec_list.bats` | 单元测试 | 异常处理 — 输出格式非 JSON 数组时返回 `[]` 并记录警告 |
| AC-4 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_planner_capabilities_flow.sh` | 集成测试 | 正向功能 — Planner 调用 `openspec_spec_list()` 并区分 New/Modified |
| AC-4 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_planner_capabilities_flow.sh` | 集成测试 | 正向功能 — 生成的 proposal.md 包含 Capabilities 章节且分类正确 |
| AC-5 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_evaluator_capabilities_check.sh` | 集成测试 | 正向功能 — Evaluator checklist 包含 Capabilities 章节检查项（标记为必须） |
| AC-5 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_evaluator_capabilities_check.sh` | 集成测试 | 异常处理 — 有 Capabilities 章节的 proposal 检查通过 |
| AC-5 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_evaluator_capabilities_check.sh` | 集成测试 | 异常处理 — 缺失 Capabilities 章节的 proposal 检查失败 |
| AC-6 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_cli_fail_fallback.sh` | 集成测试 | 异常处理 — CLI `spec list` 失败时 agent 仍正常生成 proposal，所有条目标记为 New |
| AC-6 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_cli_fail_fallback.sh` | 集成测试 | 异常处理 — `openspec_spec_list()` 输出格式异常时 fallback 到空列表 |
| AC-7 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_e2e_full_flow.sh` | 端到端测试 | 回归 — 完整的 P→E 流程在变更后正确运行，eval.json verdict 为 pass |
| AC-7 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_e2e_full_flow.sh` | 端到端测试 | 回归 — evajson 格式与现有 schema 一致 |
| AC-7 | `openspec/changes/enrich-proposal-with-capabilities/tests/test_e2e_full_flow.sh` | 端到端测试 | 正向功能 — 生成的 proposal.md 包含 Capabilities 章节并通过 evaluator |

---

## 3. 测试策略

### 3.1 方法

采用"分层验证 + 回归优先"策略。本变更涉及五个文件的修改（template、SKILL.md、CLI wrapper、planner agent、evaluator agent），测试需要覆盖每个文件变更的正确性及它们之间的交互。

1. **封装 CLI 交互层**：新增的 `openspec_spec_list()` 函数在 `openspec-cli.sh` 中实现，对包装器编写 bats 单元测试（沿用现有模式）
2. **以回归测试为锚点**：AC-2（移除 template 注入）、AC-7（P→E 循环完整性）是 P0 回归保护，确保现有功能不受影响
3. **手动验证脚本 + JSON 断言**：集成测试和 E2E 测试通过 Shell 脚本模拟技能执行路径，验证副作用（文件内容、JSON 写入、prompt 字符串等）
4. **Fixture 驱动**：使用预置的 spec 目录 fixture 和 proposal fixture 作为测试输入，确保测试可重现
5. **模板结构静态验证**：通过 grep/字符串匹配验证模板文件包含预期的 Capabilities 子章节标题

### 3.2 测试分类

- **单元测试** (`test_openspec_spec_list.bats`): 覆盖 `openspec-cli.sh` 中新增的 `openspec_spec_list()` 函数。包括：
  - `openspec spec list --json` 正常调用返回格式正确的 JSON 数组
  - CLI 命令不存在（`command not found`）时返回 `[]`
  - CLI 返回非零退出码时返回 `[]`
  - CLI 输出不是合法 JSON 数组时返回 `[]` 并记录 stderr 警告
  - 有 spec 目录时列表中包含已知 capability ID
  - 无 spec 目录时列表为空数组

- **集成测试** (`.sh` 脚本):
  - `test_proposal_template_structure.sh` — 验证 `proposal.md.template` 文件包含 `New Capabilities` 和 `Modified Capabilities` 章节标题，以及对应的模板引导注释
  - `test_prompt_no_template_injection.sh` — 模拟 SKILL.md 中 Step 3a 的 prompt 构造，确认 `template` 字段不再被注入到 Planner prompt 字符串中，`context` 和 `rules` 字段仍然保留
  - `test_planner_capabilities_flow.sh` — 验证 Planner agent 定义的 Process 步骤中增加了 spec list 调用和新旧区分逻辑；模拟执行流程验证 Capabilities 章节输出
  - `test_evaluator_capabilities_check.sh` — 验证 Evaluator checklist 新增 Capabilities 章节检查项（R8/R9），分别用包含 Capabilities 和缺少 Capabilities 的 proposal 作为输入检查 verdict
  - `test_cli_fail_fallback.sh` — 模拟 `openspec` CLI 不可用或 `spec list --json` 失败，验证 agent 仍能正常生成 proposal，Capabilities 章节存在且所有条目标记为 New

- **端到端测试** (`.sh` 脚本):
  - `test_e2e_full_flow.sh` — 模拟完整的 phase-requirements 流程：从技能触发、Planner 编写 proposal（含 Capabilities 章节）、Evaluator 检查、到 eval.json 写入的全程；验证最终 verdict 为 pass

### 3.3 模拟策略

| 模拟对象 | 方法 | 适用范围 |
|---------|------|---------|
| `openspec` CLI 命令（含 `spec list`） | 在 bats 测试中使用 `mock_openspec()` 函数覆盖 PATH，模拟 `openspec spec list --json` 的正常输出和错误输出 | 单元测试 |
| `openspec spec list --json` 返回值 | 使用预制的 JSON fixture 文件模拟不同场景（有 spec、无 spec、输出损坏、CLI 故障） | 集成测试 |
| 已有 spec 目录 | 在 sandbox 中创建 `openspec/specs/<capability>/spec.md` 目录结构，模拟全局已存在的 capability | 集成测试 |
| Planner agent 输出 | 使用预制的 `proposal.md` fixture（含 Capabilities 章节和不含 Capabilities 章节）作为 Evaluator 测试的输入 | 集成测试 |
| Evaluator checklist | 通过 Python 脚本模拟静态 checklist 的执行，对给定的 proposal 逐项检查并输出 verdict | 集成测试 |
| 文件系统 | 使用 `mktemp` 创建的临时目录作为测试沙箱，确保不污染真实变更目录 | 所有测试 |

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| Capabilities 章节包含空子章节（无 New 也无 Modified） | proposal.md 中存在 `## New Capabilities` 和 `## Modified Capabilities` 标题但下方无列表项 | Evaluator 的 Capabilities 检查项（"至少有一个 New 或 Modified 条目"）判定为 fail，给出具体证据提示"无能力和列表为空" | `test_evaluator_capabilities_check.sh` |
| `openspec spec list --json` 返回非数组的 JSON 对象 | CLI 输出 `{"error":"internal error","code":500}` | `openspec_spec_list()` 检测到输出不是 JSON 数组，返回 `[]` 并输出 stderr 警告信息 | `test_openspec_spec_list.bats` |
| `openspec spec list --json` 返回包含未知字段的有效数组 | CLI 输出 `[{"id":"auth","name":"Auth","extra":"ignored"}]` | 函数仍返回有效数组 `["auth"]`，提取 `id` 字段或整个条目作为 capability 标识 | `test_openspec_spec_list.bats` |
| `openspec` CLI 完全不可用（command not found） | `openspec_spec_list()` 中执行 `openspec spec list --json` 返回 127 | 函数捕获错误，返回 `[]`，不抛出异常 | `test_openspec_spec_list.bats` |
| `openspec` CLI 可用但 `spec` 子命令不存在（旧版本 CLI） | CLI 版本不支持 `spec list` 子命令，返回非零退出码 | `openspec_spec_list()` 返回 `[]`，因为输出为空或非 JSON | `test_openspec_spec_list.bats` |
| 多个 capabilities 混合 New 和 Modified | 已有 spec 目录包含 "auth"、"storage"，planner 新增 "logging"、"messaging" | Capabilities 章节中 "auth" 和 "storage" 在 Modified 子章节，"logging" 和 "messaging" 在 New 子章节 | `test_planner_capabilities_flow.sh` |
| 没有任何全局 spec 存在 | `openspec/specs/` 目录不存在或为空 | `openspec_spec_list()` 返回 `[]`，所有 capabilities 标记为 New | `test_planner_capabilities_flow.sh` |
| SKILL.md 中 `INSTRUCTIONS_JSON` 的 `template` 字段非空但不被注入 | CLI 返回的 instructions JSON 包含 `"template":"..."`，但 prompt 构造代码不再引用它 | prompt 字符串中不出现该 template 字段的内容；`rules` 和 `context` 字段仍正常注入 | `test_prompt_no_template_injection.sh` |
| `INSTRUCTIONS_JSON` 为 `{}`（CLI 未提供 instructions） | Step 3a 中 status 和 instructions 均返回 `{}` | prompt 回退到基本用法（仅引用静态模板），不包含 CLI Instructions 章节 | `test_prompt_no_template_injection.sh` |
| 已有 proposal（无 Capabilities 章节）在升级后首次被 Evaluator 检查 | evaluator 读取包含 Capabilities 模板但 proposal 无该章节 | Evaluator 的 R8 检查项 fail，整体 verdict 为 fail，给出证据提示"缺少 Capabilities 章节" | `test_evaluator_capabilities_check.sh` |
| E2E 流程中 Planner 只输出 New Capabilities 场景 | 执行 phase-requirements 时全局无已有 spec，Planner 仅输出 New 条目 | Evaluator 检查通过（至少有一个 New 条目），eval.json verdict 为 pass | `test_e2e_full_flow.sh` |
| E2E 流程中 Planner 输出空 Capabilities 章节 | Planner 生成的 proposal 包含 Capabilities 标题但无任何条目 | Evaluator 的 R8 检查项判定 fail，触发 P→E 循环重试 | `test_e2e_full_flow.sh` |

---

## 5. 测试数据

- **Fixture 目录**: `tests/fixtures/`
  - `fixtures/specs/` — 模拟 `openspec/specs/` 目录结构，包含若干已有 spec：
    - `fixtures/specs/auth/spec.md` — 已存在的 capability，用于 Modified 场景验证
    - `fixtures/specs/storage/spec.md` — 已存在的 capability，用于 Modified 场景验证
  - `fixtures/specs_empty/` — 空的 spec 目录，模拟无全局 capability 的场景
  - `fixtures/openspec_spec_list_output.json` — 模拟 `openspec spec list --json` 的标准输出：
    ```json
    ["auth", "storage"]
    ```
  - `fixtures/openspec_spec_list_empty.json` — 模拟空列表输出：
    ```json
    []
    ```
  - `fixtures/openspec_spec_list_invalid.json` — 模拟非法输出（非数组）：
    ```json
    {"error": "internal error"}
    ```
  - `fixtures/proposal_with_capabilities.md` — 包含 Capabilities 章节的完整 proposal fixture，用于 Evaluator 正向测试
  - `fixtures/proposal_without_capabilities.md` — 无 Capabilities 章节的 proposal fixture，用于 Evaluator 异常测试
  - `fixtures/proposal_empty_capabilities.md` — Capabilities 章节为空的 proposal fixture，用于 Evaluator 边界测试
  - `fixtures/openspec_instructions.json` — 包含 `rules`、`context`、`template` 字段的 instructions fixture，用于验证 template 字段不再被注入
  - `fixtures/openspec_status.json` — 预录制的 `openspec status --json` 输出

- **共享辅助库**: `tests/helpers/setup_test_env.sh`
  - 基于 `openspec/changes/archive/2026-05-19-enhance-phase-requirements-skill/tests/helpers/setup_test_env.sh` 扩展
  - 新增 `mock_spec_list()` — 模拟 `openspec spec list --json` 命令，支持正常、空、错误三种模式
  - 新增 `mock_specs_dir()` — 在 sandbox 中创建模拟的 `openspec/specs/` 目录
  - 新增 `assert_template_section()` — 断言文件中存在指定 Markdown 章节标题
  - 新增 `assert_capability_classified()` — 断言 Capabilities 章节中 capability 被正确分类为 New 或 Modified
  - 保留 `setup_sandbox()`、`teardown_sandbox()`、`mock_openspec()`、`assert_dir_exists()`、`assert_file_exists()`、`assert_json_field()`、`assert_string_contains()`、`assert_string_not_contains()`

- **测试报告输出**: `tests/reports/`
  - `reports/unit-test.xml` — bats 运行的 JUnit 格式报告
  - `reports/integration-results.json` — 集成测试结果摘要
  - `reports/e2e-results.json` — 端到端测试结果摘要

---

## 6. 不可测试项

- `requirements-planner` agent 实际对 capability ID 的语义分类质量（如将实质上属于 Modified 的 capability 误判为 New） — **原因**: Planner agent 是外部 AI 模型，其分类质量依赖于模型能力和提示词设计。仅验证 agent 定义中是否包含 spec list 调用逻辑和分类指令，以及生成的 Capabilities 章节在结构上是否正确区分了 New/Modified 子章节
- `requirements-evaluator` agent 对 Capabilities 章节内容的实质性判断质量（如评估 capability 描述是否充分） — **原因**: Evaluator 同样是 AI agent，其判断质量超出自动化测试范围。仅验证 checklist 中是否包含 Capabilities 检查项，以及当 proposal 明确缺失该章节时是否判定为 fail
- `openspec` CLI 的 `spec list --json` 命令在不同操作系统上的行为差异 — **原因**: 需要跨平台安装测试环境。通过 `openspec_spec_list()` 中的输出格式校验（是否符合 JSON 数组）来统一处理，并在 bats 测试中模拟各种输出格式
- Skill 执行过程中 Claude Code harness 的 Agent() 调用行为 — **原因**: `Agent()` 调用由 Claude Code 运行时执行，测试环境无法拦截和断言。集成测试通过验证技能步骤的输入构建（CLI 调用参数、prompt 字符串内容）来间接验证
- 模板文件渲染后的视觉格式（排版、缩进等） — **原因**: 模板的视觉格式不影响功能，仅做静态结构检查（章节标题存在）。渲染后格式依赖于实际 agent 输出
