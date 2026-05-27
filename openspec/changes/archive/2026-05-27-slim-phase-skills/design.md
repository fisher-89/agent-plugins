# 设计: slim-phase-skills

> **变更**: slim-phase-skills
> **日期**: 2026-05-27
> **基于**: proposal.md, test-design.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 阶段技能编排器 (7个) | 纯编排层：解析 change name、调用 eval-check CLI 进行门控验证、启动 subagent（单行 prompt）、读取 eval.json 检查 verdict、报告结果 | `plugins/dev-team/skills/phase-{requirements,test-design,dev-proposal,implement,test-gen,code-review,acceptance}/SKILL.md` | `dev-team eval-check` CLI、`dev-team eval-log` CLI、eval.json 文件读取 | Skill (YAML + Markdown + Agent 声明式调用) |
| eval-check CLI | 统一流程控制：前置阶段门控验证、timestamp order 检查、backtrack 检测、phase_state 报告 | `plugins/dev-team/bin/src/commands/eval-check.ts` | eval.json 文件、workflow.ts PHASES 数组 | TypeScript (CLI via cac) |
| Planner subagent (4个) | 读取上游产物，撰写阶段产出物。由技能编排器通过 Agent 工具以单行 prompt 启动 | `plugins/dev-team/agents/{test-design-planner,dev-proposal-planner,implementation-generator,test-gen-generator}.md` | 上游阶段产物（proposal.md、test-design.md、design.md + tasks.md） | Agent (YAML frontmatter + Markdown 指令，包含完整 Input/Process/Output/Constraints) |
| Evaluator subagent (7个) | 对阶段产出物执行静态 checklist 评估，写入 eval.json。由技能编排器通过 Agent 工具以单行 prompt 启动 | `plugins/dev-team/agents/{requirements,test-design,dev-proposal,implementation,test-gen,code-review,acceptance}-evaluator.md` | 阶段产出物文件、eval.json | Agent (YAML frontmatter + Markdown 指令 + CLI eval-log 调用) |
| 主 agent (phase-requirements) | 直接撰写 proposal.md + specs/ 文件，依赖完整对话上下文而非通过 subagent prompt 传递 | 无独立文件 — 在 `phase-requirements/SKILL.md` 中通过 Bash/Write 工具直接操作 | 对话历史、CLI 指令、CLAUDE.md | Claude 主 agent (工具直接调用) |

### 组件图

```
Skill 编排器 (~20-60 行)
  │
  ├─ 1. 解析 change name（交互式 / 参数）
  │     └── phase-requirements 特有：Branch A/B 决策树检测 explore context
  │
  ├─ 2. dev-team eval-check --change <name> --phase <phase-code>
  │     └── CLI 执行门控验证，失败则中断并报告
  │
  ├─ 3. 启动阶段 subagent（单行 prompt）
  │     ├── DESIGN (P→E): Planner → Evaluator
  │     ├── EXECUTION (G→E/AUTO): Generator → [AUTO] → Evaluator
  │     └── EVALUATOR-ONLY (E): Evaluator（单次）
  │
  ├─ 4. 读取 eval.json 最新 verdict
  │     └── fail 则回退到步骤 3（最多 5 次）
  │
  └─ 5. 报告结果
```

---

## 数据流

### 流程描述

每个阶段技能的精简编排流程如下：

```
用户输入 `/dev-team:phase-<name> [change-name]`
  │
  ▼
Skill 编排器
  │
  ├── [Step 1] 解析 change name
  │     ├── 如果提供了参数 → 验证格式，scaffold 若需要
  │     ├── 如果未提供参数 → phase-requirements 执行 Branch A/B 决策树
  │     │                    (检测 explore context → 交互式询问)
  │     └── 其他阶段 → 扫描 openspec/changes/ 目录获取活跃 change
  │
  ├── [Step 2] 门控验证: eval-check CLI
  │     └── dev-team eval-check --change <name> --phase <phase-code>
  │           ├── 前置阶段 pass 记录检查 → 缺失则 block
  │           ├── timestamp order 检查 → 乱序则 block
  │           ├── backtrack 检测 → 活跃则 block
  │           └── phase_state 报告 → first_run / retry / passed
  │
  ├── [Step 3] 阶段执行
  │     ├── DESIGN phases: Agent(Planner) → Agent(Evaluator)
  │     ├── EXECUTION phases: Agent(Generator) → [AUTO shell] → Agent(Evaluator)
  │     └── EVALUATOR-ONLY phases: Agent(Evaluator)
  │
  ├── [Step 4] Verdict 检查
  │     └── 读取 openspec/changes/<name>/phases/eval.json 最新条目
  │           ├── pass → 继续
  │           ├── fail → 循环回 Step 3（最多 5 次）
  │           └── EVALUATOR-ONLY: 单次运行，无循环
  │
  └── [Step 5] 报告结果
        └── 显示 verdict、pass/total、notes、backtrack 建议（如适用）
```

