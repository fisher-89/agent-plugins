## Context

当前 3 个 Planner agent（`requirements-planner`、`test-design-planner`、`dev-proposal-planner`）的 prompt 和模板全部为英文。它们生成的 4 个 .md 制品（proposal.md、test-design.md、design.md、tasks.md）是设计评审和团队协作的核心文档，需要切换为中文输出。

## Goals / Non-Goals

**Goals:**
- Planner agent 生成的 .md 制品的**正文内容**使用中文
- 模板文件（`.template`）的章节标题和标签使用中文
- `phase-agents` spec 明确约束输出语言

**Non-Goals:**
- 不修改 Evaluator agent（E1-E3）的 checklist 语言 —— LLM 多语言能力足以处理中文制品
- 不修改 Generator agent —— 它们生成代码而非文档
- 不修改已有 changes 下的历史制品
- 不翻译代码标识符、文件路径、CLI 命令、技术术语（API、JSON、SDK 等）

## Decisions

### D1: 在每个 Planner prompt 中增加独立语言约束

**决定**: 在每个 Planner agent 的 prompt 末尾增加 `## Language` 约束段，而非抽取共享配置。

**理由**: 3 个 Planner 各自生成不同类型的制品，且已有的 prompt 为独立自包含文件。增加独立约束保持每个 agent 定义的自包含性，比引入共享配置更简单。

**备选方案**: 在 template 中标注语言要求 —— 被拒绝，因为模板只约束结构不约束行为，语言约束应在 agent prompt 中。

### D2: 模板章节标题完整汉化

**决定**: 将 3 个 `.template` 文件的章节标题、表格列头、标签文字全部翻译为中文。模板变量（`{{change_name}}`、`{{date}}` 等）保持英文标识符不变。

**理由**: 模板是 Planner 的写作大纲，中文章节标题引导 Planner 输出中文内容。变量保持英文是因为它们是程序化替换的占位符，与内容语言无关。

### D3: 保持技术术语的英文形式

**决定**: 以下内容始终使用英文：
- 代码标识符（变量名、函数名、类名）
- 文件路径和 CLI 命令
- 广泛接受的技术缩写（API、JSON、SDK、CI/CD、URL 等）
- 模板变量（`{{change_name}}`）

**理由**: 这些术语在代码和终端中直接使用，翻译反而造成歧义。Planner prompt 中明确列出此规则。

### D4: Evaluator 不做修改

**决定**: 不修改 3 个 DESIGN Evaluator（requirements-evaluator、test-design-evaluator、dev-proposal-evaluator）的 checklist 或 prompt。

**理由**: Evaluator 的 checklist 关注的是内容完整性（"是否覆盖了所有 AC"、"是否列出了至少 3 个干系人"），与语言无关。LLM 的多语言能力足以用英文 checklist 评估中文制品。如果后续发现评估质量下降，再单独调整。

## Risks / Trade-offs

- **术语不一致风险**: Planner 可能自行翻译技术术语（如将 API 译为"接口"）→ 通过在 prompt 中明确列出保持英文的术语类型来缓解
- **Evaluator 评估偏差**: 英文 checklist 评估中文内容可能产生轻微偏差 → 影响较小（checklist 关注结构性而非语言质量），且可在后续迭代中观察调整
- **模板变量冲突**: 汉化模板时需确保不破坏变量占位符 → 通过代码审查和实际运行验证
