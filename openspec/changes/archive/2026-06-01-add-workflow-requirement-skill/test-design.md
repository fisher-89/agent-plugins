# 测试设计: add-workflow-requirement-skill

> **变更**: add-workflow-requirement-skill
> **日期**: 2026-06-01
> **基于**: proposal.md, design.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | eval-next.ts 服务端编排逻辑（gate check、skip、retry、backtrack、round limit）；workflow.ts phase 数组与辅助函数；eval.json schema 验证 | vite-plus (vitest compatible) | 核心编排逻辑全分支覆盖 >= 90%；确保所有 phase 状态转换（first_run / retry / passed / backtrack / done / error）被完整测试 |
| 集成测试 | 临时测试脚本验证 phase-proposal skill 和 workflow-requirement skill 的完整 P→E 循环和全流程编排；验证 proposal-planner / proposal-evaluator agent 的调用链路 | Bash scripts + Claude Code 手动执行 | 验证实际文件写入、MCP 调用、PushNotification 通知等跨组件交互在开发环境中正常工作 |

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` | 单元测试 | eval/next 返回 planner + evaluator 的完整 phase 执行路径 |
| AC-1 | `openspec/changes/add-workflow-requirement-skill/tests/test-phase-proposal.sh` | 集成测试 | 手动执行 `/dev-team:phase-proposal <test-change>` 验证 P→E 循环至 verdict=pass |
| AC-2 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` | 单元测试 | eval/next 返回的 planner prompt 包含指定 change name；planner agent_type 为 proposal-planner |
| AC-2 | `openspec/changes/add-workflow-requirement-skill/tests/test-planner-output.sh` | 集成测试 | 检查 openspec/changes/\<name\>/ 下 proposal.md + specs/ 所有文件已写入且结构正确 |
| AC-3 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` | 单元测试 | eval/next 返回的 evaluator agent_type 为 proposal-evaluator（非 requirements-evaluator）；checklist 引用匹配 |
| AC-4 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` | 单元测试 | eval/next 从 phase 01-proposal 到 09-acceptance 的完整遍历；done=true 在全部 pass 后返回 |
| AC-4 | `openspec/changes/add-workflow-requirement-skill/tests/test-workflow-full.sh` | 集成测试 | 运行 `/dev-team:workflow-requirement` 验证 Phase 01-09 自动执行 |
| AC-5 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` | 单元测试 | eval/next 输入参数中 explore_context 被正确传递给 planner prompt（或 skill 层处理逻辑） |
| AC-5 | `openspec/changes/add-workflow-requirement-skill/tests/test-explore-context.sh` | 集成测试 | 在有 explore 上下文时检查 proposal-planner 收到的 prompt 包含 explore 摘要 |
| AC-6 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` | 单元测试 | 对已有 pass 记录的 phase，eval/next 跳过返回下一未 pass phase |
| AC-7 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` | 单元测试 | verdict=fail 且 attempt >= 5 时 eval/next 返回 max_retries_exceeded 错误 |
| AC-8 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` | 单元测试 | backtrack_to 非空时 eval/next 清空目标 phase 及后续条目，跳转到目标 phase |
| AC-9 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` | 单元测试 | 所有 phase pass 时 eval/next 返回 done=true、planner=null、evaluator=null |
| AC-9 | `openspec/changes/add-workflow-requirement-skill/tests/test-archive.sh` | 集成测试 | workflow 完成后检查不自动执行 archive，用户需手动 `/dev-team:openspec-archive-change` |
| AC-10 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` | 单元测试 | round 计数 > 20 时 eval/next 返回 round_limit_exceeded 错误 |

---

## 3. 测试策略

### 3.1 方法

采用"单元测试全覆盖核心逻辑 + 集成脚本验证端到端流程"的双层策略。eval/next 服务端编排逻辑是整个变更的核心（gate check、skip、retry、backtrack、round limit 均由服务端决定），因此需要高覆盖率的单元测试。phase-proposal 和 workflow-requirement 的 SKILL.md 文件是 Claude Code 技能定义（disable-model-invocation: true），通过集成测试脚本手动验证。

### 3.2 测试分类

- **单元测试**: 不依赖外部服务或文件系统（文件读写由 eval-json.ts 的 readEvalJson/appendEntry 封装，在测试中通过 mock eval.json 数据模拟），测试 eval-next.ts 的独立函数和完整决策树。使用 vite-plus 框架，测试文件与源文件同目录（`*.test.ts`）。
- **集成测试**: 手动执行的 bash 脚本，调用 Claude Code 运行实际的 skill 命令，验证文件写入、MCP 调用链、PushNotification 等跨组件交互。脚本存放在 `openspec/changes/add-workflow-requirement-skill/tests/` 目录。

> **注意**: 不再使用端到端测试 (E2E)。所有单元测试均使用 mock 的 eval.json 数据，不依赖真实外部环境。

### 3.3 模拟策略

