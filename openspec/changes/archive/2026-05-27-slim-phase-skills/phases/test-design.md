# 测试设计: slim-phase-skills

> **变更**: slim-phase-skills
> **日期**: 2026-05-27
> **基于**: proposal.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 验证脚本 | 7 个阶段技能文件的结构属性（行数、eval-check CLI 调用、prompt 行数、Phase Pattern 章节移除）、废弃文件/目录删除确认、阶段代码一致性检查、CLI 引用检查 | Python 3 (unittest + re 模块) | 100% 覆盖 AC-01 ~ AC-10 的静态可验证属性，每项至少一个断言 |
| 手动测试 | Skill 在 Claude Code 运行时的加载和可用性、phase-requirements 交互式 change name 解析功能 | Claude Code 运行时手动执行 | AC-11 和 AC-12 分别执行一次，确认技能响应正常 |

> **注**：本变更为纯重构变更 — 不引入新功能，删除废弃文件（code-review skill/agent、eval-check.py、requirements-planner.md），不修改行为逻辑。测试重心放在静态产物验证上。由于 eval-check.py 被删除、无其他 Python/TypeScript 模块变更，本变更不需要单元测试或集成测试。

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-01 | `openspec/changes/slim-phase-skills/tests/verify_skill_properties.py` | 验证脚本 | 行数 — 每个技能 `wc -l` <= 60 |
| AC-02 | `openspec/changes/slim-phase-skills/tests/verify_skill_properties.py` | 验证脚本 | 调用 — `eval-check --change` 引用存在 |
| AC-03 | `openspec/changes/slim-phase-skills/tests/verify_skill_properties.py` | 验证脚本 | Prompt 结构 — 所有 Agent 调用 prompt 为单行 |
| AC-04 | `openspec/changes/slim-phase-skills/tests/verify_deleted_files.py` | 验证脚本 | 目录存在性 — `code-review/` 目录已删除 |
| AC-05 | `openspec/changes/slim-phase-skills/tests/verify_deleted_files.py` | 验证脚本 | 文件存在性 — `code-review.md` agent 已删除 |
| AC-06 | `openspec/changes/slim-phase-skills/tests/verify_deleted_files.py` | 验证脚本 | 文件存在性 — `eval-check.py` 已删除 |
| AC-06b | `openspec/changes/slim-phase-skills/tests/verify_deleted_files.py` | 验证脚本 | 文件存在性 — `requirements-planner.md` 已删除（proposal scope item 4） |
| AC-07 | `openspec/changes/slim-phase-skills/tests/verify_phase_code_consistency.py` | 验证脚本 | 字符串匹配 — `implementation-evaluator.md` 使用 `05-implement` 而非 `05-implementation` |
| AC-07a | `openspec/changes/slim-phase-skills/tests/verify_phase_code_consistency.py` | 验证脚本 | CLI 引用 — `openspec-archive-change/SKILL.md` 使用 `dev-team eval-check` 而非 `eval-check.py` |
| AC-08 | `openspec/changes/slim-phase-skills/tests/verify_skill_properties.py` | 验证脚本 | 章节存在性 — 无 `## DESIGN/EXECUTION/EVALUATOR-ONLY Phase Pattern` 章节 |
| AC-09 | `openspec/changes/slim-phase-skills/tests/verify_skill_properties.py` | 验证脚本 | AUTO 命令保留 — `phase-implement` 包含 `lint-runner.py` 调用 |
| AC-10 | `openspec/changes/slim-phase-skills/tests/verify_skill_properties.py` | 验证脚本 | 逻辑保留 — `phase-requirements` 包含 explore context 检测决策树 |
| AC-11 | 手动执行：`/dev-team:phase-test-design`（空运行） | 手动测试 | 功能确认 — 技能加载正常、响应正确 |
| AC-12 | 手动执行：`/dev-team:phase-requirements`（不带参数） | 手动测试 | 功能确认 — 进入 explore context 检测流程 |

> **注**：AC-06b 是 proposal scope item 4（删除 `requirements-planner.md`）对应的隐式验收条件，虽然没有独立的 AC 编号，但在验证脚本中一并覆盖以保证 scope 完整性。

---

## 3. 测试策略

### 3.1 方法

