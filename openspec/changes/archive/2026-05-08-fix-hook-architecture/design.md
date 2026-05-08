# Design: Fix Hook Architecture Gaps

> **Change**: fix-hook-architecture
> **Date**: 2026-05-08

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        修复后的 Hook 架构                                    │
└─────────────────────────────────────────────────────────────────────────────┘

hooks.json
├── UserPromptSubmit
│   └── on-user-prompt.py          (不变)
│
├── PreToolUse
│   ├── matcher: "Write|Edit"
│   │   └── pre-tool-openspec-test.py   (增强: 可选调用 test-generator)
│   │
│   ├── matcher: "Bash"
│   │   └── pre-tool-commit-review.py   (修复: find_active_change 逻辑)
│   │
│   └── matcher: "Skill"                (新增)
│       └── pre-tool-skill.py           (新增: archive 合规阻断)
│
└── SessionStart
    └── session-start-*.py          (不变)
```

---

## Component 1: Fix `find_active_change()`

### Current Problem

```python
def find_active_change(changes_dir):
    for entry in os.listdir(changes_dir):
        total, done = count_tasks(tasks_path)
        if done < total:        # ← BUG: 全部完成后返回 None
            return entry
    return None
```

### New Design

**策略：优先级排序**

```
┌──────────────────────────────────────────────────────────┐
│              find_active_change 新逻辑                    │
└──────────────────────────────────────────────────────────┘

Step 1: 查找有未完成 task 的 change (最高优先级)
   │
   ├─ 找到 → 返回
   │
   └─ 未找到 ↓

Step 2: 通过 git staged files 推断当前工作的 change
   │
   ├─ 找到 → 返回
   │
   └─ 未找到 ↓

Step 3: 返回最近修改的 change
   │
   ├─ 找到 → 返回
   │
   └─ 未找到 → 返回 None
```

**关键实现：**

```python
def find_active_change(changes_dir, cwd=""):
    """Find the active change with priority ordering.

    Priority:
    1. Change with pending tasks (original behavior)
    2. Change matching staged files (inferred from git)
    3. Most recently modified change (fallback)
    """
    changes = []

    for entry in os.listdir(changes_dir):
        entry_path = os.path.join(changes_dir, entry)
        if not os.path.isdir(entry_path) or entry == "archive":
            continue

        tasks_path = os.path.join(entry_path, "tasks.md")
        has_pending = False
        if os.path.isfile(tasks_path):
            total, done = count_tasks(tasks_path)
            has_pending = done < total

        mtime = get_latest_mtime(entry_path)
        changes.append({
            "name": entry,
            "path": entry_path,
            "has_pending": has_pending,
            "mtime": mtime,
        })

    if not changes:
        return None

    # Priority 1: Pending tasks
    pending = [c for c in changes if c["has_pending"]]
    if len(pending) == 1:
        return (pending[0]["name"], pending[0]["path"])
    if len(pending) > 1:
        # Multiple pending — try git inference
        staged_match = match_staged_to_change(cwd, pending)
        if staged_match:
            return (staged_match["name"], staged_match["path"])
        # Fallback: most recently modified pending
        pending.sort(key=lambda c: c["mtime"], reverse=True)
        return (pending[0]["name"], pending[0]["path"])

    # Priority 2: All tasks done — infer from staged files
    staged_match = match_staged_to_change(cwd, changes)
    if staged_match:
        return (staged_match["name"], staged_match["path"])

    # Priority 3: Most recently modified
    changes.sort(key=lambda c: c["mtime"], reverse=True)
    return (changes[0]["name"], changes[0]["path"])


def match_staged_to_change(cwd, changes):
    """Match staged git files to a change directory."""
    staged_files = get_staged_files(cwd)
    if not staged_files:
        return None

    for change in changes:
        change_dir = change["path"]
        for staged in staged_files:
            # Check if staged file is under change directory
            staged_abs = os.path.normpath(os.path.join(cwd, staged))
            change_abs = os.path.normpath(change_dir)
            if staged_abs.startswith(change_abs):
                return change

    # Also check: if there's only one change, it's likely the one
    if len(changes) == 1:
        return changes[0]

    return None


def get_latest_mtime(directory):
    """Get the most recent modification time in a directory tree."""
    latest = 0
    for root, dirs, files in os.walk(directory):
        for f in files:
            try:
                mtime = os.path.getmtime(os.path.join(root, f))
                latest = max(latest, mtime)
            except OSError:
                pass
    return latest


