---
name: update-architecture
description: Update the C4 architecture model, validate code against it, create ADRs, or review model quality.
disable-model-invocation: true
license: MIT
---

Route architecture requests to the dedicated architecture subagent.

## Usage

```
/dev-team:update-architecture
```

## Process

Use the Agent tool to spawn the architecture subagent:

```
Agent({
  description: "...",
  subagent_type: "architecture",
  prompt: "<tailored prompt based on user intent>"
})
```

The subagent will handle four modes based on the user's intent:

- **PROPOSE**: Read models and code, draft DSL changes, validate, present diff for confirmation
- **VALIDATE**: Run archi-validate.py on staged or specified files, explain violations
- **DECIDE**: Help create/list/update Architecture Decision Records via archi-decide.py
- **REVIEW**: Read all model files, critique completeness/consistency/coupling

## Prompt Guidance

Tailor the prompt based on the user's request. Parse user intent to determine the mode:

- "add/update/model/propose" → PROPOSE: Include the user's full request and relevant context. "Propose architecture changes: <user request>"
- "validate/check/verify" → VALIDATE: "Validate staged code against the architecture model. Explain any violations."
- "decide/adr/decision" → DECIDE: "Help create an ADR: <user request>"
- "review/critique/quality" → REVIEW: "Review the architecture model for completeness, consistency, and coupling."

If the user's intent is ambiguous, ask which mode they want.
