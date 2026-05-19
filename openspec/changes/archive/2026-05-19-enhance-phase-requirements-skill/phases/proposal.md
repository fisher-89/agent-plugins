# 提案: 增强 phase-requirements 技能的上下文感知能力

> **变更**: enhance-phase-requirements-skill
> **日期**: 2026-05-19
> **作者**: dev-team

---

## 1. 问题 (Why)

### 1.1 背景

`phase-requirements` 是 PGE 工作流中的第一个 DESIGN 阶段技能，负责产出 `proposal.md`。当前实现存在三个关键缺口：

**缺口一：无法处理"变更不存在"的场景。** `phase-requirements` 作为整个 PGE 工作流的入口，假设用户已经在某个变更上下文中工作。但现实中用户可能刚刚完成 `openspec-explore` 探索，尚未创建任何变更。当前技能直接假定 `openspec/changes/<name>/` 目录存在，如果不存在则报错。相比之下，已被废弃的 `openspec-propose` 技能能够通过 `openspec new change` 自动创建变更目录——这个能力在迁移到 PGE 工作流时丢失了。

**缺口二：Planner 提示词完全静态。** 当前技能直接向 Planner 传递硬编码的模板路径 (`plugins/dev-team/templates/artifacts/proposal.md.template`)，没有利用 OpenSpec CLI 的动态指令能力。`openspec status` 和 `openspec instructions` 能够根据 schema 提供准确的 artifact 依赖关系、上下文约束和模板规则，但这些信息完全没有传递给 Planner。

**缺口三：探索上下文丢失。** 当用户从 `openspec-explore` 会话结束后直接调用 `phase-requirements`，探索过程中形成的决策、分析、设计思路全部丢失。Planner 从零开始构建提案，无法利用已形成的洞察。

### 1.2 动机

这三个缺口导致了以下实际问题：

- 用户必须先手动创建变更才能调用 `phase-requirements`，增加了工作流摩擦
- Planner 产出的 `proposal.md` 质量高度依赖当前对话上下文，如果对话被重置或探索会话已过期，产出质量大幅下降
- 已废弃的 `openspec-propose` 技能的部分能力（变更脚手架、CLI 指令获取）尚未被 PGE 技能继承

### 1.3 解决方向

在不破坏现有 P→E 循环结构的前提下，为 `phase-requirements` 技能增加三层上下文感知能力：

1. **变更感知**：检测变更是否存在，不存在时自动通过 `openspec new change` 创建
2. **CLI 感知**：使用 `openspec status` / `openspec instructions` 动态获取 artifact 模板和约束
3. **探索感知**：检测是否来自 `openspec-explore`，将探索会话内容传递给 Planner

该方案不修改现有 P→E 循环逻辑、`phases/` 目录布局、backtrack 机制和 `eval.json` 格式。

---

## 2. 范围 (What Changes)

### 2.1 范围内

- 修改 `plugins/dev-team/skills/phase-requirements/SKILL.md` 中的 Step 1（检测变更）和 Step 3a（调用 Planner）
- 增加变更不存在时的脚手架流程：调用 `openspec new create "<name>"` 创建变更目录
- 增加 `openspec status --json` 调用以验证 artifacts 状态
- 增加 `openspec instructions` 调用以获取动态模板、规则和上下文约束
- 增加探索上下文传递：当技能检测到来自 `openspec-explore` 时，在 Planner 提示词中包含探索洞察摘要
- 增加 AskUserQuestion 兜底逻辑：当既无变更名又无探索上下文时，询问用户意图并推导 kebab-case 名称
- 所有数据写入位置保持不变：`openspec/changes/<name>/phases/proposal.md`

### 2.2 不在范围内

- 修改 P→E 循环结构（Step 3b/3c 保持不变）
- 修改 `phases/` 子目录布局
- 修改 backtrack 检测和 re-evaluation 逻辑
- 修改 `eval.json` 格式或 Evaluator 行为
- 修改 `requirements-planner` agent 提示词（Planner 本身不需要改动，仅传入的上下文变化）
- 修改其他 phase 技能（test-design, dev-proposal, implement 等）
- 修改 `openspec-propose` 废弃技能的迁移消息
- 增加新的 CLI 命令或工具

### 2.3 能力清单

#### 新增能力

| 能力 | 描述 | 对应 Spec |
|------|------|-----------|
| `change-scaffolding` | 当变更不存在时，phase-requirements 自动通过 `openspec new change` 创建变更目录和 `.openspec.yaml` | 新建 |
| `explore-context-pass-through` | 当来自 openspec-explore 时，将探索会话洞察摘要传递给 Planner | 新建 |
| `ask-user-fallback` | 当既无变更名又无探索上下文时，通过 AskUserQuestion 询问用户意图 | 新建 |

