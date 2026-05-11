## Context

当前 UserPromptSubmit hook (`on-user-prompt.py`) 实现了意图路由器，但只能通过 `additionalContext` 输出文本建议。Claude Code 没有强制执行机制，Claude 可以忽略这些建议直接响应用户。

本设计引入 PreToolUse 拦截层，在 Claude 尝试执行写操作前强制检查是否有待处理的 skill 路由。

### 当前流程（问题）

```
用户输入 → UserPromptSubmit → additionalContext (建议)
                                     ↓
                               Claude 忽略建议
                                     ↓
                               直接 Write/Edit 代码
```

### 目标流程（方案 C）

```
用户输入 → UserPromptSubmit → 写入 .pending-skill.json
                             → additionalContext (建议)
                                     ↓
                               Claude 尝试 Write/Edit
                                     ↓
                          PreToolUse skill-gate 拦截
                                     ↓
                          检查 .pending-skill.json 存在？
                                     ↓
                          是 → deny + 指令调用 skill
                          否 → allow
```

## Goals / Non-Goals

**Goals:**
- 强制 Claude 在应该使用 OpenSpec 工作流时先调用对应 skill
- 不改变 UserPromptSubmit hook 的输出格式（保持 `additionalContext`）
- 状态文件机制足够健壮，能处理各种边界情况
- 支持 auto 和 suggest 两种路由模式的不同处理策略

**Non-Goals:**
- 不改变 Claude Code 平台能力（仍使用标准 hook 输出格式）
- 不实现跨会话的状态持久化（状态文件在 skill 调用后即清除）
- 不处理 question/explore 等不需要 skill 调用的 intent 类型

## Decisions

### D1: 状态文件位置和命名

**决定**: 状态文件位于 `.claude/pending-skill.json`

**原因**:
- 不放在 `openspec/changes/` 下，因为这是全局路由状态，不属于特定 change
- `.claude/` 目录已用于 Claude Code 配置，语义一致
- 单文件而非每 change 一个文件，简化管理

**替代方案**:
- `openspec/.pending-skill.json` — 混合 OpenSpec 内部结构
- `/tmp/pending-skill.json` — 不可靠，重启丢失

### D2: 状态文件格式

**决定**:
```json
{
  "version": 1,
  "decision": {
    "action": "explore",
    "mode": "suggest",
    "reason": "Implement intent detected without active change"
  },
  "intent": {
    "type": "implement",
    "confidence": "medium",
    "scope": "large"
  },
  "options": [
    "Start workflow with /openspec-explore",
    "Implement directly"
  ]
}
```

**原因**:
- 保留完整路由决策信息，便于 PreToolUse hook 做精细判断
- 无需 timestamp，采用 turn-scope 过期机制（见 D5）
- version 字段便于未来格式升级

### D3: 拦截时机

**决定**: 匹配 `Write|Edit` 工具，在写代码文件前拦截

**原因**:
- Write/Edit 是实际开始实现的行为，此时拦截最合理
- 不拦截 Read/Grep/Glob 等只读操作（允许 Claude 探索代码库）
- 不拦截 Bash（可能用于查询信息，不一定是写操作）

**边界情况**:
- Claude 先 Read 后 Write → Read 允许，Write 拦截 ✓
- Claude 直接 Write → 拦截 ✓
- Claude 调用 Skill → 由 PreToolUse Skill hook 处理（清除状态）

### D4: auto vs suggest 模式处理

**决定**:
- `mode: auto` → 直接 deny，指令明确要求调用 skill
- `mode: suggest` → 也 deny，但给出选项让 Claude 选择

**原因**:
- 即使是 suggest，我们也希望 Claude 先考虑 skill 选项
- 用户显式选择了"implement directly"时，状态会被清除（通过 Skill hook 或超时）

**auto 模式 deny 消息**:
```
SKILL GATE: A pending skill route exists.

Action: /openspec-explore
Reason: Implement intent detected without active change

INSTRUCTION: Invoke the Skill tool with skill="dev-team:openspec-explore" before writing any code.
```

**suggest 模式 deny 消息**:
```
SKILL GATE: A pending skill route exists.

Detected intent: implement (confidence: medium, scope: large)
Reason: Implement intent detected without active change

Options:
1. Invoke Skill tool with skill="dev-team:openspec-explore" (recommended)
2. Respond "implement directly" to skip workflow

INSTRUCTION: Choose one of the above options before writing code.
```

### D5: 状态清除机制

**决定**:
1. Skill 调用时 → PreToolUse Skill hook 检测到目标 skill → 清除
2. 用户新提问 → UserPromptSubmit 先清除旧状态再写入新状态（自动过期）
3. 用户说 "implement directly" 或类似 → UserPromptSubmit 检测关键词 → 不写 pending

**原因**:
- "用户新提问 = 重新考虑" 比 "N分钟超时" 语义更清晰
- 实现更简单：不需要时间比较，只需 UserPromptSubmit 先清除再写入
- 无残留风险：即使异常退出，下一轮用户提问也会自动清除旧状态
- 无需配置：不需要 timeout_minutes 参数

## Risks / Trade-offs

### R1: 用户意图被误判
**风险**: UserPromptSubmit 判断需要 skill，但用户实际想直接实现
**缓解**: suggest 模式给出选项；用户可以输入 "implement directly" 绕过

### R2: 状态文件残留
**风险**: 异常退出导致状态文件未清除
**缓解**: Turn-scope 机制自动解决——用户下一轮提问时 UserPromptSubmit 会先清除旧状态

### R3: 嵌套 Skill 调用
**风险**: 用户在 openspec-explore skill 内部又触发新的路由
**缓解**: UserPromptSubmit 检测当前是否在 skill 上下文中，若是则跳过路由

### R4: 多 change 场景
**风险**: 存在多个 active change 时路由不准确
**缓解**: 当前设计已处理（使用第一个 change 做路由决策）

## Component Design

### routing-state.py (新工具模块)

```python
# 核心函数
def write_pending_state(cwd: str, decision: RouteDecision, intent: IntentResult)
def read_pending_state(cwd: str) -> PendingState | None
def clear_pending_state(cwd: str)
# 无需 is_state_expired - turn-scope 机制由 UserPromptSubmit 自动清除旧状态
```

### on-user-prompt.py (修改)

```python
# 1. 先清除旧状态 (turn-scope 过期机制)
clear_pending_state(cwd)

# 2. 根据路由决策写入新状态 (在 output_user_prompt_submit() 之前)
if decision.mode in ("auto", "suggest") and decision.action != "direct":
    write_pending_state(cwd, decision, intent)
```

### pre-tool-skill-gate.py (新 hook)

```python
def main():
    input_data = json.load(sys.stdin)
    cwd = input_data.get("cwd", "")

    state = read_pending_state(cwd)

    if state is None:
        output_pre_tool_use("allow")
        return

    # 无需过期检查 - turn-scope 机制由 UserPromptSubmit 自动清除
    # 只要 state 存在就拦截
    message = build_deny_message(state)
    output_pre_tool_use("deny", message)
```

### pre-tool-skill.py (修改)

```python
# 在处理 archive-change/apply-change 时
skill_name = input_data.get("tool_input", {}).get("skill", "")
if skill_name.startswith("dev-team:openspec"):
    clear_pending_state(cwd)
```