### 阶段代码与映射

| 阶段技能 | 阶段代码 | 编排模式 | Subagent (Planner/Generator) | Subagent (Evaluator) | AUTO |
|----------|----------|----------|-----------------------------|---------------------|------|
| phase-requirements | `01-requirements` | P→E | 主 agent 直接写文件（无 subagent） | requirements-evaluator | 无 |
| phase-test-design | `02-test-design` | P→E | test-design-planner | test-design-evaluator | 无 |
| phase-dev-proposal | `03-dev-proposal` | P→E | dev-proposal-planner | dev-proposal-evaluator | 无 |
| phase-test-gen | `04-test-gen` | G→E | test-gen-generator | test-gen-evaluator | 无 |
| phase-implement | `05-implement` | G→E+AUTO | implementation-generator | implementation-evaluator | lint-runner.py |
| phase-code-review | `07-code-review` | E (单次) | 无 | code-review-evaluator | 无 |
| phase-acceptance | `09-acceptance` | E (单次) | 无 | acceptance-evaluator | 无 |

### 各技能详细编排逻辑

#### phase-requirements

```
Skill:
  - 解析 change name（参数/交互式）
    - Branch A: 有参数 → 验证并 scaffold
    - Branch B: 无参数 → 检测 explore context → 交互式询问
  - dev-team eval-check --change <name> --phase 01-requirements
  - 主 agent 直接撰写 proposal.md + specs/（无 Planner subagent）
  - Agent({ subagent_type: "requirements-evaluator", prompt: "单行" })
  - 读取 eval.json 检查 verdict（fail 则重新撰写）
  - 报告结果
```

#### phase-test-design

```
Skill:
  - 解析 change name
  - dev-team eval-check --change <name> --phase 02-test-design
  - Agent({ subagent_type: "test-design-planner", prompt: "单行" })
  - Agent({ subagent_type: "test-design-evaluator", prompt: "单行" })
  - 读取 eval.json 检查 verdict（fail 则循环）
  - 报告结果
```

#### phase-dev-proposal

```
Skill:
  - 解析 change name
  - dev-team eval-check --change <name> --phase 03-dev-proposal
  - Agent({ subagent_type: "dev-proposal-planner", prompt: "单行" })
  - Agent({ subagent_type: "dev-proposal-evaluator", prompt: "单行" })
  - 读取 eval.json 检查 verdict（fail 则循环）
  - 报告结果
```

#### phase-implement

```
Skill:
  - 解析 change name
  - dev-team eval-check --change <name> --phase 05-implement
  - Agent({ subagent_type: "implementation-generator", prompt: "单行" })
  - AUTO: python plugins/dev-team/utils/lint-runner.py ...
  - Agent({ subagent_type: "implementation-evaluator", prompt: "单行" })
  - 读取 eval.json 检查 verdict（fail 则循环）
  - 报告结果
```

#### phase-test-gen

```
Skill:
  - 解析 change name
  - dev-team eval-check --change <name> --phase 04-test-gen
  - Agent({ subagent_type: "test-gen-generator", prompt: "单行" })
  - Agent({ subagent_type: "test-gen-evaluator", prompt: "单行" })
  - 读取 eval.json 检查 verdict（fail 则循环）
  - 报告结果
```

#### phase-code-review

```
Skill:
  - 解析 change name
  - dev-team eval-check --change <name> --phase 07-code-review
  - Agent({ subagent_type: "code-review-evaluator", prompt: "单行" })
  - 读取 eval.json 检查 verdict 和 backtrack_to
  - 报告结果（含 backtrack 建议）
```

#### phase-acceptance

```
Skill:
  - 解析 change name
  - dev-team eval-check --change <name> --phase 09-acceptance
  - Agent({ subagent_type: "acceptance-evaluator", prompt: "单行" })
  - 读取 eval.json 检查 verdict 和 backtrack_to
  - 报告结果（含 backtrack 建议）
```

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| eval.json 条目 | `phase`, `verdict`, `items[]` (item_id, pass, evidence), `attempt`, `timestamp`, `schema_version`, `backtrack_to`, `report`, `skipped` | 每条记录一个阶段的一次评估；按 `timestamp` 排序，取每个 `phase` 的最新一条 | `openspec/changes/<name>/phases/eval.json` (JSON 数组) |
| eval-check 结果 | `passed`, `phase`, `prior_phases[]`, `block_reasons[]`, `phase_state`, `details` (prior_phase_gate, timestamp_order, backtrack) | 每次调用实时计算，不持久化 | 无持久化 — stdout / JSON 输出 |
| SKILL.md (技能文件) | YAML frontmatter (name, description, license, metadata) + Markdown 编排逻辑 | 每个技能独立，通过 CLI 和 agent.md 与外部协作 | 文件系统 |