#### 修改能力

| 能力 | 变更描述 | 对应 Spec |
|------|---------|-----------|
| `planner-prompt-enrichment` | Planner 提示词从静态模板路径改为：静态模板 + `openspec instructions` 动态输出的合成内容 | `phase-skills` |
| `openspec-cli-integration` | 技能在 Step 1 中集成 `openspec new change`, `openspec status`, `openspec instructions` 调用 | `phase-skills` |

---

## 3. 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|---------|
| `openspec new change` CLI 命令在不同版本 OpenSpec 中输出格式变化导致解析失败 | 中等：变更创建失败，用户需手动重试 | 低 | 将 CLI 交互封装在 `.openspec-cli.sh` 包装器中，统一处理版本差异；在 CI 中测试与当前已安装 `openspec` 版本的兼容性 |
| 探索上下文传递给 Planner 时包含无关或误导性信息，导致提案偏离轨道 | 中等：Planner 产出低质量提案，需更多 P→E 循环迭代 | 中 | 在传递前对探索上下文做摘要处理，仅提取结构化洞察（决策、设计选择、排除方案）；Planner 提示词中明确"探索上下文仅供参考，以 CLI 指令和模板为准" |
| AskUserQuestion 兜底流程中用户提供了模糊描述，推导出的 kebab-case 名称不准确 | 低：变更名不准确但仍可工作，后续可重命名 | 低 | 在推导前向用户展示建议名称并确认："I'll create a change named '<name>'. Proceed?"；允许用户输入自定义名称 |
| 增加 CLI 调用次数导致技能响应变慢（`openspec status` + `openspec instructions` 各一次） | 低：增加约 1-2 秒延迟 | 中 | CLI 调用是轻量级本地操作；将 `openspec status` 结果缓存用于多条 `openspec instructions` 调用（单次 status 后多次 instructions 复用依赖关系） |

---

## 4. 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-01 | 当 `openspec/changes/<name>/` 不存在时，phase-requirements 自动调用 `openspec new change <name>` 创建目录和 `.openspec.yaml` | 在无变更目录的情况下调用 `/dev-team:phase-requirements test-change`，验证目录被创建且 `.openspec.yaml` 包含正确 schema 值 | P0 |
| AC-02 | 当变更已存在时，phase-requirements 行为与当前一致（直接进入 P→E 循环，不调用脚手架） | 对已有变更调用技能，验证不触发 `openspec new change`，直接进入 Planner 调用 | P0 |
| AC-03 | Planner 收到的提示词包含 `openspec instructions` 动态输出的 rule、context 和 template 信息 | 在技能执行过程中捕获传递给 Planner agent 的 prompt 内容，验证包含来自 `openspec instructions` 的 `rules` 和 `context` 字段 | P1 |
| AC-04 | 当从 openspec-explore 会话进入时，Planner 提示词包含探索洞察摘要作为额外上下文 | 完成一次 openspec-explore 会话后调用 `/dev-team:phase-requirements`，验证输出 `proposal.md` 包含探索中讨论的关键决策 | P1 |
| AC-05 | 当无变更名且无探索上下文时，技能通过 AskUserQuestion 询问用户意图 | 在空对话中调用 `/dev-team:phase-requirements`（不带参数），验证 AskUserQuestion 被触发且问题包含"想构建什么" | P1 |
| AC-06 | 用户回答模糊问题后，技能推导出 kebab-case 变更名并在创建前请求确认 | 在 AskUserQuestion 中输入模糊描述"加个登录功能"，验证技能建议 `add-login` 并等待确认 | P2 |
| AC-07 | 现有 P→E 循环结构不受影响：Step 3b/3c 的 Evaluator 调用、循环逻辑、eval.json 写入均保持不变 | 对已有变更运行完整的 phase-requirements 流程（P→E→P→E 直到 pass），验证 eval.json 格式与当前完全一致 | P0 |
| AC-08 | 现有 backtrack 检测逻辑不受影响：Step 2 的 backtrack marker 检测和 re-evaluation 流程保持不变 | 在 eval.json 中注入 `backtrack_to: "01-requirements"` 标记，运行技能，验证触发 re-evaluation 而非直接进入 Planner | P0 |
| AC-09 | 所有数据仍写入 `openspec/changes/<name>/phases/proposal.md`，不改变输出路径 | 运行完整流程，验证 `proposal.md` 始终位于 `phases/` 子目录中 | P0 |
