# phase-skills Specification（workflow-file-inventory 增量）

## ADDED Requirements

### Requirement: phase skill 范围核对文案对齐文件清单

`plugins/dev-team/skills/phase-implement/SKILL.md` 与 `plugins/dev-team/skills/phase-test-gen/SKILL.md` 中涉及「改了哪些文件 / 变更范围」的描述 SHALL 与文件清单机制对齐：

- 范围核对语义 SHALL 表述为 design 变更清单 × `workflow.json` 的 `files` 清单 × 文件系统对账（判定细则引用 `workflow-file-inventory`）
- MUST NOT 把 `git diff` / git 工作区状态描述为范围权威
- 文案 MUST NOT 指示用 Write/Edit 直接改 `workflow.json`（清单由 hook 记录，补录/修正走 `change_files`）

其余 skill（proposal / dev-design / test-design / test-execution / code-review / acceptance / workflow-* / openspec-*）SHALL NOT 引入清单相关新文案，保持既有行为。

#### Scenario: phase-implement 文案对齐

- **WHEN** 读取 `phase-implement/SKILL.md`
- **THEN** 范围相关描述以文件清单对账表述
- **AND** SHALL NOT 出现"用 git diff 核对本次改动"类指引

#### Scenario: phase-test-gen 文案对齐

- **WHEN** 读取 `phase-test-gen/SKILL.md`
- **THEN** 产出范围描述以清单与设计文档为准
- **AND** 不含 git 工作区即产出的表述

#### Scenario: 清单修正走 change_files

- **WHEN** 任一 skill 文案需要说明"清单与实际不符时如何修正"
- **THEN** 指引使用 MCP `change_files`（append/set）
- **AND** SHALL NOT 指引直接编辑 `workflow.json`

## Module Contract

### Skill 源（增量）

| 文件 | 变更 |
|------|------|
| `plugins/dev-team/skills/phase-implement/SKILL.md` | 范围核对文案改清单对账 |
| `plugins/dev-team/skills/phase-test-gen/SKILL.md` | 产出范围文案改清单 |
| 其余 SKILL.md | 不变 |