def get_staged_files(cwd):
    """Get list of staged files from git."""
    try:
        result = subprocess.run(
            ["git", "diff", "--cached", "--name-only"],
            cwd=cwd, capture_output=True, text=True, timeout=10, shell=True
        )
        if result.returncode == 0:
            return [f.strip() for f in result.stdout.splitlines() if f.strip()]
    except (subprocess.TimeoutExpired, OSError, FileNotFoundError):
        pass
    return []
```

### Behavioral Changes

| 场景 | 旧行为 | 新行为 |
|------|--------|--------|
| 1 个 change, task 全完成 | 返回 None, 门禁跳过 | 返回该 change, 门禁执行 |
| 多个 change, 1 个未完成 | 返回未完成的 | 不变 |
| 多个 change, 全完成 | 返回 None | 通过 staged files 推断 |
| 无 staged files, 全完成 | 返回 None | 返回最近修改的 |
| 无 change | 返回 None | 不变 |

---

## Component 2: `pre-tool-skill.py` (New)

### Purpose

拦截 Skill 调用，在关键操作前强制执行检查。

### Hook Configuration

```json
{
  "matcher": "Skill",
  "hooks": [
    {
      "type": "command",
      "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/pre-tool-skill.py"
    }
  ]
}
```

### Logic

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      pre-tool-skill.py 逻辑                                 │
└─────────────────────────────────────────────────────────────────────────────┘

Input: tool_name="Skill", skill="openspec-archive-change"

    │
    ▼
┌─────────────────────────┐
│ 1. 解析 skill name     │
└────────────┬────────────┘
             │
             ▼
┌──────────────────────────────────────────────────┐
│ 2. Route by skill type                           │
├──────────────────────────────────────────────────┤
│                                                  │
│ archive-change:                                  │
│   └─ 运行 compliance-check.py (C1-C8)           │
│      ├─ 通过 → allow                            │
│      └─ 失败 → deny + 显示 blocking issues      │
│                                                  │
│ apply-change:                                    │
│   └─ 检查 review-loop-state                     │
│      ├─ paused → allow + 注入人工介入提醒       │
│      └─ 正常 → allow                            │
│                                                  │
│ 其他 skill:                                      │
│   └─ allow (不做干预)                           │
│                                                  │
└──────────────────────────────────────────────────┘
```

### Implementation

```python
def main():
    input_data = json.load(sys.stdin)
    tool_name = input_data.get("tool_name", "")
    tool_input = input_data.get("tool_input", {})
    cwd = input_data.get("cwd", "")

    if tool_name != "Skill":
        output_result("allow", "")
        return

    skill_name = tool_input.get("skill", "")

    # Route by skill
    if "archive" in skill_name:
        handle_archive_skill(cwd)
    elif "apply" in skill_name:
        handle_apply_skill(cwd)
    else:
        output_result("allow", "")


def handle_archive_skill(cwd):
    """Force compliance check before archive."""
    changes_dir = os.path.join(cwd, "openspec", "changes")

    # Find active change
    active_change = find_active_change(changes_dir, cwd)
    if not active_change:
        output_result("allow", "")
        return

    change_name, change_dir = active_change

    # Run compliance check
    try:
        result = subprocess.run(
            ["python", os.path.join(PLUGIN_ROOT, "utils", "compliance-check.py"),
             "--change", change_name, "--project-root", cwd, "--json"],
            capture_output=True, text=True, timeout=30, shell=True
        )

        data = json.loads(result.stdout)

        if not data.get("passed", False):
            blocking = data.get("blocking_issues", [])
            issues = format_blocking_issues(blocking)
            context = (
                f"ARCHIVE BLOCKED: {len(blocking)} compliance issue(s) for '{change_name}'.\n"
                f"{issues}\n"
                f"Fix these issues before archiving, or explicitly override."
            )
            output_result("deny", context)
            return

    except (subprocess.TimeoutExpired, json.JSONDecodeError, OSError):
        # Non-blocking on error
        pass

    output_result("allow", f"Compliance check passed for '{change_name}'.")


def handle_apply_skill(cwd):
    """Check review loop state before apply."""
    changes_dir = os.path.join(cwd, "openspec", "changes")
    active_change = find_active_change(changes_dir, cwd)

    if not active_change:
        output_result("allow", "")
        return

    change_name, change_dir = active_change

    # Check review loop state
    state_path = os.path.join(change_dir, "review-loop-state.json")
    if os.path.isfile(state_path):
        try:
            with open(state_path, "r", encoding="utf-8") as f:
                state = json.load(f)

            if state.get("status") == "paused":
                context = (
                    f"Review loop PAUSED for '{change_name}'. "
                    f"Loop {state.get('loop_count', '?')}/{state.get('max_loops', 3)} "
                    f"reached. Manual intervention required."
                )
                output_result("allow", context)
                return
        except (json.JSONDecodeError, OSError):
            pass

    output_result("allow", "")
```

