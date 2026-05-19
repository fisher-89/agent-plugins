# 测试设计: enhance-phase-requirements-skill

> **变更**: enhance-phase-requirements-skill
> **日期**: 2026-05-19
> **基于**: proposal.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | `.openspec-cli.sh` 工具函数：change 名称校验、kebab-case 推导、JSON 解析辅助、目录存在性检测 | bats (Bash Automated Testing System) v6+ | 覆盖所有 CLI 包装器函数的逻辑分支，确保参数校验、返回值解析和错误处理正确 |
| 集成测试 | 修改后的 SKILL.md Step 1 和 Step 3a 各执行路径：变更不存在、变更已存在、探索上下文传递、AskUserQuestion 兜底、backtrack 检测、P→E 循环 | 手动验证脚本（Shell + 断言，位于 `tests/` 目录） | 验证技能执行的每一个关键路径产生预期行为，包括 CLI 交互的副作用验证 |
| 端到端测试 | 完整的 phase-requirements 技能执行：从技能触发到 proposal.md 产出再到 eval.json 写入的全程 | 手动 E2E 验证脚本（Shell + JSON 断言，位于 `tests/` 目录） | 验证完整的 P→E 工作流在引入变更后仍然正确运行，输出路径和 eval.json 格式与当前一致 |

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-01 | `openspec/changes/enhance-phase-requirements-skill/tests/test_change_scaffolding.sh` | 集成测试 | 正向功能 — 变更不存在时自动脚手架 |
| AC-02 | `openspec/changes/enhance-phase-requirements-skill/tests/test_change_scaffolding.sh` | 集成测试 | 回归 — 变更已存在时不触发脚手架 |
| AC-03 | `openspec/changes/enhance-phase-requirements-skill/tests/test_planner_prompt.sh` | 集成测试 | 正向功能 — Planner 提示词包含动态 CLI 指令 |
| AC-04 | `openspec/changes/enhance-phase-requirements-skill/tests/test_explore_context.sh` | 集成测试 | 正向功能 — 探索上下文传递给 Planner |
| AC-05 | `openspec/changes/enhance-phase-requirements-skill/tests/test_ask_user_fallback.sh` | 集成测试 | 正向功能 — 无参数时触发 AskUserQuestion |
| AC-06 | `openspec/changes/enhance-phase-requirements-skill/tests/test_ask_user_fallback.sh` | 集成测试 | 正向功能 — 模糊描述推导名称并确认 |
| AC-07 | `openspec/changes/enhance-phase-requirements-skill/tests/test_pe_loop_preservation.sh` | 端到端测试 | 回归 — P→E 循环结构不变 |
| AC-08 | `openspec/changes/enhance-phase-requirements-skill/tests/test_backtrack.sh` | 集成测试 | 回归 — backtrack 检测逻辑不变 |
| AC-09 | `openspec/changes/enhance-phase-requirements-skill/tests/test_output_path.sh` | 集成测试 | 回归 — 输出路径不变 |

---

## 3. 测试策略

### 3.1 方法

采用"分层验证 + 回归优先"策略。由于 `phase-requirements` 是一个 Claude Code 技能（SKILL.md 定义的行为流程而非传统可执行代码），测试主要依赖：

1. **封装 CLI 交互层**：将 `openspec new change`, `openspec status`, `openspec instructions` 等 CLI 调用封装在 `.openspec-cli.sh` 包装器中，对包装器编写 bats 单元测试
2. **以回归测试为锚点**：AC-07、AC-08、AC-09 是 P0 回归保护，确保现有 P→E 循环不受新增三层能力影响
3. **手动验证脚本 + JSON 断言**：集成测试和 E2E 测试通过 Shell 脚本模拟技能执行路径，验证副作用（目录创建、JSON 写入、名称推导等）
4. **Fixture 驱动**：使用预置的变更目录和 eval.json fixture 作为测试输入，确保测试可重现

### 3.2 测试分类

- **单元测试** (`test_cli_wrapper.bats`): 覆盖 `.openspec-cli.sh` 中 `openspec_new_change()`, `openspec_status_json()`, `openspec_instructions()`, `derive_kebab_case()`, `change_exists()`, `validate_change_name()` 等函数。包括：
  - 正常输入输出路径
  - CLI 返回非零退出码的处理
  - CLI 输出格式变化（JSON 解析容错）
  - 名称推导：中文短语到 kebab-case 的转换
  - 名称校验：空字符串、特殊字符、长度限制