| 模拟对象 | 策略 | 适用测试 |
|----------|------|----------|
| eval.json 数据 | 直接在测试中构造 entries 数组（模拟已 pass、fail、backtrack 等状态的 eval.json 内容） | eval-next.test.ts 中的全部单元测试 |
| eval/next 输入参数 | 直接构造符合 schema 的调用参数 | eval-next.test.ts 的输入验证 |
| fs 文件操作（read/write） | 通过 eval-json.ts 的 readEvalJson / appendEntry 间接模拟；测试中不读写真实文件系统 | eval-next.test.ts |
| MCP eval/check 结果 | 不直接调用 MCP（eval-next.ts 不依赖 eval/check，eval/next 内部读取 eval.json 并自行判断） | eval-next.test.ts |
| skill 调用 | 通过集成测试脚本手动验证完整链路 | tests/*.sh |

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| 空 eval.json（首次运行 workflow） | eval.json 不存在或为空数组 | eval/next 返回 01-proposal 的 planner + evaluator，round=1 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| 所有 phase 均为 first_run | entries = [] | eval/next 返回 01-proposal 的 planner + evaluator，round=1 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| 单 phase 连续 fail 至第 5 次重试 | entries 中 01-proposal 有 5 条 fail 记录 | eval/next 返回 error: max_retries_exceeded | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| 单 phase 第 4 次 fail 后第 5 次重试 | entries 中 01-proposal 有 4 条 fail 记录 | eval/next 返回 01-proposal 的 retry（attempt 5） | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| phase 01 pass、02 pass、03 backtrack_to=01 | entries: 01 pass, 02 pass, 03 backtrack_to=01 | eval/next 清空 phase 01 及后续所有条目，返回 01-proposal 的 planner + evaluator | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| backtrack 后重新 pass 再继续 | 回溯后 01 重新 pass，02 重新 pass | eval/next 正常返回 03 的 planner + evaluator | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| backtrack 循环（02→01→02→01...）超过 20 轮 | entries 中的 backtrack 记录使 round 持续递增 | eval/next 返回 error: round_limit_exceeded | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| 刚好第 20 轮 pass（不超限） | round=20 时全部 phase pass | eval/next 返回 done=true（不触发 round limit） | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| 部分 phase 被 skipped（no-op） | phase 04 有 skipped=true 条目 | eval/next 将 skipped 条目视为 pass，跳到 05 | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| 中断恢复：evaluator 条目缺失但 planner 已执行 | entries 中只有 03 的 planner 记录，无 evaluator 记录 | eval/next 识别 evaluator 缺失，重新返回 03 的 evaluator | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| workflow_type=requirement 默认值 | 调用 eval/next 不传 workflow_type | eval/next 使用默认 requirement 的 phase 表（9 phases） | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| change name 包含特殊字符（中文字符、空格） | change = "my-test-变更" | eval/next 正常处理（eval/next 不做路径操作，仅字符串传递） | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| eval.json 中混入旧 phase ID（01-requirements） | entries 中既有 01-requirements（旧）又有 01-proposal（新） | eval/next 按字符串匹配，区分新旧 ID | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |
| 09-acceptance pass 后调用 eval/next | 全部 9 个 phase 均有 pass 记录 | eval/next 返回 done=true, planner=null, evaluator=null | `plugins/dev-team/bin/src/commands/eval-next.test.ts` |

---

## 5. 测试数据

单元测试中的 eval.json 模拟数据涵盖以下六种基本状态模式：

| 模式 | 数据特征 | 用途 |
|------|----------|------|
| 空 | `[]` | 首次运行 |
| 正常推进 | 各 phase 按顺序 pass | 验证 skip 逻辑和正常推进 |
| 部分失败 | 某 phase 连续 fail 4~5 次 | 验证 max_retries 边界 |
| 回溯 | 最新条目含 backtrack_to 非空 | 验证 backtrack 清空和跳转 |
| 混合新旧 ID | 同时包含 01-requirements 和 01-proposal 条目 | 验证向后兼容 |
| 中断恢复 | 某 phase 有 planner 记录但无 evaluator | 验证中断恢复逻辑 |

所有测试数据在测试文件中以内联数组构造，不依赖外部 JSON fixture 文件。

---

## 6. 不可测试项

- **SKILL.md 文件的 disable-model-invocation: true 行为** — 原因: 该属性由 Claude Code 运行时而非代码逻辑处理，无法通过单元测试验证。通过集成测试手动确认 skill 正确加载。
- **Agent 模型选择（proposal-planner 使用 opus）** — 原因: agent.md 中的 model 声明由 Claude Code 运行时解析，不经过 MCP 服务端代码逻辑。通过 code review 验证。
- **PushNotification 在实际环境中的送达** — 原因: PushNotification 由 Claude Code 运行时提供，MCP 服务端和 skill 仅调用该 API。无法通过本地测试验证实际推送。通过 code review 验证调用点位置。
- **explore 上下文自动提取与传递的最终质量** — 原因: 该逻辑位于 workflow-requirement SKILL.md 中 skill 层面的 prompt 组装，由 Claude 主模型的自然语言理解能力决定，无法通过确定性测试验证。通过集成测试验证提取链路完整（AC-5），其内容质量由评估阶段（proposal-evaluator）的 checklist 保障。
- **workflow-requirement 与 archive 的联动** — 原因: archive 由用户手动执行 `/dev-team:openspec-archive-change`，workflow-requirement skill 仅停止并通知。无法通过自动化测试验证用户行为。通过集成测试验证 workflow 完成后不自动调用 archive。