---

## Component 3: Enhanced `pre-tool-openspec-test.py` (Optional)

### Current State

仅注入 TDD 提醒文本，不生成测试文件。

### Enhancement

在注入提醒时，同时调用 `test-generator.py` 自动生成测试骨架（如果不存在）。

```
┌─────────────────────────────────────────────────────────────────┐
│             enhanced pre-tool-openspec-test.py                  │
└─────────────────────────────────────────────────────────────────┘

Input: tool_name="Write", file_path="src/auth.js"

    │
    ▼
┌─────────────────────────┐
│ 1. 检测 active change  │
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────┐
│ 2. 检测是否生产代码    │
│    (非测试、非openspec)│
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────┐
│ 3. 检查对应测试是否存在│
│    src/auth.js →        │
│    src/auth.test.js     │
└────────────┬────────────┘
             │
             ├─ 存在 → allow + 简短提醒
             │
             └─ 不存在 ↓
                ┌─────────────────────────────┐
                │ 4. 调用 test-generator.py   │
                │    生成测试骨架             │
                └────────────┬────────────────┘
                             │
                             ▼
                ┌─────────────────────────────┐
                │ 5. allow + 注入完整上下文   │
                │    "已生成测试骨架，请      │
                │     先完善测试再实现"       │
                └─────────────────────────────┘
```

### Constraint

Hook 只能 `allow` / `deny` + 注入 context，**不能直接写文件**。

因此增强方案为：
- 检测测试文件是否已存在
- 如果不存在，在 context 中**明确指令** Claude 先调用 `test-generator.py` 生成测试
- 这是更强的软约束，从 "TDD 提醒" 变为 "TDD 指令"

```python
# 增强后的 context
context = (
    f"OpenSpec change '{change_name}' is active. "
    f"TDD GATE: No test file found for '{relative_path}'. "
    f"BEFORE implementing, you MUST run: "
    f"python plugin/utils/test-generator.py --source '{file_path}' --project-root '{cwd}' "
    f"to generate a test skeleton. Write tests first, then implement."
)
```

---

## Updated `hooks.json`

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/on-user-prompt.py"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [
          {
            "type": "command",
            "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/pre-tool-openspec-test.py"
          }
        ]
      },
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/pre-tool-commit-review.py"
          }
        ]
      },
      {
        "matcher": "Skill",
        "hooks": [
          {
            "type": "command",
            "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/pre-tool-skill.py"
          }
        ]
      }
    ],
    "SessionStart": [
      {
        "matcher": "startup|resume",
        "hooks": [
          {
            "type": "command",
            "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/session-start-sync-skills.py"
          },
          {
            "type": "command",
            "command": "python3 \"${CLAUDE_PLUGIN_ROOT}\"/hooks/session-start-worktree.py"
          }
        ]
      }
    ]
  }
}
```

---

## Component 5: ERROR → Task → Re-enter Apply Loop

### Problem

当前 `pre-tool-commit-review.py` 检测到 review BLOCK 时仅 `allow + warn`，用户需手动操作。缺少自动闭环：
- 未调用 `review-parser.py` 生成 fix tasks
- 未调用 `review-loop-state.py` 记录轮次
- 未引导重入 apply

### Design Principle

Hook **不能直接写文件**（仅能 allow/deny + 注入 context），因此采用 **指令注入** 策略：
- 检测到 BLOCK 时，注入明确指令让 Claude 执行 `review-parser` 和 `review-loop-state` 命令
- 这是比 warn 更强的约束——从"建议"变为"指令"

### Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│              ERROR → Task → Re-enter Apply 流程                             │
└─────────────────────────────────────────────────────────────────────────────┘

pre-tool-commit-review.py (git commit 拦截)
    │
    ▼
Review verdict = BLOCK, 非安全错误
    │
    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ inject context (allow + actionable instruction):                         │
│                                                                          │
│ "REVIEW BLOCK: N error(s) found in '{change_name}'.                     │
│  BEFORE committing, you MUST:                                           │
│  1. Generate fix tasks:                                                 │
│     python plugin/utils/review-parser.py <report_path>                  │
│       --generate-tasks --append-to <tasks_md_path>                      │
│  2. Record loop state:                                                  │
│     python plugin/utils/review-loop-state.py <change_dir>               │
│       --record <error_count> <tasks_generated>                          │
│  3. Re-enter apply: invoke the openspec-apply-change skill              │
│  This ensures all review errors are addressed before commit."            │
└──────────────────────────────────────────────────────────────────────────┘

pre-tool-skill.py (apply-change 拦截)
    │
    ▼
review-loop-state.json exists with current_errors > 0
    │
    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│ inject context (allow + reminder):                                       │
│                                                                          │
│ "Review loop active for '{change_name}':                                │
│  Loop N/3, M error(s) remaining.                                       │
│  Fix tasks have been appended to tasks.md.                              │
│  Implement the fix tasks before proceeding with other work."             │
└──────────────────────────────────────────────────────────────────────────┘
```