本变更是纯重构变更：7 个技能文件内容被精简（逻辑从技能层迁移到 eval-check CLI 和 agent.md），4 个废弃文件/目录被删除，1 个字符串在 implementation-evaluator.md 中被修正，1 个 CLI 引用在 openspec-archive-change 中被修正。行为不变。

采取 **静态验证 + 手动确认** 双层策略：

1. **静态验证层** — Python 脚本对文件系统执行属性检查（行数、grep 模式匹配、路径存在性）。这一层捕获除运行时行为外的所有结构性违规，可在 CI 中全自动执行且零依赖。包括：
   - 7 个技能 SKILL.md 的结构属性
   - 4 个废弃文件/目录的删除确认
   - 阶段代码一致性（implementation-evaluator.md）
   - CLI 引用一致性（openspec-archive-change/SKILL.md）
   - 项目中残留引用的扫描

2. **手动确认层** — 开发者加载 Claude Code 后手动执行 `/dev-team:phase-test-design` 和 `/dev-team:phase-requirements`，确认技能可加载且交互逻辑正常。

### 3.2 测试分类

- **验证脚本**（共 3 个）：
  - `verify_skill_properties.py`：读取 7 个 `SKILL.md` 文件，逐项检查：
    - 行数（<= 60）
    - `eval-check --change` 调用存在性
    - Agent prompt 为单行字符串
    - 无 `## DESIGN Phase Pattern` / `## EXECUTION Phase Pattern` / `## EVALUATOR-ONLY Pattern` 章节残留
    - `phase-implement` 中 `lint-runner.py` 调用保留
    - `phase-requirements` 中 explore context 检测保留（Branch A/B 决策树）
    - 无对已删除 agent（`requirements-planner`）的引用
  - `verify_deleted_files.py`：通过 `os.path.isdir()` / `os.path.isfile()` 确认以下废弃文件/目录已被删除：
    - `plugins/dev-team/skills/code-review/`（目录）
    - `plugins/dev-team/agents/code-review.md`
    - `plugins/dev-team/utils/eval-check.py`
    - `plugins/dev-team/agents/requirements-planner.md`
    - 额外扫描项目中是否有对 `/code-review` 命令的残留引用
  - `verify_phase_code_consistency.py`：
    - 确认 `plugins/dev-team/agents/implementation-evaluator.md` 中的 eval-log 示例使用 `05-implement`（而非 `05-implementation`）
    - 确认 `plugins/dev-team/skills/openspec-archive-change/SKILL.md` 使用 `dev-team eval-check` CLI 调用（而非 `python plugins/dev-team/utils/eval-check.py`）

- **手动测试**：
  - AC-11：在 Claude Code 中执行 `/dev-team:phase-test-design`（可带已有 change name），确认技能进入 PGE 流程而非报错
  - AC-12：在 Claude Code 中执行 `/dev-team:phase-requirements` 不带参数，确认进入交互式 change name 解析流程（先检测 explore context 再询问）

> **注意**: 不再使用端到端测试 (E2E)。所有测试均使用 mock，不依赖真实外部环境。

### 3.3 模拟策略

| 被测对象 | 模拟策略 | 说明 |
|---------|----------|------|
| `verify_skill_properties.py` | 无模拟，直接读取真实文件系统 | 验证脚本的测试对象就是项目目录中的 SKILL.md 文件本身 |
| `verify_deleted_files.py` | 无模拟，直接检查文件系统 | 确认文件/目录存在性是验证脚本的核心目的；残留引用扫描使用纯文本 grep |
| `verify_phase_code_consistency.py` | 无模拟，直接读取文件内容 | 字符串匹配直接针对项目文件 |
| AC-11 / AC-12（手动测试） | 在 Claude Code 真实运行时环境中执行 | 不模拟，属于人工验收 |

