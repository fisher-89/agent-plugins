## 权威边界

归档流与 eval 结果落盘约定。回溯路由见 `pipeline-backtrack`（决议 C3=A）；phase ID 无数字前缀（见 `pge-workflow-engine`）。

## ADDED Requirements

### Requirement: Commit gate is removed

系统 SHALL NOT 在 commit 时做 evaluation 校验。归档流末步的 git commit 不依赖已删除的 commit-gate hooks。

#### Scenario: Commit proceeds without eval gate

- **WHEN** archive 流触发 git commit
- **THEN** hooks 不对 eval JSON / code review 做门禁拦截

### Requirement: Archive flow executes sequentially

归档流严格顺序：

1. **eval 链校验**：确认各 phase 最新有效条目为 pass（见 `eval-check-cli`）
2. **openspec archive**：`openspec archive <change-name>`
3. **git commit**

步骤 1 失败则不得执行 2、3。

#### Scenario: 全通过则归档完成

- **WHEN** 校验通过且 archive、commit 成功
- **THEN** change 已归档

#### Scenario: 校验失败则中止

- **WHEN** 校验非零退出
- **THEN** 不执行 archive / commit，并输出失败原因

### Requirement: Eval results stored in eval.json

Evaluator 经 `phase_log` 追加到 `openspec/changes/<name>/eval.json`（单文件数组）。条目字段以现行 schema 为准（含 `phase`、`timestamp`、`attempt`、`verdict`、`report`、`items` 等）。

`backtrack_to` / `backtrack_reason` 若出现在条目中，SHALL 仅由 `backtrack` 工具事后写入，不得由 evaluator/`phase_log` 在创建时设置。phase 使用无前缀 ID（如 `acceptance`、`proposal`）。

#### Scenario: code-review 发现写入 checklist

- **WHEN** code-review-evaluator 发现安全问题
- **THEN** 以 checklist item（pass=false + evidence）经 `phase_log` 追加；条目不设 `backtrack_to`

#### Scenario: acceptance 缺口只写诊断

- **WHEN** acceptance-evaluator 发现未满足 AC
- **THEN** 经 `phase_log` 追加 fail 与证据；由 skill 决定是否调 `backtrack`（见 `pipeline-backtrack`）

### Requirement: Hook / 用户可见摘要（可选）

若 hook 展示评估摘要，SHALL 使用现行 phase ID，并可在条目已被 `backtrack` 改写后展示目标 phase。MUST NOT 要求 evaluator 在返回时已设置 `backtrack_to`。

#### Scenario: fail 摘要

- **WHEN** 最新条目 `verdict` 为 fail，phase 为 `acceptance`
- **THEN** 摘要可形如 `Phase acceptance: FAIL (n/m)`；若已 backtrack，可附加目标如 `→ proposal`

## REMOVED Requirements

### Requirement: Hook files import hook-output module directly
**Reason**: UserPromptSubmit hook 已移除。  
**Migration**: 仅保留仍存在的 PreToolUse / SessionStart 等 hook 的导入要求（若适用）。
