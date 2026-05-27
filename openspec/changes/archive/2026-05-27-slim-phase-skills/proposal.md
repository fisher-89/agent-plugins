# 提案: slim-phase-skills

> **变更**: slim-phase-skills
> **日期**: 2026-05-27
> **状态**: 起草中

---

## 问题

当前 7 个 PGE 阶段技能（phase-* skills）存在以下问题：

**1. 体积臃肿，重复严重**

7 个技能合计约 633 行，每个技能都独立实现了：
- eval.json 解析和 verdict 读取逻辑
- backtrack 检测逻辑
- 前置条件文件检查逻辑
- Agent prompt 中硬编码了模板路径、输出路径、详细 process 描述

而对应的 subagent 的 agent.md 文件已经完整定义了 Input、Process、Output、Constraints。技能中的描述与 agent.md 重复，且容易不同步。

**2. 流程控制逻辑不一致**

每个技能手动实现 eval.json 解析，没有统一的入口。部分技能检查 prior phase gate，部分不检查；部分技能支持 backtrack，部分不支持。逻辑分散导致行为不一致。

**3. 废弃 skill 残留**

`code-review` skill（非 `phase-code-review`）已被 PGE 工作流取代，但仍保留在技能目录和 agent 目录中，增加维护负担。

**4. 阶段代码不一致**

`plugins/dev-team/utils/eval-check.py` 中使用了 `05-implementation`，而 `workflow.ts` PHASES 数组中使用 `05-implement`，两者不一致。`implementation-evaluator.md` 中也引用了旧的阶段代码。

---

## 提案

### 方案对比

#### 方案 A：最小清理

仅移除每个技能的 Phase Pattern 文档段落（非执行逻辑），不改变编排方式。

- **优点**：改动最小，风险低
- **缺点**：冗余问题未解决，流程控制仍分散，技能体积仍然偏大（~580 行）
- **结论**：治标不治本

#### 方案 B：精简编排器（推荐）

将技能重构为纯编排器模式（~20-60 行），将领域知识下沉到 subagent 的 agent.md 中。具体做法：

1. **流程控制统一**：使用 `dev-team eval-check --change <name> --phase <phase-code>` CLI 命令处理 prior phase gate 验证、timestamp order 检查、backtrack 检测、phase_state 报告
2. **Agent prompt 极简化**：技能只传递一行 prompt，subagent 从自己的 agent.md 获取完整的 Input/Process/Output/Constraints 指令
3. **Change name 解析保留在 Skill**：依赖对话历史和用户交互，subagent 无法访问
4. **AUTO 阶段保留在 Skill**：lint/test runner shell 命令是编排层职责
5. **删除 Phase Pattern 文档段落**：技能不再包含非执行逻辑的文档
6. **删除废弃 code-review skill**：同时删除对应的 code-review.md agent
7. **修正阶段代码**：`05-implementation` -> `05-implement`，与 workflow.ts PHASES 数组对齐

- **优点**：
  - 技能精简到 ~20-60 行，消除冗余
  - 流程控制统一到 eval-check CLI，行为一致
  - 领域知识集中在 agent.md，消除双重复
  - 删除废弃代码，降低维护成本
- **缺点**：
  - 需要确认所有相关 agent.md 已经承载了领域知识
  - 需要将技能中的 eval.json 解析逻辑迁移到 eval-check CLI 调用
- **结论**：推荐方案 B

#### 方案 C：完全去技能化

移除所有技能编排，让 subagent 完全自编排。用户直接调用 agent。

- **优点**：架构最简洁
- **缺点**：
  - 失去编排层的流程控制（loop、verdict 检查）
  - 失去 AUTO 阶段的 shell 命令执行能力
  - 失去 change name 的交互式解析
  - 与 Claude Code 的 skill 架构模式不匹配
- **结论**：过度精简，不可行

### 推荐方案

采纳方案 B。目标架构如下：

```
Skill (~20-60 行纯编排器)
├── Usage 说明
├── Change name 解析（保留在 Skill）
├── eval-check CLI gate（替代手动 eval.json 解析）
├── 执行阶段:
│   ├── phase-requirements: 主 agent 直接写文件（不启动 Planner subagent）
│   ├── 其他 DESIGN: Planner subagent（一行 prompt）
│   ├── EXECUTION: Generator subagent（一行 prompt）
│   └── 所有阶段: Evaluator subagent（一行 prompt）
├── AUTO 阶段（仅 phase-implement，shell 命令）
├── Verdict loop（读取 eval.json 最新条目）
└── Report 结果
```

