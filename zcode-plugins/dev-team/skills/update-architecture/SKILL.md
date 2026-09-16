---
name: update-architecture
description: Update the C4 architecture model.
disable-model-invocation: true
license: MIT
---

Route architecture requests to the dedicated architecture subagent.

## Usage

```
dev-team:update-architecture
```

## Process

Use the Agent tool to spawn the architecture subagent:

```
Agent({
  description: "...",
  subagent_type: "dev-team:architecture",
  prompt: "Propose architecture changes: <user request>"
})
```

This skill only supports PROPOSE mode: Read models and code, draft DSL changes, validate, present diff for confirmation.
