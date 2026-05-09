# Pre-Archive Compliance Check

> **Change**: {{change_name}}
> **Date**: {{date}}
> **Result**: {{result}}

---

## Checklist

| ID | Check | Status | Details |
|----|-------|--------|---------|
{{#each checks}}
| {{check_id}} | {{name}} | {{status_icon}} | {{details}} |
{{/each}}

---

## Summary

- **Passed**: {{pass_count}}
- **Failed**: {{fail_count}}
- **Skipped**: {{skip_count}}

{{#if blocking_issues}}
## Blocking Issues

{{#each blocking_issues}}
{{@index}}. **[{{check_id}}] {{name}}**: {{details}}
{{/each}}
{{/if}}

## Verdict

{{#if passed}}
**PASS** — All compliance checks passed. Safe to archive.
{{else}}
**FAIL** — Fix {{blocking_issues.length}} blocking issue(s) before archiving.
{{/if}}

---

## Check Definitions

| ID | Check | Level | Description |
|----|-------|-------|-------------|
| C1 | proposal.md exists | MUST | 规范文档存在 |
| C2 | tasks.md exists | MUST | 任务清单存在 |
| C3 | All tasks complete | MUST | 所有 task 已勾选 (100% 完成率) |
| C4 | Test files exist | MUST | 至少一个测试文件存在 |
| C5 | Tests pass | MUST | 全量测试记录显示通过 |
| C6 | Code review exists | MUST | 有 code review 报告 |
| C7 | Review no ERROR | MUST | 最新 review 为 PASS 或无 ERROR |
| C8 | No uncommitted | MUST | `git status` 干净 |
| C9 | design.md exists | OPT | 配置驱动，可选检查 |
| C10 | Spec compliance | OPT | 实现覆盖 proposal scope，可选检查 |