---

## 路由/API 设计

### 技能命令

| 命令 | 技能文件 | 阶段代码 | 模式 | 输入 |
|------|----------|----------|------|------|
| `/dev-team:phase-requirements` | `phase-requirements/SKILL.md` | `01-requirements` | P→E (主 agent Planner) | `[change-name]`（可选） |
| `/dev-team:phase-test-design` | `phase-test-design/SKILL.md` | `02-test-design` | P→E | `[change-name]`（可选） |
| `/dev-team:phase-dev-proposal` | `phase-dev-proposal/SKILL.md` | `03-dev-proposal` | P→E | `[change-name]`（可选） |
| `/dev-team:phase-test-gen` | `phase-test-gen/SKILL.md` | `04-test-gen` | G→E | `[change-name]`（可选） |
| `/dev-team:phase-implement` | `phase-implement/SKILL.md` | `05-implement` | G→E+AUTO | `[change-name]`（可选） |
| `/dev-team:phase-code-review` | `phase-code-review/SKILL.md` | `07-code-review` | E (单次) | `[change-name]`（可选） |
| `/dev-team:phase-acceptance` | `phase-acceptance/SKILL.md` | `09-acceptance` | E (单次) | `[change-name]`（可选） |

### CLI 调用

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| `dev-team eval-check` | `plugins/dev-team/bin/src/commands/eval-check.ts` | 前置阶段门控验证 | `--change <name> --phase <code> [--json]` | stdout (human/JSON), exit code 0/1 | 无 |
| `dev-team eval-log` | `plugins/dev-team/bin/src/commands/eval-log.ts` | 追加评估记录到 eval.json | `--change <name> --phase <code> --verdict <pass|fail> --report <str> --items <json>` | stdout, exit code | 无 |
| `python .../lint-runner.py` | `plugins/dev-team/utils/lint-runner.py` | AUTO 静态检查（仅 phase-implement） | `--change <name> --project-root <dir> --save-report` | stdout, exit code | 无 |

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D01 | 技能重构为纯编排器模式（20-60 行），领域知识下沉到 agent.md | 消除技能层与 agent.md 之间的知识重复；agent.md 已包含完整 Input/Process/Output/Constraints；技能只保留编排逻辑 | **方案 A（最小清理）**：仅移除 Phase Pattern 文档段落，不改变编排方式。拒绝理由：约 580 行的冗余未解决，流程控制仍分散 |
| D02 | 使用 `dev-team eval-check` CLI 统一流程控制 | eval-check CLI 已实现 prior phase gate、timestamp order、backtrack、phase_state 四项检查；TypeScript CLI 是团队技术栈；消除每个技能手动解析 eval.json 的重复 | **每个技能继续手动解析 eval.json**：每个技能需重复实现 ~20-40 行的 JSON 解析和判定逻辑，行为不一致风险高 |
| D03 | phase-requirements 由主 agent 直接执行，不启动 Planner subagent | proposal.md + specs/ 的创作依赖完整对话上下文（explore context、CLI instructions、用户偏好）；subagent prompt 转述导致信息丢失 | **保留 requirements-planner.md subagent**：需将对话上下文、CLI 指令、explore context 拼接到 prompt 中，prompt 体积大且易丢失上下文 |
| D04 | 删除废弃的 code-review skill 和 code-review.md agent | 已被 `phase-code-review` 完全取代，已标记 `[DEPRECATED]`；保留增加维护负担 | **保留废弃代码**：用户仍可使用 `/code-review`，但已推荐使用 `/dev-team:phase-code-review`。拒绝理由：增加新手迷惑性和维护成本 |
| D05 | 删除废弃的 requirements-planner.md agent | phase-requirements 由主 agent 直接执行后，此 agent 不再被任何技能引用 | **保留但标记 DEPRECATED**：无调用方却保留只会造成混淆 |
| D06 | 删除冗余的 eval-check.py（Python），统一使用 TypeScript CLI | `dev-team eval-check` (TypeScript) 功能完整，已用于 workflow.ts 体系；Python 版本使用 `05-implementation`（错误阶段代码）且功能子集 | **保留 eval-check.py 并修正阶段代码**：多一个重复实现增加维护成本，且 Python 版本缺少 timestamp order 等检查能力 |
| D07 | 修正 `05-implementation` -> `05-implement` 以对齐 workflow.ts | workflow.ts PHASES 数组使用 `05-implement`，eval-check.py 和 implementation-evaluator.md 使用 `05-implementation`，不一致导致 eval-check 验证失败 | **修改 workflow.ts 为 `05-implementation`**：但这会破坏现有已评估变更的兼容性，且需同步修改 9 处引用 |
| D08 | Agent prompt 精简为单行字符串 | agent.md 已包含完整 Input/Process/Output/Constraints，技能层的 prompt 只需指示阶段和 change name | **保留多行 prompt 包含模板路径和输出路径**：agent.md 中已明确写出这些路径，技能层重复这些信息造成双重复，且易不同步 |
| D09 | 删除 Phase Pattern 文档段落（DESIGN/EXECUTION/EVALUATOR-ONLY Pattern） | 这些段落是非执行逻辑的文档，不产生任何行为；技能精简后职责清晰，无需额外文档 | **保留 Phase Pattern 段落**：作为技能文档参考，但与其他章节（Usage、Process）内容重叠，移除后技能更聚焦编排逻辑 |
| D10 | 保留 phase-requirements 的 explore context 检测逻辑（Branch A/B 决策树） | explore context 需要分析对话历史，只能在技能层（主 agent 上下文）完成；subagent 无法访问对话历史 | **将 explore context 检测移至独立 CLI 或 subagent**：无法访问对话历史，技术上不可行 |

