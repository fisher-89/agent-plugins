# 任务: slim-phase-skills

> **变更**: slim-phase-skills
> **日期**: 2026-05-27
> **基于**: design.md

---

## 阶段 0: 前置准备

- [x] 确认当前工作目录是项目根目录 `D:\Projects\wps-claude-plugin`
- [x] 确认 `plugins/dev-team/agents/` 中 6 个 agent.md 已承载领域知识（模板路径、输出路径、Input/Process/Output/Constraints），无需修改：
  - `test-design-planner.md` — 包含 proposal.md 输入路径、模板路径、test-design.md 输出路径
  - `dev-proposal-planner.md` — 包含 proposal.md + test-design.md 输入路径、模板路径、design.md + tasks.md 输出路径
  - `implementation-generator.md` — 包含 design.md + tasks.md 输入路径、代码写入规则
  - `test-gen-generator.md` — 包含 test-design.md 输入路径、输出路径、文件类型黑名单
  - `code-review-evaluator.md` — 包含 design.md 路径引用
  - `acceptance-evaluator.md` — 包含 proposal.md 路径引用
- [x] 阅读当前 7 个技能文件并理解其完整逻辑，确保精简时不遗漏任何必需行为
- [x] 提交一个 git commit 保存所有当前技能文件和待删除文件的原始版本作为回滚点

---

## 阶段 1: 修正废弃引用和阶段代码

- [x] 修正 `plugins/dev-team/agents/implementation-evaluator.md` 中的 eval-log 示例：将 `--phase 05-implementation` 改为 `--phase 05-implement`
- [x] 修正 `plugins/dev-team/skills/openspec-archive-change/SKILL.md` 中的 eval-check 调用：将 `python plugins/dev-team/utils/eval-check.py --change "<name>" --project-root . --json` 改为 `dev-team eval-check --change "<name>" --phase 09-acceptance --json`

---

## 阶段 2: 重写 7 个技能文件为纯编排器模式

按照依赖顺序（无严格依赖，可并行），每个技能重写后行数不超过 60 行。

### phase-requirements （主 agent 直接执行，保留 Branch A/B）

- [x] 重写 `plugins/dev-team/skills/phase-requirements/SKILL.md`：
  - YAML frontmatter: 保留 name、description、license、disable-model-invocation、metadata
  - Usage 说明
  - Step 1: Change name 解析 — 保留完整的 Branch A（有参数：validate + scaffold）和 Branch B（无参数：explore context 检测 + 交互式询问）决策树
  - Step 2: 门控验证 — `dev-team eval-check --change "<name>" --phase 01-requirements`
  - Step 3: 主 agent 直接撰写 proposal.md + specs/（不启动 subagent，由 Skill 中的自然语言指令引导主 agent 工具调用）
  - Step 4: Evaluator — `Agent({ subagent_type: "requirements-evaluator", prompt: "单行" })`
  - Step 5: Verdict 循环 — 读取 eval.json，fail 则回到 Step 3
  - Step 6: 报告结果
  - 不包含 `## DESIGN Phase Pattern` 段落
  - 不包含详细的 Process 步骤描述（已在 agent.md 中）
  - 确认行数不超过 60 行

### phase-test-design （Planner subagent）

- [x] 重写 `plugins/dev-team/skills/phase-test-design/SKILL.md`：
  - YAML frontmatter: 保留 name、description、license、disable-model-invocation、metadata
  - Usage 说明
  - Step 1: Change name 解析
  - Step 2: 门控验证 — `dev-team eval-check --change "<name>" --phase 02-test-design`
  - Step 3: P→E 循环 — `Agent({ subagent_type: "test-design-planner", prompt: "单行" })` → `Agent({ subagent_type: "test-design-evaluator", prompt: "单行" })`
  - Step 4: Verdict 检查
  - Step 5: 报告结果
  - 不包含 `## DESIGN Phase Pattern` 段落
  - 确认行数不超过 60 行

### phase-dev-proposal （Planner subagent）

- [x] 重写 `plugins/dev-team/skills/phase-dev-proposal/SKILL.md`：
  - YAML frontmatter: 保留 name、description、license、disable-model-invocation、metadata
  - Usage 说明
  - Step 1: Change name 解析
  - Step 2: 门控验证 — `dev-team eval-check --change "<name>" --phase 03-dev-proposal`
  - Step 3: P→E 循环 — `Agent({ subagent_type: "dev-proposal-planner", prompt: "单行" })` → `Agent({ subagent_type: "dev-proposal-evaluator", prompt: "单行" })`
  - Step 4: Verdict 检查
  - Step 5: 报告结果
  - 不包含 `## DESIGN Phase Pattern` 段落
  - 确认行数不超过 60 行

### phase-test-gen （Generator subagent）

- [x] 重写 `plugins/dev-team/skills/phase-test-gen/SKILL.md`：
  - YAML frontmatter: 保留 name、description、license、disable-model-invocation、metadata
  - Usage 说明
  - Step 1: Change name 解析
  - Step 2: 门控验证 — `dev-team eval-check --change "<name>" --phase 04-test-gen`
  - Step 3: G→E 循环 — `Agent({ subagent_type: "test-gen-generator", prompt: "单行" })` → `Agent({ subagent_type: "test-gen-evaluator", prompt: "单行" })`
  - Step 4: Verdict 检查
  - Step 5: 报告结果
  - 不包含 `## EXECUTION Phase Pattern` 段落
  - 确认行数不超过 60 行

### phase-implement （Generator subagent + AUTO）

