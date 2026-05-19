# 任务: enhance-phase-requirements-skill

> **变更**: enhance-phase-requirements-skill
> **日期**: 2026-05-19
> **基于**: design.md, test-design.md

---

## 第一阶段: CLI 包装器 (openspec-cli.sh)

- [x] 1.1 在 `plugins/dev-team/utils/` 下创建 `openspec-cli.sh`，定义 shell 函数库框架（头部注释、set -e 保护、函数声明模板）
- [x] 1.2 实现 `change_exists()` 函数：检测 `openspec/changes/<name>/` 目录是否存在，返回退出码 0/1
- [x] 1.3 实现 `validate_change_name()` 函数：校验名称非空、长度 <= 128 字符、匹配 kebab-case 正则 `^[a-z0-9]+(-[a-z0-9]+)*$`
- [x] 1.4 实现 `derive_kebab_case()` 函数：将用户描述转换为 kebab-case（小写、去除非字母数字字符、连字符替换、折叠连字符、截断至 128 字符）
- [x] 1.5 实现 `openspec_new_change()` 函数：封装 `openspec new change "<name>"` 调用，捕获退出码和 stderr，返回结构化结果
- [x] 1.6 实现 `openspec_status_json()` 函数：封装 `openspec status --json` 调用，处理空 JSON `{}` 和解析失败的回退
- [x] 1.7 实现 `openspec_instructions()` 函数：封装 `openspec instructions` 调用，容忍缺失顶层字段（rules、context、template），仅返回存在的字段
- [x] 1.8 实现 `openspec_cli_cache()` 函数：基于 `openspec/changes/<name>/` 目录 mtime 的 LRU 缓存，避免单次技能执行中重复 CLI 调用

## 第二阶段: SKILL.md Step 1 — 变更检测增强

- [x] 2.1 修改 Step 1 头部逻辑：解析用户传入的 change-name 参数，区分"有参数"和"无参数"两条路径
- [x] 2.2 实现"有参数 → 检测存在性"分支：调用 `change_exists()`，若存在则直接进入 Step 2，若不存在则调用 `openspec_new_change()` 脚手架后进入 Step 2
- [x] 2.3 实现"无参数 → 探索上下文检测"分支：检查对话历史中是否存在 openspec-explore 会话特征（"What We Figured Out" 摘要、设计决策、ASCII 图等标记）
- [x] 2.4 实现"探索上下文 → 提取摘要"逻辑：指示 Claude 从对话中提取结构化洞察（关键决策、设计选择、排除方案、开放问题），形成探索洞察摘要
- [x] 2.5 实现"有探索上下文 → 询问变更名"流程：使用 AskUserQuestion 询问用户变更名，调用 `derive_kebab_case()` 推导 kebab-case 名称
- [x] 2.6 实现"无参数 + 无探索上下文 → AskUserQuestion 兜底"：使用 AskUserQuestion（无预设选项）询问"想构建什么变更"，推导 kebab-case 名称
- [x] 2.7 实现名称确认流程：向用户展示建议的 kebab-case 名称，请求确认（"I'll create a change named '<name>'. Proceed?"），支持用户输入自定义名称或取消
- [x] 2.8 实现用户取消处理：当用户拒绝确认时，输出友好终止消息"已取消提案编写，你可以稍后通过 /dev-team:phase-requirements <name> 重新开始"
- [x] 2.9 实现脚手架冲突处理：当 `openspec_new_change()` 返回"already exists"错误时，跳过脚手架，自动追加序号（如 `add-login-2`）并重新请求确认

## 第三阶段: SKILL.md Step 3a — Planner 提示词富化

- [x] 3.1 在 Step 3a 调用 Agent 之前加入 CLI 指令收集步骤：依次调用 `openspec_status_json()` 和 `openspec_instructions()`，获取动态上下文
- [x] 3.2 构建富化 Planner 提示词框架：以静态模板路径为基础，追加 `openspec instructions` 的 `rules`、`context`、`template` 字段内容
- [x] 3.3 在 Planner 提示词中加入探索上下文段（如有）：在提示词末端追加"## 探索上下文（仅供参考）"章节，包含从 Step 1 提取的结构化洞察摘要
- [x] 3.4 实现 CLI 输出回退逻辑：当 `openspec status --json` 返回空 JSON `{}` 或解析失败时，Planner 提示词中省略 `openspec instructions` 部分，仅使用静态模板
- [x] 3.5 实现长探索上下文的截断：当探索上下文摘要超过 10KB 时截断，在末尾添加"（以下内容已截断，共 N 行）"标记
- [x] 3.6 在 Step 3a 注释中明确 CRITICAL 提示：探索上下文仅供参考，以 CLI 指令和静态模板为准，避免探索信息误导 Planner