**关键决策: phase-requirements 的 Planner 使用主 agent 直接执行 + requirements-planner.md 删除**

`phase-requirements` 是唯一将 Planner 改为主 agent 直接执行的 DESIGN 阶段：
- **原因**: proposal.md + specs/ 的创作受益于完整对话上下文（explore context、CLI instructions、用户偏好）。主 agent 直接读写文件，避免通过 subagent prompt 转述上下文导致信息丢失。
- **删除 requirements-planner.md**: Planner subagent 不再需要，对应的 `plugins/dev-team/agents/requirements-planner.md` 随本次变更删除。
- **其他 DESIGN 阶段**（test-design, dev-proposal）保持 Planner subagent 模式不变。
- **Evaluator 保持为 subagent**：checklist 评估是确定性工作，subagent 的隔离性确保评估独立于 Planner 的推理过程。

具体影响：
- `phase-requirements`: 主 agent 直接写 proposal.md + specs/ + 删除 `requirements-planner.md`，`requirements-evaluator` subagent 评估
- `phase-test-design`: `test-design-planner` subagent 写 test-design.md，`test-design-evaluator` subagent 评估（不变）
- `phase-dev-proposal`: `dev-proposal-planner` subagent 写 design.md + tasks.md，`dev-proposal-evaluator` subagent 评估（不变）
- `phase-implement`: `implementation-generator` subagent 写代码，`implementation-evaluator` subagent 评估（不变）
- `phase-test-gen`: `test-gen-generator` subagent 写测试文件，`test-gen-evaluator` subagent 评估（不变）
- EVALUATOR-ONLY 阶段（code-review, acceptance）：保持单 subagent 模式不变

---

## 能力

### 新增能力

（无）

### 修改的能力

- `phase-skills` — 7 个阶段技能重构为纯编排器模式；废弃的 `code-review` skill 删除；冗余的 `eval-check.py` 删除并统一到 TypeScript CLI `dev-team eval-check`；阶段代码对齐
- `phase-agents` — 删除废弃的 `code-review.md` agent；删除 `requirements-planner.md` agent（phase-requirements 由主 agent 直接执行）；确认其余 subagent 的 agent.md 已承载领域知识

---

## 变更范围

### 实现以下特性

1. 精简 7 个技能文件（phase-requirements, phase-test-design, phase-dev-proposal, phase-implement, phase-test-gen, phase-code-review, phase-acceptance）为纯编排器模式
   - 每个技能保留 Usage 说明、change name 解析、eval-check CLI gate、Agent 调用（一行 prompt）、verdict loop、结果报告
   - 移除详细的 Process 步骤描述（已在 agent.md 中）
   - 移除 Phase Pattern 文档段落
2. 删除废弃的 `code-review` skill 目录（`plugins/dev-team/skills/code-review/`）
3. 删除废弃的 `code-review.md` agent（`plugins/dev-team/agents/code-review.md`）
4. 删除 `requirements-planner.md` agent（`plugins/dev-team/agents/requirements-planner.md`）— phase-requirements 由主 agent 直接执行，不再需要 Planner subagent
5. 删除冗余的 `eval-check.py`（`plugins/dev-team/utils/eval-check.py`）— 功能已被 TypeScript CLI `dev-team eval-check` 完全取代
   - 更新 `plugins/dev-team/skills/openspec-archive-change/SKILL.md` 中的 eval-check 引用：`python plugins/dev-team/utils/eval-check.py` → `dev-team eval-check`
6. 修复 `05-implementation` -> `05-implement` 阶段代码：
   - 更新 `plugins/dev-team/agents/implementation-evaluator.md` 中的 eval-log 示例
6. 在所有 7 个技能中集成 `dev-team eval-check --change <name> --phase <phase-code>`，替代手动 eval.json 解析和 backtrack 检测
7. 确认以下 agent.md 已承载领域知识（无需修改）：
   - `test-design-planner.md` — 模板路径、输出路径
   - `dev-proposal-planner.md` — 模板路径、输出路径
   - `implementation-generator.md` — input/output 路径、task completion 指令
   - `test-gen-generator.md` — 输出路径、文件类型黑名单
   - `code-review-evaluator.md` — design.md 路径引用
   - `acceptance-evaluator.md` — proposal.md 路径引用

### 不要修改