- [x] 重写 `plugins/dev-team/skills/phase-implement/SKILL.md`：
  - YAML frontmatter: 保留 name、description、license、disable-model-invocation、metadata
  - Usage 说明
  - Step 1: Change name 解析
  - Step 2: 门控验证 — `dev-team eval-check --change "<name>" --phase 05-implement`
  - Step 3: G→E+AUTO 循环 — `Agent({ subagent_type: "implementation-generator", prompt: "单行" })` → AUTO: `python plugins/dev-team/utils/lint-runner.py ...` → `Agent({ subagent_type: "implementation-evaluator", prompt: "单行" })`
  - Step 4: Verdict 检查
  - Step 5: 报告结果
  - 不包含 `## EXECUTION Phase Pattern` 段落
  - 确认行数不超过 60 行

### phase-code-review （Evaluator-only）

- [x] 重写 `plugins/dev-team/skills/phase-code-review/SKILL.md`：
  - YAML frontmatter: 保留 name、description、license、disable-model-invocation、metadata
  - Usage 说明
  - Step 1: Change name 解析
  - Step 2: 门控验证 — `dev-team eval-check --change "<name>" --phase 07-code-review`
  - Step 3: 单次 Evaluator — `Agent({ subagent_type: "code-review-evaluator", prompt: "单行" })`
  - Step 4: 读取 eval.json 检查 backtrack_to
  - Step 5: 报告结果（含 backtrack 建议）
  - 不包含 `## EVALUATOR-ONLY Pattern` 段落
  - 确认行数不超过 60 行

### phase-acceptance （Evaluator-only）

- [x] 重写 `plugins/dev-team/skills/phase-acceptance/SKILL.md`：
  - YAML frontmatter: 保留 name、description、license、disable-model-invocation、metadata
  - Usage 说明
  - Step 1: Change name 解析
  - Step 2: 门控验证 — `dev-team eval-check --change "<name>" --phase 09-acceptance`
  - Step 3: 单次 Evaluator — `Agent({ subagent_type: "acceptance-evaluator", prompt: "单行" })`
  - Step 4: 读取 eval.json 检查 backtrack_to
  - Step 5: 报告结果（含 backtrack 建议）
  - 不包含 `## EVALUATOR-ONLY Pattern` 段落
  - 确认行数不超过 60 行

---

## 阶段 3: 删除废弃文件

- [x] 删除废弃的 `code-review` skill 目录：`rm -rf plugins/dev-team/skills/code-review/`
- [x] 删除废弃的 `code-review.md` agent：`rm plugins/dev-team/agents/code-review.md`
- [x] 删除废弃的 `requirements-planner.md` agent：`rm plugins/dev-team/agents/requirements-planner.md`
- [x] 删除冗余的 `eval-check.py`：`rm plugins/dev-team/utils/eval-check.py`

---

## 阶段 4: 验证

- [x] 编写验证脚本 `openspec/changes/slim-phase-skills/tests/verify_skill_properties.py`：
  - 检查 7 个技能 SKILL.md 行数 <= 60
  - 检查每个技能包含 `eval-check --change` 调用
  - 检查每个技能中所有 Agent prompt 为单行字符串（不含换行符）
  - 检查无 `## DESIGN Phase Pattern` / `## EXECUTION Phase Pattern` / `## EVALUATOR-ONLY Pattern` 章节残留
  - 检查 `phase-implement` 包含 `lint-runner.py` 调用
  - 检查 `phase-requirements` 包含 `Branch A` 和 `Branch B` 特征标记
  - 检查无对 `requirements-planner` 作为 subagent 的引用
- [x] 编写验证脚本 `openspec/changes/slim-phase-skills/tests/verify_deleted_files.py`：
  - 确认 `plugins/dev-team/skills/code-review/` 目录不存在
  - 确认 `plugins/dev-team/agents/code-review.md` 文件不存在
  - 确认 `plugins/dev-team/utils/eval-check.py` 文件不存在
  - 确认 `plugins/dev-team/agents/requirements-planner.md` 文件不存在
  - 扫描项目中其他文件是否有残留引用（`/code-review`、`eval-check.py` 等）
- [x] 编写验证脚本 `openspec/changes/slim-phase-skills/tests/verify_phase_code_consistency.py`：
  - 确认 `implementation-evaluator.md` 使用 `05-implement`（不包含 `05-implementation`）
  - 确认 `openspec-archive-change/SKILL.md` 使用 `dev-team eval-check`（不包含 `eval-check.py`）
- [x] 运行所有验证脚本，确认 100% 通过：
  - `python openspec/changes/slim-phase-skills/tests/verify_skill_properties.py`
  - `python openspec/changes/slim-phase-skills/tests/verify_deleted_files.py`
  - `python openspec/changes/slim-phase-skills/tests/verify_phase_code_consistency.py`

---

## 阶段 5: 人工确认

- [x] 手动执行 `/dev-team:phase-test-design`（空运行，可带已有 change name），确认技能加载正常、eval-check 门控运行、Usage 说明显示
- [x] 手动执行 `/dev-team:phase-requirements`（不带参数），确认进入 explore context 检测流程（Branch B），交互式询问正常
- [x] 确认 `plugins/dev-team/agents/code-review.md` 已删除后没有残留的外部引用

---

## 附件: 每个技能的目标行数预期

| 技能 | 当前行数 | 精简体例行数 | 预期行数 |
|------|---------|-------------|---------|
| phase-requirements | 231 | 保留 Branch A/B + 主 agent 直接执行 | ~50-60 |
| phase-test-design | 66 | P→E 编排器 | ~30-40 |
| phase-dev-proposal | 71 | P→E 编排器 | ~30-40 |
| phase-implement | 93 | G→E+AUTO 编排器 | ~40-50 |
| phase-test-gen | 81 | G→E 编排器 | ~30-40 |
| phase-code-review | 59 | E 编排器 | ~25-35 |
| phase-acceptance | 59 | E 编排器 | ~25-35 |
