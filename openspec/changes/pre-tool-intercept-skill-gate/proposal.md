## Why

UserPromptSubmit hook 的 `additionalContext` 只是文本建议，Claude 经常忽略它而不调用推荐的 OpenSpec skill（如 `/openspec-explore`）。这导致意图路由器的工作白费，用户仍需手动输入 skill 命令。需要一种强制执行机制，确保 Claude 在应该使用 OpenSpec 工作流时必须先调用对应 skill。

## What Changes

- 新增 `PreToolUse` hook（匹配 `Write|Edit`），在 Claude 尝试写代码前检查是否存在未处理的 intent 路由建议
- 引入状态文件 `openspec/changes/<name>/.pending-skill.json`，由 UserPromptSubmit hook 写入，记录待执行的 skill 路由决策
- PreToolUse hook 读取该状态文件，若存在未处理的 skill 路由则 `deny` 并注入指令要求先调用对应 skill
- UserPromptSubmit hook 在输出 `additionalContext` 的同时写入路由状态文件
- Skill 调用完成后（通过 PreToolUse Skill hook）清除对应的 pending 状态

## Capabilities

### New Capabilities
- `skill-gate`: PreToolUse gate that intercepts Write/Edit when a pending skill route exists, denying the operation and instructing Claude to invoke the required skill first
- `routing-state`: Persistent routing state file written by UserPromptSubmit and consumed/cleared by skill-gate, tracking pending skill invocations

### Modified Capabilities

## Impact

- `plugins/dev-team/hooks/on-user-prompt.py` — 需要增加写入路由状态文件的逻辑
- `plugins/dev-team/hooks/hooks.json` — 新增 PreToolUse Write|Edit matcher 的 skill-gate hook
- `plugins/dev-team/hooks/` — 新增 `pre-tool-skill-gate.py` hook 脚本
- `plugins/dev-team/utils/` — 新增 `routing-state.py` 工具模块
- `plugins/dev-team/hooks/pre-tool-skill.py` — 需要增加清除 pending 状态的逻辑