## 第四阶段: 回归保护（保持不变的部分验证）

- [x] 4.1 验证 Step 2 backtrack 检测代码未被修改：确认 `backtrack_to: "01-requirements"` 的处理逻辑与当前行为一致
- [x] 4.2 验证 Step 3b (Evaluator 调用) 代码未被修改：Agent 调用参数格式与当前一致
- [x] 4.3 验证 Step 3c (verdict 检查 + 循环逻辑) 代码未被修改：eval.json 读取位置、verdict 判断、循环条件与当前一致
- [x] 4.4 验证 Step 4 (结果报告) 代码未被修改：输出格式和内容与当前一致
- [x] 4.5 验证输出路径未改变：所有 artifact 仍写入 `openspec/changes/<name>/phases/proposal.md`

## 第五阶段: 测试基础设施

- [x] 5.1 创建测试目录结构：`tests/helpers/`、`tests/fixtures/`、`tests/openspec-cli/`
- [x] 5.2 创建测试辅助库 `tests/helpers/setup_test_env.sh`：提供 `setup_sandbox()`、`teardown_sandbox()`、`mock_openspec()`、`assert_dir_exists()`、`assert_json_field()` 等公共函数
- [x] 5.3 录制 CLI fixture 数据：从当前环境运行 `openspec status --json` 和 `openspec instructions`，保存输出到 `tests/fixtures/openspec_status.json` 和 `tests/fixtures/openspec_instructions.json`
- [x] 5.4 创建空状态 fixture `tests/fixtures/openspec_status_empty.json`：内容为 `{}`
- [x] 5.5 创建已有变更 fixture 目录 `tests/fixtures/existing_change/`：包含 `.openspec.yaml` 和空的 `phases/` 子目录
- [x] 5.6 创建 eval.json fixtures：`tests/fixtures/existing_change/phases/eval.json`（标准 pass/fail 条目）和 `tests/fixtures/backtrack_change/phases/eval.json`（含 backtrack_to 标记）
- [x] 5.7 创建探索会话输出 fixture `tests/fixtures/explore_output.md`：模拟包含关键决策、设计选择、ASCII 图的探索会话 Markdown
- [x] 5.8 编写 bats 单元测试 `tests/openspec-cli/test_cli_wrapper.bats`：覆盖 `derive_kebab_case`、`validate_change_name`、`change_exists` 的所有逻辑分支（正常输入、空输入、中文输入、过长输入、特殊字符、区分大小写）

## 第六阶段: 集成测试脚本

- [x] 6.1 编写 `tests/test_change_scaffolding.sh`：验证 AC-01（变更不存在时脚手架）和 AC-02（变更已存在时不触发脚手架）
- [x] 6.2 编写 `tests/test_planner_prompt.sh`：验证 AC-03（Planner 提示词包含 `openspec instructions` 动态输出）
- [x] 6.3 编写 `tests/test_explore_context.sh`：验证 AC-04（探索上下文摘要注入 Planner 提示词）
- [x] 6.4 编写 `tests/test_ask_user_fallback.sh`：验证 AC-05（AskUserQuestion 触发）和 AC-06（模糊描述推导名称并确认）
- [x] 6.5 编写 `tests/test_backtrack.sh`：验证 AC-08（backtrack marker 检测逻辑不变）
- [x] 6.6 编写 `tests/test_output_path.sh`：验证 AC-09（输出路径不变）

## 第七阶段: 端到端测试脚本

- [x] 7.1 编写 `tests/test_pe_loop_preservation.sh`：验证 AC-07（完整 P→E 循环执行后 eval.json 格式与 schema 一致）
- [x] 7.2 编写 `tests/test_e2e_full_flow.sh`：从空状态开始模拟完整 skill 执行流程，验证端到端成功

## 第八阶段: 验证与文档

- [x] 8.1 在真实环境中运行 `openspec-cli.sh` 的 bats 单元测试，修复所有失败用例
- [x] 8.2 手动执行集成测试脚本，验证所有 AC（AC-01 到 AC-09）通过
- [x] 8.3 对已有变更运行完整的 phase-requirements 流程，验证 P→E 循环不受影响（回归验证）
- [x] 8.4 最终审阅 SKILL.md 完整文件，确认所有新增指令格式一致、注释清晰、无死代码
