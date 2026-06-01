# 提案: add-workflow-requirement-skill

> **变更**: add-workflow-requirement-skill
> **日期**: 2026-06-01
> **状态**: draft

---

## 问题

当前 PGE workflow 的 9 个 phase 需要用户手动逐个执行 `/dev-team:phase-xxx`，流程繁琐且容易遗漏。explore 阶段积累的上下文（决策表、架构分析、What We Figured Out 总结）在进入 implementation 时无法自动传递，用户需要在 phase-requirements 中重复描述。

此外，phase-01（phase-requirements）的 artifact 生成由主模型直接完成，而非通过子代理（Planner→Evaluator），与 Phase 02-05 的 P→E 模式不一致，导致 workflow 技能无法复用 phase-01 的逻辑。

## 提案

新增三个核心组件：

1. **phase-proposal 技能** — 重构 phase-01，采用 P→E 模式：
   - `proposal-planner` 子代理（新）写入 proposal.md + specs/
   - `proposal-evaluator` 子代理（由 requirements-evaluator 重命名）评估
   - 与 phase-dev-design、phase-test-design 保持一致的模式

2. **workflow-requirement 技能** — 薄编排循环，无硬编码 phase 知识：
   - 调用 `eval/next` MCP 接口获取下一步要执行的 phase、subagent、prompt
   - 循环：eval/next → Agent(planner) → Agent(evaluator) → eval/next → ...
   - eval/next 服务端处理 gate check、skip、retry、backtrack 自动跳转、20 轮上限
   - 从 explore 会话中提取上下文传递给 proposal-planner
   - 不包含硬编码 phase table，不自行生成 agent prompt

3. **预留扩展** — 未来 workflow-bug-fix（跳过 test-design/test-gen）、workflow-refactor（强调设计审查）

## 能力

### 新增能力

- **workflow-orchestration** — 全流程编排能力：串联所有 phase、gate check 前置检查、verdict 判定、失败停止

### 修改的能力

- **phase-skills** — 新增 phase-proposal 技能（P→E），新增 workflow-requirement 技能（编排器），phase-requirements 标记为 deprecated
- **phase-agents** — 新增 proposal-planner 代理，requirements-evaluator 重命名为 proposal-evaluator
- **pge-workflow-engine** — 新增 workflow 层级（skill 编排 phase），扩展 eval.schema.json phase ID "01-requirements" → "01-proposal"

---

## 变更范围

### 实现以下特性

- 新增 `eval/next` MCP 接口 — 返回 next_phase、agent_type、prompt；服务端处理 gate/skip/retry/backtrack/round_limit
- 新增 `proposal-planner` 代理定义（Model: opus, Tools: Read/Write/Grep/Glob/Bash）
- 重命名 `requirements-evaluator` → `proposal-evaluator`，phase ID 改为 `"01-proposal"`
- 新增 `phase-proposal` 技能（P→E 循环，与 phase-dev-design 一致）
- 删除 `phase-requirements` 技能及目录
- 新增 `workflow-requirement` 技能（薄循环：eval/next → Agent → eval/next → ...，完成后停止，由用户手动 archive）
- 更新 `eval.schema.json` phase 枚举值
- 更新 `plugin.json` 版本号

### 不要修改

- Phase 02-09 的内部逻辑（仅 phase-01 重构）
- 现有 proxy 定义（implementation-generator, test-gen-generator 等）
- openspec-cli.sh（无新 CLI 操作）
- MCP 服务端（eval/log / eval/check 接口不变）
- 现有 changes 的 eval.json（旧 phase ID 保留，不迁移）

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | phase-proposal 技能可独立执行，完成 P→E 循环至 verdict=pass | 运行 `/dev-team:phase-proposal <test-change>` 验证 | P0 |
| AC-2 | proposal-planner 代理正确写入 proposal.md + specs/ 所有文件 | 检查 openspec/changes/<name>/ 输出 | P0 |
| AC-3 | proposal-evaluator 使用与 requirements-evaluator 相同的 checklist | 代码对比验证 checklist 不变 | P0 |
| AC-4 | workflow-requirement 自动执行 Phase 01-09（有 explore 上下文时，自动推断 change name） | 运行 `/dev-team:workflow-requirement` 验证 | P0 |
| AC-5 | workflow-requirement 在有 explore 上下文时，自动提取并传递给 proposal-planner | 检查 proposal-planner 收到的 prompt 包含 explore 摘要 | P1 |
| AC-6 | 已通过的 phase 被自动跳过 | 检查 workflow 日志确认跳过逻辑 | P1 |
| AC-7 | 任意 phase 失败（verdict=fail after 5 retries）时 workflow 停止并通知 | 模拟失败场景验证 | P1 |
| AC-8 | 任意 phase 设置 backtrack_to 时 workflow 自动清除目标 phase 及后续 eval.json 条目，跳转到目标 phase 继续执行 | 模拟 backtrack 场景验证自动跳转 | P1 |
| AC-9 | 所有 phase pass 后停止并通知用户手动执行 archive | 运行完整流程后检查 workflow 不自动执行 archive，用户需手动 `/dev-team:openspec-archive-change` | P0 |
| AC-10 | 全局轮次超过 20 时 workflow 停止并报告 | 构造循环回溯场景验证 | P1 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| explore 上下文提取不完整，planner 缺少关键信息 | 中 | 中 | proposal-planner 不仅依赖传递的摘要，也读取 explore 技能的输出结构（决策表、What We Figured Out） |
| workflow 长时间运行（10-30 min），用户等待焦虑 | 低 | 中 | 每 phase 完成后输出进度（含 round 计数）；PushNotification 通知完成/阻塞；backtrack 自动恢复无需人工干预 |
| phase ID 变更导致现有 eval.json 解析异常 | 低 | 低 | MCP eval/check 按字符串匹配，旧 entry 与新 entry 不冲突；新 change 使用新 ID |
| proposal-planner 缺乏探索上下文导致提案质量下降 | 中 | 低 | planner 保留 Read 权限，可自行读取 conversation 上下文中的 CLAUDE.md |
| backtrack 循环导致无限重试（如 02→07→02→07...）| 高 | 低 | 全局 20 轮硬限制，超限立即停止；每轮回溯清空目标 phase 后续 eval 条目，确保 re-evaluation 生效 |