> 所有验证脚本均不涉及 mock，因为它们直接检查文件系统的静态属性。无需模拟外部服务或数据库。

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| 技能行数恰好为 60 行（含 YAML frontmatter） | `len(lines) == 60` | 判定为通过（<= 60 含等于），不报错 | `verify_skill_properties.py` |
| 技能行数超过 60 行 | `len(lines) == 61` 或更多 | 报错并列出超行数的技能名称和实际行数 | `verify_skill_properties.py` |
| 技能行数较少（精简到极致） | e.g. `phase-code-review` 仅 ~25 行 | 通过，不设下限 | `verify_skill_properties.py` |
| Agent prompt 跨多行（包含换行符） | `prompt: "line1\nline2"` 或 YAML 多行折叠 | 判定为违规，报错列出对应技能 | `verify_skill_properties.py` |
| `phase-implement` 的 AUTO 阶段 `lint-runner.py` 调用缺失 | 技能中无 `lint-runner.py` 字符串 | 报错：AUTO 命令缺失 | `verify_skill_properties.py` |
| `phase-requirements` 缺少 Branch A/B 决策树 | 技能中无 `Branch A` 或 `explore context` 等特征字符串 | 报错：explore context 检测逻辑缺失 | `verify_skill_properties.py` |
| `requirements-planner.md` 被删除但 `phase-requirements` 技能仍引用了 `requirements-planner` subagent | 文件已不存在，但技能中仍有 `subagent_type: "requirements-planner"` 或类似 | `verify_skill_properties.py` 检测到技能中引用已删除的 agent（通过 grep 确认为 `requirements-planner`），报错 | `verify_skill_properties.py` |
| `eval-check.py` 被删除但项目中有其他文件仍引用该路径 | 非 openspec-archive-change 的文件中包含 `eval-check.py` | `verify_deleted_files.py` 扫描项目中的残留引用并报出 | `verify_deleted_files.py` |
| `code-review` skill 目录被删除但 `code-review.md` agent 仍存在 | 目录已删除，文件保留 | `verify_deleted_files.py` 仅报错文件残留，两个检查独立不连锁 | `verify_deleted_files.py` |
| `code-review` agent 被删除但 skill 目录仍存在 | 文件已删除，目录保留 | `verify_deleted_files.py` 仅报错目录残留，两个检查独立不连锁 | `verify_deleted_files.py` |
| 仅 `implementation-evaluator.md` 修正而 `openspec-archive-change` 未修正 | `implementation-evaluator.md` 使用 `05-implement`，`openspec-archive-change/SKILL.md` 仍引用 `eval-check.py` | `verify_phase_code_consistency.py` 分别检查，仅 openspec-archive-change 报错 | `verify_phase_code_consistency.py` |
| `implementation-evaluator.md` 中同时存在 `05-implement` 和 `05-implementation` 混合使用 | 文件包含 `05-implement` 但也包含 `05-implementation` | 正则检查发现 `05-implementation` 字符串，报错 | `verify_phase_code_consistency.py` |
| 项目中存在其他 `.md` 文件仍引用 `/code-review` 命令（非 `phase-code-review`） | 非 phase-code-review 的文件中包含 `/code-review` 或 `skills/code-review/` | `verify_deleted_files.py` 扫描并报出残留引用列表 | `verify_deleted_files.py` |
| `openspec-archive-change/SKILL.md` 使用正确的 `dev-team eval-check` 但拼写错误或参数错误 | 包含 `dev-team eval-check` 但缺少 `--change` 或 `--phase` 参数 | `verify_phase_code_consistency.py` 通过正则 `dev-team eval-check` 确认基本引用，但不验证参数完整性（参数验证依赖运行时） | `verify_phase_code_consistency.py` |
| 手动测试：`/dev-team:phase-test-design` 带存在的 change name | 执行空运行 | Skill 正常加载，显示 Usage 说明，执行 eval-check 门控验证 | （手动测试） |
| 手动测试：`/dev-team:phase-requirements` 不带参数 | 无参数 | 进入交互式流程，先检测 explore context 再询问 change name | （手动测试） |

---

## 5. 测试数据

### 5.1 验证脚本测试数据

验证脚本不使用独立 fixture 数据。测试数据即项目文件系统本身：

```
plugins/dev-team/skills/phase-requirements/SKILL.md
plugins/dev-team/skills/phase-test-design/SKILL.md
plugins/dev-team/skills/phase-dev-proposal/SKILL.md
plugins/dev-team/skills/phase-implement/SKILL.md
plugins/dev-team/skills/phase-test-gen/SKILL.md
plugins/dev-team/skills/phase-code-review/SKILL.md
plugins/dev-team/skills/phase-acceptance/SKILL.md
plugins/dev-team/agents/implementation-evaluator.md
plugins/dev-team/skills/openspec-archive-change/SKILL.md
```