### Implementation: pre-tool-commit-review.py Enhancement

```python
# In the Phase 2.6 section (non-security errors with review comparison)
# Replace the current "allow + warn" with "allow + actionable instruction"

if verdict == "BLOCK" or error_count > 0:
    # ... existing comparison logic ...

    # Build actionable instruction
    report_filename = os.path.basename(latest_review["path"])
    tasks_md_path = os.path.join(change_dir, "tasks.md")

    context = (
        f"REVIEW BLOCK: {error_count} error(s) found in '{change_name}' "
        f"(verdict: {verdict}).{comparison}\n"
        f"BEFORE committing, you MUST:\n"
        f"1. Generate fix tasks: python plugin/utils/review-parser.py "
        f"openspec/changes/{change_name}/test-reports/{report_filename} "
        f"--generate-tasks --append-to {tasks_md_path}\n"
        f"2. Record loop state: python plugin/utils/review-loop-state.py "
        f"{change_dir} --record {error_count} {error_count}\n"
        f"3. Re-enter apply: invoke the openspec-apply-change skill "
        f"to implement the fix tasks.\n"
        f"This ensures all review errors are addressed before commit."
    )
    output_result("allow", context)
    return
```

### Implementation: pre-tool-skill.py Enhancement

```python
def handle_apply_skill(cwd):
    """Check review loop state before apply — inject guidance if errors pending."""
    # ... existing find_active_change logic ...

    # Check review loop state
    state_path = os.path.join(change_dir, "review-loop-state.json")
    if os.path.isfile(state_path):
        try:
            with open(state_path, "r", encoding="utf-8") as f:
                state = json.load(f)

            status = state.get("status", "idle")
            loop_count = state.get("loop_count", 0)
            max_loops = state.get("max_loops", 3)
            current_errors = state.get("current_errors", 0)

            if status == "paused":
                context = (
                    f"Review loop PAUSED for '{change_name}'. "
                    f"Loop {loop_count}/{max_loops} reached. "
                    f"Manual intervention required."
                )
                output_result("allow", context)
                return

            if status == "running" and current_errors > 0:
                context = (
                    f"Review loop active for '{change_name}': "
                    f"Loop {loop_count}/{max_loops}, {current_errors} error(s) remaining. "
                    f"Fix tasks have been appended to tasks.md. "
                    f"Implement the fix tasks before proceeding with other work."
                )
                output_result("allow", context)
                return

        except (json.JSONDecodeError, OSError):
            pass

    output_result("allow", "")
```

---

## Testing Strategy

### Unit Tests

| 组件 | 测试场景 |
|------|----------|
| `find_active_change()` | 1 个未完成 change → 返回该 change |
| `find_active_change()` | 1 个已完成 change → 返回该 change (不再返回 None) |
| `find_active_change()` | 多个已完成 change + staged files → 推断 |
| `find_active_change()` | 无 change → 返回 None |
| `match_staged_to_change()` | staged 文件匹配 change 目录 |
| `get_latest_mtime()` | 目录内最近修改时间 |
| `pre-tool-skill.py` | archive + 合规失败 → deny |
| `pre-tool-skill.py` | archive + 合规通过 → allow |
| `pre-tool-skill.py` | 非 archive skill → allow |
| `pre-tool-skill.py` | apply + paused loop → allow + 提醒 |

### Integration Tests

1. **端到端 commit 门禁**：
   - 创建 change → 完成 task → `git commit` → 门禁仍生效

2. **端到端 archive 阻断**：
   - 创建 change → 缺少文件 → archive → deny

3. **端到端 TDD 提醒**：
   - active change → 写生产代码 → 注入 TDD GATE 指令

---

## Migration Notes

1. `find_active_change()` 逻辑变更可能影响 `pre-tool-openspec-test.py` 和 `on-user-prompt.py`
2. 建议将 `find_active_change()` 和 `count_tasks()` 提取为共享模块 `plugin/utils/active-change.py`
3. 现有 Hook 保持兼容，新增 `Skill` matcher 不影响其他 PreToolUse Hook