- **集成测试** (`.sh` 脚本):
  - `test_change_scaffolding.sh` — 模拟 Step 1 的变更检测+脚手架流程
  - `test_planner_prompt.sh` — 验证 Step 3a 调用 Planner 时的提示词构建
  - `test_explore_context.sh` — 验证探索上下文摘要的提取和注入
  - `test_ask_user_fallback.sh` — 验证 AskUserQuestion 触发和名称确认流程
  - `test_backtrack.sh` — 验证 Step 2 backtrack marker 检测
  - `test_output_path.sh` — 验证 artifact 输出位置

- **端到端测试** (.sh 脚本):
  - `test_pe_loop_preservation.sh` — 执行完整 P→E 循环（Planner→Evaluator→检查 verdict→循环），验证 eval.json 格式与 schema 一致
  - `test_e2e_full_flow.sh` — 从空状态开始，执行完整 skill 流程，验证端到端成功

### 3.3 模拟策略

| 模拟对象 | 方法 | 适用范围 |
|---------|------|---------|
| `openspec` CLI 命令 | 在 bats 测试中 mock CLI 命令，通过 `mock_openspec()` 函数覆盖 PATH 或通过 shunit2 的 `stub` 机制 | 单元测试 |
| OpenSpec CLI 输出 | 使用预录制的 `openspec status --json` 和 `openspec instructions` 输出 fixture 文件 | 集成测试 |
| 探索会话上下文 | 使用 `fixtures/explore_output.md` 模拟来自 `openspec-explore` 的对话摘要 | 集成测试 |
| AskUserQuestion | 通过环境变量 `MOCK_ASK_USER=true` 和输入文件模拟用户交互（确认/拒绝） | 集成测试 |
| 文件系统 | 使用 `mktemp` 创建的临时目录作为测试沙箱，确保不污染真实变更目录 | 所有测试 |
| `requirements-planner` agent | 不模拟 — E2E 和集成测试中 Agent 调用由手动验证替代，测试仅验证技能步骤的输入构建是否正确 | 集成测试 |

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| 变更名包含非法字符 | change name = "My Feature & Bug Fix!" | 推导为 `my-feature-bug-fix`，忽略非法字符，在创建前提示 "I'll create a change named 'my-feature-bug-fix'. Proceed?" 并等待确认 | `test_cli_wrapper.bats` |
| 变更名仅为中文无英文 | 用户回答 "登录功能" | `derive_kebab_case("登录功能")` 返回 `login`（或 `deng-lu-gong-neng`），向用户展示并确认 | `test_cli_wrapper.bats` |
| 变更名长度超过 128 字符 | 用户回答一段长描述 | 截断至 128 字符，保留尾部唯一性 hash，在确认时展示完整 kebab-case | `test_cli_wrapper.bats` |
| 变更名已存在（脚手架阶段冲突） | `openspec new change <name>` 返回 "already exists" 错误 | 检测到已存在错误，跳过脚手架直接进入 Step 2；将错误消息记录到调试日志但不中断流程 | `test_change_scaffolding.sh` |
| `openspec status --json` 返回空 JSON | CLI 输出为 `{}` | 技能继续执行，Planner 提示词中省略 `openspec instructions` 部分，回退到仅使用静态模板 | `test_planner_prompt.sh` |
| `openspec instructions` 输出不包含 `rules` 字段 | CLI 输出的 JSON 缺少顶层 key | 技能容忍缺失字段，仅将存在的字段注入 Planner 提示词；对缺失字段进行 log 记录 | `test_planner_prompt.sh` |
| 探索上下文内容为空 | 来自 `openspec-explore` 的摘要为空字符串 | 不注入探索上下文模块，Planner 提示词中省略探索相关段落，行为等同于无探索上下文 | `test_explore_context.sh` |
| 探索上下文过大（超过 20KB） | 探索会话包含大量历史对话 | 摘要截断至 10KB，在摘要末尾添加 "（以下内容已截断，共 N 行）" 标记 | `test_explore_context.sh` |
| AskUserQuestion 被用户拒绝 | 用户回答 "不，取消" | 技能终止，输出友好提示 "已取消提案编写，你可以稍后通过 /dev-team:phase-requirements <name> 重新开始" | `test_ask_user_fallback.sh` |
| AskUserQuestion 输入为空字符串 | 用户直接回车不输入任何内容 | 技能再次询问 "请描述你想要实现的变更，或者输入 'cancel' 取消"；连续 3 次空输入后自动终止 | `test_ask_user_fallback.sh` |
| askUser 后用户输入与建议名称冲突 | 用户确认名称后，脚手架步骤发现名称已被占用 | 技能检测到 "already exists" 错误，自动追加序号（如 `add-login-2`）并重新请求确认 | `test_ask_user_fallback.sh` |
| Backtrack marker 存在但 eval.json 格式损坏 | eval.json 不是合法 JSON 数组 | 技能检测到解析错误，跳过 backtrack 检测，回退到标准 P→E 循环（Step 3a），同时输出警告日志 | `test_backtrack.sh` |
| eval.json 中 backtrack_to 为 null | 存在 eval.json 但所有 `backtrack_to` 字段均为 null | 正常进入 Step 3a（不触发 re-evaluation），行为与无 eval.json 一致 | `test_backtrack.sh` |
| 技能在 Step 1 中多次调用 `openspec status` | 同一技能执行中 Step 1 后 Step 3a 再次调用 status | 第二次调用使用前次缓存结果（LRU 缓存，基于 `openspec/changes/<name>/` 路径的 mtime 判断是否失效） | `test_planner_prompt.sh` |