验证脚本运行前确保当前工作目录是项目根目录（`D:\Projects\wps-claude-plugin`），所有文件路径使用相对路径。

### 5.2 检查列表定义

验证脚本中定义的静态检查清单：

`verify_skill_properties.py` 的检查维度：
- **行数**：`wc -l` 结果 <= 60
- **eval-check CLI 引用**：文件中包含 `eval-check --change` 字符串
- **单行 prompt**：所有 `prompt:` 字段对应的 value 不包含换行符
- **Phase Pattern 章节**：文件中不包含 `## DESIGN Phase Pattern`、`## EXECUTION Phase Pattern`、`## EVALUATOR-ONLY Pattern`
- **AUTO 命令保留**（仅 `phase-implement`）：包含 `lint-runner.py` 
- **Explore context 保留**（仅 `phase-requirements`）：包含 `Branch A` 和 `Branch B` 特征标记
- **无残留 agent 引用**：不包含 `requirements-planner` 作为 subagent 引用

`verify_deleted_files.py` 的文件删除清单：
| 待删除路径 | 类型 | 检查方法 |
|-----------|------|----------|
| `plugins/dev-team/skills/code-review/` | 目录 | `os.path.isdir()` == False |
| `plugins/dev-team/agents/code-review.md` | 文件 | `os.path.isfile()` == False |
| `plugins/dev-team/utils/eval-check.py` | 文件 | `os.path.isfile()` == False |
| `plugins/dev-team/agents/requirements-planner.md` | 文件 | `os.path.isfile()` == False |

`verify_phase_code_consistency.py` 的一致性检查清单：
| 检查目标 | 模式 | 不应包含 |
|----------|------|----------|
| `plugins/dev-team/agents/implementation-evaluator.md` | 应包含 `05-implement` 用于 eval-log 示例 | `05-implementation`（旧值） |
| `plugins/dev-team/skills/openspec-archive-change/SKILL.md` | 应包含 `dev-team eval-check` | `eval-check.py`（旧引用路径） |

### 5.3 阶段顺序定义

验证中使用的工作流阶段顺序（与 `workflow.ts` PHASES 数组对齐）：

```
01-requirements, 02-test-design, 03-dev-proposal, 04-test-gen,
05-implement, 06-code-review, 07-acceptance
```

---

## 6. 不可测试项

- Skill 在 Claude Code 运行时的实际行为（AC-11、AC-12 的完整流程） — **原因**: Skill 的执行依赖 Claude Code 的 Agent 运行时，包括模型调用、工具执行、对话历史交互等，无法在自动化 CI 环境中复现。AC-11 和 AC-12 通过手动测试验证，自动化层仅覆盖静态属性检查。
- Agent prompt 极简化后 subagent 输出的质量 — **原因**: subagent 是否因 prompt 信息不足产生错误输出取决于模型行为，不属于确定性测试范畴。通过 spec.md 中的 agent.md 领域知识确认和精简前的 `git diff` 审查来缓解风险。
- Skill 编排器的 Verdict loop 在达到 5 次上限时的行为 — **原因**: verdict loop 是技能运行时逻辑，涉及 Agent 调用和 eval.json 读写。自动化测试无法模拟 Agent 调用的往返。该行为依赖 spec.md 中"最多 5 次尝试"的约束定义。
- 删除废弃 skill/agent 后用户侧的体验 — **原因**: 用户运行 `/code-review` 时收到的"skill not found"提示由 Claude Code 运行时处理，超出本变更控制范围。已在 proposal.md 中标明风险。
- 每个 agent.md 是否真正承载了从技能中移除的领域知识 — **原因**: agent.md 的内容完整性检查是人工审查工作（proposal.md 中列出了 6 个待确认的 agent.md），自动化脚本无法判断知识的完整性和准确性。通过 `git diff` 审查和人工确认覆盖。
- `dev-team eval-check` CLI 命令本身的正确性 — **原因**: 根据 proposal 的"不要修改"范围，CLI 实现不在本变更范围内。测试仅验证技能层是否正确调用了该 CLI，不测试 CLI 自身的门控逻辑。