- `phase-unit-test` 和 `phase-integration-test` 技能的编排逻辑（已在目标形态附近，不在本次精简范围内）
- 非阶段技能（`openspec-propose`, `openspec-apply-change`, `openspec-archive-change`, `openspec-explore`, `update-architecture`）
- Subagent evaluator 的静态 checklist 内容
- Agent evaluator 的评估逻辑和决策树
- `dev-team eval-check` CLI 命令本身的实现
- `workflow.ts` PHASES 数组结构（已是 `05-implement`）
- `dev-team-bundle.cjs` 和 `dev-team-bundle.cjs.map`（自动生成文件）


---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-01 | 7 个技能文件均已精简，每个不超过 60 行 | 对每个技能文件执行 `wc -l`，确认行数不超过 60 | P0 |
| AC-02 | 每个精简后的技能均调用 `dev-team eval-check` 进行 prior phase 验证 | grep 搜索每个技能文件，确认包含 `eval-check --change` 调用 | P0 |
| AC-03 | 每个精简后的技能使用一行 agent prompt（不再包含模板路径、输出路径等详细描述） | 检查每个技能中的 Agent 调用，prompt 字段为单行字符串 | P0 |
| AC-04 | 废弃的 `code-review` skill 目录已被删除 | 确认 `plugins/dev-team/skills/code-review/` 目录不存在 | P0 |
| AC-05 | 废弃的 `code-review.md` agent 已被删除 | 确认 `plugins/dev-team/agents/code-review.md` 文件不存在 | P0 |
| AC-06 | 冗余的 `eval-check.py` 已被删除 | 确认 `plugins/dev-team/utils/eval-check.py` 文件不存在 | P0 |
| AC-07 | `implementation-evaluator.md` 中的 eval-log 示例使用 `05-implement` | grep 搜索 `implementation-evaluator.md`，确认 eval-log 示例使用 `05-implement` | P0 |
| AC-07a | `openspec-archive-change` skill 使用 `dev-team eval-check` CLI 而非 `eval-check.py` | grep 搜索 `openspec-archive-change/SKILL.md`，确认包含 `dev-team eval-check` 调用 | P0 |
| AC-08 | 所有 7 个技能文件已移除 Phase Pattern 文档段落 | 确认每个技能文件末尾不存在 `## DESIGN Phase Pattern` 或 `## EXECUTION Phase Pattern` 或 `## EVALUATOR-ONLY Pattern` 章节 | P0 |
| AC-09 | 所有 7 个技能保留 AUTO 阶段 shell 命令（如适用） | 对于 `phase-implement`，确认保留 lint-runner.py 调用 | P1 |
| AC-10 | `phase-requirements` 保留 explore context 检测逻辑 | 检查 `phase-requirements` 技能文件，确认包含 explore context 的 Branch A/B 决策树 | P1 |
| AC-11 | 独立技能文件可正常加载，`/dev-team:phase-*` 命令可用 | 执行 `/dev-team:phase-test-design`（空运行），确认技能响应正常 | P1 |
| AC-12 | `phase-requirements` 的 change name 交互式解析（含 explore context 检测）功能正常 | 执行 `/dev-team:phase-requirements` 不带参数，确认进入 explore context 检测流程 | P2 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Agent prompt 过度精简导致 subagent 行为异常 | 技能调用 agent 时因 prompt 信息不足产生错误输出 | 中 | 在精简前逐个确认每个 agent.md 已包含完整的 Input/Process/Output/Constraints；精简后对每个技能执行一次空运行验证 |
| eval-check CLI 与部分技能的 phase code 不匹配 | eval-check 验证错误导致流程阻塞 | 低 | 在集成 eval-check 前统一修正所有 phase code 引用到 `workflow.ts` PHASES 数组 |
| code-review skill 删除后仍有用户依赖 | 用户无法使用 `/code-review` 命令 | 低 | skill 已标记为 DEPRECATED 并提示使用 `/dev-team:phase-code-review`；删除后用户收到"skill not found"错误，但在 CLI 面板中仍有提示 |
| 精简过程中遗漏某个技能的回滚路径 | 某个技能不可逆地丢失流程细节 | 低 | 每个技能精简前使用 git commit 保存原始版本，精简后通过 git diff 验证变更内容 |
| 行数目标过于严格导致某些技能的必需逻辑被裁剪 | 技能功能不完整 | 低 | 在每个精简技能文件中附加行数注释，如果某个技能因必需逻辑无法精简到 60 行以内，调整行数目标而非裁剪逻辑 |
