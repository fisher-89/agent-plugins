## ADDED Requirements

### Requirement: generator Process 结束前引用 static-analysis-gate include

源文件 `plugins/dev-team/agents/implementation-generator.md` 与 `plugins/dev-team/agents/test-gen-generator.md` SHALL 在各自 `## Process` 结束前（全部既有步骤之后）包含字面量：

```
__INCLUDE:static-analysis-gate__
```

该引用 SHALL 作为构建期片段注入点，而非在源文件中手写平台分支正文。Claude 组装后该处展开为空；Cursor / cursorHome 组装后展开为 `run_static_analysis` 软门禁（见 `include-fragments`）。

两 generator MUST NOT 在源正文中再次嵌入与 Claude `SubagentStop` hook 重复的硬编码静态检查步骤清单（避免 Claude 双重要求）；门禁差异仅通过 include 的平台档实现。

#### Scenario: implementation-generator 源含 include

**WHEN** 读取 `plugins/dev-team/agents/implementation-generator.md`
**THEN** `## Process` 区域内 SHALL 包含 `__INCLUDE:static-analysis-gate__`
**AND** 该标记位于既有 Process 步骤之后

#### Scenario: test-gen-generator 源含 include

**WHEN** 读取 `plugins/dev-team/agents/test-gen-generator.md`
**THEN** `## Process` 区域内 SHALL 包含 `__INCLUDE:static-analysis-gate__`
**AND** 该标记位于既有 Process 步骤之后

#### Scenario: Claude 组装后 generator 无软门禁步骤

**WHEN** assemble 完成 `claude` 产物
**AND** 读取产物中对应 implementation-generator / test-gen-generator 文件
**THEN** 文件 SHALL NOT 因该 include 含有 `run_static_analysis` 结束前门禁段落

#### Scenario: Cursor 组装后 generator 含软门禁步骤

**WHEN** assemble 完成 `cursor` 或 `cursorHome` 产物
**AND** 读取产物中对应 implementation-generator / test-gen-generator 文件（含 `namePrefix` 重命名后的文件名）
**THEN** 文件 SHALL 含结束前执行 `run_static_analysis` 并在未通过时不得结束的说明

## Module Contract

### Agent 源：`implementation-generator` / `test-gen-generator`

| 方面 | 描述 |
|------|------|
| **注入点** | `## Process` 末尾 `__INCLUDE:static-analysis-gate__` |
| **Claude 行为** | include → 空；硬门禁仍由 `SubagentStop` + `static-check` |
| **Cursor 行为** | include → 软门禁文案；无 `subagentStop` hook |
| **禁止** | 源文件手写双端重复静态检查长文；多 fragments 根 |