---

## 5. 测试数据

- **Fixture 目录**: `tests/fixtures/`
  - `fixtures/existing_change/` — 预置的已有变更目录，包含 `.openspec.yaml` 和空的 `phases/` 子目录，用于 AC-02 验证
  - `fixtures/existing_change/phases/eval.json` — 包含一组标准 eval 条目（pass + fail 组合）的 eval.json fixture，用于 AC-07/AC-08 验证
  - `fixtures/backtrack_change/phases/eval.json` — 包含 `backtrack_to: "01-requirements"` 条目的 eval.json fixture，用于 AC-08 backtrack 场景验证
  - `fixtures/explore_output.md` — 模拟的 openspec-explore 会话输出 Markdown，包含关键决策和设计选择，用于 AC-04 验证
  - `fixtures/openspec_status.json` — 预录制的 `openspec status --json` 输出，包含正确的 change-context 和 artifacts 依赖信息
  - `fixtures/openspec_instructions.json` — 预录制的 `openspec instructions` 输出，包含 rules、context 和 template 字段
  - `fixtures/openspec_status_empty.json` — 空状态 CLI 输出，用于边界场景测试

- **共享辅助库**: `tests/helpers/setup_test_env.sh` — 提供 `setup_sandbox()`, `teardown_sandbox()`, `mock_openspec()`, `assert_dir_exists()`, `assert_json_field()` 等函数

---

## 6. 不可测试项

- `requirements-planner` agent 实际生成的 proposal.md 内容质量 — **原因**: Planner agent 是外部 AI 模型，其输出质量依赖于模型能力和提示词设计，无法通过自动化测试断言。仅验证提示词中是否包含预期的上下文信息（AC-03、AC-04），不验证最终撰写质量
- `requirements-evaluator` 的 checklist 准确性 — **原因**: Evaluator 同样是 AI agent，其 checklist 和执行逻辑不在本变更范围内。本变更仅确保 P→E 循环结构不变（AC-07），不验证 Evaluator 内部逻辑
- Skill 执行过程中 Claude Code harness 的 Agent 调用行为 — **原因**: `Agent()` 调用由 Claude Code 运行时执行，测试环境无法拦截和断言。集成测试通过验证技能步骤的输入构建（CLI 调用参数、提示词字符串）来间接验证
- `openspec` CLI 的跨版本兼容性 — **原因**: 需要真实安装不同版本的 `openspec` CLI 才能测试，CI 环境难以覆盖。通过在 `.openspec-cli.sh` 中统一处理版本差异，并在项目 CI 中针对当前安装版本做回归测试来缓解