---

## 依赖

### 运行时依赖

- `dev-team eval-check` CLI — 每个技能的门控验证依赖，必选项
- `dev-team eval-log` CLI — Evaluator subagent 写入评估结果的通道
- `plugins/dev-team/utils/lint-runner.py` — phase-implement 的 AUTO 静态检查
- `plugins/dev-team/utils/openspec-cli.sh` — phase-requirements 的 change name 验证和 scaffold（validate_change_name, openspec_new_change）
- 各 subagent 的 agent.md 文件 — 领域知识、模板路径、输出路径、Input/Process/Output/Constraints

### 构建/测试依赖

- Python 3 (unittest + re) — 验证脚本运行时
- 无额外第三方包依赖（验证脚本只使用标准库）

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Agent prompt 过度精简导致 subagent 行为异常 | 技能调用 agent 时因 prompt 信息不足产生错误输出 | 中 | 在精简前逐个确认每个 agent.md 已包含完整 Input/Process/Output/Constraints；精简后对每个技能执行空运行 |
| eval-check CLI 调用的 phase code 不匹配 | eval-check 验证返回 phase 无效导致流程阻塞 | 低 | 使用 `workflow.ts` PHASES 数组中的正式阶段代码：`01-requirements`, `02-test-design`, `03-dev-proposal`, `04-test-gen`, `05-implement`, `07-code-review`, `09-acceptance` |
| code-review skill 删除后用户仍运行 `/code-review` | 用户收到 "skill not found" 错误 | 低 | skill 已标记 `[DEPRECATED]` 并提示使用 `/dev-team:phase-code-review`，删除后用户可在 CLI 面板中看到提示 |
| 行数目标 60 行过严导致必需逻辑被裁剪 | 技能功能不完整 | 低 | 每个技能附加行数注释；如果因必需逻辑无法精简，调整目标而非裁剪逻辑 |

---

## 迁移步骤

1. 备份当前 7 个技能文件和 3 个待删除的 agent 文件（通过 git commit 保存原始版本）
2. 逐个重写 7 个技能文件为纯编排器模式，每次修改后验证行数 <= 60、eval-check 调用、单行 prompt
3. 删除废弃的 `code-review` skill 目录
4. 删除废弃的 `code-review.md` agent
5. 删除废弃的 `requirements-planner.md` agent
6. 删除废弃的 `eval-check.py`
7. 修正 `implementation-evaluator.md` 中的 eval-log 示例为 `05-implement`
8. 修正 `openspec-archive-change/SKILL.md` 中的 eval-check 调用为 `dev-team eval-check`
9. 运行验证脚本确认全部 AC 覆盖
10. 在 Claude Code 中手动执行 `/dev-team:phase-test-design` 和 `/dev-team:phase-requirements` 进行运行时确认

---

## 待决问题

- 无 — 所有设计决策已在 proposal.md 中充分讨论，本设计文档基于方案 B 共识完成
