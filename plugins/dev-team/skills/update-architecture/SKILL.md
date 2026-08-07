---
name: __SKILL:update-architecture__
description: Update the C4 architecture model.
disable-model-invocation: true
license: MIT
---

Route architecture requests to the dedicated architecture subagent.

## Usage

```
__SKILL_SLASH:update-architecture__
```

## Process

Use the Agent tool to spawn the architecture subagent:

```
Agent({
  description: "...",
  subagent_type: "__AGENT:architecture__",
  prompt: "Propose architecture changes: <user request>"
})
```

This skill only supports PROPOSE mode: Read models and code, draft DSL changes, validate, present diff for confirmation.
