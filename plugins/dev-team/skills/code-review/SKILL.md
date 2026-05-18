---
name: code-review
description: |
  [DEPRECATED] This skill is migrated to the PGE workflow. Use /dev-team:phase-code-review for evaluator-only code review against design.md with static checklist. This skill remains functional during migration but will be removed in a future update.
disable-model-invocation: true
license: MIT
---

Route code review to the dedicated code-review subagent.

## Usage

```
/code-review
```

## Process

Use the Agent tool to spawn the code-review subagent:

```
Agent({
  description: "Code review staged changes",
  subagent_type: "code-review",
  prompt: "<tailored review prompt>"
})
```

The subagent will:
1. Get staged changes via `git diff --cached`
2. Read relevant source files for context
3. Analyze for logical errors, null/boundary handling, redundant logic, and security issues
4. Generate a structured review report
5. Save the report to `openspec/changes/<change-name>/test-reports/code-review-<timestamp>.md`

## Prompt Guidance

Tailor the prompt based on the user's request. If they asked generically (/code-review), the prompt can be brief: "Review staged changes. Focus on logic, null handling, redundancy, and security."

If the user mentions specific concerns, include them in the prompt:
- "Review staged changes. The user is concerned about database connection handling."
- "Review staged changes. Focus on error handling in the new API endpoint."
